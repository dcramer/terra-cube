#!/usr/bin/env python3
"""`make features`: every creature in the world held to assets/creatures/requirements.toml.
One line: ok, or FAIL with what is missing; and how many clips nobody has looked at
(assets/creatures/seen.toml). The whole table is written to shots/features.md."""
import pathlib
import sys
import tomllib

root = pathlib.Path(__file__).resolve().parents[1]
creatures = root / "assets" / "creatures"
needs = tomllib.loads((creatures / "requirements.toml").read_text())
seen = tomllib.loads((creatures / "seen.toml").read_text())
held = tomllib.loads((root / "assets" / "world" / "held.toml").read_text())
sounds = {p.name for p in (root / "assets" / "audio").glob("*.ogg")}
missing, rows, unseen, total = [], [], 0, 0
for spec in sorted(creatures.glob("*/creature.toml")):
    c = tomllib.loads(spec.read_text())
    life, meta_file = c.get("life"), spec.parent / "meta.toml"
    if not life or life.get("groups", 1) == 0 or not meta_file.exists():
        continue
    name, meta = spec.parent.name, tomllib.loads(meta_file.read_text())
    clips, cells = meta.get("clips", {}), []
    about = {"plan": c.get("plan"), "temper": life.get("temper"), "diet": life.get("diet")}
    for feature, need in needs.items():
        if any(about.get(k) not in v for k, v in need.get("who", {}).items()):
            cells.append("")
            continue
        lacks = [f"no {clip} clip" for clip in need.get("clips", []) if clip not in clips and clip not in c.get("without", [])]
        lacks += [f"{clip} is made up" for clip in need.get("recorded", []) if clips.get(clip, {}).get("source") == "made"]
        if need.get("speeds") and not 0 < meta.get("walk_speed", 0) < meta.get("run_speed", 0):
            lacks.append(f"walk {meta.get('walk_speed', 0):.2f} run {meta.get('run_speed', 0):.2f}")
        if need.get("voice") and "voice" not in c.get("without", []) and not any(s.startswith(life.get("voice", "\0") + "_") or s == life.get("voice", "\0") + ".ogg" for s in sounds):
            if about["temper"] != "person":
                lacks.append("no voice" if "voice" not in life else f"voice {life['voice']} has no file")
        if need.get("says") and need["says"] not in c:
            lacks.append(f"does not say how it {need['says']}")
        if need.get("holds") and life.get("holds") and life["holds"] not in held:
            lacks.append(f"holds {life['holds']}, which is not a thing")
        looked = [clip for clip in need.get("clips", []) if clip in clips]
        total += len(looked)
        unlooked = [clip for clip in looked if clip not in seen.get(name, [])]
        unseen += len(unlooked)
        missing += [f"{name} {feature}: {lack}" for lack in lacks]
        cells.append("**" + "; ".join(lacks) + "**" if lacks else "ok" if not unlooked else "ok, not looked at: " + " ".join(unlooked))
    rows.append(f"| {name} | " + " | ".join(cells) + " |")
out = root / "shots" / "features.md"
out.parent.mkdir(exist_ok=True)
out.write_text("| creature | " + " | ".join(needs) + " |\n|" + "---|" * (len(needs) + 1) + "\n" + "\n".join(rows) + "\n")
tail = f"{len(rows)} creatures, {len(needs)} features · {total - unseen} of {total} clips have been looked at · shots/features.md"
print(f"FAIL features: {len(missing)} missing ({'; '.join(missing[:5])}) · {tail}" if missing else f"ok features: {tail}")
sys.exit(1 if missing else 0)
