"""Engine adapters. Upstream sources are pinned under vendor/tiktok-engines."""
import contextlib
import importlib.util
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
VENDOR = HERE.parent / 'vendor' / 'tiktok-engines'
CATALOG = json.loads((HERE / 'tiktok-engines.json').read_text(encoding='utf-8'))


def validate(engine, options):
    spec = next((item for item in CATALOG if item['id'] == engine), None)
    if spec is None:
        raise ValueError('Motor TikTok desconocido.')
    if not isinstance(options, dict) or options.keys() - {field['id'] for field in spec['fields']}:
        raise ValueError('Opciones de motor inválidas.')
    values = {}
    for field in spec['fields']:
        value = options.get(field['id'], field['default'])
        if value not in [choice[0] for choice in field['choices']]:
            raise ValueError(f"Valor inválido para {field['label']}.")
        values[field['id']] = value
    return spec, values


def executable(name, configured=''):
    candidates = [configured] if configured else [shutil.which(name)]
    if not configured and os.name == 'nt':
        candidates += ([str(Path(os.environ.get('ProgramFiles', 'C:/Program Files')) / 'nodejs/node.exe')]
                       if name == 'node' else [f'C:/ffmpeg/{name}.exe', f'C:/ffmpeg/bin/{name}.exe'])
    for candidate in candidates:
        if candidate and (Path(candidate).is_file() or shutil.which(candidate)):
            return candidate
    raise RuntimeError(f'No se encontró {name}. Instalalo o configurá su ruta en Neeko.')


def run(args, timeout=None):
    flags = subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0
    # Logs go to a temporary file to avoid unbounded RAM use on long encodes.
    import tempfile
    with tempfile.TemporaryFile() as errors:
        result = subprocess.run([str(x) for x in args], stdout=subprocess.PIPE, stderr=errors,
                                creationflags=flags, timeout=timeout, stdin=subprocess.DEVNULL)
        if result.returncode:
            errors.seek(0, 2)
            errors.seek(max(0, errors.tell() - 4000))
            raise RuntimeError(errors.read().decode('utf-8', errors='replace').strip() or f'El proceso terminó con código {result.returncode}.')
        return result.stdout


def fps_value(source, value, ffprobe):
    if value != 'auto':
        return float(value)
    output = run([executable('ffprobe', ffprobe), '-v', 'error', '-select_streams', 'v:0',
                  '-show_entries', 'stream=r_frame_rate', '-of', 'json', source], timeout=30)
    streams = json.loads(output).get('streams', [])
    if not streams:
        raise ValueError('No se encontró una pista de video.')
    from fractions import Fraction
    fps = round(float(Fraction(streams[0]['r_frame_rate'])), 2)
    if not math.isfinite(fps) or fps <= 0:
        raise ValueError('No pude detectar los FPS del video.')
    return fps


def load_luis():
    spec = importlib.util.spec_from_file_location('luis_patcher', VENDOR / 'luisalves/patcher.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def mistic_encode(source, staged, options, ffmpeg):
    encoder = options['encoder']
    if encoder == 'auto':
        encoder = 'libx265'
        for candidate in ['hevc_nvenc', 'hevc_amf', 'hevc_qsv']:
            try:
                run([ffmpeg, '-f', 'lavfi', '-i', 'nullsrc=s=128x128:d=0.04', '-frames:v', '1',
                     '-c:v', candidate, '-f', 'null', '-'], timeout=10)
                encoder = candidate
                break
            except (RuntimeError, subprocess.TimeoutExpired):
                pass
    # Match upstream's bounding square, bitrate, FPS and encoder parameters.
    dimension, bitrate = {'1080': (1920, 20), '720': (1280, 12), '540': (960, 7)}[options['preset']]
    args = ['-c:v', encoder, '-b:v', f'{bitrate}M', '-bufsize', f'{bitrate * 2}M']
    if encoder != 'libx265':
        args += ['-maxrate', f'{round(bitrate * 1.2)}M']
    args += {'hevc_nvenc': ['-rc', 'vbr', '-preset', 'p7', '-spatial_aq', '1', '-temporal_aq', '1'],
             'hevc_amf': ['-quality', 'quality'], 'hevc_qsv': ['-look_ahead', '1'],
             'libx265': ['-preset', 'slow']}[encoder]
    run([ffmpeg, '-y', '-i', source, '-vf',
         f'scale={dimension}:{dimension}:force_original_aspect_ratio=decrease,fps=60',
         *args, '-c:a', 'copy', '-tag:v', 'hvc1', staged])
    data = bytearray(staged.read_bytes())
    index = data.find(b'elst')
    if index < 0 or index + 12 > len(data):
        raise ValueError('Mistic: el encoder no produjo el box elst necesario para el parche.')
    data[index + 8:index + 12] = b'\x10\x00\x00\x01'
    staged.write_bytes(data)
    return {'encoder': encoder, 'preset': options['preset'], 'fps': 60}


def transform_selected(source, staged, engine, options, ffmpeg='', ffprobe=''):
    spec, options = validate(engine, options)
    if source.suffix.lower() not in spec['extensions']:
        raise ValueError(f"{spec['name']} admite: {', '.join(spec['extensions'])}.")
    if engine == 'bastien':
        from tiktok_quality import transform
        return transform(str(source), str(staged), verbose=False)
    if engine == 'upload120':
        return json.loads(run([executable('node'), HERE / 'upload120-runner.cjs',
                              VENDOR / 'upload120/docs/patcher.browser.js', source, staged,
                              json.dumps(options)]))
    if engine in ['utoku', 'luisalves']:
        fps = fps_value(source, options['fps'], ffprobe)
        if engine == 'utoku':
            if fps not in [60, 120]:
                raise ValueError('ut0ku admite únicamente entrada de 60 o 120 FPS.')
            from tiktok_utoku import patch
            data = bytearray(source.read_bytes())
            count = patch(data, 4 if fps == 120 else 2)
            staged.write_bytes(data)
            return {'fps': fps, 'patched_boxes': count}
        with contextlib.redirect_stdout(sys.stderr):
            load_luis().patch_mp4(str(source), str(staged), scale_factor=30 / fps)
        if staged.read_bytes() == source.read_bytes():
            raise ValueError('LuisAlves no produjo cambios: comprobá que el archivo tenga cabeceras compatibles y más de 30 FPS.')
        return {'fps': fps, 'scale_factor': 30 / fps}
    ffmpeg = executable('ffmpeg', ffmpeg)
    if engine == 'paschafps':
        scale = {'60': 2, '120': 6, '240': 12}[options['fps']]
        run([ffmpeg, '-y', '-itsscale', scale, '-i', source, '-c:v', 'copy', '-c:a', 'copy', staged])
        return {'fps': int(options['fps']), 'scale': scale}
    return mistic_encode(source, staged, options, ffmpeg)
