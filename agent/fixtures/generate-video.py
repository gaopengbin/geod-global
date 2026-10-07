"""Own test-pattern clips; ffmpeg authors fixtures and is not an app dependency."""
from pathlib import Path
import shutil
import subprocess

out = Path(__file__).resolve().parent
ffmpeg = shutil.which("ffmpeg")
if not ffmpeg:
    raise RuntimeError("ffmpeg authoring tool is required")
source = [ffmpeg, "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=10", "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=16000", "-t", "3", "-map_metadata", "-1"]
for name, codecs in [
    ("clip.mp4", ["-c:v", "libx264", "-crf", "28", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "32k", "-movflags", "+faststart"]),
    ("clip.webm", ["-c:v", "libvpx-vp9", "-crf", "40", "-b:v", "0", "-c:a", "libopus", "-b:a", "24k"]),
    ("clip-vp8.webm", ["-c:v", "libvpx", "-b:v", "120k", "-c:a", "libvorbis", "-q:a", "2"]),
    ("clip-unsupported.mp4", ["-c:v", "mpeg4", "-q:v", "6", "-an"]),
]:
    subprocess.run([*source, *codecs, str(out / name)], check=True)
subprocess.run([ffmpeg, "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "color=c=blue:size=32x32:rate=1", "-t", "601", "-an", "-c:v", "libx264", "-crf", "35", "-movflags", "+faststart", str(out / "clip-too-long.mp4")], check=True)
print("Generated three owned short clips and two rejection fixtures.")
