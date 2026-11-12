#!/bin/sh
# `tools/library.sh`: fetches the hand-animated human motion forge carries onto Tripo's people
# (`crates/forge/src/library.rs`): Quaternius's Universal Animation Library, the free standard
# zip, CC0, from its page at itch.io, to .work/library/UAL1_Standard.glb. 15 MB, once.
# Needs curl, unzip and python3. If itch.io changes its pages, download the zip by hand from
# https://quaternius.itch.io/universal-animation-library and unzip the GLB there.
# The animals' motion is one more file, a wolf of the same author's (CC0), to be put beside it
# by hand as .work/library/Wolf.glb: the GLB from https://poly.pizza/m/P1gU3Qkr9r .
set -e
out=.work/library
[ -f "$out/Wolf.glb" ] || echo "note: $out/Wolf.glb is missing (animals' motion): the GLB from https://poly.pizza/m/P1gU3Qkr9r"
[ -f "$out/UAL1_Standard.glb" ] && { echo "ok library: $out/UAL1_Standard.glb"; exit 0; }
mkdir -p "$out/fetch"
page=https://quaternius.itch.io/universal-animation-library
agent="Mozilla/5.0"
json() { python3 -I -c "import sys,json;print(json.load(sys.stdin)['url'])"; }
down=$(curl -fsS -m 60 -A "$agent" -X POST "$page/download_url" | json)
curl -fsS -m 60 -A "$agent" -c "$out/fetch/cookies" "$down" -o "$out/fetch/page.html"
id=$(grep -o 'data-upload_id="[0-9]*"' "$out/fetch/page.html" | head -1 | tr -dc 0-9)
token=$(grep -o 'name="csrf_token" value="[^"]*"' "$out/fetch/page.html" | head -1 | sed 's/.*value="//; s/"$//')
file=$(curl -fsS -m 60 -A "$agent" -b "$out/fetch/cookies" -X POST "$page/file/$id?source=game_download" --data-urlencode "csrf_token=$token" | json)
curl -fsSL -m 600 -A "$agent" -o "$out/fetch/library.zip" "$file"
unzip -o -q -j "$out/fetch/library.zip" '*UAL1_Standard.glb' '*License.txt' -d "$out"
rm -rf "$out/fetch"
[ -f "$out/UAL1_Standard.glb" ] && echo "ok library: $out/UAL1_Standard.glb" || { echo "FAIL library: no GLB in the zip"; exit 1; }
