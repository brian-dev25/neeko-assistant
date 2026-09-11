"""Extract the reviewed MicYou core; usage: python scripts/vendor-micyou.py CHECKOUT.

Only run against the pinned revision. Keeps upstream headers and tests; removes
the desktop UI/runtime and MicYou's own plugin host, retaining its wire schema.
"""
from pathlib import Path
import re
import shutil
import subprocess
import sys

REV = '0bf286b0d06552a5fd406c0b1695a1bd52e9916b'
source = Path(sys.argv[1])
assert subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip() == REV
dest = Path(__file__).resolve().parents[1] / 'src-tauri/vendor/micyou'
if (dest / 'core/src').exists():
    raise SystemExit('This extracts the upstream baseline only. Refusing to overwrite reviewed Neeko adaptations.')
dest.mkdir(parents=True, exist_ok=True)
shutil.copyfile(source / 'LICENSE', dest / 'LICENSE')
for name in ['micyou-audio', 'micyou-protocol']:
    shutil.copytree(source / 'tauri-app/crates' / name, dest / name, dirs_exist_ok=True)
    manifest = dest / name / 'Cargo.toml'
    s = manifest.read_text(encoding='utf-8')
    for key, val in [('version', '2.0.3'), ('authors', ['LanRhyme']), ('edition', '2021'), ('license', 'GPL-3.0-or-later')]:
        import json
        s = s.replace(f'{key}.workspace = true', f'{key} = {json.dumps(val)}')
    manifest.write_text(s, encoding='utf-8')
core = dest / 'core/src'
core.mkdir(parents=True, exist_ok=True)
upstream = source / 'tauri-app/src-tauri/src'
modules = ['audio_output', 'audio_stream', 'jitter_buffer', 'network', 'opus', 'server', 'stats', 'tcp_server', 'udp_server', 'adb_manager']
for name in modules:
    s = (upstream / f'{name}.rs').read_text(encoding='utf-8')
    if name == 'server':
        s = '\n'.join(line for line in s.splitlines() if 'plugins:' not in line)
    if name == 'tcp_server':
        s = re.sub(r'\s*plugins.broadcast_event\(&micyou_plugin::PluginEvent::DeviceConnected \{.*?\}\);', '', s, flags=re.S)
        s = re.sub(r'    if let Some\(plugin_message\) = msg.plugin_message \{.*?\n    \}', '    // Neeko does not load MicYou plugins; ignore their optional wire messages.', s, flags=re.S)
        s = '\n'.join(line for line in s.splitlines() if not any(x in line for x in ['plugins:', 'let plugins', 'plugins.', 'plugins,', 'plugins_reader.', 'plugin sync', 'dispatcher, pending RPCs']))
    (core / f'{name}.rs').write_text(s + '\n', encoding='utf-8')
s = (upstream / 'events.rs').read_text(encoding='utf-8')
s = s[:s.index('/// Tauri adapter:')]
s = '\n'.join(line for line in s.splitlines() if 'use tauri' not in line and 'SpectrumPayload' not in line)
(core / 'events.rs').write_text(s + '\n', encoding='utf-8')
s = (upstream / 'commands/system.rs').read_text(encoding='utf-8')
header = s[:s.index('use serde::Serialize;')]
helpers = s[s.index('fn validate_server_port'):s.index('#[derive(serde::Serialize, Clone)]\npub struct SpectrumPayload')]
helpers = helpers.replace('use crate::tray::{TrayContext, TrayMenuStrings, TrayState};', '')
start = s[s.index('pub async fn start_server_inner'):s.index('#[tauri::command]\npub async fn stop_server(')]
start = start[:start.index('    // Wire control plane handlers')] + start[start.index('    let dsp_settings = state.dsp_settings.clone();'):]
start = start.replace('    state.plugins.ensure_plugin_chain_node(&dsp_settings);', '')
start = re.sub(r'        if let Some\(hook\) = plugins_shared.dsp_hook\(\) \{.*?\n        \}', '', start, flags=re.S)
start = '\n'.join(line for line in start.splitlines() if 'let plugins_' not in line and '            plugins_tcp,' not in line)
start = start.replace('eprintln!("[Audio] Output device unavailable; audio will be silent");', 'let _ = ready_tx.send(Err("Virtual microphone output unavailable".to_string()));\n            return;')
stop = s[s.index('pub async fn stop_server_inner'):s.index('#[tauri::command]\npub fn set_tray_strings')]
imports = '''use tokio_util::sync::CancellationToken;
use crate::audio_stream::{validate_audio_packet, AudioStreamEvent, ExpectedAudioSession};
use crate::server::{await_startup_ready, ServerState, AUDIO_JOIN_TIMEOUT, STARTUP_TIMEOUT};
use crate::udp_server::ActiveAudioSession;
use micyou_audio::AecFailure;
const NETWORK_TASK_JOIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
'''
(core / 'system.rs').write_text(header + imports + helpers + start + '\n' + stop, encoding='utf-8')
(core / 'lib.rs').write_text('// Adapted from MicYou; see ../LICENSE and ../../README.md.\n' + '\n'.join(f'pub mod {m};' for m in modules + ['events', 'system']) + '\n', encoding='utf-8')
# An explicitly selected virtual endpoint must NEVER fall back to speakers.
engine = dest / 'micyou-audio/src/engine.rs'
s = engine.read_text(encoding='utf-8').replace('matched_device.or_else(|| host.default_output_device())', 'matched_device')
s = s.replace('cable_device.or_else(|| host.default_output_device())', 'cable_device')
s = s.replace('falling back to default.', 'refusing speaker fallback.')
engine.write_text(s, encoding='utf-8')
print(f'Extracted MicYou {REV} to {dest}')
