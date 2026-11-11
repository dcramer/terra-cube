#!/bin/sh
# `make floors`: the ground of each kind of place seen from head height looking down a
# little, tiled into shots/floors.png: a trail, forest floor, meadow, shore. Look at it after
# any change to what the ground is made of or what grows on it (`terrain::fill`).
shot() {
    cargo sandbox --wild --scene "shots/floors/$1.png" --at "$2" --dist "${3:-9}" --pitch "${4:-0.35}" --hour 14 --after 4 2>&1 | tail -1
}
place() {
    cargo world find "$1" | head -1 | cut -d' ' -f3
}
# The trail is seen as whoever walks it sees it.
cargo sandbox --wild --scene shots/floors/trail.png --play --at "$(place trail)" --script '1 stop' --after 3 --view three --hour 14 2>&1 | tail -1
shot forest "$(place forest)"
shot meadow 6,4
shot shore "$(place shore)" 12 0.3
python3 "$(dirname "$0")/sheet.py" shots/floors.png shots/floors/trail.png shots/floors/forest.png shots/floors/meadow.png shots/floors/shore.png
