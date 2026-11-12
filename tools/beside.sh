#!/bin/sh
# `make beside`: the figure the hand-animated library was made on (left) and one of ours that
# carries the same clips (right) through the same keys, as one moving picture: shots/beside.gif.
# Where the two move differently the carrying is at fault (`forge/src/library.rs`); where they
# move alike and both look wrong, how the game plays clips is (`creatures::drive`).
#   tools/beside.sh [ID] [SCRIPT]     default: player, a walk, a run, a sprint, a stop, a leap
id=${1:-player}; script=${2:-0.5 fwd walk; 2.5 fwd run; 4.5 fwd sprint; 6.0 stop; 7.0 fwd run; 7.8 jump}
for who in mannequin "$id"; do
    cargo sandbox --scene "shots/beside-$who.gif" --play --as "$who" --gif 9 --view three --script "$script" > /dev/null 2>&1
    [ -f "shots/beside-$who.gif" ] || { echo "FAIL beside: no picture of $who"; exit 1; }
done
ffmpeg -hide_banner -loglevel error -y -i shots/beside-mannequin.gif -i "shots/beside-$id.gif" -filter_complex "[0][1]hstack,split[a][b];[a]palettegen[p];[b][p]paletteuse" shots/beside.gif || exit 1
rm -f shots/beside-mannequin.gif "shots/beside-$id.gif"
echo "ok beside: the library's figure and $id · shots/beside.gif"
