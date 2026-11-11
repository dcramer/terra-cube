#!/usr/bin/env python3
"""Judges what a run of the game sounded like, by measurement: nobody here can listen, but
the game writes every sound it plays to its trace with how loud it reached the ear
(`sound NAME gain=G t=T kind=air|voice|step|hand`, and each second `bed NAME gain=G t=T` for
the wind, insects and water that play on), and every file's own loudness can be read. From
the two, for the air of the place (the beds and the small sounds over them):

  how many different files were heard, and the longest run of one file played again and
  again with nothing else of its kind between;
  the mean level of the air over the whole run, and its loudest fifth of a second;
  the same against one of the walker's own footfalls in that run, which is the measure of
  loud here: the air must lie well under it;
  how much of the time one of the small sounds was sounding at all.

`mix.py [TRACE] [--name meadow] [--under 6] [--over 3] [--run 2] [--files 8] [--busy 60]`
reads the trace (a file, or what is piped in) and prints one line, `ok` or `FAIL`:
the air's mean must be --under dB under a footfall, its peak no more than --over dB over
one, no file more than --run times running, at least --files different files, and a small
sound sounding no more than --busy % of the time. `listen.sh` makes the run. Needs ffmpeg."""
import collections, math, pathlib, re, struct, subprocess, sys

args = sys.argv[1:]


def opt(name, default):
    if name in args:
        i = args.index(name)
        v = args[i + 1]
        del args[i:i + 2]
        return type(default)(v)
    return default


name, under, over, run_bar, files_bar, busy_bar = opt("--name", "air"), opt("--under", 6.0), opt("--over", 3.0), opt("--run", 2), opt("--files", 8), opt("--busy", 60.0)
root = pathlib.Path(__file__).resolve().parents[2] / "assets" / "audio"
lines = (open(args[0]) if args else sys.stdin).read().splitlines()
RATE, PIECE = 11025, 2205  # fifths of a second
BIN = PIECE / RATE
FULL = 32768.0 ** 2
powers = {}


def power(file):
    """A file's power in each fifth of a second, of full scale."""
    if file not in powers:
        raw = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(root / f"{file}.ogg"), "-ac", "1", "-ar", str(RATE), "-f", "s16le", "-"], capture_output=True).stdout
        s = struct.unpack(f"<{len(raw) // 2}h", raw)
        powers[file] = [sum(v * v for v in s[i:i + PIECE]) / PIECE / FULL for i in range(0, max(len(s) - PIECE // 2, 1), PIECE)] or [0.0]
    return powers[file]


def db(p):
    return 10 * math.log10(max(p, 1e-12))


sounds, beds = [], []
for line in lines:
    m = re.match(r"(sound|bed) (\S+) .*?gain=([\d.]+) t=([\d.]+)(?: kind=(\w+))?", line)
    if m:
        (sounds if m.group(1) == "sound" else beds).append((float(m.group(4)), m.group(2), float(m.group(3)), m.group(5) or "air"))
if not sounds and not beds:
    print(f"FAIL sound {name}: nothing in the trace (the game prints `sound` lines in --scene --play runs)")
    sys.exit(1)
start = min(t for t, *_ in sounds + beds)
end = max(t for t, *_ in sounds + beds) + 1.0
bins = int((end - start) / BIN) + 1
air, small = [0.0] * bins, [0.0] * bins
for t, file, gain, _ in beds:
    # A bed is said once a second and plays on through it; where in the file is not known,
    # so it counts at the file's mean.
    p = power(file)
    mean = sum(p) / len(p) * gain * gain
    first = int((t - start) / BIN)
    for i in range(first, min(first + int(1.0 / BIN), bins)):
        air[i] += mean
steps, step_peaks = [], []
loudest_small = (0.0, "")
for t, file, gain, kind in sounds:
    p = [v * gain * gain for v in power(file)]
    if kind == "step" and file.startswith("step_"):
        steps.append(sum(p) / len(p))
        step_peaks.append(max(p))
    if kind == "air":
        if max(p) > loudest_small[0]:
            loudest_small = (max(p), file)
        first = int((t - start) / BIN)
        for i, v in enumerate(p):
            if first + i < bins:
                air[first + i] += v
                small[first + i] += v
# The longest run of one file, among the sounds of each kind in the order they played.
longest, which = 0, ""
for kind in ("air", "voice", "step", "hand"):
    last, n = None, 0
    for _, file, _, k in sounds:
        if k != kind:
            continue
        n = n + 1 if file == last else 1
        last = file
        if n > longest:
            longest, which = n, file
heard = collections.Counter(file for _, file, _, kind in sounds if kind == "air")
bed_files = {file for _, file, _, _ in beds if not file.startswith("torch")}
mean, peak = db(sum(air) / bins), db(max(air))
busy = 100.0 * sum(1 for v in small if v > 1e-7) / bins
fails = []
if steps:
    step, step_peak = db(sum(steps) / len(steps)), db(max(step_peaks))
    against = f"a footfall {step:.0f} dB (peak {step_peak:.0f})"
    if mean > step - under:
        fails.append(f"mean {mean:.1f} dB (bar {step - under:.1f})")
    if peak > step_peak + over:
        fails.append(f"peak {peak:.1f} dB (bar {step_peak + over:.1f})")
else:
    against = "no footfall in the run to set it against"
    fails.append("no footfall")
if longest > run_bar:
    fails.append(f"{which} {longest} times running (bar {run_bar})")
if len(heard) + len(bed_files) < files_bar:
    fails.append(f"{len(heard) + len(bed_files)} files (bar {files_bar})")
if busy > busy_bar:
    fails.append(f"a small sound {busy:.0f}% of the time (bar {busy_bar:.0f})")
top = ", ".join(f"{f}×{n}" for f, n in heard.most_common(3))
print(f"{'FAIL' if fails else 'ok'} sound {name}: {end - start:.0f} s · {len(heard)} small files in {sum(heard.values())} plays and {len(bed_files)} beds · longest run {longest} · air mean {mean:.0f} dB, peak {peak:.0f} (loudest alone: {loudest_small[1]} {db(loudest_small[0]):.0f}) · {against} · a small sound {busy:.0f}% of the time · most: {top}" + ("" if not fails else " · FAIL " + "; ".join(fails)))
sys.exit(1 if fails else 0)
