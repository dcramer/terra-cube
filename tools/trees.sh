#!/bin/sh
# `make trees`: a tree worked with an axe in the real game at four places (a thin birch, an
# ordinary tree, and two great ones of 60,000 and 86,000 cells), by a script's blows. At each
# it must be limbed, then bucked, and three of its logs split; nothing of it may come loose
# as a piece of no tree (a great one's crown did, where nobody could see it); and no tick may
# take longer than 60 ms (working a great one took 160).
#   tools/trees.sh [X,Z ...]     default: -60,20 47,28 10,60 -32,-40
script=$(python3 -c "print('; '.join(['%.2f chop' % (0.6 + i * 0.2) for i in range(140)]))")
bad=""; worst=0; n=0
for at in ${*:--60,20 47,28 10,60 -32,-40}; do
    got=$(LOOSE_TRACE=1 SLOW_TRACE=1 cargo sandbox --scene shots/trees.png --play --at "$at" --hour 13 --after 34 --script "$script" 2>&1 | grep -E '^loose: |^slow' | awk '/split into/{s++} /bucked into/{b++; if (!k) early=1} /boughs and leaf knocked/{k++} /^loose: take.*thing None/{tk++} /^slow system tick/{if ($4+0>m) m=$4+0} END{printf "%d %d %d %d %d %d", k, b, s, tk, m, early}')
    set -- $got
    [ "$1" -ge 1 ] && [ "$2" -ge 1 ] && [ "$3" -ge 3 ] && [ "$4" -eq 0 ] && [ "$5" -le 60 ] && [ "$6" -eq 0 ] || bad="$bad · at $at: limbed $1, bucked $2 (before it was limbed: $6), split $3, pieces of no tree $4, worst tick $5 ms"
    [ "$5" -gt "$worst" ] && worst=$5; n=$((n + 1))
done
[ -z "$bad" ] && { echo "ok trees: $n trees limbed, bucked and split in the real game, a thin one and two great ones among them, nothing of any come loose unseen, worst tick $worst ms"; exit 0; }
echo "FAIL trees$bad (bars: limbed 1, bucked 1 and not before, split 3, pieces 0, tick 60 ms)"; exit 1
