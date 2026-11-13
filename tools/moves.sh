#!/bin/sh
# `make moves ID=wolf`: the whole routine for one creature's movement, in one line and one
# picture. Forge fits the walker to the model, walks it with no window and bakes each gait
# (`crates/forge/src/walker.rs`), and judges every clip by number; then one sheet of its
# walk, run and sprint from the side is drawn (shots/ID-side.png). Read the line; look at
# the picture once; then add what was looked at to assets/creatures/seen.toml.
#   tools/moves.sh ID [CLIPS]     default clips: walk,run,sprint
id=${1:?which creature}; clips=${2:-walk,run,sprint}
out=$(cargo forge finish "$id" 2>&1)
echo "$out" | grep -E "^$id:|FAIL" | head -6
how=$(awk '/^\[clips\.(walk|run|sprint)\]/{n=substr($0,8,length($0)-8)} /^source/{if(n!="")printf "%s %s · ", n, $3; n=""}' "assets/creatures/$id/meta.toml" | tr -d '"')
echo "gaits: $how"
cargo sandbox --shot "$id" --view side --clips "$clips" --cols 8 > /dev/null 2>&1 && echo "shots/$id-side.png" || { echo "FAIL moves $id: no sheet (is there a display? DISPLAY=:1)"; exit 1; }
