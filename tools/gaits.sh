#!/bin/sh
# `make gaits ID="wolf deer"`: each creature's walk, run and sprint from the side, eight
# moments a clip, large enough to see a spine and a hoof, stacked into shots/gaits.png.
# `CLIPS=jump,fall,land tools/gaits.sh player` draws other clips the same way.
clips=${CLIPS:-walk,run,sprint}
ins=""
n=0
for c in "$@"; do
    cargo sandbox --shot "$c" --clips "$clips" --cols 8 --view side >/dev/null 2>&1 || { echo "FAIL gaits $c: no picture"; exit 1; }
    cp "shots/$c-side.png" "shots/gaits-$n.png"
    ins="$ins -i shots/gaits-$n.png"
    n=$((n + 1))
done
if [ "$n" = 1 ]; then
    mv shots/gaits-0.png shots/gaits.png
else
    # shellcheck disable=SC2086
    ffmpeg -hide_banner -loglevel error -y $ins -filter_complex "vstack=inputs=$n" shots/gaits.png || exit 1
    i=0
    while [ "$i" -lt "$n" ]; do rm -f "shots/gaits-$i.png"; i=$((i + 1)); done
fi
echo "ok gaits: $* · $clips · shots/gaits.png"
