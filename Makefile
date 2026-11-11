# The everyday commands, so nobody has to remember the cargo ones. `make` alone checks the
# machine is set up; `make help` lists them.
# Each is one line over `cargo forge`, `cargo sandbox` or `cargo world`; those take more options (--help).

SPAWN ?= all
ARGS ?= --play --wild
WINDOWS := x86_64-pc-windows-gnu
# In WSL the game is built for Windows and opened there: the real GPU, a native window.
WSL := $(shell grep -qi microsoft /proc/version 2>/dev/null && echo 1)

.DEFAULT_GOAL := setup
.PHONY: sound years seasons grown floors prove swim window fight slice help setup run play windows list scene sheet gif status creature finish world map test lint cross check

help: ## this list
	@awk -F ':.*## ' '/^[a-z]+:.*## / { printf "make %-22s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

setup: ## (plain `make`) check this machine: node, Tripo login, display, models
	cargo forge doctor

run: ## the sandbox window, one of each creature to pose and inspect (SPAWN=wolf,player for some)
ifeq ($(WSL),1)
	@$(MAKE) --no-print-directory windows ARGS="--spawn $(SPAWN)"
else
	cargo sandbox --spawn $(SPAWN)
endif

play: ## the living world, as the player: WASD, Shift sprints, Ctrl walks, Space jumps, V first person, Tab frees the mouse
ifeq ($(WSL),1)
	@$(MAKE) --no-print-directory windows ARGS="--play --wild"
else
	cargo sandbox --play --wild
endif

windows: ## build for Windows and, in WSL, open it there; `make play` and `make run` do this themselves in WSL
	@command -v x86_64-w64-mingw32-gcc >/dev/null || { echo "needs the Windows linker: sudo apt install mingw-w64"; exit 2; }
	CARGO_PROFILE_RELEASE_DEBUG=false CARGO_PROFILE_RELEASE_STRIP=true cargo build --release --package cube_game --target $(WINDOWS)
	@if grep -qi microsoft /proc/version 2>/dev/null; then target/$(WINDOWS)/release/cube_game.exe $(ARGS); \
	else echo "built target/$(WINDOWS)/release/cube_game.exe; this is not WSL, so it was not started"; fi

list: ## the creatures that can be spawned
	@cargo sandbox --list 2>/dev/null

scene: ## no window: shots/scene.png, the sandbox with creatures in it
	cargo sandbox --scene shots/scene.png --spawn $(SPAWN)

world: ## no window: is the world sound? the land, then five minutes of its creatures, in numbers
	cargo world survey
	cargo world run --seconds 300

map: ## no window: shots/map.png, the island from above with where each creature went
	cargo world map shots/map.png

# The vertical slice: the same six places at the same hours, every time. Look at all of them
# after any change to how the world is drawn; a view that got worse is a regression.
SLICE = cargo sandbox --wild --scene shots/slice
slice: ## no window: shots/slice/*.png, seven fixed views and two moving ones, to judge any change to its look against
	$(SLICE)/meadow-noon.png --play --script '2 fwd walk' --after 3 --view three --hour 13
	$(SLICE)/meadow-dawn.png --play --script '2 fwd walk' --after 3 --view three --hour 6.4
	$(SLICE)/meadow-night.png --play --script '2 fwd walk' --after 6 --view three --hour 21
	$(SLICE)/meadow-torch.png --play --script '2 fwd walk' --after 6 --view three --hour 21 --torch
	$(SLICE)/forest-day.png --at 19,34 --dist 10 --pitch 0.1 --hour 15 --after 8
	$(SLICE)/shore-evening.png --at 48,19 --dist 14 --pitch 0.3 --hour 17
	$(SLICE)/island-high.png --view high --hour 15
	$(SLICE)/meadow-autumn.png --play --script '2 fwd walk' --after 3 --view three --day 17 --hour 13
	$(SLICE)/meadow-winter.png --play --script '2 fwd walk' --after 3 --view three --day 24 --hour 13
	$(SLICE)/hud-pack.png --play --script '2 fwd walk' --after 3 --view three --hour 13 --hud pack
	$(SLICE)/hud-admin.png --play --script '2 fwd walk' --after 3 --view three --hour 13 --hud admin
	$(SLICE)/far-high.png --at 0,0 --view high --dist 420 --pitch 0.4 --hour 13
	$(SLICE)/forest-moving.gif --at 19,34 --dist 10 --pitch 0.1 --hour 15 --gif 6
	$(SLICE)/meadow-walk.gif --play --script '1 fwd walk; 3 fwd run; 5 stop' --gif 7 --view three --hour 13

