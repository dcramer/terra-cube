#!/bin/sh
# `make held`: the player with the sword, then the torch, in hand, close and from the side, at
# four moments each (standing, walking, stopped, a blow), then a blow with the sword, the
# torch and the bow at six moments a tenth of a second apart: shots/held.png. Look at it after
# any change to assets/world/held.toml or to a biped's clips.
shot() { cargo sandbox --scene "shots/held-$1.png" --play --script "$2" --strip "$3" --after "$4" --every "$5" --view side --dist 3.2 --hour "$6" 2>&1 | grep -q "^shot" || { echo "FAIL held $1: no picture"; exit 1; }; }
shot sword "0.1 stop hold1; 1.2 fwd walk; 3.2 stop; 4.0 use" 4 0.8 1.2 13
shot torch "0.1 stop torch; 1.2 fwd walk; 3.2 stop; 4.0 use" 4 0.8 1.2 21
shot cut "0.1 stop hold1; 1.0 use" 6 1.05 0.1 13
shot thrust "0.1 stop torch; 1.0 use" 6 1.05 0.1 13
shot loose "0.1 stop hold5; 1.0 use" 6 1.05 0.1 13
ffmpeg -hide_banner -loglevel error -y -i shots/held-sword.png -i shots/held-torch.png -filter_complex vstack shots/held-a.png || exit 1
ffmpeg -hide_banner -loglevel error -y -i shots/held-cut.png -i shots/held-thrust.png -i shots/held-loose.png -filter_complex vstack=3 shots/held-b.png || exit 1
ffmpeg -hide_banner -loglevel error -y -i shots/held-a.png -i shots/held-b.png -filter_complex "[1][0]scale2ref=iw:-1[b][a];[a][b]vstack" shots/held.png || exit 1
rm -f shots/held-sword.png shots/held-torch.png shots/held-cut.png shots/held-thrust.png shots/held-loose.png shots/held-a.png shots/held-b.png
echo "ok held: sword and torch, standing · walking · stopped · a blow; then a cut, a thrust and a shot by tenths of a second · shots/held.png"
