"""Integration checks using real H.264 videos and the untouched upstream engine.
Run: python -B scripts/tiktok.test.py (requires FFmpeg).
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENGINE = ROOT / 'src-tauri/vendor/tiktok-quality/src'
RUNNER = ROOT / 'src-tauri/scripts/tiktok_runner.py'
sys.path.insert(0, str(ENGINE))
from tiktok_quality import transform
spec = importlib.util.spec_from_file_location('tiktok_runner', RUNNER)
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class TikTokTests(unittest.TestCase):
    def run_neeko(self, source, engine='bastien', options=None):
        return subprocess.run([sys.executable, '-I', '-B', '-X', 'utf8', str(RUNNER), str(ENGINE), str(source), engine, json.dumps(options or {})], capture_output=True, text=True, encoding='utf-8')

    def test_all_additional_engines_with_real_video(self):
        from tiktok_quality.mp4.parser import find_box
        from tiktok_engines import load_luis
        import contextlib
        import io
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / 'entrada ñ con espacios.mp4'
            subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=s=320x180:r=60',
                            '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000',
                            '-t', '0.2', '-c:v', 'libx264', '-c:a', 'aac', str(source)], check=True)
            original = source.read_bytes()
            def payload(data):
                pos, size = find_box(data, 'mdat')
                return data[pos + 8:pos + size]
            cases = [('upload120', {'method': method}) for method in ['extension-signal', 'balanced-sync', 'classic-force']]
            cases += [('utoku', {'fps': fps}) for fps in ['60', '120']]
            cases += [('luisalves', {'fps': fps}) for fps in ['auto', '60', '120', '240']]
            cases += [('paschafps', {'fps': fps}) for fps in ['60', '120', '240']]
            cases += [('mistic', {'preset': '540', 'encoder': 'libx265'})]
            for engine, options in cases:
                with self.subTest(engine=engine, options=options):
                    result = self.run_neeko(source, engine, options)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    info = json.loads(result.stdout)
                    output = Path(info['path'])
                    self.assertIn(engine, output.name)
                    self.assertEqual(info['engine'], engine)
                    data = output.read_bytes()
                    self.assertNotEqual(data, original)
                    if engine in ['utoku', 'luisalves', 'upload120']:
                        self.assertEqual(payload(data), payload(original))
                    if engine == 'luisalves':
                        expected = bytearray(original)
                        factor = 30 / (60 if options['fps'] == 'auto' else float(options['fps']))
                        with contextlib.redirect_stdout(io.StringIO()):
                            for box in ['mvhd', 'mdhd']:
                                load_luis().patch_atom(box, expected, factor)
                        self.assertEqual(data, expected)
                    if engine == 'paschafps':
                        reference = Path(tmp) / 'pascha-reference.mp4'
                        scale = {'60': '2', '120': '6', '240': '12'}[options['fps']]
                        subprocess.run(['ffmpeg', '-v', 'error', '-y', '-itsscale', scale, '-i', str(source),
                                        '-c:v', 'copy', '-c:a', 'copy', str(reference)], check=True)
                        self.assertEqual(data, reference.read_bytes())
                    if engine == 'mistic':
                        index = data.index(b'elst')
                        self.assertEqual(data[index + 8:index + 12], b'\x10\x00\x00\x01')
                        probe = subprocess.run(['ffprobe', '-v', 'error', '-select_streams', 'v:0',
                                                '-show_entries', 'stream=codec_name,width,height', '-of', 'json', str(output)], capture_output=True, check=True)
                        video = json.loads(probe.stdout)['streams'][0]
                        self.assertEqual(video, {'codec_name': 'hevc', 'width': 960, 'height': 540})
            self.assertEqual(source.read_bytes(), original)
            self.assertFalse(list(Path(tmp).glob('.neeko-tiktok-*')))

    def test_invalid_engine_options_and_missing_dependencies(self):
        from tiktok_engines import validate, executable
        for name, options in [('unknown', {}), ('bastien', {'fps': '60'}), ('mistic', {'encoder': 'shell'}),
                              ('upload120', {'divider': '0'}), ('utoku', {'fps': '240'})]:
            with self.subTest(name=name):
                with self.assertRaises(ValueError):
                    validate(name, options)
        with self.assertRaises(RuntimeError):
            executable('ffmpeg', 'Z:/definitely-missing-neeko/ffmpeg.exe')

    def test_identical_output_original_preserved_and_duplicate_names(self):
        with tempfile.TemporaryDirectory() as tmp:
            for audio in [False, True]:
                with self.subTest(audio=audio):
                    source = Path(tmp) / f'video con espacios ñ {audio}.mp4'
                    command = ['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i', 'color=c=blue:s=1920x1080:r=60']
                    if audio:
                        command += ['-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000']
                    subprocess.run(command + ['-t', '0.5', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-c:a', 'aac', str(source)], check=True)
                    before = hashlib.sha256(source.read_bytes()).digest()
                    if sys.platform == 'win32':
                        self.assertTrue(runner.windows_video_support(source))
                    reference = Path(tmp) / 'reference.mp4'
                    transform(str(source), str(reference), verbose=False)
                    outputs = []
                    for _ in range(2):
                        result = self.run_neeko(source)
                        self.assertEqual(result.returncode, 0, result.stderr)
                        data = json.loads(result.stdout)
                        output = Path(data['path'])
                        self.assertEqual(output.read_bytes(), reference.read_bytes())
                        self.assertEqual(data['stats']['declared_frames'], data['stats']['original_frames'] * 10)
                        self.assertEqual(data['source'], str(source.resolve()))
                        if sys.platform == 'win32':
                            self.assertIs(data['windows_video'], False,
                                          'The unchanged ghost-frame output must be reported as unsupported by Windows')
                        outputs.append(output)
                    self.assertNotEqual(*outputs)
                    self.assertEqual(hashlib.sha256(source.read_bytes()).digest(), before)
                    # Comparing to upstream alone can reproduce an upstream bug.
                    # Decode real frames separately; ghost NALs can make FFmpeg
                    # return an error even after all real frames were decoded.
                    decoded = []
                    for video in [source, outputs[0]]:
                        result = subprocess.run(['ffmpeg', '-v', 'error', '-i', str(video),
                                                 '-map', '0:v:0', '-frames:v', '30',
                                                 '-f', 'framemd5', '-'], capture_output=True)
                        frames = [line for line in result.stdout.splitlines() if not line.startswith(b'#')]
                        self.assertEqual(len(frames), 30)
                        decoded.append(frames)
                    self.assertEqual(*decoded)

    def test_invalid_input_leaves_no_output_or_temporary_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / 'broken.mp4'
            source.write_bytes(b'not a video')
            result = self.run_neeko(source)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('motor original', result.stderr)
            self.assertEqual(list(Path(tmp).iterdir()), [source])
            self.assertNotEqual(self.run_neeko(Path(tmp) / 'missing.mp4').returncode, 0)


if __name__ == '__main__':
    unittest.main()
