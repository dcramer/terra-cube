#!/bin/sh
# `make grown`: every kind of thing that grows stood alone at the start and pictured, the
# trees whole and close to (shots/grown/KIND.png, KIND-near.png). Look at these after any
# change to how what grows is shaped or drawn. `tools/grown.sh oak pine` does only those.
kinds=${*:-oak birch pine willow snag young sapling bush log stump rock}
for k in $kinds; do
    case $k in
        oak | birch | pine | willow | snag) far=30 ;;
        *) far=7 ;;
    esac
    cargo sandbox --wild --scene "shots/grown/$k.png" --specimen "$k" --dist "$far" --pitch 0.05 --hour 14 --after 4 2>&1 | tail -1
    if [ "$far" = 30 ]; then
        cargo sandbox --wild --scene "shots/grown/$k-near.png" --specimen "$k" --dist 6 --pitch 0.12 --hour 14 --after 4 2>&1 | tail -1
    fi
done
