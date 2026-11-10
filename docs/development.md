# Development

## Setup

1. **Rust**: `rustup` installs the toolchain pinned in `rust-toolchain.toml` on first build.
2. **Node 22.18 or newer**, for the Tripo CLI (no packages to install).
3. **A GPU with Vulkan** (or Metal, or DX12), and Bevy's Linux libraries (`libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`).
4. **A display.** winit wants one even to draw offscreen. A server with no screen runs `Xvfb :1 -screen 0 1920x1080x24` and sets `DISPLAY=:1`; contact sheets then draw on the real GPU into an image. The sandbox window itself needs a real screen.
5. **`.env`** at the root of the main checkout with `TRIPO_STUDIO_COOKIE=…` ([tools/tripo/README.md](../tools/tripo/README.md)). Worktrees read the main checkout's. `node tools/tripo/tripo.ts balance` checks it.

`cargo forge doctor` checks 2, 4 and 5 and says what to do about each one that is wrong.

The first build compiles Bevy: several minutes. After that a change to the game links in seconds.

## Commands

`make` alone checks the machine is set up, and `make help` lists the everyday commands: all a newcomer needs is `make`, then `make play`. The Makefile is one line a target over the commands below, which take more options.

| Command | What it does |
|---|---|
| `cargo forge …` | The creature pipeline; `cargo forge help` lists it ([pipeline.md](pipeline.md)) |
| `cargo forge doctor` | One line when the machine is set up; a `FAIL` line with the fix for each thing that is not |
| `cargo sandbox` | The sandbox window. `--spawn a,b` starts with creatures in it, `--spawn all` with one of each. `--help` lists every way to start it, `--list` the creatures |
| `cargo sandbox --play --wild` | The window, as the player in the living world: every creature where it lives, by its own mind (`make play`). `--seed N` is another island |
| `cargo world survey` | No window: the land in numbers (water, biomes, trees, cliffs, how much is reached from the start), then `ok` or `FAIL name value (bar)` lines. `WORLD_VERBOSE=1` prints the passing lines too |
| `cargo world run --seconds 300` | No window: the creatures living for that long, with a person wandering at the start. A line a kind (how far each went, the share of time in each clip, how often it fled or was hit), then `ok` or `FAIL` lines. `make world` is survey then this |
| `WORLD_TRACE=deer cargo world run` | The same, following the deer that ends up most stuck: a line a second (place, state, clip, what is asked of its body) and the ground round it whenever it is caught |
| `cargo world map shots/map.png` | No window: the island from above after a minute, a dot where each creature was every half second (`make map`). White is the person, yellow timid, orange wary, red hunters and monsters |
| `cargo world dig --holes 120` | No window: that many holes and stones cut to the 5 cm bit all over the land. One line of what it cost (cells broken, memory, faces, time to draw a chunk again), then `ok` or `FAIL` lines for the creatures living among them |
| `cargo sandbox --scene FILE --spawn player --dig 3,1.5,1.6 --build 3.6,-1.2,1` | No window: a hole and a stone that far east and south of the start and that far across, for looking at broken cells up close |
| `cargo world who` | Where each group of creatures starts, in metres from the start: what to give `--at` |
| `cargo sandbox --scene FILE --wild --at 47,28` | No window: a picture of that place, creatures and all. `--dist`, `--pitch`, `--yaw` move the camera; `--view high` is from the air |
| `cargo sandbox --scene FILE --play --after 6` | No window: a scripted run pictured from the camera at that second, with `play t= pos= speed= grounded= swimming= clip=` printed each second. `--as wolf` runs it as another creature, `--view first` from its eyes, `--yaw 2.16` aims it at the pond instead of the hills |
| `… --script '1 jump; 2 fwd run; 4 stop'` | The route: times in seconds and what changes then. `fwd back left right stop`, `walk run sprint`, `jump hop`, `dive surface`, `dig put`, `first third` |
| `… --strip 10 --every 0.1 --view side` | The run as a strip of pictures from `--after`, one image: how a movement is judged. Side view is level and square on, for reading feet |
| `cargo sandbox --scene shots/wolf.gif --play --as wolf --gif 12 --view three` | No window: a moving picture of the show reel (walk, run, turn and sprint back, jump, stop, attack, flinch, run, jump, walk), so every clip and every change between clips is seen. `make gif ID=wolf`. About 8 MB; made to be watched by a person, and costs an agent nothing to make. `--script` gives another route; `attack`, `eat` and `hurt` are steps too |
| `cargo sandbox --bodies` | Draws the shape each creature is bumped as (also a tick box in the panel) |
| `cargo sandbox --trace` | In the window too: the controlled body printed four times a second |
| `cargo sandbox --shot ID` | A contact sheet to `shots/ID.png`, no window. `--view quarter\|side\|front`, `--cols N`, `--clips a,b` |
| `cargo sandbox --scene FILE --spawn all` | A picture of the test world with creatures in it, no window |
| `cargo test --workspace` | Unit tests: voxel world, meshing and rays, terrain, body movement, creatures' behaviour, skeleton roles and turns |
| `cargo clippy --workspace -- -D warnings` | Lints |
| `node --test tools/tripo/` | The Tripo CLI's own tests, against a fake Studio |

