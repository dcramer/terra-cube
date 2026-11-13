#!/bin/sh
# `make digpace`: what digging costs a frame, in the real game on the graphics card. The
# player walks and digs a hole every quarter of a second, sixty of them, and every frame is
# timed; held to the bars a frame is held to anywhere (`make pace`), and to chunks having
# been drawn again at all. Measured 2026-11-13 after the developer asked for dug ground to
# be cheaper: it was not dear (p50 8.8 ms against 8.5 walking the same way without digging).
#   tools/digpace.sh [X,Z]     default: -60,20 (a meadow)
at=${1:--60,20}
script=$(python3 -c "print('; '.join(['0.5 fwd walk'] + ['%.2f dig' % (1 + i * 0.25) for i in range(60)]))")
line=$(cargo sandbox --scene shots/digpace.png --play --at "$at" --hour 13 --after 17 --strip 2 --script "$script" 2>&1 | grep '^frames')
echo "$line" | awk '{ for (i = 1; i <= NF; i++) { if ($i ~ /^p50=/) p50 = substr($i, 5) + 0; if ($i ~ /^p99=/) p99 = substr($i, 5) + 0; if ($i ~ /^worst=/) worst = substr($i, 7) + 0; if ($i ~ /^\(/) remade = substr($i, 2) + 0 }
  ok = (p50 > 0 && p50 < 12 && p99 < 33 && worst < 80 && remade > 100)
  printf "%s digpace: 60 holes dug on the move · %d chunks drawn again · frames p50 %.1f ms (bar 12), p99 %.1f (bar 33), worst %d (bar 80)\n", ok ? "ok" : "FAIL", remade, p50, p99, worst; exit !ok }'
