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

use crate::stats::AudioMetrics;
use crate::tcp_server::DeviceInfo;
use std::sync::Arc;

/// Events emitted by the audio server core, decoupled from Tauri.
///
/// The GUI implements this via `TauriEventSink` (wraps `AppHandle.emit`); CLI/TUI
/// implement it by updating TUI state or writing log lines. This keeps the server
/// core callable without a running Tauri runtime.
pub trait ServerEvents: Send + Sync + 'static {
    fn device_connected(&self, info: DeviceInfo);
    fn device_disconnected(&self);
    fn audio_metrics(&self, metrics: AudioMetrics);
    fn udp_audio_warning(&self);
    fn mute_state_changed(&self, is_muted: bool);
    fn audio_level(&self, level: u32);
    fn input_level(&self, _db: f32) {}
    fn audio_spectrum(&self, raw: Vec<f32>, processed: Vec<f32>);
    fn server_stopped(&self);
    fn web_client_count(&self, count: u32);
    fn install_progress(&self, message: String);
    fn aec_status_changed(&self, status: AecStatus);
    fn monitoring_state_changed(&self, _enabled: bool) {}
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct AecStatus {
    pub available: bool,
    pub enabled: bool,
    pub reason: Option<micyou_audio::AecFailure>,
}

pub type SharedEvents = Arc<dyn ServerEvents>;
