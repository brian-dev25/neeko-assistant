/*
 * MicYou — Turns your Android device into a high-quality PC microphone.
 * Copyright (C) 2026 LanRhyme <https://github.com/LanRhyme/MicYou>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version, with the MicYou Plugin Exception.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 */

use crate::audio_stream::AudioStreamEvent;
use crate::server::{
    await_startup_ready, ServerLifecycleState, ServerState, AUDIO_JOIN_TIMEOUT, STARTUP_TIMEOUT,
};
use crate::udp_server::ActiveAudioSession;
use micyou_audio::AecFailure;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
const NETWORK_TASK_JOIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
fn validate_server_port(port: u16, mode: &str) -> Result<Option<u16>, String> {
    if mode == "web" {
        return if port == 0 {
            Err("Web server port must be between 1 and 65535".to_string())
        } else {
            Ok(None)
        };
    }

    if port == 0 {
        return Err("Audio server port must be between 1 and 65534".to_string());
    }

    port.checked_add(1).map(Some).ok_or_else(|| {
        "Audio server port must be between 1 and 65534 so the following UDP port is valid"
            .to_string()
    })
}

fn disable_aec_runtime(
    runtime_available: &mut bool,
    events: &crate::events::SharedEvents,
    reason: AecFailure,
) {
    if !std::mem::replace(runtime_available, false) {
        return;
    }
    events.aec_status_changed(crate::events::AecStatus {
        available: false,
        enabled: false,
        reason: Some(reason),
    });
}

fn restore_aec_runtime(
    runtime_available: &mut bool,
    settings: &std::sync::Arc<std::sync::RwLock<micyou_audio::dsp::AudioDspSettings>>,
    events: &crate::events::SharedEvents,
) {
    *runtime_available = true;
    let enabled = settings
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .aec_enabled;
    events.aec_status_changed(crate::events::AecStatus {
        available: true,
        enabled,
        reason: None,
    });
}

fn should_capture_loopback(
    transport_active: bool,
    audio_received: bool,
    aec_enabled: bool,
    runtime_available: bool,
) -> bool {
    transport_active && audio_received && aec_enabled && runtime_available
}

/// Platform-specific ONNX Runtime shared library filename.
const fn ort_runtime_filename() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "onnxruntime.dll"
    }
    #[cfg(target_os = "macos")]
    {
        "libonnxruntime.dylib"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        "libonnxruntime.so"
    }
}

/// Find the ONNX Runtime shared library bundled alongside the application.
/// Searches `libs/` relative to the executable / build tree, and the resource
/// root itself (production builds copy the lib into `resources/`).
fn find_ort_runtime(resource_root: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    let filename = ort_runtime_filename();
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    // Resource root (production: DLL copied into resources/ before bundling)
    if let Some(root) = resource_root {
        candidates.push(root.join(filename));
        if let Some(parent) = root.parent() {
            candidates.push(parent.join("libs").join(filename));
            candidates.push(parent.join(filename));
        }
    }

    // Executable-relative
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
    {
        candidates.push(exe_dir.join(filename));
        candidates.push(exe_dir.join("resources").join(filename));
        candidates.push(exe_dir.join("libs").join(filename));
        if let Some(prefix) = exe_dir.parent() {
            candidates.push(
                prefix
                    .join("lib")
                    .join("micyou")
                    .join("libs")
                    .join(filename),
            );
            candidates.push(
                prefix
                    .join("lib")
                    .join("micyou")
                    .join("resources")
                    .join(filename),
            );
        }
    }

    // Dev mode: src-tauri/libs/
    candidates.push(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("libs")
            .join(filename),
    );

    candidates.into_iter().find(|p| p.exists())
}

/// Locate the directory containing MicYou's bundled runtime resources (ONNX
/// models and the ALSA config). Linux packages use the standard
/// /usr/bin + /usr/lib/micyou/resources layout, while AppImage exposes the
/// same tree below its temporary mount point.
fn find_resource_dir(resource_dir: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    const MARKERS: [&str; 2] = ["purevox6.onnx", "aec7_ep0185.onnx"];

    let mut candidates = Vec::new();

    // Runtime resource dir resolved by Tauri (correct in dev and when the
    // binary name matches the product name).
    if let Some(dir) = resource_dir {
        candidates.push(dir.to_path_buf());
        candidates.push(dir.join("resources"));
    }

    // Executable-relative (covers `cargo run`, `tauri dev` and installs where
    // resources live next to the binary).
    if let Some(executable_dir) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
    {
        candidates.push(executable_dir.clone());
        candidates.push(executable_dir.join("resources"));
        if let Some(prefix) = executable_dir.parent() {
            candidates.push(prefix.join("lib").join("micyou").join("resources"));
        }
    }

    // Compile-time fallback for `cargo run` from the workspace.
    candidates.push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources"));

    candidates
        .into_iter()
        .find(|directory| MARKERS.iter().any(|model| directory.join(model).exists()))
}

