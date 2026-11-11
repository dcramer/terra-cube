#!/usr/bin/env python3
"""Makes every loop in assets/audio (amb_*, torch_loop) join its own start without a click,
and says so by measurement: the jump in level from a loop's last sample to its first, against
the largest jump between neighbouring samples anywhere inside it. A loop whose seam is no
worse than its own middle cannot be heard to repeat at the seam.

`loops.py` reports; `loops.py --fix` rewrites those whose seam is worse, by fading the last
second into the first (the file gets a second shorter). Needs ffmpeg. Run `level.py --fix`
after it."""
import pathlib, struct, subprocess, sys, tempfile

root = pathlib.Path(__file__).resolve().parents[2] / "assets" / "audio"
fix = "--fix" in sys.argv


def samples(path):
    raw = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(path), "-ac", "1", "-f", "s16le", "-"], capture_output=True).stdout
    return struct.unpack(f"<{len(raw) // 2}h", raw)


def seam(path):
    s = samples(path)
    if len(s) < 1000:
        return None
    inside = sorted(abs(a - b) for a, b in zip(s, s[1:]))
    # What is ordinary inside it: all but the sharpest thousandth of its steps.
    usual = max(inside[int(len(inside) * 0.999)], 1)
    return abs(s[-1] - s[0]) / usual, len(s)


bad = fixed = 0
loops = sorted(p for p in root.glob("*.ogg") if p.stem.startswith("amb_") or p.stem == "torch_loop")
for path in loops:
    got = seam(path)
    if got is None:
        print(f"FAIL {path.name}: could not be read")
        bad += 1
        continue
    jump, _ = got
    if jump <= 1.0:
        continue
    if not fix:
        print(f"off {path.name}: the seam jumps {jump:.1f} times what is usual inside it")
        bad += 1
        continue
    # The last second faded into the first, on the samples themselves: what was the first
    # second is then heard only as the tail, and the loop starts where that second ended.
    rate, channels = 44100, 2
    raw = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(path), "-ac", str(channels), "-ar", str(rate), "-f", "s16le", "-"], capture_output=True).stdout
    pcm = list(struct.unpack(f"<{len(raw) // 2}h", raw))
    fade = rate * channels
    if len(pcm) < fade * 3:
        print(f"FAIL {path.name}: too short to rejoin")
        bad += 1
        continue
    head, body = pcm[:fade], pcm[fade:]
    for i in range(fade):
        t = (i // channels) / rate
        body[len(body) - fade + i] = int(body[len(body) - fade + i] * (1 - t) + head[i] * t)
    with tempfile.TemporaryDirectory() as tmp:
        out = pathlib.Path(tmp) / path.name
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "s16le", "-ar", str(rate), "-ac", str(channels), "-i", "-", "-c:a", "libvorbis", "-q:a", "4", str(out)], input=struct.pack(f"<{len(body)}h", *body), check=True)
        if out.stat().st_size < 10000:
            print(f"FAIL {path.name}: the rejoined file came out empty; left as it was")
            bad += 1
            continue
        path.write_bytes(out.read_bytes())
    after = seam(path)
    fixed += 1
    if after and after[0] > 1.0:
        print(f"FAIL {path.name}: still jumps {after[0]:.1f} times after the fade")
        bad += 1
if fix:
    print(f"{'ok' if bad == 0 else 'FAIL'} loops: {fixed} of {len(loops)} rejoined")
else:
    print(f"{'ok' if bad == 0 else 'FAIL'} loops: {len(loops) - bad} of {len(loops)} join their own start" + ("" if bad == 0 else " (loops.py --fix)"))
sys.exit(1 if bad else 0)
