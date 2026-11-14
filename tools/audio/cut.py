#!/usr/bin/env python3
"""Cuts the single calls out of a recording, by measurement: nobody here can listen, but a
call stands out of the quiet between calls and that can be read. The recording's loudness is
taken in pieces of a fiftieth of a second; what is well over the quiet of the recording and
lasts between --min and --max seconds is a call; the clearest --count of them are written to
assets/audio as NAME_1.ogg, NAME_2.ogg, ... (mono, the rumble under --high Hz taken off,
faded in and out), and a line says where in the recording each came from, for the README.

With `--swell SECONDS` it cuts instead the loudest stretches of that length from a sound that
never stops (leaves in a wind: the gusts), well apart, each faded in and out over a third of
its length.

`cut.py SOURCE NAME [--count 4] [--min 0.4] [--max 3] [--high 1000] [--low 0] [--gap 0.3]
[--from N] [--clear 10] [--out DIR] [--tail 0] [--lead 0.06]`. A blow (an axe, a stone) wants
`--lead 0.015 --tail 0.3`: heard at once, and its ring let die away, not cut. A call less than --clear dB over the quiet is not taken: it
could as well be the quiet. Needs ffmpeg. Run `level.py --fix` after it."""
import math, pathlib, struct, subprocess, sys

args = sys.argv[1:]


def opt(name, default):
    if name in args:
        i = args.index(name)
        v = args[i + 1]
        del args[i:i + 2]
        return type(default)(v)
    return default


count, shortest, longest, high, low, gap, first = opt("--count", 4), opt("--min", 0.4), opt("--max", 3.0), opt("--high", 1000), opt("--low", 0), opt("--gap", 0.3), opt("--from", 1)
clear_bar = opt("--clear", 10.0)
tail, lead = opt("--tail", 0.0), opt("--lead", 0.06)
swell = opt("--swell", 0.0)
out = pathlib.Path(opt("--out", str(pathlib.Path(__file__).resolve().parents[2] / "assets" / "audio")))
if len(args) != 2:
    sys.exit(__doc__)
source, name = args
band = f"highpass=f={high}" + (f",lowpass=f={low}" if low else "")
RATE, PIECE = 22050, 441
raw = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", source, "-af", band, "-ac", "1", "-ar", str(RATE), "-f", "s16le", "-"], capture_output=True).stdout
s = struct.unpack(f"<{len(raw) // 2}h", raw)
if len(s) < RATE:
    sys.exit(f"FAIL {source}: could not be read")
level = [10 * math.log10(max(sum(v * v for v in s[i:i + PIECE]) / PIECE, 1.0)) for i in range(0, len(s) - PIECE, PIECE)]
ranked = sorted(level)
quiet, top = ranked[len(ranked) // 5], ranked[-1]
bar = quiet + max(8.0, 0.35 * (top - quiet))
step = PIECE / RATE
# Runs over the bar, joined across gaps shorter than --gap.
runs, start, last = [], None, None
for i, v in enumerate(level + [quiet]):
    if v > bar:
        if start is None:
            start = i
        last = i
    elif start is not None and (i - last) * step > gap:
        runs.append((start, last + 1))
        start = None
calls = []
for a, b in runs:
    length = (b - a) * step
    clear = sum(level[a:b]) / (b - a) - quiet
    if shortest <= length <= longest and clear >= clear_bar:
        calls.append((clear, a * step, length))
if swell:
    # Every stretch of that length by its mean level; the loudest, none within two lengths
    # of another.
    n = int(swell / step)
    sums = [sum(level[i:i + n]) / n for i in range(0, len(level) - n, max(n // 8, 1))]
    calls = []
    for mean, i in sorted(((m, i * max(n // 8, 1)) for i, m in enumerate(sums)), reverse=True):
        if all(abs(i * step - at) > 2 * swell for _, at, _ in calls):
            calls.append((mean - quiet, i * step, swell))
calls.sort(reverse=True)
chosen = sorted(calls[:count], key=lambda c: c[1])
if not chosen:
    sys.exit(f"FAIL {name}: no call of {shortest} to {longest} s stands out of {source} ({len(runs)} runs over the bar)")
for n, (clear, at, length) in enumerate(chosen, first):
    a, d = max(at - lead, 0.0), length + 0.2 + tail
    fades = f"afade=t=in:d={d / 3:.3f},afade=t=out:st={d * 2 / 3:.3f}:d={d / 3:.3f}" if swell else f"afade=t=in:d={min(0.04, lead * 0.6):.3f},afade=t=out:st={d - 0.12 - tail:.3f}:d={0.12 + tail:.3f}"
    path = out / f"{name}_{n}.ogg"
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-ss", f"{a:.3f}", "-t", f"{d:.3f}", "-i", source, "-af", f"{band},{fades}", "-ac", "1", "-ar", "44100", "-c:a", "libvorbis", "-q:a", "4", str(path)], check=True)
    print(f"{path.name}\t{d:.2f}\t{a:.2f} to {a + d:.2f} s of {len(s) / RATE:.0f} s\t{clear:.0f} dB over the quiet\t{pathlib.Path(source).name}")
print(f"ok cut {name}: {len(chosen)} of {len(calls)} calls found")