async fn join_tasks_bounded(
    mut tasks: Vec<tokio::task::JoinHandle<()>>,
    timeout_duration: std::time::Duration,
) {
    let deadline = tokio::time::Instant::now() + timeout_duration;
    while let Some(mut task) = tasks.pop() {
        if tokio::time::timeout_at(deadline, &mut task).await.is_err() {
            task.abort();
            let _ = task.await;
            for task in &tasks {
                task.abort();
            }
            for task in tasks {
                let _ = task.await;
            }
            break;
        }
    }
}

async fn rollback_start(
    state: &ServerState,
    cancel_token: &CancellationToken,
    tasks: Vec<tokio::task::JoinHandle<()>>,
) -> Result<(), String> {
    cancel_token.cancel();
    crate::tcp_server::cleanup_session_state(&state.active_connection, &state.active_audio_session)
        .await;
    join_tasks_bounded(tasks, NETWORK_TASK_JOIN_TIMEOUT).await;
    let _: Option<CancellationToken> = state.cancel_token.lock().await.take();
    let mdns_opt: Option<crate::network::NetworkManager> = state.mdns_manager.lock().await.take();
    if let Some(mdns) = mdns_opt {
        mdns.stop_mdns();
    }
    let mut lifecycle: tokio::sync::MutexGuard<'_, ServerLifecycleState> =
        state.lifecycle.lock().await;
    lifecycle.begin_stopping();
    lifecycle.join_audio_bounded(AUDIO_JOIN_TIMEOUT).await
}

use micyou_audio::dsp::DspProcessor;

/// Normalize a raw persisted output-device value ("", "auto", "default" all
/// mean "no explicit device") to the Option form used by the audio engine.
pub fn normalize_output_device(raw: &str) -> Option<String> {
    let d = raw.trim();
    if d.is_empty() || d == "auto" || d == "default" {
        None
    } else {
        Some(d.to_string())
    }
}

/// Create and open the persistent audio output device (cpal stream, plus the
/// PipeWire virtual sink/source on Linux). Idempotent: re-opening only happens
/// if the stream is not already open, so repeated calls from app startup and
/// server start are safe. Called at GUI startup and lazily from the audio
/// thread on the first server start (CLI/TUI).
pub fn ensure_audio_output_started(
    audio_output: &std::sync::Arc<crate::audio_output::AudioOutputHandle>,
    output_device: Option<String>,
    output_buffer_ms: usize,
    _resource_dir: Option<&std::path::Path>,
) -> bool {
    audio_output.ensure_open(output_device, output_buffer_ms)
}

/// Tear down the persistent audio output device. Only called when the process
/// is exiting (GUI `RunEvent::Exit`, CLI/TUI shutdown), never on server stop.
pub fn shutdown_audio_output(state: &ServerState) {
    state.audio_output.shutdown();
}

