#!/bin/sh
# `make held`: the player with the sword, then the torch, in hand, close and from the side, at
# four moments each (standing, walking, stopped, a blow): shots/held.png. Look at it after any
# change to assets/world/held.toml or to a biped's clips.
shot() { cargo sandbox --scene "shots/held-$1.png" --play --script "0.1 stop $2; 1.2 fwd walk; 3.2 stop; 4.0 use" --strip 4 --after 0.8 --every 1.2 --view side --dist 3.2 --hour "$3" 2>&1 | grep -q "^shot" || { echo "FAIL held $1: no picture"; exit 1; }; }
shot sword hold1 13
shot torch torch 21
ffmpeg -hide_banner -loglevel error -y -i shots/held-sword.png -i shots/held-torch.png -filter_complex vstack shots/held.png || exit 1
rm -f shots/held-sword.png shots/held-torch.png
echo "ok held: sword and torch, standing · walking · stopped · a blow · shots/held.png"
