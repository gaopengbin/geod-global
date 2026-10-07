"""Owned synthetic audio fixtures; ffmpeg is an authoring tool, not app runtime."""
from pathlib import Path
import math
import shutil
import struct
import subprocess
import wave

out = Path(__file__).resolve().parent
ffmpeg = shutil.which("ffmpeg")
if not ffmpeg:
    raise RuntimeError("ffmpeg authoring tool is required")
with wave.open(str(out / "tone.wav"), "wb") as audio:
    audio.setparams((1, 2, 16000, 0, "NONE", "not compressed"))
    audio.writeframes(b"".join(struct.pack("<h", round(4500 * math.sin(2 * math.pi * 440 * index / 16000))) for index in range(48000)))
def encode(name, options):
    subprocess.run([ffmpeg, "-hide_banner", "-loglevel", "error", "-y", "-i", str(out / "tone.wav"), "-map_metadata", "-1", *options, str(out / name)], check=True)
encode("tone.mp3", ["-c:a", "libmp3lame", "-b:a", "64k"])
encode("tone.flac", ["-c:a", "flac"])
encode("tone.ogg", ["-c:a", "libvorbis", "-q:a", "4"])
encode("tone-wrong-codec.ogg", ["-c:a", "flac", "-f", "ogg"])
subprocess.run([ffmpeg, "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "anullsrc=r=8000:cl=mono", "-t", "601", "-c:a", "flac", str(out / "tone-too-long.flac")], check=True)
print("Generated four 3-second owned tones and two bounded rejection fixtures.")