pub async fn start_server_inner(
    state: &ServerState,
    port: u16,
    mode: String,
    bind_address: Option<String>,
    output_device: Option<String>,
    resource_dir: Option<std::path::PathBuf>,
    events: crate::events::SharedEvents,
) -> Result<String, String> {
    let udp_port = validate_server_port(port, &mode)?;

    let _lifecycle_guard = state.lifecycle_gate.enter().await;
    state.lifecycle.lock().await.begin_start().await?;
    let bind_addr = bind_address.unwrap_or_else(|| "0.0.0.0".to_string());
    let cancel_token = {
        let mut token_lock: tokio::sync::MutexGuard<'_, Option<CancellationToken>> =
            state.cancel_token.lock().await;
        if token_lock.is_some() {
            return Err("Server is already running".to_string());
        }
        let token = CancellationToken::new();
        *token_lock = Some(token.clone());
        token
    };

    // Start mDNS
    {
        let mut mdns_lock = state.mdns_manager.lock().await;
        match crate::network::NetworkManager::start_mdns(port, &bind_addr) {
            Ok(manager) => {
                *mdns_lock = Some(manager);
            }
            Err(e) => {
                eprintln!("Failed to start mDNS: {}", e);
            }
        }
    }

    let dsp_settings = state.dsp_settings.clone();

    let output_buffer_ms = dsp_settings
        .read()
        .map(|s| (s.output_buffer_ms as usize).clamp(100, 1200))
        .unwrap_or(800);

    // Locate bundled resources (ONNX models + alsa config) once for the whole
    // startup. On a packaged deb this does NOT equal Tauri's resource_dir().
    let resource_root = find_resource_dir(resource_dir.as_deref());

    // Load the ONNX Runtime shared library.  The official Microsoft build uses
    // runtime CPUID dispatch for AVX2/SSE kernels, so it works on CPUs without
    // AVX2 (unlike the pykeio prebuilt binaries with x86-64-v3 baseline).
    if let Some(ort_path) = find_ort_runtime(resource_root.as_deref()) {
        if let Err(e) = micyou_audio::init_ort_runtime(&ort_path) {
            log::error!(
                "Failed to load ONNX Runtime from {}: {e}",
                ort_path.display()
            );
        }
    } else {
        log::warn!(
            "ONNX Runtime library ({}) not found",
            ort_runtime_filename()
        );
    }

    let resolved_output_device = output_device;
    // Bound queued latency: Android packets are ~7 ms, so 128 slots provide ample
    // scheduling headroom without retaining seconds of stale audio.
    let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel(128);

    // Start audio output pipeline (shared by all modes)
    let events_audio = events.clone();
    let is_web_mode = mode == "web";
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();

    let ptt_blocked = state.ptt_blocked.clone();
    let is_monitoring_flag = state.is_monitoring.clone();
    let spectrum_streaming_enabled = state.spectrum_streaming_enabled.clone();
    let active_audio_session_audio = state.active_audio_session.clone();
    // The audio output device is persistent (created at app startup or on the
    // first server start) and shared across server restarts. The audio thread
    // only pushes decoded PCM into it; it never owns or tears it down.
    let audio_output_shared = state.audio_output.clone();
    let audio_allowed = state.audio_allowed.clone();
    let local_muted = state.local_muted.clone();
    let network_stats_audio = state.network_stats.clone();
    let pcm_sink = state.pcm_sink.clone();
    let skip_audio_output = state.skip_audio_output.clone();

    let audio_thread = std::thread::spawn(move || {
        let using_sink_only = skip_audio_output.load(std::sync::atomic::Ordering::Relaxed);
        if !using_sink_only {
            // Ensure the virtual device is open. This is normally a no-op (already
            // opened at app startup); it also covers CLI/TUI first run and the rare
            // case where opening failed earlier and a later attempt succeeds.
            if !ensure_audio_output_started(
                &audio_output_shared,
                resolved_output_device,
                output_buffer_ms,
                resource_root.as_deref(),
            ) {
                let _ = ready_tx.send(Err("Virtual microphone output unavailable".to_string()));
                return;
            }
        }
        let _ = ready_tx.send(Ok(()));
        let mut dsp_processor = DspProcessor::new(dsp_settings.clone(), resource_root);
        // Attach the plugin DSP stage (runs when the chain reaches "Plugins").

        let mut jb = crate::jitter_buffer::JitterBuffer::new(12);
        let mut frame_counter: u32 = 0;
        let mut input_resampler: Option<micyou_audio::RubatoResampler> = None;
        let mut current_input_sample_rate: u32 = 0;
        let mut resample_out_buf = Vec::new();
        let mut pcm_f32 = Vec::new();
        // Opus decoder is keyed by (sample_rate, channel_count); recreated whenever
        // those change or a new transport session starts (stateful codec).
        let mut opus_decoder: Option<(u32, usize, crate::opus::Decoder)> = None;
        let mut opus_float_buf: Vec<f32> = Vec::new();

        // Speaker loopback capture for the AEC far-end reference. Windows uses
        // WASAPI loopback; Linux records the default physical playback sink.
        // Both start lazily only after an AEC-enabled session sends audio.
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        let loopback: Option<micyou_audio::LoopbackCapture> =
            Some(micyou_audio::LoopbackCapture::new());
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        let loopback: Option<micyou_audio::LoopbackCapture> = None;

        let mut audio_received_for_session = false;
        let mut aec_runtime_available = true;
        // A newly started server always begins with a fresh runtime state, even
        // before the first client session arrives.
        if loopback.is_some() {
            restore_aec_runtime(&mut aec_runtime_available, &dsp_settings, &events_audio);
        }
        // Sync the AEC far-end capture with actual audio flow. A control session
        // alone is not enough: while waiting for the first valid audio packet,
        // there is no microphone stream that needs an echo reference.
        let sync_loopback = |audio_received: &mut bool, runtime_available: &mut bool| {
            let transport_active = !matches!(
                *active_audio_session_audio.read().unwrap_or_else(
                    |poisoned: std::sync::PoisonError<
                        std::sync::RwLockReadGuard<'_, ActiveAudioSession>,
                    >| poisoned.into_inner()
                ),
                ActiveAudioSession::Inactive
            );
            if !transport_active {
                *audio_received = false;
            }

            let Some(lb) = &loopback else {
                return transport_active;
            };
            let aec_enabled = dsp_settings
                .read()
                .unwrap_or_else(
                    |poisoned: std::sync::PoisonError<
                        std::sync::RwLockReadGuard<'_, micyou_audio::dsp::AudioDspSettings>,
                    >| poisoned.into_inner(),
                )
                .aec_enabled;
            let should_capture = should_capture_loopback(
                transport_active,
                *audio_received,
                aec_enabled,
                *runtime_available,
            );

            if !should_capture {
                if lb.is_active() {
                    lb.stop();
                }
                return transport_active;
            }
            if lb.is_active() {
                return transport_active;
            }

            let failure = lb.take_failure_reason().or_else(|| lb.start().err());
            if let Some(reason) = failure {
                disable_aec_runtime(runtime_available, &events_audio, reason);
            } else {
                log::info!("[Audio] Starting speaker loopback capture for AEC");
            }
            transport_active
        };

        loop {
            // Idle heartbeat every 500ms: with no device session the loopback
            // capture stream stays stopped (biggest idle CPU win).
            match audio_rx.try_recv() {
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    // Poll fast (10ms) while a session is active: audio packets
                    // can arrive after a silence gap and must not sit in the
                    // channel for up to 500ms (that caused audible dropouts at
                    // the start of each utterance). Idle servers sleep 500ms.
                    let session_active =
                        sync_loopback(&mut audio_received_for_session, &mut aec_runtime_available);
                    std::thread::sleep(std::time::Duration::from_millis(if session_active {
                        10
                    } else {
                        500
                    }));
                }
                Ok(event) => {
                    audio_output_shared.set_monitoring(
                        is_monitoring_flag.load(std::sync::atomic::Ordering::Relaxed),
                    );
                    match event {
                        AudioStreamEvent::SessionStarting { expected, epoch } => {
                            audio_received_for_session = false;
                            if let Some(lb) = &loopback {
                                lb.reset_session();
                            }
                            dsp_processor.reset_aec_session();
                            opus_decoder = None;
                            if loopback.is_some() {
                                restore_aec_runtime(
                                    &mut aec_runtime_available,
                                    &dsp_settings,
                                    &events_audio,
                                );
                            }
                            jb.prepare_transport_session_epoch(expected, epoch);
                            continue;
                        }
                        AudioStreamEvent::Packet { packet, epoch } => {
                            audio_received_for_session = true;
                            sync_loopback(
                                &mut audio_received_for_session,
                                &mut aec_runtime_available,
                            );
                            jb.push_epoch(packet, epoch);
                        }
                    }
                    let packets: Vec<_> = std::iter::from_fn(|| jb.pop()).collect();

                    for ordered_packet in packets {
                        if let Some(audio_data) = ordered_packet.audio_packet {
                            if audio_data.codec == micyou_protocol::CODEC_OPUS {
                                // Opus decode: reorder on flags/sample-rate changes, then
                                // decode directly into f32 so it feeds the DSP chain intact.
                                let channels = audio_data.channel_count as usize;
                                let sample_rate = audio_data.sample_rate as u32;
                                let needs_decoder = match &opus_decoder {
                                    Some((sr, ch, _)) => *sr != sample_rate || *ch != channels,
                                    None => true,
                                };
                                if needs_decoder {
                                    let created =
                                        crate::opus::Channels::from_channel_count(channels)
                                            .and_then(|ch| {
                                                crate::opus::Decoder::new(sample_rate, ch).ok()
                                            });
                                    opus_decoder = created.map(|dec| (sample_rate, channels, dec));
                                    if opus_decoder.is_none() {
                                        eprintln!(
                                            "[Audio] Failed to create Opus decoder for {}Hz/{}ch",
                                            sample_rate, channels
                                        );
                                    }
                                }
                                if let Some((_, _, decoder)) = opus_decoder.as_mut() {
                                    let target_frames = (sample_rate as usize / 50) * channels; // 20ms
                                    if opus_float_buf.len() != target_frames {
                                        opus_float_buf.resize(target_frames, 0.0);
                                    }
                                    match decoder
                                        .decode_float(&audio_data.buffer, &mut opus_float_buf)
                                    {
                                        Ok(frames) => {
                                            pcm_f32.clear();
                                            pcm_f32.extend_from_slice(
                                                &opus_float_buf[..frames * channels],
                                            );
                                        }
                                        Err(e) => {
                                            eprintln!("[Audio] Opus decode error: {}", e);
                                            pcm_f32.clear();
                                        }
                                    }
                                } else {
                                    pcm_f32.clear();
                                }
                            } else {
                                let capacity = match audio_data.audio_format {
                                    2 => audio_data.buffer.len() / 2,
                                    3 => audio_data.buffer.len(),
                                    4 => audio_data.buffer.len() / 4,
                                    6 => audio_data.buffer.len() / 3,
                                    _ => 0,
                                };
                                pcm_f32.clear();
                                pcm_f32.reserve(capacity);
                                match audio_data.audio_format {
                                    2 => {
                                        for chunk in audio_data.buffer.chunks_exact(2) {
                                            let sample_i16 =
                                                i16::from_le_bytes([chunk[0], chunk[1]]);
                                            pcm_f32.push(sample_i16 as f32 / 32768.0);
                                        }
                                    }
                                    3 => {
                                        for &byte in &audio_data.buffer {
                                            let sample_f32 = (byte as f32 - 128.0) / 128.0;
                                            pcm_f32.push(sample_f32);
                                        }
                                    }
                                    4 => {
                                        for chunk in audio_data.buffer.chunks_exact(4) {
                                            let sample_f32 = f32::from_le_bytes([
                                                chunk[0], chunk[1], chunk[2], chunk[3],
                                            ]);
                                            pcm_f32.push(sample_f32);
                                        }
                                    }
                                    6 => {
                                        for chunk in audio_data.buffer.chunks_exact(3) {
                                            let sample24 = (chunk[0] as i32)
                                                | ((chunk[1] as i32) << 8)
                                                | ((chunk[2] as i8 as i32) << 16);
                                            let sample_f32 = (sample24 as f32) / 8388608.0;
                                            pcm_f32.push(sample_f32);
                                        }
                                    }
                                    _ => {
                                        eprintln!(
                                            "Unsupported audio format: {}",
                                            audio_data.audio_format
                                        );
                                    }
                                }
                            }
                            if !pcm_f32.is_empty() {
                                let channels = audio_data.channel_count as usize;
                                let sample_rate = audio_data.sample_rate as u32;

                                if sample_rate > 0 && sample_rate != 48000 {
                                    if current_input_sample_rate != sample_rate {
                                        match micyou_audio::RubatoResampler::new(
                                            sample_rate,
                                            48000,
                                            channels.max(1),
                                        ) {
                                            Ok(res) => {
                                                input_resampler = Some(res);
                                                current_input_sample_rate = sample_rate;
                                            }
                                            Err(e) => {
                                                eprintln!("Failed to create resampler: {}", e);
                                                input_resampler = None;
                                                current_input_sample_rate = 48000;
                                            }
                                        }
                                    }
                                    if let Some(ref mut resampler) = input_resampler {
                                        resampler.resample(
                                            &pcm_f32,
                                            channels.max(1),
                                            &mut resample_out_buf,
                                        );
                                        pcm_f32.clear();
                                        pcm_f32.extend_from_slice(&resample_out_buf);
                                    }
                                } else {
                                    input_resampler = None;
                                    current_input_sample_rate = 48000;
                                }

                                let queued_samples = audio_output_shared.queued_samples();
                                let queued_ms = if channels > 0 {
                                    (queued_samples as f64 / channels as f64) / 48.0
                                } else {
                                    0.0
                                };

                                // Web mode: skip DSP for now, output raw audio directly
                                let input_rms = (pcm_f32.iter().map(|v| v * v).sum::<f32>()
                                    / pcm_f32.len().max(1) as f32)
                                    .sqrt();
                                let processed_rms = if is_web_mode {
                                    let sum: f32 = pcm_f32.iter().map(|x| x * x).sum();
                                    (sum / pcm_f32.len() as f32).sqrt()
                                } else {
                                    // Read speaker loopback for AEC far-end reference.
                                    // This captures the ACTUAL speaker output (WASAPI/BlackHole/PipeWire),
                                    // which is the true echo source the phone mic picks up.
                                    // Feed one mono reference sample for each near-end frame.
                                    // Matching the processed frame count prevents drift when
                                    // packet sizes or input sample rates vary.
                                    let near_frames = pcm_f32.len() / channels.max(1);
                                    if let Some(far_data) = loopback
                                        .as_ref()
                                        .filter(|capture| capture.is_active())
                                        .map(|capture| capture.read(near_frames))
                                    {
                                        dsp_processor.set_far_end_audio(&far_data);
                                    }
                                    let (_raw, processed) = dsp_processor.process(
                                        &mut pcm_f32,
                                        channels.max(1),
                                        queued_ms,
                                    );
                                    if let Some(reason) = dsp_processor.take_aec_failure() {
                                        disable_aec_runtime(
                                            &mut aec_runtime_available,
                                            &events_audio,
                                            reason,
                                        );
                                    }
                                    processed
                                };

                                // Neeko: gate at the final output, independently of Android controls.
                                let audible = audio_allowed
                                    .load(std::sync::atomic::Ordering::Acquire)
                                    && !local_muted.load(std::sync::atomic::Ordering::Acquire)
                                    && !ptt_blocked.load(std::sync::atomic::Ordering::Acquire)
                                    && !network_stats_audio.is_muted();
                                if !audible {
                                    pcm_f32.fill(0.0);
                                }
                                if !using_sink_only {
                                    audio_output_shared.push(pcm_f32.clone(), channels.max(1));
                                }
                                // Route PCM to the optional sink (Neeko driver bridge).
                                // Clone the callback under the lock, call after releasing it.
                                let sink_clone = {
                                    if let Ok(guard) = pcm_sink.lock() {
                                        guard.clone()
                                    } else {
                                        None
                                    }
                                };
                                if let Some(ref sink) = sink_clone {
                                    // Normalize to mono for the bridge if stereo
                                    if channels > 1 {
                                        let mono: Vec<f32> = pcm_f32
                                            .chunks(channels)
                                            .map(|c| c.iter().sum::<f32>() / channels as f32)
                                            .collect();
                                        sink(&mono);
                                    } else {
                                        sink(&pcm_f32);
                                    }
                                }

                                frame_counter = frame_counter.wrapping_add(1);
                                if frame_counter.is_multiple_of(6) {
                                    let level = if audible {
                                        (processed_rms * 500.0).min(100.0) as u32
                                    } else {
                                        0
                                    };
                                    events_audio.input_level(
                                        (20.0 * input_rms.max(0.00001).log10()).clamp(-100.0, 0.0),
                                    );
                                    events_audio.audio_level(level);

                                    if spectrum_streaming_enabled
                                        .load(std::sync::atomic::Ordering::Acquire)
                                    {
                                        let (raw_spec, proc_spec) = dsp_processor.get_spectrums();
                                        events_audio.audio_spectrum(raw_spec, proc_spec);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(lb) = &loopback {
            let was_active = lb.is_active();
            lb.stop();
            if was_active {
                log::info!("[Audio] Speaker loopback stopped");
            }
        }
    });

    // This guard must end before startup/rollback acquires the lifecycle again.
    {
        let mut lifecycle_lock = state.lifecycle.lock().await;
        lifecycle_lock.set_audio_thread(audio_thread);
    }
    let audio_ready = await_startup_ready(ready_rx, "Audio output", STARTUP_TIMEOUT)
        .await
        .map_err(|error| format!("Failed to start audio output: {}", error));
    if let Err(error) = audio_ready {
        let rollback = rollback_start(state, &cancel_token, Vec::new()).await;
        return Err(match rollback {
            Ok(()) => error,
            Err(cleanup) => format!("{}; {}", error, cleanup),
        });
    }

    let events_tcp = events.clone();
    let token_tcp = cancel_token.clone();
    let port_tcp = port;
    let audio_tx_tcp = audio_tx.clone();
    let stats_tcp = state.network_stats.clone();
    let mode_tcp = mode.clone();
    let bind_addr_tcp = bind_addr.clone();
    let active_connection_tcp = state.active_connection.clone();
    let takeover_lock_tcp = state.takeover_lock.clone();
    let active_audio_session_tcp = state.active_audio_session.clone();
    let (tcp_ready_tx, tcp_ready_rx) = tokio::sync::oneshot::channel();
    let tcp_task = tokio::spawn(async move {
        if let Err(e) = crate::tcp_server::start_tcp_server(
            events_tcp,
            port_tcp,
            bind_addr_tcp,
            token_tcp,
            audio_tx_tcp,
            stats_tcp,
            mode_tcp,
            active_connection_tcp,
            takeover_lock_tcp,
            active_audio_session_tcp,
            tcp_ready_tx,
        )
        .await
        {
            eprintln!("TCP Server error: {}", e);
        }
    });

    let token_udp = cancel_token.clone();
    let port_udp = udp_port.expect("non-web port validation must produce a UDP port");
    let stats_udp = state.network_stats.clone();
    let active_audio_session_udp = state.active_audio_session.clone();
    let bind_addr_udp = bind_addr.clone();
    let (udp_ready_tx, udp_ready_rx) = tokio::sync::oneshot::channel();
    let udp_task = tokio::spawn(async move {
        if let Err(e) = crate::udp_server::start_udp_server(
            audio_tx,
            port_udp,
            bind_addr_udp,
            token_udp,
            stats_udp,
            active_audio_session_udp,
            udp_ready_tx,
        )
        .await
        {
            eprintln!("UDP Server error: {}", e);
        }
    });

    let (tcp_ready, udp_ready) = tokio::join!(
        await_startup_ready(tcp_ready_rx, "TCP server", STARTUP_TIMEOUT),
        await_startup_ready(udp_ready_rx, "UDP server", STARTUP_TIMEOUT),
    );
    if let Err(error) = tcp_ready.and(udp_ready) {
        let error = format!("Failed to start network server: {}", error);
        let rollback = rollback_start(state, &cancel_token, vec![tcp_task, udp_task]).await;
        return Err(match rollback {
            Ok(()) => error,
            Err(cleanup) => format!("{}; {}", error, cleanup),
        });
    }
    {
        let mut tasks: tokio::sync::MutexGuard<'_, Vec<JoinHandle<()>>> =
            state.background_tasks.lock().await;
        tasks.extend([tcp_task, udp_task]);
    }
    let mut lifecycle_mark: tokio::sync::MutexGuard<'_, ServerLifecycleState> =
        state.lifecycle.lock().await;
    lifecycle_mark.mark_running();

    Ok(format!("Server started on port {}", port))
}

pub async fn stop_server_inner(
    state: &ServerState,
    events: crate::events::SharedEvents,
) -> Result<String, String> {
    let _lifecycle_guard = state.lifecycle_gate.enter().await;
    state
        .spectrum_streaming_enabled
        .store(false, std::sync::atomic::Ordering::Release);

    let mut mdns_lock: tokio::sync::MutexGuard<'_, Option<crate::network::NetworkManager>> =
        state.mdns_manager.lock().await;
    if let Some(mdns) = mdns_lock.take() {
        mdns.stop_mdns();
    }

    let token: Option<CancellationToken> = state.cancel_token.lock().await.take();
    let had_token = token.is_some();
    if let Some(token) = token {
        token.cancel();
    }
    {
        let mut lifecycle_stop = state.lifecycle.lock().await;
        lifecycle_stop.begin_stopping();
    }
    let tasks = std::mem::take(&mut *state.background_tasks.lock().await);
    crate::tcp_server::cleanup_session_state(&state.active_connection, &state.active_audio_session)
        .await;
    join_tasks_bounded(tasks, NETWORK_TASK_JOIN_TIMEOUT).await;
    let mut lifecycle_join: tokio::sync::MutexGuard<'_, ServerLifecycleState> =
        state.lifecycle.lock().await;
    let audio_result = lifecycle_join.join_audio_bounded(AUDIO_JOIN_TIMEOUT).await;
    audio_result?;
    if had_token {
        events.server_stopped();
        Ok("Server stopped".to_string())
    } else {
        Ok("Server already stopped".to_string())
    }
}
