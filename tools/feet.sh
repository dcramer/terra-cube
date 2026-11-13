#!/bin/sh
# `make feet`: a creature walked over uneven ground, once with its feet as the clip has them
# and once put on the ground that is there (`cube_game::feet`), and how the standing foot sat
# each time: the middle of how far it was above the ground drawn under it, and how far that
# strayed. Planted, it should stray less. (A person the two-legged walker moves, `stride`,
# is set against its own clips with the walker off: its feet are the walker's.)
#   tools/feet.sh [ID] [X,Z] [YAW]     default: player 30,-40 0.5
# `FEET_TRACE=1` on any `cargo sandbox --scene --play` prints, each frame, what the planting
# worked from (each foot's height in the clip, the ground under it, how far the body went down):
# the player's lines are the ones with reach=0.30. A wide stance on a ledge reads as a large
# stray here though the feet are on the ground: both feet hang over lower ground.
id=${1:-player}; at=${2:-30,-40}; yaw=${3:-0.5}
run() { cargo sandbox --scene shots/feet.png --play --wild --as "$id" --at "$at" --script '0.5 fwd walk' --strip 6 --after 2 --every 2.5 --view third --yaw "$yaw" $1 $2 2>&1 | grep '^feet' | sed 's/.*spread=//; s/cm//'; }
loose=$(run --loose-feet --clips-only)
planted=$(run)
[ -n "$loose" ] && [ -n "$planted" ] || { echo "FAIL feet $id: no measure (does it have legs in its meta.toml?)"; exit 1; }
awk -v l="$loose" -v p="$planted" -v id="$id" -v at="$at" 'BEGIN { ok = (p <= l + 0.5); printf "%s feet %s at %s: the standing foot strays %.1f cm as the clip has it, %.1f cm planted\n", ok ? "ok" : "FAIL", id, at, l, p; exit !ok }'
