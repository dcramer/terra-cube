#!/usr/bin/env python3
"""Brings every sound in assets/audio to one loudness, by measurement: nobody here can listen,
but a level can be read. One-shots to a mean of -20 dB, loops (amb_*, torch_loop) to -27 dB,
no peak over -1 dB. `level.py` reports; `level.py --fix` rewrites the files that are more than
1.5 dB out. Needs ffmpeg. Run again after adding a file."""
import pathlib, re, subprocess, sys, tempfile

root = pathlib.Path(__file__).resolve().parents[2] / "assets" / "audio"
fix = "--fix" in sys.argv


def measure(path):
    out = subprocess.run(["ffmpeg", "-hide_banner", "-i", str(path), "-af", "volumedetect", "-f", "null", "-"], capture_output=True, text=True).stderr
    mean = re.search(r"mean_volume: (-?[\d.]+) dB", out)
    peak = re.search(r"max_volume: (-?[\d.]+) dB", out)
    return (float(mean.group(1)), float(peak.group(1))) if mean and peak else None


off = changed = 0
for path in sorted(root.glob("*.ogg")):
    loop = path.stem.startswith("amb_") or path.stem == "torch_loop"
    target = -27.0 if loop else -20.0
    got = measure(path)
    if not got:
        print(f"FAIL {path.name}: could not be measured")
        off += 1
        continue
    mean, peak = got
    # As near the target as the peak allows.
    gain = min(target - mean, -1.0 - peak)
    if abs(gain) <= 1.5:
        continue
    off += 1
    if fix:
        with tempfile.TemporaryDirectory() as tmp:
            out = pathlib.Path(tmp) / path.name
            subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(path), "-af", f"volume={gain:.2f}dB", "-c:a", "libvorbis", "-q:a", "4", str(out)], check=True)
            path.write_bytes(out.read_bytes())
        changed += 1
    else:
        print(f"off {path.name}: mean {mean:.1f} dB, peak {peak:.1f} dB, wants {gain:+.1f} dB")
total = len(list(root.glob("*.ogg")))
if fix:
    print(f"ok audio: {changed} of {total} files brought to level")
else:
    print(f"{'ok' if off == 0 else 'FAIL'} audio: {total - off} of {total} files at level" + ("" if off == 0 else " (level.py --fix)"))
    sys.exit(1 if off else 0)
