#!/bin/sh
# `make stone`: a rock worked with a pick in the real game (a script's `pick`, at the nearest
# stone) until it breaks into hunks, one taken up and put down, and the region dropped and
# made again from seed and journal (`forget`). The rock must give at least one hunk (a small
# one broke into nothing), and blows that do nothing must not be kept as edits (sixty were).
#   tools/stone.sh [X,Z]     default: -60,20 (a meadow with a rock ten metres off)
at=${1:--60,20}
script=$(python3 -c "print('; '.join(['%.2f pick' % (0.6 + i * 0.25) for i in range(60)] + ['17 lift', '18 use', '23 forget']))")
out=$(LOOSE_TRACE=1 cargo sandbox --scene shots/stone.png --play --at "$at" --hour 13 --after 26 --script "$script" 2>&1 | grep -E '^(loose: (chip: a boulder|lifted|laid)|(ok|FAIL) forget)')
hunks=$(echo "$out" | sed -n 's/^loose: chip: a boulder.* into \([0-9]*\) hunks.*/\1/p' | head -1); lifted=$(echo "$out" | grep -c '^loose: lifted Cobble'); laid=$(echo "$out" | grep -c '^loose: laid Cobble')
forget=$(echo "$out" | grep -E '^(ok|FAIL) forget'); edits=$(echo "$forget" | sed -n 's/.* \([0-9]*\) of [0-9]* edits.*/\1/p')
case "$forget" in ok*) [ "${hunks:-0}" -ge 1 ] && [ "$lifted" -ge 1 ] && [ "$laid" -ge 1 ] && [ "${edits:-99}" -le 12 ] && { echo "ok stone: a rock broken with a pick into $hunks hunks, one taken up and put down, $edits edits kept of sixty blows, and the region made again is the same"; exit 0; } ;; esac
echo "FAIL stone: hunks ${hunks:-0} (bar 1), taken up $lifted (bar 1), put down $laid (bar 1), edits kept ${edits:-?} (no more than 12) · ${forget:-nothing was dropped and made again}"; exit 1