prove: ## every judgment there is, a line each: code, land, creatures, ecology, sound levels, what is heard, a fight, a swim, the real window
	@$(MAKE) --no-print-directory check > /dev/null 2>&1 && echo "ok check: tests, clippy, the Windows build" || echo "FAIL check (make check)"
	@cargo world survey | tail -1 | sed 's/^ok$$/ok survey: the land/'
	@cargo world run --seconds 300 | tail -1 | sed 's/^ok$$/ok run: five minutes of creatures/'
	@cargo world life | tail -1 | sed 's/^ok$$/ok life: every kind has somewhere to live/'
	@python3 tools/audio/level.py | tail -1
	@python3 tools/audio/loops.py | tail -1
	@$(MAKE) --no-print-directory sound
	@$(MAKE) --no-print-directory fight
	@$(MAKE) --no-print-directory swim
	@$(MAKE) --no-print-directory window
	@tools/seasons.sh
	@cargo world years 8 | tail -1 | sed 's/^ok$$/ok years: eight years, no kind dies out or overruns/'

years: ## the island's creatures by the day for eight years: each kind's numbers year by year, and whether they hold
	@cargo world years 8

sound: ## no window: five minutes heard in a meadow by day and by a shore from dusk, judged from the trace: how many files, how loud against a footfall (tools/audio/listen.sh forest 21 for any place and hour)
	@cargo build -q --release -p cube_game
	@tools/audio/listen.sh meadow 10
	@tools/audio/listen.sh shore 18 10 --busy 100

seasons: ## no window: the meadow in summer and winter, judged by the ground's colour: green, then snow
	@tools/seasons.sh

swim: ## no window: the player is put in deep water in the real game; passes if he floats, swims and is heard
	@tools/xkeys/swim.sh

window: ## the real window on a screenless display, driven by real key presses: the loading screen holds W off and lifts, then W walks, 2 takes the torch, I opens the pack
	@cargo build -q --release -p cube_game && tools/xkeys/window.sh target/release/cube_game

fight: ## no window: the player fights a goblin in the real game; passes if he is hit, kills it, and the sounds played
	@cargo sandbox --scene shots/fight.png --play --script '0.5 stop; 1 spawn=goblin; 1.2 use; 2.4 use; 3.6 use; 4.8 use; 6 use; 7.2 use' --after 9 --view three --hour 13 2>&1 | awk '/^play/ { if (match($$0, /health=[0-9]+/)) { h = substr($$0, RSTART + 7, RLENGTH - 7) + 0; if (h < low || low == "") low = h } if ($$0 ~ /kills=1/) killed = 1 } /^sound swing/ { swing = 1 } /^sound monster_die|^sound goblin/ { voice = 1 } END { ok = (low < 100 && low > 0 && killed && swing && voice); printf "%s fight: lowest health %s, killed %d, swing heard %d, goblin heard %d\n", ok ? "ok" : "FAIL", low, killed, swing, voice; exit !ok }'

sheet: need-id ## no window: shots/ID.png, every clip of one creature (ID=wolf)
	cargo forge sheet $(ID)

grown: ## no window: shots/grown/*.png, every kind of tree and rock alone, whole and close (tools/grown.sh oak for one)
	@tools/grown.sh

floors: ## no window: shots/floors.png, the ground of a trail, forest, meadow and shore in one sheet
	@tools/floors.sh

gif: need-id ## no window: shots/ID.gif, creature ID moving through every gait, a jump, a blow and a flinch (ID=wolf)
	cargo sandbox --scene shots/$(ID).gif --play --as $(ID) --gif 12 --view three

status: ## every creature: Tripo steps done, credits, model
	cargo forge status

creature: need-id ## build what creature ID is missing; spends Tripo credits once (ID=fox)
	cargo forge build $(ID)

finish: ## remake every model's clips and checks, free, after changing clips.rs or style.toml
	cargo forge finish --all

test: ## unit tests
	cargo test --workspace

lint: ## clippy, warnings as errors
	cargo clippy --workspace -- -D warnings

cross: ## type-check everything for Windows too (no Windows linker needed)
	cargo check --workspace --target $(WINDOWS)

check: test lint cross ## all three; must pass before a commit

.PHONY: need-id
need-id:
	@test -n "$(ID)" || { echo "give a creature: make $(firstword $(MAKECMDGOALS)) ID=wolf"; exit 2; }
