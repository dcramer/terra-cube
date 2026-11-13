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
# A cricket's chirp, four of them: three or four pulses of a tone near 4.6 kHz with a little
# of its octave, each pulse a fiftieth of a second. A field cricket's song is as plain as
# that, and no recording of single crickets free of conditions was found.
k() { f -f lavfi -i "aevalsrc='(sin(2*PI*$2*t)+0.2*sin(4*PI*$2*t))*pow(sin(PI*mod(t,$3)/($3*0.55)),2)*lt(mod(t,$3),$3*0.55)*lt(t,$3*$4)*0.5':d=0.32:s=44100" -af "afade=t=out:st=0.26:d=0.06" -c:a libvorbis -q:a 4 "cricket_$1.ogg"; }
k 1 4600 0.042 3
k 2 4350 0.046 3
k 3 4900 0.040 4
k 4 4150 0.050 3
echo "ok made: cricket"
# What cutting and falling sound like (2026-11-12), until recordings are found: an axe in
# wood (a knock with a crack on it, three of them), a spade in earth (a short scrape, two),
# a pick on stone (a ring and a clack, two), a tree beginning to go (a run of cracks), a
# great thing coming down (a thud and a long rattle), and rubble settling (clatter, two).
c() { f -f lavfi -i "aevalsrc='(0.9*sin(2*PI*$2*t)*exp(-t*28)+0.5*sin(2*PI*$3*t)*exp(-t*45))':d=0.28:s=44100" -f lavfi -i "anoisesrc=color=white:duration=0.28:seed=$4" \
    -filter_complex "[1]highpass=f=900,lowpass=f=4200,afade=t=out:st=0.005:d=0.07,volume=0.9[n];[0][n]amix=inputs=2:normalize=0,afade=t=out:st=0.16:d=0.12" -ar 44100 -c:a libvorbis -q:a 4 "chop_$1.ogg"; }
c 1 190 520 11
c 2 170 610 12
c 3 215 480 13
d() { f -f lavfi -i "anoisesrc=color=brown:duration=0.34:seed=$2" -af "highpass=f=180,lowpass=f=$3,afade=t=in:d=0.03,afade=t=out:st=0.1:d=0.24,volume=2.2" -ar 44100 -c:a libvorbis -q:a 4 "dig_$1.ogg"; }
d 1 21 1500
d 2 22 1900
p() { f -f lavfi -i "aevalsrc='(0.6*sin(2*PI*$2*t)*exp(-t*16)+0.35*sin(2*PI*$2*2.76*t)*exp(-t*26))':d=0.4:s=44100" -f lavfi -i "anoisesrc=color=white:duration=0.4:seed=$3" \
    -filter_complex "[1]highpass=f=2000,lowpass=f=7000,afade=t=out:st=0.003:d=0.04,volume=0.8[n];[0][n]amix=inputs=2:normalize=0,afade=t=out:st=0.25:d=0.15" -ar 44100 -c:a libvorbis -q:a 4 "pick_$1.ogg"; }
p 1 1250 31
p 2 1480 32
f -f lavfi -i "anoisesrc=color=white:duration=1.1:seed=41" -af "highpass=f=700,lowpass=f=3800,volume='if(lt(mod(t*t*9+0.03,0.19),0.016),1.6,0.02)':eval=frame,afade=t=out:st=0.9:d=0.2" -ar 44100 -c:a libvorbis -q:a 4 crack_1.ogg
f -f lavfi -i "aevalsrc='1.2*sin(2*PI*58*t)*exp(-t*7)':d=1.8:s=44100" -f lavfi -i "anoisesrc=color=brown:duration=1.8:seed=43" \
    -filter_complex "[1]lowpass=f=900,volume=3.2,afade=t=in:d=0.01,afade=t=out:st=0.08:d=1.7[n];[0][n]amix=inputs=2:normalize=0" -ar 44100 -c:a libvorbis -q:a 4 crash_1.ogg
r() { f -f lavfi -i "anoisesrc=color=white:duration=0.9:seed=$2" -af "highpass=f=300,lowpass=f=2600,volume='if(lt(mod(t*$3+0.01,0.13),0.02),1.5,0.03)*exp(-t*2.5)':eval=frame,afade=t=out:st=0.6:d=0.3" -ar 44100 -c:a libvorbis -q:a 4 "rubble_$1.ogg"; }
r 1 51 1.0
r 2 52 1.37
echo "ok made: chop dig pick crack crash rubble"
