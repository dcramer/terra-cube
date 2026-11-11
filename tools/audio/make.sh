#!/bin/sh
# The sounds made here and not recorded, written into assets/audio: a dragonfly's wings, a
# lizard's hiss, a crab's claws, a tortoise's grunt, and a beaver's tail on the water (cut
# and lowered from the splash). Run again to make them again; then `level.py --fix`.
cd "$(dirname "$0")/../../assets/audio" || exit 1
f() { ffmpeg -hide_banner -loglevel error -y "$@"; }
f -f lavfi -i "sine=f=170:d=0.5" -f lavfi -i "sine=f=340:d=0.5" -f lavfi -i "sine=f=510:d=0.5" \
    -filter_complex "[0][1][2]amix=inputs=3:normalize=0,tremolo=f=38:d=0.7,afade=t=in:d=0.06,afade=t=out:st=0.32:d=0.18" -ar 44100 -c:a libvorbis -q:a 4 buzz_1.ogg
f -f lavfi -i "anoisesrc=color=white:duration=0.6:seed=3" -af "highpass=f=2500,lowpass=f=7000,afade=t=in:d=0.08,afade=t=out:st=0.18:d=0.42" -ar 44100 -c:a libvorbis -q:a 4 hiss_1.ogg
f -f lavfi -i "anoisesrc=color=white:duration=0.3:seed=5" -af "highpass=f=1500,lowpass=f=5000,volume='if(lt(mod(t,0.1),0.012),1,0)':eval=frame" -ar 44100 -c:a libvorbis -q:a 4 click_1.ogg
f -f lavfi -i "anoisesrc=color=brown:duration=0.3:seed=9" -af "lowpass=f=380,afade=t=in:d=0.04,afade=t=out:st=0.1:d=0.2" -ar 44100 -c:a libvorbis -q:a 4 grunt_1.ogg
f -i splash_1.ogg -af "asetrate=44100*0.78,aresample=44100,atrim=0:0.7,afade=t=out:st=0.4:d=0.3" -c:a libvorbis -q:a 4 beaver_1.ogg
echo "ok made: buzz hiss click grunt beaver"
