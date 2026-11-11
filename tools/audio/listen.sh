#!/bin/sh
# `make sound`: five minutes of the real game (no window, its clock run six times as fast, so
# a minute and a half of waiting) standing in a place from an hour on, and what was heard
# judged from the trace by `mix.py`: one line, ok or FAIL. Five minutes of the game are five
# hours of its day, so `meadow 10` is a meadow from ten in the morning to three.
# `listen.sh [PLACE] [HOUR] [DAY] [bars for mix.py...]`: PLACE is meadow (where the game
# starts), or forest, shore, rock... (the nearest, by `cargo world find`); DAY 3 is spring,
# 10 summer, 17 autumn, 24 winter. The trace is left in shots/sound-PLACE-HOUR.txt.
place="${1:-meadow}"; hour="${2:-10}"; day="${3:-10}"
[ $# -gt 3 ] && shift 3 || shift $#
at=""
if [ "$place" != meadow ]; then
    at="--at $(cargo world find "$place" | head -1 | cut -d' ' -f3)"
fi
mkdir -p shots
trace="shots/sound-$place-$hour.txt"
# A few steps first, so there is a footfall to measure the air against; then standing still.
# shellcheck disable=SC2086
cargo sandbox --wild --scene shots/sound.png --play $at --script '1 fwd walk; 7 stop' --after 300 --speed 6 --view three --hour "$hour" --day "$day" > "$trace" 2>&1
python3 "$(dirname "$0")/mix.py" "$trace" --name "$place from $hour h, day $day" "$@"
