# The everyday commands, so nobody has to remember the cargo ones. `make` alone checks the
# machine is set up; `make help` lists them.
# Each is one line over `cargo forge`, `cargo sandbox` or `cargo world`; those take more options (--help).

SPAWN ?= all
ARGS ?= --play --wild
WINDOWS := x86_64-pc-windows-gnu
# In WSL the game is built for Windows and opened there: the real GPU, a native window.
WSL := $(shell grep -qi microsoft /proc/version 2>/dev/null && echo 1)

.DEFAULT_GOAL := setup
.PHONY: help setup run play windows list scene sheet status creature finish world map test lint cross check

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

sheet: need-id ## no window: shots/ID.png, every clip of one creature (ID=wolf)
	cargo forge sheet $(ID)

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
