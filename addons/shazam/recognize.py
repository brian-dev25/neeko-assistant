"""One requested desktop-loopback capture; no microphone or saved recording."""
import asyncio
import io
import json
import wave


async def recognize():
    import numpy as np
    import soundcard as sc
    from shazamio import Shazam

    speaker = sc.default_speaker()
    if speaker is None:
        raise RuntimeError("No hay un dispositivo de salida de audio activo")
    loopback = sc.get_microphone(id=speaker.id, include_loopback=True)
    if not loopback.isloopback:
        raise RuntimeError("No se pudo acceder al audio del escritorio")
    # Capture all channels: WASAPI single-channel capture can produce bad data.
    with loopback.recorder(samplerate=48000) as recorder:
        audio = recorder.record(numframes=48000 * 10)
    if not np.isfinite(audio).all() or np.max(np.abs(audio)) < 0.0001:
        return {"message": "No escuché sonido. Reproducí música y volvé a intentarlo."}
    mono = np.mean(audio, axis=1)
    pcm = (np.clip(mono, -1, 1) * 32767).astype("<i2")
    data = io.BytesIO()
    with wave.open(data, "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(48000)
        wav.writeframes(pcm.tobytes())
    result = await asyncio.wait_for(Shazam().recognize(data.getvalue()), timeout=35)
    track = result.get("track") or {}
    if not track.get("title"):
        return {"message": "No encontré la canción. Probá con un fragmento más claro."}
    return {"title": track["title"], "artist": track.get("subtitle", ""),
            "message": f'{track["title"]} — {track.get("subtitle", "")}'.strip(" —")}


if __name__ == "__main__":
    try:
        output = asyncio.run(recognize())
    except Exception as error:
        output = {"error": "No se pudo reconocer la música: " + str(error)}
    print(json.dumps(output, ensure_ascii=True))
