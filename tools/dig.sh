#!/bin/sh
# `make dig`: the ground dug and put back in the real game. The player digs with the shovel,
# takes the earth in hand and puts it down where he looks, and the changed region is dropped
# and made again from the seed and the journal (a script's `forget`): no cell may differ.
#   tools/dig.sh [X,Z]     default: -60,20 (a meadow)
at=${1:--60,20}
script=$(python3 -c "print('; '.join(['0.5 hold3'] + ['%.1f use' % (1 + i * 0.6) for i in range(10)] + ['7.5 hold8'] + ['%.1f use' % (8 + i * 0.4) for i in range(8)] + ['12 forget']))")
out=$(LOOSE_TRACE=1 cargo sandbox --scene shots/dig.png --play --at "$at" --hour 13 --after 16 --script "$script" 2>&1 | grep -E '^(loose: (cut|put)|(ok|FAIL) forget)')
cuts=$(echo "$out" | grep -c '^loose: cut'); puts=$(echo "$out" | grep -c '^loose: put'); forget=$(echo "$out" | grep -E '^(ok|FAIL) forget')
case "$forget" in ok*) [ "$cuts" -ge 4 ] && [ "$puts" -ge 6 ] && { echo "ok dig: $cuts holes dug, $puts lumps of earth put back, and the region made again from seed and journal is the same"; exit 0; } ;; esac
echo "FAIL dig: $cuts holes dug (bar 4), $puts lumps put back (bar 6) · ${forget:-nothing was dropped and made again}"; exit 1
