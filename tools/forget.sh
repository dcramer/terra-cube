#!/bin/sh
# `make forget`: a tree felled and cut up in the real game, then every changed region dropped
# and made again from the seed and the journal of edits (`cube_sim::loose::replay`, the
# script's `forget`): no cell may differ from how it was left. Leaf overhanging from a
# neighbouring region is counted apart (the streaming's own seam, not the journal's).
#   tools/forget.sh [X,Z]     default: 47,28 (a wood)
at=${1:-47,28}
script=$(python3 -c "print('; '.join(['%.2f chop' % (0.6 + i * 0.12) for i in range(40)] + ['%.2f chop' % (9 + i * 0.3) for i in range(8)] + ['12 forget']))")
out=$(cargo sandbox --scene shots/forget.png --play --at "$at" --hour 13 --after 19 --script "$script" 2>&1 | grep -E '^(ok|FAIL) forget')
[ -n "$out" ] || { echo "FAIL forget: nothing was dropped and made again"; exit 1; }
echo "$out"
case "$out" in ok*) exit 0 ;; *) exit 1 ;; esac
