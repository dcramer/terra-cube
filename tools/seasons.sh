#!/bin/sh
# `make seasons`: the same meadow pictured in summer and in winter, judged by number: the
# ground (the lower third of the picture, taken as one colour) is green in summer and, under
# snow, no greener than it is blue in winter. (The season's tint once went unapplied for a
# day when the renderer changed, and every check passed.)
shot() {
    cargo sandbox --wild --scene "shots/seasons/$1.png" --play --script '2 fwd walk' --after 3 --view three --day "$2" --hour 13 --quiet > /dev/null 2>&1
    ffmpeg -hide_banner -loglevel error -i "shots/seasons/$1.png" -vf "crop=iw:ih/3:0:ih*2/3,scale=1:1:flags=area" -f rawvideo -pix_fmt rgb24 - | od -An -tu1 | awk '{ printf "%d %d %d", $1, $2, $3 }'
}
summer=$(shot summer 10)
winter=$(shot winter 24)
echo "$summer $winter" | awk '{
    ok = ($2 > $3 * 1.25 && $5 < $6 * 1.08)
    printf "%s seasons: summer ground %d,%d,%d · winter ground %d,%d,%d%s\n", ok ? "ok" : "FAIL", $1, $2, $3, $4, $5, $6, ok ? "" : " (summer wants green well over blue, winter not: is the season applied in both wind shaders?)"
    exit !ok
}'