Both `forge` and `sandbox` build in release: Bevy unoptimised is too slow to judge motion by.

## Windows, from WSL

In WSL, `make play` and `make run` build the game as a Windows program and start it there (`make windows ARGS="…"` does the same with any arguments), so it opens as an ordinary Windows window on the real GPU (DirectX 12 or Vulkan). The code, the agent and every other command stay in WSL. `SPAWN=wolf,player` picks the creatures, as with `make run`.

- **Once:** `sudo apt install mingw-w64` for the Windows linker. The Rust half (`x86_64-pc-windows-gnu`) comes with the pinned toolchain. `make setup` says if the linker is missing.
- **Why not run the Linux build through WSLg:** `make run` does open a window that way, but WSL reaches the GPU through a Vulkan-on-DirectX layer, which is slower than the native path and a known source of trouble for wgpu. Try it if you like; `make windows` is the supported way.
- **How it finds its files:** the program looks for `assets/` in the folder it was started from and then above where it sits, so it runs from the repository in WSL with nothing copied to the Windows side.
- The first build compiles Bevy a second time, for Windows. Outside WSL the target only builds the `.exe` (`target/x86_64-pc-windows-gnu/release/cube_game.exe`).

`make cross`, part of `make check`, type-checks every crate for Windows without needing the linker. macOS has no such check yet; it is expected to work and has not been built. The Windows program builds and links here but has not yet been started on a Windows machine.

## The sandbox

A panel on the left: spawn any creature by kind, or one of each; pick one; play a clip, set its speed, hold and scrub a frame; send it wandering or running at its measured speed, or give it its own mind; draw skeletons. Right-drag orbits, the wheel zooms, WASD and QE move the focus, F looks at the picked creature.

**Playing.** Tab takes control of the picked creature (the player, with `--play`, from the start; `--as wolf` for another) and Tab again lets go. Controlled: WASD runs relative to the camera, Shift sprints, Ctrl walks, Space jumps (held longer, higher) and swims up, Ctrl in water dives, the mouse turns the camera, left click digs a hole where it looks and right click builds a lump of stone there (both cut to the 5 cm bit), the wheel moves it in and out, V switches to first person and back. The mouse is locked to the window while controlling, so Tab is also how to reach the panel. Any creature can be controlled, which shows its gaits at real speeds. Creatures cannot be walked through; wandering ones use the same movement and bump the same way.

A name that is not a creature, or no display, stops the sandbox with one line before the engine starts.

Creatures are found by scanning `assets/creatures/*/` for a `creature.toml` beside a `model.glb`, so a creature forge has just built appears on the next start.

## The window with no screen

The real window, not an offscreen picture, runs on a server through Mesa's software Vulkan driver: `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json` and a `DISPLAY` that is an Xvfb. It is slow but it is the same code path a player's machine takes, and it found a startup bug the offscreen paths could not.

`tools/drive/drive.py OUT -- game args…` starts a private Xvfb and the game in it, then runs the steps in `$STEPS`: `hold(['w','Shift_L'], 1.5)`, `tap('space')`, `mouse(200, 0)`, `shot('name')`, split by `;`. It needs `python-xlib` on `PYTHONPATH` (pure Python; unzip its wheel anywhere). The game's `--trace` lines land in `OUT/game.log` between `## step` marks.

## Reading Bevy

Bevy 0.20's source and examples are on disk once built: `~/.cargo/registry/src/*/bevy-0.20.0/examples/`. Names changed in recent releases (`WorldAssetRoot`, `WorldInstanceReady`, `GlobalAmbientLight`, `RenderTarget` as a component, egui's `Panel`), so read the example for what you are about to write. The ones this code was written from: `animation/animated_mesh_control.rs`, `3d/render_to_texture.rs`, `window/screenshot.rs`, `app/headless_renderer.rs`.
