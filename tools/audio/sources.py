#!/usr/bin/env python3
"""`python3 tools/audio/sources.py`: is every sound accounted for? Each file in assets/audio
has a row in its README saying where it came from, with a licence of CC0 or public domain, or says it was made here;
every creature's `voice` has at least one file; and which living creatures have no voice.
One line: ok, or FAIL with what is missing."""
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parents[2]
audio = root / "assets" / "audio"
rows = {}
for line in (audio / "README.md").read_text().splitlines():
    cells = [c.strip() for c in line.split("|")]
    if len(cells) > 3 and cells[1].endswith(".ogg"):
        rows[cells[1]] = line
files = sorted(p.name for p in audio.glob("*.ogg"))
unlisted = [f for f in files if f not in rows]
unlicensed = [f for f in files if f in rows and not re.search(r"\|\s*(CC0|Public domain|ours \(generated)[^|]*\|", rows[f])]
gone = sorted(set(rows) - set(files))
made = sum('ours (generated' in rows[f] for f in files if f in rows)
voiceless, silent = [], []
for toml in sorted((root / "assets" / "creatures").glob("*/creature.toml")):
    text = toml.read_text()
    # (A trial that is not in the world has `groups = 0`; small life the game flies sings by its own tables.)
    if re.search(r"^groups\s*=\s*0\s*$", text, re.M) or not (toml.parent / "model.glb").exists():
        continue
    voice = re.search(r'^voice\s*=\s*"([^"]+)"', text, re.M)
    if not voice:
        if "[life]" in text and "[built]" not in text:
            silent.append(toml.parent.name)
    elif not any(re.fullmatch(re.escape(voice.group(1)) + r"(_\d+)?\.ogg", f) for f in files):
        voiceless.append(f"{toml.parent.name} ({voice.group(1)})")
bad = [f"{len(x)} {what}: {' '.join(x[:6])}" for x, what in [(unlisted, "files with no row in assets/audio/README.md"), (unlicensed, "rows that are neither CC0, public domain nor made here"), (gone, "rows whose file is gone"), (voiceless, "creatures whose voice has no file")] if x]
quiet = f" · no voice: {' '.join(silent)}" if silent else ""
print(f"FAIL sources: {'; '.join(bad)}" if bad else f"ok sources: {len(files)} sounds, each with its source: {made} made here, the rest CC0 or public domain{quiet}")
sys.exit(1 if bad else 0)
