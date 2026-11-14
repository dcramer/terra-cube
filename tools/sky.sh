#!/bin/sh
# `make sky`: the sky and the light at fixed hours, from over the trees so most of each picture
# is sky (`--view sky`), into shots/sky/ and one sheet of them all (shots/sky.png). Look at the sheet after
# any change to the sky, the sun, the moon, the clouds, the exposure or a light source: a
# view that got worse is a regression. `tools/sky.sh noon dusk` renders only those.
# Yaw 0 looks north, 1.57 west (where the sun sets), -1.57 east, 3.14 south (the noon sun).
# Eighteen views, a sheet six by three: add one and the sheet wants another row.
set -e
mkdir -p shots/sky
only="$*"
want() { [ -z "$only" ] || echo " $only " | grep -q " $1 "; }
shot() {
    name=$1
    shift
    want "$name" || return 0
    cargo sandbox --wild --quiet --scene "shots/sky/$name.png" "$@" 2>&1 | awk -v n="$name" '/^shot/ { sub(/.*pace /, ""); print "sky " n ": " $0; seen = 1 } END { if (!seen) { print "FAIL sky " n ": no picture"; exit 1 } }'
}
# From over the trees by the start, the land's edge low in the picture.
wide="--at 0,0 --view sky"
shot dawn $wide --yaw -1.57 --hour 6.1
shot morning $wide --yaw -2.4 --hour 8.5
shot noon $wide --yaw 3.14 --hour 13
shot north $wide --yaw 0 --hour 13
shot evening $wide --yaw 1.57 --hour 17.2
shot sunset $wide --yaw 1.57 --hour 17.85
shot dusk $wide --yaw 1.57 --hour 18.3
shot moonrise $wide --yaw -1.57 --day 3 --hour 19.6
shot night $wide --yaw 3.14 --day 3 --hour 23.5
shot dark $wide --yaw 0 --day 7 --hour 23.5
# Nearly straight up, by day: what is overhead.
shot zenith $wide --pitch 1.2 --hour 14
# What is overhead by day, closer: the clouds' cubes and their undersides.
shot clouds $wide --pitch 0.75 --yaw 2.4 --hour 10
# Light sources: the torch in his hand at night in the open (from behind, and through his
# eyes) and under trees, and at dusk.
shot torch --play --script '0.3 torch; 2 fwd walk' --after 6 --view three --day 7 --hour 22
shot torch-first --play --script '0.3 torch; 2 stop' --after 5 --view first --day 7 --hour 22
shot torch-wood --play --at 19,34 --script '0.3 torch; 2 fwd walk' --after 6 --view three --day 7 --hour 22
shot torch-dusk --play --script '0.3 torch; 2 fwd walk' --after 6 --view three --hour 18.6
# A wood by day and a low sun through it: shafts and shade.
# Across a lake toward a low sun, the far bank's crowns in the haze; and a torch on a shore at sunset.
shot lake --at 173,-92 --dist 45 --pitch 0.1 --yaw 1.57 --hour 17.5 --after 6
shot torch-shore --play --at 48,2 --yaw 0.7 --script '0.3 torch; 1 fwd walk; 2 stop' --after 5 --view three --hour 18.2
shot wood --at 19,34 --dist 10 --pitch 0.1 --hour 15 --after 8
shot wood-low --at 19,34 --dist 10 --pitch 0.05 --yaw 1.57 --hour 17.3 --after 8
[ -n "$only" ] || ffmpeg -hide_banner -loglevel error -y -pattern_type glob -i 'shots/sky/*.png' -vf "scale=640:-1,tile=5x4" -frames:v 1 shots/sky.png
