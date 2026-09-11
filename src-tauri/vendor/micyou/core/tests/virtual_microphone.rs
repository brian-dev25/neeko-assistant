//! Explicit hardware test: synthetic TCP/UDP phone -> MicYou core -> VB-CABLE capture.
//! Run with --ignored --nocapture; never captures a physical microphone.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use micyou_protocol::{
    micyou::*, HANDSHAKE_CLIENT_STR, HANDSHAKE_SERVER_STR, PACKET_MAGIC, UDP_PACKET_MAGIC,
};
use neeko_micyou_core::{
    events::{AecStatus, ServerEvents},
    server::ServerState,
    stats::AudioMetrics,
    system::{start_server_inner, stop_server_inner},
    tcp_server::DeviceInfo,
};
use prost::Message;
use std::{
    sync::{atomic::Ordering, Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

struct Sink;
impl ServerEvents for Sink {
    fn device_connected(&self, _: DeviceInfo) {}
    fn device_disconnected(&self) {}
    fn audio_metrics(&self, _: AudioMetrics) {}
    fn udp_audio_warning(&self) {}
    fn mute_state_changed(&self, _: bool) {}
    fn audio_level(&self, _: u32) {}
    fn audio_spectrum(&self, _: Vec<f32>, _: Vec<f32>) {}
    fn server_stopped(&self) {}
    fn web_client_count(&self, _: u32) {}
    fn install_progress(&self, _: String) {}
    fn aec_status_changed(&self, _: AecStatus) {}
}
fn framed(message: MessageWrapper, magic: i32) -> Vec<u8> {
    let body = message.encode_to_vec();
    let mut result = magic.to_be_bytes().to_vec();
    result.extend_from_slice(&(body.len() as i32).to_be_bytes());
    result.extend_from_slice(&body);
    result
}

#[test]
#[ignore = "requires installed VB-CABLE; sends a synthetic test tone only to that device"]
fn phone_transport_approval_mute_and_virtual_capture() {
    let host = cpal::default_host();
    let output = host
        .output_devices()
        .unwrap()
        .find_map(|d| d.name().ok().filter(|n| n.starts_with("CABLE Input")))
        .expect("VB-CABLE required");
    let input = host
        .input_devices()
        .unwrap()
        .find(|d| d.name().unwrap_or_default().starts_with("CABLE Output"))
        .unwrap();
    let config = input.default_input_config().unwrap();
    assert_eq!(config.sample_format(), cpal::SampleFormat::F32);
    let measurement = Arc::new(Mutex::new((0u64, 0f64)));
    let measured = measurement.clone();
    let capture = input
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let mut m = measured.lock().unwrap();
                m.0 += data.len() as u64;
                m.1 += data.iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
            },
            |e| panic!("VB-CABLE capture: {e}"),
            None,
        )
        .unwrap();
    capture.play().unwrap();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let state = ServerState::default();
        // Reserve consecutive localhost ports without touching the user's configured server.
        let (tcp_probe, udp_probe, port) = (0..50)
            .find_map(|_| {
                let tcp = std::net::TcpListener::bind("127.0.0.1:0").ok()?;
                let port = tcp.local_addr().ok()?.port();
                let udp = std::net::UdpSocket::bind(("127.0.0.1", port.checked_add(1)?)).ok()?;
                Some((tcp, udp, port))
            })
            .unwrap();
        drop(tcp_probe);
        drop(udp_probe);
        tokio::time::timeout(
            Duration::from_secs(15),
            start_server_inner(
                &state,
                port,
                "usb".into(),
                Some("127.0.0.1".into()),
                Some(output),
                None,
                Arc::new(Sink),
            ),
        )
        .await
        .expect("server start must return, not deadlock")
        .unwrap();
        let mut phone = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        phone.write_all(HANDSHAKE_CLIENT_STR).await.unwrap();
        let mut answer = vec![0; HANDSHAKE_SERVER_STR.len()];
        phone.read_exact(&mut answer).await.unwrap();
        assert_eq!(answer, HANDSHAKE_SERVER_STR);
        phone
            .write_all(&framed(
                MessageWrapper {
                    connect: Some(ConnectMessage { session_id: 123 }),
                    ..Default::default()
                },
                PACKET_MAGIC,
            ))
            .await
            .unwrap();
        let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mut seq = 0;
        for (label, approved, muted, ptt_blocked, via_udp, audible) in [
            ("pending TCP", false, false, false, false, false),
            ("approved TCP", true, false, false, false, true),
            ("local mute", true, true, false, false, false),
            ("PTT released", true, false, true, false, false),
            ("PTT held", true, false, false, false, true),
            ("mute overrides PTT", true, true, false, false, false),
            ("approved UDP", true, false, false, true, true),
        ] {
            state.audio_allowed.store(approved, Ordering::Release);
            state.local_muted.store(muted, Ordering::Release);
            state.ptt_blocked.store(ptt_blocked, Ordering::Release);
            for block in 0..120 {
                if block == 60 {
                    *measurement.lock().unwrap() = (0, 0.0);
                }
                let pcm: Vec<u8> = (0..480)
                    .flat_map(|i| {
                        let value = (2.0 * std::f32::consts::PI * 440.0 * (seq * 480 + i) as f32
                            / 48000.0)
                            .sin()
                            * 3276.0;
                        (value as i16).to_le_bytes()
                    })
                    .collect();
                let message = MessageWrapper {
                    audio_packet: Some(AudioPacketMessageOrdered {
                        sequence_number: seq,
                        session_id: 123,
                        audio_packet: Some(AudioPacketMessage {
                            buffer: pcm,
                            sample_rate: 48000,
                            channel_count: 1,
                            audio_format: 2,
                            codec: 0,
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                if via_udp {
                    udp.send_to(&framed(message, UDP_PACKET_MAGIC), ("127.0.0.1", port + 1))
                        .await
                        .unwrap();
                } else {
                    phone
                        .write_all(&framed(message, PACKET_MAGIC))
                        .await
                        .unwrap();
                }
                seq += 1;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let (count, energy) = *measurement.lock().unwrap();
            let rms = (energy / count.max(1) as f64).sqrt();
            println!("{label}: CABLE Output RMS={rms:.6}, {count} samples");
            assert!(count > 0);
            if audible {
                assert!(rms > 0.005, "{label} must reach the Windows microphone");
            } else {
                assert!(rms < 0.001, "{label} must be silent");
            }
        }
        tokio::time::timeout(
            Duration::from_secs(10),
            stop_server_inner(&state, Arc::new(Sink)),
        )
        .await
        .expect("server stop must return, not deadlock")
        .unwrap();
        state.audio_output.shutdown();
        assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_ok());
        assert!(std::net::UdpSocket::bind(("127.0.0.1", port + 1)).is_ok());
    });
}
