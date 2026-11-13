#!/bin/sh
# `make build`: what is cut is carried and built with, in the real game. The player fells a
# tree with the axe, knocks off its boughs, breaks it into logs and splits a log into hunks
# (`Loose::chop`); takes hunks up one at a time and puts each down on the last (`lift`,
# `use`). Each must stay where it is put (nothing lands after the first is laid but what
# was laid), and the region dropped and made again from seed and journal must be the same.
#   tools/build.sh [X,Z]     default: 47,28 (a wood)
at=${1:-47,28}
script=$(python3 -c "print('; '.join(['%.2f chop' % (0.6 + i * 0.2) for i in range(110)] + [s for i in range(4) for s in ('%.1f lift' % (23.5 + i * 1.5), '%.1f use' % (24.2 + i * 1.5))] + ['31 forget']))")
out=$(LOOSE_TRACE=1 cargo sandbox --scene shots/build.png --play --at "$at" --hour 13 --after 35 --script "$script" 2>&1 | grep -E '^(loose: (lifted|laid|chop: a log)|(ok|FAIL) forget)')
hunks=$(echo "$out" | grep -c '^loose: chop: a log'); lifted=$(echo "$out" | grep -c '^loose: lifted'); laid=$(echo "$out" | grep -c '^loose: laid'); forget=$(echo "$out" | grep -E '^(ok|FAIL) forget')
# (On another, by what was under each as it was put: taken up from the nearest, the same
# hunk went up and down four times, and four taken up and four put down passed.)
high=$(echo "$out" | grep -c '^loose: laid.* on Some(Billet)')
case "$forget" in ok*) [ "$hunks" -ge 1 ] && [ "$lifted" -ge 4 ] && [ "$laid" -ge 4 ] && [ "$high" -ge 2 ] && { echo "ok build: a tree felled, limbed, broken and split into hunks, $lifted taken up one at a time and $laid put down, $high of them on another, and the region made again is the same"; exit 0; } ;; esac
echo "FAIL build: $hunks logs split (bar 1), $lifted hunks taken up (bar 4), $laid put down (bar 4), $high of them on another (bar 2) · ${forget:-nothing was dropped and made again}"; exit 1
