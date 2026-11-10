# The everyday commands, so nobody has to remember the cargo ones. `make` lists them.
# Each is one line over `cargo forge` or `cargo sandbox`; those take more options (--help).

SPAWN ?= all

.DEFAULT_GOAL := help
.PHONY: help setup run list scene sheet status creature finish test lint check

help: ## this list
	@awk -F ':.*## ' '/^[a-z]+:.*## / { printf "make %-22s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

setup: ## check this machine: node, Tripo login, display, models
	cargo forge doctor

run: ## the game window, one of each creature (SPAWN=wolf,player for some)
	cargo sandbox --spawn $(SPAWN)

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

check: test lint ## both; must pass before a commit

.PHONY: need-id
need-id:
	@test -n "$(ID)" || { echo "give a creature: make $(firstword $(MAKECMDGOALS)) ID=wolf"; exit 2; }
