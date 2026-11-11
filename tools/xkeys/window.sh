#!/bin/sh
# The real game window, driven by real key presses on a display with no person at it (an
# Xvfb), and judged from its trace: `make window`. The software Vulkan driver is used, since a
# graphics card will not present to an Xvfb; it is slow, which does not matter here.
# Passes if W walked the player, 2 put the torch in hand, and I opened the pack.
set -e
log="${TMPDIR:-/tmp}/terra-window.log"
icd=/usr/share/vulkan/icd.d/lvp_icd.json
[ -f "$icd" ] || { echo "FAIL window: no software Vulkan driver ($icd)"; exit 1; }
VK_ICD_FILENAMES="$icd" timeout 150 "$1" --play --wild --quiet --trace > "$log" 2>&1 &
game=$!
# Until the player is in the world and the first trace line is out.
for _ in $(seq 120); do grep -q '^play' "$log" 2>/dev/null && break; sleep 1; done
sleep 8
python3 "$(dirname "$0")/xkeys.py" 'w:4 wait:1 2 wait:1 i wait:3'
python3 - <<'PY'
from PIL import ImageGrab
ImageGrab.grab().crop((0, 0, 1600, 900)).save("shots/window.png")
PY
python3 "$(dirname "$0")/xkeys.py" esc
sleep 1
kill "$game" 2>/dev/null || true
wait "$game" 2>/dev/null || true
awk '/^play/ { split($0, a, "pos="); split(a[2], p, /[, ]/); if (first == "") { fx = p[1]; fz = p[3]; first = 1 } lx = p[1]; lz = p[3]; if ($0 ~ /held=Torch/) torch = 1 }
     /^sound ui_open/ { pack = 1 }
     END { moved = sqrt((lx - fx) ^ 2 + (lz - fz) ^ 2); ok = (moved > 2 && torch && pack);
           printf "%s window: walked %.1f m on W, torch in hand %d, pack opened %d · shots/window.png\n", ok ? "ok" : "FAIL", moved, torch, pack; exit !ok }' "$log"
