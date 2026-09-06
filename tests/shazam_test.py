import asyncio
import importlib.util
import io
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import patch
import wave
import numpy as np

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('recognition', Path(__file__).parents[1] / 'addons/shazam/recognize.py')
recognition = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recognition)


class RecognitionTests(unittest.TestCase):
    def run_capture(self, audio, result, loopback=True):
        calls = []
        class Recorder:
            def __enter__(self): return self
            def __exit__(self, *args): pass
            def record(self, numframes):
                self_test.assertEqual(numframes, 480000)
                return audio
        class Shazam:
            async def recognize(self, data):
                with wave.open(io.BytesIO(data)) as wav:
                    self_test.assertEqual(wav.getnchannels(), 1)
                    self_test.assertEqual(wav.getframerate(), 48000)
                    self_test.assertEqual(wav.getsampwidth(), 2)
                calls.append(data)
                return result
        self_test = self
        def get_microphone(id, include_loopback):
            self.assertEqual(id, 'output-device')
            self.assertTrue(include_loopback)
            return types.SimpleNamespace(isloopback=loopback, recorder=lambda samplerate: Recorder())
        soundcard = types.SimpleNamespace(default_speaker=lambda: types.SimpleNamespace(id='output-device'), get_microphone=get_microphone)
        with patch.dict(sys.modules, {'soundcard': soundcard, 'shazamio': types.SimpleNamespace(Shazam=Shazam)}):
            response = asyncio.run(recognition.recognize())
        return response, calls

    def test_silence_never_contacts_recognition_service(self):
        response, calls = self.run_capture(np.zeros((480, 2)), {})
        self.assertIn('No escuché sonido', response['message'])
        self.assertEqual(calls, [])

    def test_stereo_capture_becomes_wav_and_returns_track(self):
        response, calls = self.run_capture(np.full((480, 2), .2), {'track': {'title': 'Song', 'subtitle': 'Artist'}})
        self.assertEqual(response['message'], 'Song — Artist')
        self.assertEqual(len(calls), 1)

    def test_no_match(self):
        response, _ = self.run_capture(np.full((480, 2), .2), {})
        self.assertIn('No encontré', response['message'])

    def test_never_falls_back_to_microphone(self):
        with self.assertRaisesRegex(RuntimeError, 'audio del escritorio'):
            self.run_capture(np.full((480, 2), .2), {}, loopback=False)


if __name__ == '__main__':
    unittest.main()
