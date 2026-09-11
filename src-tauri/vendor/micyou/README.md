# MicYou core adapted for Neeko

Source: https://github.com/LanRhyme/MicYou

Pinned revision: `0bf286b0d06552a5fd406c0b1695a1bd52e9916b` (2.0.3).
Copyright © 2026 LanRhyme. See the full upstream [LICENSE](LICENSE).

`micyou-protocol` and `micyou-audio` originate in `tauri-app/crates`.
`core/src` originates in `tauri-app/src-tauri/src`; `core/src/system.rs` extracts
server lifecycle and audio pipeline from `commands/system.rs`. Headers and tests
are retained. The upstream desktop application, Tauri entrypoint, Vue frontend,
plugin host, tray, web mode and CLI/TUI were not included.

Neeko adaptations include local approval/mute gates, private-network restriction,
no active session takeover, no output-device fallback, finite PCM/codec validation,
TCP telemetry, joined audio output thread and bounded hidden ADB commands with
reverse cleanup. See [the integration audit](../../../docs/phone-microphone.md).

`scripts/vendor-micyou.py` records the original extraction procedure. It extracts
an upstream baseline, **not** all subsequent Neeko patches, and refuses to overwrite
this adapted source tree. Build directly from these checked-in sources.

These are internal components of MicYou, not a plugin loaded through its public
plugin APIs. The MicYou Plugin Exception does not waive GPL obligations for this
adaptation. Distributing the combined executable requires fulfilling GPL terms,
including corresponding source. Preserve this license and the per-file headers.
