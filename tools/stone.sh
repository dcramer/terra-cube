#!/bin/sh
# `make stone`: a rock worked with a pick in the real game (a script's `pick`, at the nearest
# stone) until it breaks into hunks, one taken up and put down, and the region dropped and
# made again from seed and journal (`forget`). The rock must give at least one hunk (a small
# one broke into nothing), and blows that do nothing must not be kept as edits (sixty were).
#   tools/stone.sh [X,Z [X,Z]]     default: -60,20 (a meadow with a rock ten metres off) and
#                                  14,95 (bare rock)
at=${1:--60,20}
script=$(python3 -c "print('; '.join(['%.2f pick' % (0.6 + i * 0.25) for i in range(60)] + ['17 lift', '18 use', '23 forget']))")
out=$(LOOSE_TRACE=1 cargo sandbox --scene shots/stone.png --play --at "$at" --hour 13 --after 26 --script "$script" 2>&1 | grep -E '^(loose: (chip: a boulder|lifted|laid)|(ok|FAIL) forget)')
hunks=$(echo "$out" | sed -n 's/^loose: chip: a boulder.* into \([0-9]*\) hunks.*/\1/p' | head -1); lifted=$(echo "$out" | grep -c '^loose: lifted Cobble'); laid=$(echo "$out" | grep -c '^loose: laid Cobble')
forget=$(echo "$out" | grep -E '^(ok|FAIL) forget'); edits=$(echo "$forget" | sed -n 's/.* \([0-9]*\) of [0-9]* edits.*/\1/p')
# And the land's own stone dug with the pick in hand (a script's `use`, at the ground a stride
# on): a bite takes stone and leaves a hunk beside it, which is taken up and put down.
dug=$(LOOSE_TRACE=1 cargo sandbox --scene shots/stone.png --play --at "${2:-14,95}" --hour 13 --after 18 --script "$(python3 -c "print('; '.join(['0.5 hold7'] + ['%.1f use' % (1 + i * 0.7) for i in range(14)] + ['11.5 lift', '12.5 use', '15 forget']))")" 2>&1 | grep -E '^(loose: (cut|lifted Cobble|laid Cobble)|(ok|FAIL) forget)')
bites=$(echo "$dug" | grep -c 'of Some(Stone) · took \[0, 0, [1-9]'); up=$(echo "$dug" | grep -c '^loose: lifted Cobble'); down=$(echo "$dug" | grep -c '^loose: laid Cobble'); again=$(echo "$dug" | grep -E '^(ok|FAIL) forget')
case "$again" in ok*) [ "$bites" -ge 1 ] && [ "$up" -ge 1 ] && [ "$down" -ge 1 ] || { echo "FAIL stone: of the land's stone, bites that took any $bites (bar 1), hunks taken up $up (bar 1), put down $down (bar 1)"; exit 1; } ;; *) echo "FAIL stone: the land's stone dug: ${again:-nothing was dropped and made again}"; exit 1 ;; esac
case "$forget" in ok*) [ "${hunks:-0}" -ge 1 ] && [ "$lifted" -ge 1 ] && [ "$laid" -ge 1 ] && [ "${edits:-99}" -le 12 ] && { echo "ok stone: a rock broken with a pick into $hunks hunks, one taken up and put down, $edits edits kept of sixty blows, the land's stone dug to a hunk and carried, and each region made again is the same"; exit 0; } ;; esac
echo "FAIL stone: hunks ${hunks:-0} (bar 1), taken up $lifted (bar 1), put down $laid (bar 1), edits kept ${edits:-?} (no more than 12) · ${forget:-nothing was dropped and made again}"; exit 1
