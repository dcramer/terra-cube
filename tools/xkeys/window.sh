#!/bin/sh
# The real game window, driven by real key presses on a display with no person at it (an
# Xvfb), and judged from its trace: `make window`. The software Vulkan driver is used, since a
# graphics card will not present to an Xvfb; it is slow, which does not matter here.
# Passes if the loading screen held the player still against a W pressed while it was up,
# then lifted, and after it W walked the player, F put down the hunk he starts with (`--carry`)
# (walking, with it in his hands: a metre where he ran five) and F took it up again, 2 put the torch in hand, and I opened the pack.
set -e
log="${TMPDIR:-/tmp}/terra-window.log"
icd=/usr/share/vulkan/icd.d/lvp_icd.json
[ -f "$icd" ] || { echo "FAIL window: no software Vulkan driver ($icd)"; exit 1; }
VK_ICD_FILENAMES="$icd" timeout 300 "$1" --play --wild --quiet --trace --carry > "$log" 2>&1 &
game=$!
# Until the player is in the world and the first trace line is out.
for _ in $(seq 120); do grep -q '^play' "$log" 2>/dev/null && break; sleep 1; done
# W while the loading screen is up (it is, for many seconds, on a software renderer), which
# must not move him; then until it lifts (`loading done`), and a little for the fade.
python3 "$(dirname "$0")/xkeys.py" 'w:1'
for _ in $(seq 150); do grep -q '^loading done' "$log" 2>/dev/null && break; sleep 1; done
sleep 3
# (A frame is about a second here: 2 and I are kept well apart, or they land in one frame and
# the pack, open, takes the 2.)
python3 "$(dirname "$0")/xkeys.py" 'w:4 wait:2 f wait:3 f wait:3 2 wait:3 i wait:3'
mkdir -p shots
python3 - <<'PY'
from PIL import ImageGrab
ImageGrab.grab().crop((0, 0, 1600, 900)).save("shots/window.png")
PY
python3 "$(dirname "$0")/xkeys.py" esc
sleep 1
kill "$game" 2>/dev/null || true
wait "$game" 2>/dev/null || true
awk '/^play/ { split($0, a, "pos="); split(a[2], p, /[, ]/); if (first == "") { fx = p[1]; fz = p[3]; first = 1 } lx = p[1]; lz = p[3]; if ($0 ~ /held=Torch/) torch = 1; if ($0 ~ /carries=nothing/ && had) down = 1; if ($0 ~ /carries=Billet/) { had = 1; if (down) up = 1 } }
     /^loading done/ { loaded = 1; held = sqrt((lx - fx) ^ 2 + (lz - fz) ^ 2); if (match($0, /barred=[0-9]+/)) barred = substr($0, RSTART + 7, RLENGTH - 7) + 0; if (match($0, / t=[0-9.]+/)) took = substr($0, RSTART + 3, RLENGTH - 3); stalled = ($0 ~ /stalled/) }
     /^sound ui_open/ { pack = 1 }
     END { moved = sqrt((lx - fx) ^ 2 + (lz - fz) ^ 2); ok = (loaded && !stalled && barred > 0 && held < 0.1 && moved > 0.7 && torch && pack && down && up);
           printf "%s window: loading lifted %d at %ss%s, %d keys turned away while it was up and he moved %.1f m, then walked %.1f m on W, F put his hunk down %d and took it up %d, torch in hand %d, pack opened %d · shots/window.png\n", ok ? "ok" : "FAIL", loaded, took, stalled ? " (stalled)" : "", barred, held, moved, down, up, torch, pack; exit !ok }' "$log"
