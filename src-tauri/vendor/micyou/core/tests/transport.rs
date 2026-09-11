//! Neeko integration regression: real sockets, official handshake and Protobuf.
use micyou_protocol::{micyou::*, HANDSHAKE_CLIENT_STR, HANDSHAKE_SERVER_STR, PACKET_MAGIC};
use neeko_micyou_core::{
    audio_stream::AudioStreamEvent,
    events::{AecStatus, ServerEvents},
    stats::{AudioMetrics, NetworkStats},
    tcp_server::{start_tcp_server, DeviceInfo},
};
use prost::Message;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot, Mutex},
};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct Sink(AtomicUsize, std::sync::atomic::AtomicBool);
impl ServerEvents for Sink {
    fn device_connected(&self, _: DeviceInfo) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    fn device_disconnected(&self) {}
    fn audio_metrics(&self, _: AudioMetrics) {}
    fn udp_audio_warning(&self) {}
    fn mute_state_changed(&self, muted: bool) {
        self.1.store(muted, Ordering::SeqCst);
    }
    fn audio_level(&self, _: u32) {}
    fn audio_spectrum(&self, _: Vec<f32>, _: Vec<f32>) {}
    fn server_stopped(&self) {}
    fn web_client_count(&self, _: u32) {}
    fn install_progress(&self, _: String) {}
    fn aec_status_changed(&self, _: AecStatus) {}
}

#[tokio::test]
async fn stopping_an_idle_server_does_not_relock_its_own_lifecycle() {
    let state = neeko_micyou_core::server::ServerState::default();
    tokio::time::timeout(
        Duration::from_secs(2),
        neeko_micyou_core::system::stop_server_inner(&state, Arc::new(Sink::default())),
    )
    .await
    .expect("stop held a lifecycle guard while acquiring it again")
    .unwrap();
    state.audio_output.shutdown();
}

#[tokio::test]
async fn missing_virtual_output_rolls_back_start_without_deadlock_or_speaker_fallback() {
    let state = neeko_micyou_core::server::ServerState::default();
    let result = tokio::time::timeout(
        Duration::from_secs(12),
        neeko_micyou_core::system::start_server_inner(
            &state,
            19123,
            "usb".into(),
            Some("127.0.0.1".into()),
            Some("__neeko_missing_virtual_endpoint__".into()),
            None,
            Arc::new(Sink::default()),
        ),
    )
    .await
    .expect("failed startup must release lifecycle before rollback");
    assert!(result.is_err());
    assert!(state.cancel_token.lock().await.is_none());
    assert!(state.background_tasks.lock().await.is_empty());
    assert_eq!(
        state.lifecycle.lock().await.phase(),
        neeko_micyou_core::server::ServerLifecyclePhase::Stopped
    );
    state.audio_output.shutdown();
}
async fn frame(socket: &mut TcpStream, message: MessageWrapper) {
    let payload = message.encode_to_vec();
    let mut bytes = PACKET_MAGIC.to_be_bytes().to_vec();
    bytes.extend_from_slice(&(payload.len() as i32).to_be_bytes());
    bytes.extend_from_slice(&payload);
    // Deliberately fragment the TCP header/payload as real networks may do.
    for piece in bytes.chunks(3) {
        socket.write_all(piece).await.unwrap();
    }
}
async fn connect(port: u16, session: i64) -> TcpStream {
    let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    client.write_all(HANDSHAKE_CLIENT_STR).await.unwrap();
    let mut answer = vec![0; HANDSHAKE_SERVER_STR.len()];
    client.read_exact(&mut answer).await.unwrap();
    assert_eq!(answer, HANDSHAKE_SERVER_STR);
    frame(
        &mut client,
        MessageWrapper {
            connect: Some(ConnectMessage {
                session_id: session,
            }),
            ..Default::default()
        },
    )
    .await;
    client
}

#[tokio::test]
async fn official_client_frames_reconnect_and_release_listener() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let cancel = CancellationToken::new();
        let (audio_tx, mut audio_rx) = mpsc::channel(16);
        let (ready_tx, ready_rx) = oneshot::channel();
        let active = Arc::new(Mutex::new(None));
        let sink = Arc::new(Sink::default());
        let task = tokio::spawn(start_tcp_server(
            sink.clone(),
            port,
            "127.0.0.1".into(),
            cancel.clone(),
            audio_tx,
            Arc::new(NetworkStats::default()),
            "usb".into(),
            active.clone(),
            Arc::new(Mutex::new(())),
            Arc::new(RwLock::new(Default::default())),
            ready_tx,
        ));
        ready_rx.await.unwrap().unwrap();
        let mut client = connect(port, 42).await;
        assert!(matches!(
            audio_rx.recv().await,
            Some(AudioStreamEvent::SessionStarting { .. })
        ));
        for muted in [true, false] {
            neeko_micyou_core::tcp_server::send_mute_command(&active, muted)
                .await
                .unwrap();
            loop {
                let mut header = [0u8; 8];
                client.read_exact(&mut header).await.unwrap();
                assert_eq!(&header[..4], &PACKET_MAGIC.to_be_bytes());
                let size = i32::from_be_bytes(header[4..].try_into().unwrap()) as usize;
                let mut body = vec![0u8; size];
                client.read_exact(&mut body).await.unwrap();
                if let Some(m) = MessageWrapper::decode(body.as_slice()).unwrap().mute {
                    assert_eq!(m.is_muted, Some(muted));
                    break;
                }
            }
            frame(
                &mut client,
                MessageWrapper {
                    mute: Some(MuteMessage {
                        is_muted: Some(!muted),
                    }),
                    ..Default::default()
                },
            )
            .await;
            while sink.1.load(Ordering::SeqCst) != !muted {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
        let samples = vec![0, 16, 0, 32];
        frame(
            &mut client,
            MessageWrapper {
                audio_packet: Some(AudioPacketMessageOrdered {
                    sequence_number: 1,
                    session_id: 42,
                    audio_packet: Some(AudioPacketMessage {
                        buffer: samples.clone(),
                        sample_rate: 48000,
                        channel_count: 1,
                        audio_format: 2,
                        codec: 0,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await;
        match audio_rx.recv().await.unwrap() {
            AudioStreamEvent::Packet { packet, .. } => {
                assert_eq!(packet.audio_packet.unwrap().buffer, samples)
            }
            _ => panic!("expected decoded PCM packet"),
        }
        drop(client);
        while active.lock().await.is_some() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let _second = connect(port, 43).await;
        assert!(matches!(
            audio_rx.recv().await,
            Some(AudioStreamEvent::SessionStarting { .. })
        ));
        cancel.cancel();
        task.await.unwrap().unwrap();
        assert!(active.lock().await.is_none());
        assert_eq!(sink.0.load(Ordering::SeqCst), 2);
        let rebound = TcpListener::bind(("127.0.0.1", port)).await;
        assert!(rebound.is_ok(), "stop must release the listening socket");
    })
    .await
    .expect("transport lifecycle timed out");
}
