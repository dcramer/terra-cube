#!/bin/sh
# `make fells`: the tree nearest each of many places felled with an axe, no window
# (`cargo world fell X,Z`): each must stand until its last blow, come loose, go over and lie
# with its trunk on the ground. One tree near the start says little: for a day the trial
# felled one birch a seed and passed while no great tree in the game came down at all.
#   tools/fells.sh [STEP]     places every STEP metres from -45 to 45 each way (default 22: 25 trees)
step=${1:-22}; n=0; bad=0; said=""
for x in $(seq -45 "$step" 45); do for z in $(seq -45 "$step" 45); do
    n=$((n + 1))
    out=$(cargo world fell "$x,$z" 2>&1 | grep -E '^(FAIL|world:|error)' | head -2 | tr '\n' ' ')
    [ -n "$out" ] && { bad=$((bad + 1)); said="$said\n  at $x,$z: $out"; }
done; done
[ "$bad" -eq 0 ] && { echo "ok fells: $n trees felled with an axe, each stood to its last blow, came loose, went over and lies on the ground"; exit 0; }
printf "FAIL fells: %s of %s trees$said\n" "$bad" "$n"; exit 1
