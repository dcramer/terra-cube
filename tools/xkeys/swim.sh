#!/bin/sh
# `make swim`: the player is put in the nearest deep water in the real game (no window) and
# told to swim; the trace must show him floating, the swim clip playing, and the water heard.
at=$(cargo world find deep | head -1 | cut -d' ' -f3)
cargo sandbox --wild --scene shots/swim.png --play --at "$at" --script '2 fwd walk; 5 stop' --after 6 --view three --hour 13 2>&1 | awk '
/^play/ { n++; if ($0 ~ /swimming=true/) s++; if ($0 ~ /clip=swim/) c++ }
/^sound swim/ { heard++ }
/^sound splash/ { heard++ }
END {
    ok = (s >= 3 && c >= 2 && heard >= 1)
    printf "%s swim: swimming in %d of %d seconds, swim clip in %d, water heard %d times\n", ok ? "ok" : "FAIL", s, n, c, heard
    exit !ok
}'
