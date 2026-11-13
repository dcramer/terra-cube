#!/bin/sh
# `make build`: building with what was cut, in the real game. The player chops a tree for
# timber, takes the timber in hand and puts it down piece on piece where he looks. Each
# piece must stay where it is put (nothing lands after the first is put), and the region
# dropped and made again from seed and journal must be the same (`forget`).
#   tools/build.sh [X,Z]     default: 47,28 (a wood)
at=${1:-47,28}
script=$(python3 -c "print('; '.join(['%.2f chop' % (0.6 + i * 0.12) for i in range(10)] + ['3 hold8'] + ['%.1f use' % (3.5 + i * 0.4) for i in range(8)] + ['8 forget']))")
out=$(LOOSE_TRACE=1 cargo sandbox --scene shots/build.png --play --at "$at" --hour 13 --after 12 --script "$script" 2>&1 | grep -E '^(loose: (put|landed)|(ok|FAIL) forget)')
puts=$(echo "$out" | grep -c '^loose: put Timber'); fell=$(echo "$out" | sed -n '/^loose: put Timber/,$p' | grep -c '^loose: landed'); forget=$(echo "$out" | grep -E '^(ok|FAIL) forget')
case "$forget" in ok*) [ "$puts" -ge 5 ] && [ "$fell" -eq 0 ] && { echo "ok build: $puts pieces of cut timber put down one on another, each stays where it was put, and the region made again is the same"; exit 0; } ;; esac
echo "FAIL build: $puts pieces put (bar 5), $fell came down after (bar 0) · ${forget:-nothing was dropped and made again}"; exit 1
