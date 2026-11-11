#!/bin/sh
# How the sounds of the air were made from their recordings, so it can be done again: the
# beds (wind, water) as stretches of long recordings, and the small sounds (each bird's
# calls, frogs, a raven, leaves in a gust) cut out by `cut.py`, which finds them by
# measurement. `cut.sh DIR`, where DIR holds the originals under the names the README of
# assets/audio gives (they are not kept here: tens of megabytes). A source that is not there
# is passed over and said. Then `loops.py --fix` and `level.py --fix`.
here="$(dirname "$0")"; src="${1:?cut.sh DIR: the folder of original recordings}"; out="$here/../../assets/audio"
c() { s="$src/$1"; shift; if [ -s "$s" ]; then python3 -I "$here/cut.py" "$s" "$@"; else echo "passed over: $s"; fi; }
# A bed: SOURCE NAME START SECONDS, in stereo, the rumble under 180 Hz taken off, a second's
# fade at each end (loops.py then joins the end to the start).
bed() { if [ -s "$src/$1" ]; then ffmpeg -hide_banner -loglevel error -y -ss "$3" -t "$4" -i "$src/$1" -af "highpass=f=${5:-180}" -ac 2 -ar 44100 -c:a libvorbis -q:a 4 "$out/$2.ogg" && echo "$2.ogg	$4	$3 s on, of $1"; else echo "passed over: $src/$1"; fi; }

# Wind: a strong wind in trees (three stretches far apart) and wind over a mountain lake (two).
W=yell-040201LowerGeyserBasinWindInTreesBinaural01011.mp3
bed $W amb_wind_2 20 32
bed $W amb_wind_3 128 32
bed $W amb_wind_4 236 32
L=romo-WindAmbientGemLakeROMO52516Final1.mp3
bed $L amb_wind_5 30 32
bed $L amb_wind_6 170 32
# Water: a small stream at noon (three stretches far apart), beside the first river.
R=romo-StreamAmbientROMO52516BlackCanyonTrailFinal1.mp3
bed $R amb_water_2 20 32 120
bed $R amb_water_3 130 32 120
bed $R amb_water_4 240 32 120

# Songbirds, a set to each kind.
c Carolina_Wren.ogg song_wren --count 5 --min 0.8 --max 3.5
c Eastern_Meadowlark.ogg song_lark --count 4 --min 0.6 --max 3.5
c yell-YELLWEME20150425ms.mp3 song_lark --count 3 --min 0.6 --max 3.5 --from 5
c Kentucky_Warbler.ogg song_warbler --count 4 --min 0.6 --max 3.5
c Carolina_Chickadee.ogg song_chickadee --count 5 --min 0.5 --max 3.5
c White_breasted_Nuthatch.ogg song_nuthatch --count 4 --min 0.4 --max 3.5
c Turdus_merula_2.ogg song_merle --count 4 --min 0.6 --max 4
c Yellowstone_sound_library_American_Robin_001.mp3 song_robin --count 4 --min 0.5 --max 4
c romo-HETHROMO672017BierstadtLake.mp3 song_thrush --count 2 --min 0.6 --max 4
c romo-HETHROMO612016AllensParkTrail.mp3 song_thrush --count 2 --min 0.6 --max 4 --from 3
c romo-HETHROMO6102015FernLakeTrail.mp3 song_thrush --count 2 --min 0.6 --max 4 --from 5
c Black_capped_Chickadee_201947598.wav song_tit --count 3 --min 0.3 --max 3 --clear 9
c House_Sparrow.ogg song_sparrow --count 4 --min 0.2 --max 3
# Heard from far: a raven and a crow, woodpeckers, a dove; an owl by night.
c yell-RavenBlackSandBasin.mp3 far_crow --count 4 --min 0.2 --max 3 --high 400
c American_Crow.ogg far_crow --count 3 --min 0.2 --max 3 --high 400 --from 5
c romo-HAWO2ROMO5302017BeaverPonds.mp3 far_drum --count 4 --min 0.4 --max 2.5 --high 300 --low 5000
c Dove_cooing.ogg far_dove --count 3 --min 0.4 --max 4 --high 200 --low 2500
c Barred_Owl_Yellowstone_National_Park.oga owl --count 2 --min 0.5 --max 5 --high 200 --low 3000
c Otus_sunia.ogg owl --count 3 --min 0.2 --max 3 --high 300 --low 4000 --from 3
# Frogs, added to the two the frog itself has.
c yell-20150427T13BorealfrogSACRWISN.mp3 frog --count 4 --min 0.2 --max 2.8 --gap 0.15 --high 500 --from 3
c Edible_frogs_sounds._May._The_Village_of_Krzemienica_d_Voivodeship_Poland.ogg frog --count 3 --min 0.2 --max 2.5 --gap 0.15 --high 300 --from 7
# The insects of a summer's day.
c Grasshoppers.ogg insect --count 3 --swell 1.8 --high 5000
c Cicada_orni.ogg insect --count 3 --swell 2.6 --high 2500 --from 4
c Hummel_bee.ogg insect --count 2 --swell 2.2 --high 120 --low 3000 --from 7
# Leaves in a gust: the loudest stretches of the wind in the trees, without its low end.
c $W rustle --count 5 --swell 3.2 --high 700
