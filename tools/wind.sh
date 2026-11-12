#!/bin/sh
# `make wind`: the same wood pictured twice half a second apart, in still air and in a hard
# wind, judged by number: how much the picture changed between the two (of 255, over all of it). In still air only
# what lives moves; in wind the leaves and grass must (several times as much), and not so
# much that the whole picture boils. (The wind is in two shaders and a number from the game:
# any of the three can come apart from the rest and nothing else would know.)
moved() {
    cargo sandbox --scene "shots/wind-$1.png" --play --at 19,34 --script '0.1 stop' --strip 2 --after 4 --every 0.5 --view three --hour 13 --wind "$1" --quiet > /dev/null 2>&1
    ffmpeg -hide_banner -loglevel error -i "shots/wind-$1.png" -filter_complex "[0]crop=iw/2:ih:0:0[a];[0]crop=iw/2:ih:iw/2:0[b];[a][b]blend=all_mode=difference,format=gray,signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=-" -f null - | awk -F= '/YAVG/ { printf "%.2f", $2 }'
}
still=$(moved 0)
blown=$(moved 1)
echo "$still $blown" | awk '{
    ok = ($2 > $1 * 3 && $2 > 0.8 && $2 < 12)
    printf "%s wind: in half a second the picture changes by %s in still air, %s in a hard wind%s\n", ok ? "ok" : "FAIL", $1, $2, ok ? "" : " (wants three times still air, over 0.8 and under 12: shots/wind-0.png, shots/wind-1.png)"
    exit !ok
}'
