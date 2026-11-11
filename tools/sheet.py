#!/usr/bin/env python3
"""Tiles pictures into one, so many are looked at in one go: `sheet.py OUT.png A.png B.png ...`
puts them two across at half size (needs ffmpeg). `make grown` and `make slice` use it."""
import math, subprocess, sys

out, shots = sys.argv[1], sys.argv[2:]
if not shots:
    sys.exit("sheet.py OUT.png A.png B.png ...")
across = 2
rows = math.ceil(len(shots) / across)
inputs = [arg for shot in shots for arg in ("-i", shot)]
scaled = "".join(f"[{i}:v]scale=800:450[s{i}];" for i in range(len(shots)))
layout = "|".join(f"{(i % across) * 800}_{(i // across) * 450}" for i in range(len(shots)))
tiles = "".join(f"[s{i}]" for i in range(len(shots)))
graph = f"{scaled}{tiles}xstack=inputs={len(shots)}:layout={layout}:fill=black" if len(shots) > 1 else "[0:v]scale=800:450"
done = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *inputs, "-filter_complex", graph, "-frames:v", "1", out])
print(f"{'ok' if done.returncode == 0 else 'FAIL'} sheet: {len(shots)} pictures, {across} across, {rows} down · {out}")
sys.exit(done.returncode)
