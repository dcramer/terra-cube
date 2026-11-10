# terra-cube

An experiment: a voxel sandbox game with heavy simulation, better looking than Minecraft (Eco is the reference), built in Rust by one developer and coding agents. Every model in it is generated. This file is the map; the docs it points to are the record. Keep it under 120 lines.

## Where things stand

- **Works:** the creature pipeline (`cargo forge`): one line of text to an animated model the game loads, checked by numbers and by one picture. The sandbox (`cargo sandbox`) spawns them in a small voxel world, and `--play` walks the player (or any creature) about it with a follow camera.
- **Baseline only:** the voxel world is one fixed test terrain with a simple mesher. The simulation is movement alone. No saving, no block editing, no networking yet.
- **Next:** [docs/engine.md](docs/engine.md#order-of-work).

## Layout

| Path | What it is |
|---|---|
| `crates/cube_voxel` | Voxel data, test terrain, chunk meshing. No engine types, so the simulation can use it headless. |
| `crates/cube_sim` | The simulation, at a fixed 60 ticks a second, no engine types. Today: how a body moves through voxels (walk, step, jump, fall, float). |
| `crates/cube_game` | The Bevy app. Today it is the sandbox: terrain, creature spawning, clip controls, contact sheets. |
| `crates/forge` | The creature pipeline CLI. Each file's header says what it holds; `main.rs` has the map. |
| `tools/tripo` | Tripo Studio CLI (TypeScript, Node ≥ 22.18, no dependencies). Forge drives it; see its README before calling it by hand. |
| `assets/creatures/style.toml` | The art style and body plans, as forge sends them. Change the look here, not per creature. |
| `assets/creatures/<id>/` | `creature.toml` (written by hand), `forge.lock` (what Tripo made), `meta.toml` and `model.glb` (made by forge; never edit). |
| `docs/` | [engine.md](docs/engine.md) design and order of work · [pipeline.md](docs/pipeline.md) making creatures · [art-guide.md](docs/art-guide.md) how things should look · [development.md](docs/development.md) setup and commands · `research/` dated notes with sources. |
| `.work/`, `shots/` | Scratch: Tripo downloads and rendered sheets. Ignored by git; safe to delete. |

## Commands

`make` alone checks the machine is set up; `make help` lists the everyday ones (`make setup`, `make run`, `make play`, `make windows` from WSL, `make sheet ID=wolf`, `make check`). Each is one line over these; add a target when a new command becomes everyday.

```bash
cargo forge doctor                 # is this machine set up: node, Tripo login, display, models
cargo sandbox --help               # every way to start the game (--list names the creatures)
cargo forge status                 # every creature: Tripo steps done, credits, model
cargo forge build wolf             # make what is missing, then finish and check (spends credits once)
cargo forge finish --all           # the free local half again, after changing clips.rs or style.toml
cargo forge sheet wolf             # shots/wolf.png: every clip at six moments, one image
cargo sandbox --spawn all          # the interactive sandbox (needs a real screen)
cargo sandbox --play               # the same, controlling the player (make play)
cargo sandbox --scene shots/play.png --play --after 6   # no window: the player on a set run, pictured; prints its position each second
cargo sandbox --scene shots/scene.png --spawn all   # a picture of the world with creatures, offscreen
cargo test --workspace             # must pass before a commit
cargo clippy --workspace -- -D warnings
```

A machine with no screen needs `DISPLAY=:1` (an Xvfb) for `sheet` and `sandbox`; see [docs/development.md](docs/development.md).

## Rules

- **Bevy 0.20, pinned.** Its API changes every release and your memory of it is probably older. Before writing engine code, read the matching example in `~/.cargo/registry/src/*/bevy-0.20.0/examples/` rather than guessing; grep the source for a signature. Same for `bevy_egui` 0.43.
- **Repeatable work is code.** If you did something by hand twice (a Tripo call, a GLB fix, a render), it becomes a forge command or a step of one. Never hand-edit a generated file (`model.glb`, `meta.toml`); change the code or `style.toml` and run `cargo forge finish`.
- **Tools answer in a few lines.** A command prints a verdict and numbers, `ok` or `FAIL name value (bar)`, and names a file for details. Passing checks stay quiet. Keep new tools that way: output is read by an agent on every later turn.
- **Numbers first, then one picture.** `cargo forge check` judges a model before anyone looks. Then look at one contact sheet (`cargo forge sheet`), not a run of screenshots. If you catch a defect by eye that the checks missed, add a check for it.
- **Credits are real money.** `forge.lock` makes a build spend once. Look at the concept (`cargo forge build ID --to concept`, then `cargo forge concepts`) before paying for a model. Never delete a lock step to retry without a reason you can state.
- **Bones have roles, not names.** Tripo's bone names are unreliable; clips are written against roles read from the skeleton's shape (`crates/forge/src/rig.rs`). Do not match on bone names for animals.
- **Runs on Linux, Windows and macOS.** Build paths with `Path::join`, never a typed `/` or a home folder; no shell-outs beyond `node` and `cargo`; anything one system needs sits behind `cfg!` with the reason. `make check` type-checks for Windows. In WSL the game is built as a Windows program and opened there (`make windows`), so it must start from any folder: it finds `assets/` itself.
- **The simulation must not need a window.** Game rules go in crates with no rendering (`cube_voxel`, `cube_sim`), tested headless. `cube_game` only draws and takes input.
- **Secrets:** `.env` (the Tripo login) lives in the main checkout, ignored by git. Never print it or commit it.
- **Shaders** are for the environment only (terrain, water, sky, weather). Creatures use stock materials.
- **Commits:** small, one change each, tests and clippy passing. Models and textures are committed for now (a few hundred KB each); see the open question in engine.md before adding anything over 5 MB.
- **Push every commit to main, without asking.** After a commit: `git fetch origin`, rebase onto `origin/main` if it moved, `git push origin HEAD:main`, then fast-forward the main checkout (`git pull --ff-only` there) so worktrees cut from it are not stale. Fetch and rebase before starting work too. Never force-push main.
- **Docs:** when a design choice is made, write it in `docs/engine.md` or `docs/pipeline.md` under the heading it belongs to, with the reason. Research goes in `docs/research/YYYY-MM-DD-topic.md` with sources. Do not write plans or status files.

## Adding things

- **A creature:** [docs/pipeline.md](docs/pipeline.md). Four commands, about 67 credits.
- **A clip for a body plan:** a recipe in `crates/forge/src/clips.rs`, its name in the plan's `clips` in `style.toml`, then `cargo forge finish --all`.
- **A body plan (bird, spider, snake):** a `[plans.x]` in `style.toml`; the role reader and recipes are plan-agnostic, so check `cargo forge inspect ID` reads the new shape sensibly first.
