#!/bin/sh
# `make shatter`: a stone a metre and a half across let go five metres up in the real game (a
# script's `drop`). It must come down hard and break (the whole of it lands and none of it is
# put back as it was), its lumps must lie where they are thrown, the rubble must be heard,
# a light lump must be heaved by hand, a pick must break the lumps into hunks and one be
# carried (a pick did nothing to a lump: that was among what an axe does), and the region
# dropped and made again from seed and journal must be the same (`forget`).
#   tools/shatter.sh [X,Z]     default: -60,20 (a meadow)
at=${1:--60,20}
script=$(python3 -c "print('; '.join(['1 drop', '8 lift', '9 lift', '10 lift'] + ['%.2f pick' % (11 + i * 0.25) for i in range(48)] + ['24 lift', '25 use', '28 forget']))")
out=$(LOOSE_TRACE=1 cargo sandbox --scene shots/shatter.png --play --at "$at" --hour 13 --after 31 --script "$script" 2>&1 | grep -E '^(loose: (landed|shove|chip: a lump|lifted Cobble|laid Cobble)|sound rubble|(ok|FAIL) forget)')
broke=$(echo "$out" | awk '/^loose: landed/ && $3 >= 100 && $6 + 0 >= 4.5 && $8 == 0' | wc -l)
lumps=$(echo "$out" | awk '/^loose: landed/ && $3 < 100 && $8 > 0' | wc -l)
heard=$(echo "$out" | grep -c '^sound rubble'); heaved=$(echo "$out" | grep -c '^loose: shove'); hunks=$(echo "$out" | sed -n 's/^loose: chip: a lump.* into \([0-9]*\) hunks.*/\1/p' | awk '{n += $1} END {print int(n / 2)}'); carried=$(echo "$out" | grep -c '^loose: laid Cobble'); forget=$(echo "$out" | grep -E '^(ok|FAIL) forget')
case "$forget" in ok*) [ "$broke" -ge 1 ] && [ "$lumps" -ge 6 ] && [ "$heard" -ge 1 ] && [ "$heaved" -ge 3 ] && [ "$hunks" -ge 6 ] && [ "$carried" -ge 1 ] && { echo "ok shatter: a stone let go came down and broke into $lumps lumps that lie where they fell, heard $heard times, a light one heaved by hand, the lumps broken with a pick into $hunks hunks and one carried, and the region made again is the same"; exit 0; } ;; esac
echo "FAIL shatter: broke $broke (bar 1), lumps lying $lumps (bar 6), heard $heard (bar 1), heaves $heaved (bar 3), hunks from its lumps $hunks (bar 6), carried $carried (bar 1) · ${forget:-nothing was dropped and made again}"; exit 1
