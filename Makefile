# The everyday commands, so nobody has to remember the cargo ones. `make` alone checks the
# machine is set up; `make help` lists them.
# Each is one line over `cargo forge` or `cargo sandbox`; those take more options (--help).

SPAWN ?= all
WINDOWS := x86_64-pc-windows-gnu

.DEFAULT_GOAL := setup
.PHONY: help setup run windows list scene sheet status creature finish test lint cross check

help: ## this list
	@awk -F ':.*## ' '/^[a-z]+:.*## / { printf "make %-22s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

setup: ## (plain `make`) check this machine: node, Tripo login, display, models
	cargo forge doctor

run: ## the game window, one of each creature (SPAWN=wolf,player for some)
	cargo sandbox --spawn $(SPAWN)

windows: ## from WSL: build for Windows and open the game there as a native window
	@command -v x86_64-w64-mingw32-gcc >/dev/null || { echo "needs the Windows linker: sudo apt install mingw-w64"; exit 2; }
	CARGO_PROFILE_RELEASE_DEBUG=false CARGO_PROFILE_RELEASE_STRIP=true cargo build --release --package cube_game --target $(WINDOWS)
	@if grep -qi microsoft /proc/version 2>/dev/null; then target/$(WINDOWS)/release/cube_game.exe --spawn $(SPAWN); \
	else echo "built target/$(WINDOWS)/release/cube_game.exe; this is not WSL, so it was not started"; fi

list: ## the creatures that can be spawned
	@cargo sandbox --list 2>/dev/null

scene: ## no window: shots/scene.png, the world with creatures in it
	cargo sandbox --scene shots/scene.png --spawn $(SPAWN)

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
