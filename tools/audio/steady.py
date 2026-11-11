#!/usr/bin/env python3
"""Says how steady a sound is, by measurement: a loop of wind or running water should be an
even wash, and one that swells and gulps is heard as something else (a water loop that
gulped was heard as a drain). For each file: how much its loudness moves from one fifth of
a second to the next (the spread, in dB, of the level over such pieces), and where its
weight lies (the share of its power under 300 Hz: a gurgle is low, a brook's wash is not).

`steady.py FILE...` reports; with no files, every amb_* loop in assets/audio. Needs ffmpeg."""
import math, pathlib, struct, subprocess, sys

root = pathlib.Path(__file__).resolve().parents[2] / "assets" / "audio"
files = [pathlib.Path(f) for f in sys.argv[1:]] or sorted(root.glob("amb_*.ogg"))


def samples(path, extra=()):
    raw = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(path), *extra, "-ac", "1", "-ar", "22050", "-f", "s16le", "-"], capture_output=True).stdout
    return struct.unpack(f"<{len(raw) // 2}h", raw)


def power(s):
    return sum(v * v for v in s) / max(len(s), 1)


for path in files:
    s = samples(path)
    if len(s) < 22050:
        print(f"FAIL {path.name}: could not be read")
        continue
    piece = 22050 // 5
    levels = [10 * math.log10(max(power(s[i:i + piece]), 1.0)) for i in range(0, len(s) - piece, piece)]
    mean = sum(levels) / len(levels)
    spread = math.sqrt(sum((v - mean) ** 2 for v in levels) / len(levels))
    low = power(samples(path, ("-af", "lowpass=f=300"))) / max(power(s), 1.0)
    print(f"{path.name}: {len(s) / 22050:.1f} s · level moves {spread:.1f} dB · {low * 100:.0f}% of its power under 300 Hz")
