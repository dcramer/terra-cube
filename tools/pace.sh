#!/bin/sh
# `make pace`: the player run across the living world for half a minute on the real graphics
# card, no window, and what it cost: how long the world took to be ready, what was made, and
# every frame's time while he ran (the middle one, the one in a hundred, the worst). Fails if
# he did not get anywhere (a rock in the way measures nothing), if the world was slow to be
# ready, or if frames were slow. Compare numbers only with the same machine's.
#   tools/pace.sh [--at X,Z] [--yaw R]   another place or heading
log="${TMPDIR:-/tmp}/terra-pace.log"
cargo sandbox --scene shots/pace.png --play --wild --trace --script '0.5 fwd run' --strip 10 --after 2 --every 3 --view three --yaw 2.6 "$@" > "$log" 2>&1
awk -v load_bar="${PACE_LOAD:-20}" -v p99_bar="${PACE_P99:-30}" -v worst_bar="${PACE_WORST:-100}" '
    /^play/ { split($0, a, "pos="); split(a[2], p, /[, ]/); if (!seen) { fx = p[1]; fz = p[3]; seen = 1 } lx = p[1]; lz = p[3] }
    /^load / { load = $2 + 0; made = $0 }
    /^frames / { frames = $0; if (match($0, /p99=[0-9.]+/)) p99 = substr($0, RSTART + 4, RLENGTH - 4) + 0; if (match($0, /worst=[0-9.]+/)) worst = substr($0, RSTART + 6, RLENGTH - 6) + 0 }
    END {
        moved = sqrt((lx - fx) ^ 2 + (lz - fz) ^ 2)
        ok = (frames != "" && moved > 60 && load < load_bar && p99 < p99_bar && worst < worst_bar)
        printf "%s pace: ran %.0f m · %s · %s (bars: ran > 60 m, load < %ss, p99 < %sms, worst < %sms)\n", ok ? "ok" : "FAIL", moved, made, frames, load_bar, p99_bar, worst_bar
        exit !ok
    }' "$log"
