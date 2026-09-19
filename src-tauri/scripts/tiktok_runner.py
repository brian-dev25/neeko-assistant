"""Neeko file handling around the unchanged upstream transform."""
import json
import os
from pathlib import Path
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))


def windows_video_support(path):
    """Ask the same Windows media stack used by the system player.

    True means a video stream is recognized, not that full playback is proven.
    None means the check is unavailable; it must never imply compatibility.
    """
    if sys.platform != 'win32':
        return None
    import ctypes as c
    from ctypes import wintypes as w
    import uuid

    class GUID(c.Structure):
        _fields_ = [('data', c.c_ubyte * 16)]

        def __init__(self, value):
            super().__init__((c.c_ubyte * 16).from_buffer_copy(uuid.UUID(value).bytes_le))

    def call(pointer, index, types, *args):
        table = c.cast(pointer, c.POINTER(c.POINTER(c.c_void_p))).contents
        return c.WINFUNCTYPE(c.c_long, c.c_void_p, *types)(table[index])(pointer, *args)

    reader = c.c_void_p()
    initialized = started = False
    try:
        ole = c.OleDLL('ole32')
        ole.CoInitializeEx(None, 0)
        initialized = True
        mf = c.WinDLL('mfplat')
        rw = c.WinDLL('mfreadwrite')
        if mf.MFStartup(0x20070, 0) < 0:
            return None
        started = True
        rw.MFCreateSourceReaderFromURL.argtypes = [w.LPCWSTR, c.c_void_p, c.POINTER(c.c_void_p)]
        result = rw.MFCreateSourceReaderFromURL(str(path), None, c.byref(reader))
        if result < 0:
            # With no surviving audio track, Windows rejects the whole stream.
            return False if result & 0xFFFFFFFF == 0xC00D36C4 else None
        # MF_SOURCE_READER_FIRST_VIDEO_STREAM; GetNativeMediaType.
        media = c.c_void_p()
        result = call(reader, 5, [w.DWORD, w.DWORD, c.POINTER(c.c_void_p)],
                      0xFFFFFFFC, 0, c.byref(media))
        if result < 0:
            return False if result & 0xFFFFFFFF == 0xC00D36B3 else None
        try:
            major = GUID('00000000-0000-0000-0000-000000000000')
            key = GUID('48eba18e-f8c9-4687-bf11-0a74c9f96a8f')
            if call(media, 10, [c.POINTER(GUID), c.POINTER(GUID)], c.byref(key), c.byref(major)) < 0:
                return None
            return str(uuid.UUID(bytes_le=bytes(major.data))) == '73646976-0000-0010-8000-00aa00389b71'
        finally:
            call(media, 2, [])
    except (OSError, AttributeError):
        return None
    finally:
        if reader.value:
            call(reader, 2, [])
        if started:
            mf.MFShutdown()
        if initialized:
            ole.CoUninitialize()


def prepare(source, engine='bastien', options=None, ffmpeg='', ffprobe=''):
    from tiktok_engines import transform_selected, validate
    options = {} if options is None else options
    validate(engine, options)
    source = Path(source).resolve(strict=True)
    if not source.is_file():
        raise ValueError('Elegí un archivo de video.')
    # Upstream owns all video processing, including its default multiplier/tag.
    # Only publish a completed file, never replace an existing video.
    with tempfile.TemporaryDirectory(prefix='.neeko-tiktok-', dir=source.parent) as tmp:
        staged = Path(tmp) / 'output.mp4'
        stats = transform_selected(source, staged, engine, options, ffmpeg, ffprobe)
        windows_video = windows_video_support(staged)
        for index in range(10000):
            suffix = '' if index == 0 else f'_{index}'
            method_tag = '' if engine == 'bastien' else '_' + engine
            if engine == 'upload120':
                method_tag += '_' + stats['method']
            output = source.with_name(f'{source.stem}_tiktok{method_tag}{suffix}.mp4')
            try:
                os.link(staged, output)
                return {'path': str(output), 'source': str(source), 'stats': stats,
                        'windows_video': windows_video, 'engine': engine}
            except FileExistsError:
                continue
        raise ValueError('Demasiadas copias con el mismo nombre en esta carpeta.')


if __name__ == '__main__':
    sys.path.insert(0, sys.argv[1])
    try:
        if sys.version_info < (3, 10):
            raise RuntimeError('Necesitás Python 3.10 o superior.')
        print(json.dumps(prepare(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else 'bastien',
                                 json.loads(sys.argv[4]) if len(sys.argv) > 4 else {},
                                 sys.argv[5] if len(sys.argv) > 5 else '',
                                 sys.argv[6] if len(sys.argv) > 6 else ''), ensure_ascii=True))
    except (Exception, SystemExit) as error:
        print(f'No pude preparar el video con el motor seleccionado (motor original o adaptador). Detalle: {error}', file=sys.stderr)
        sys.exit(1)
