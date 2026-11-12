#!/bin/sh
# `make legs`: the player walked up a hillside, down it, and stood on the edge of a ledge, and
# how far under the ground that is drawn any knee or ankle of his went. A leg in the ground is
# what "clipping" looks like, and no check knew of it until the developer sent a picture.
#   tools/legs.sh [ID]     default: player
id=${1:-player}; worst=0; said=""
run() {
    got=$(cargo sandbox --scene shots/legs.png --play --wild --as "$id" --at "$1" --yaw "$2" --script "$3" --strip 6 --after 1.5 --every 1.2 --view third 2>&1 | grep '^legs' | sed 's/.*worst=//; s/cm.*//')
    [ -n "$got" ] || { echo "FAIL legs $id: no measure at $1"; exit 1; }
    said="$said $4 $got cm ·"
    worst=$(awk -v a="$worst" -v b="$got" 'BEGIN { print (b > a) ? b : a }')
}
set -- $(cargo world find slope | head -1 | sed 's/slope at \([^ ]*\) .*--yaw \(.*\)/\1 \2/')
run "$1" "$2" '0.3 fwd walk' "up a hillside"
run "$1" "$(awk -v y="$2" 'BEGIN { print y + 3.14159 }')" '0.3 fwd walk' "down it"
run "$1" "$2" '0.3 fwd run' "running up"
run -60,20 2.0 '0.1 stop' "standing on a ledge"
awk -v w="$worst" -v s="$said" -v id="$id" 'BEGIN { ok = (w <= 3.0); printf "%s legs %s: under the ground,%s worst %s cm%s\n", ok ? "ok" : "FAIL", id, s, w, ok ? "" : " (no more than 3: shots/legs.png, FEET_TRACE=1)"; exit !ok }'
