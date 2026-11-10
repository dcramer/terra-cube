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

`make` alone checks the machine is set up, and `make help` lists the everyday commands: all a newcomer needs is `make`, then `make run`. The Makefile is one line a target over the commands below, which take more options.

| Command | What it does |
|---|---|
| `cargo forge …` | The creature pipeline; `cargo forge help` lists it ([pipeline.md](pipeline.md)) |
| `cargo forge doctor` | One line when the machine is set up; a `FAIL` line with the fix for each thing that is not |
| `cargo sandbox` | The sandbox window. `--spawn a,b` starts with creatures in it, `--spawn all` with one of each. `--help` lists every way to start it, `--list` the creatures |
| `cargo sandbox --play` | The window, controlling the player (`make play`) |
| `cargo sandbox --scene FILE --play --after 6` | No window: a set route (walk, run, jump, sprint, jump) pictured from the camera at that second, with `play t= pos= speed= grounded= swimming= clip=` printed each second. `--as wolf` runs it as another creature, `--view first` from its eyes, `--yaw 2.16` into the pond instead of toward the hills |
| `cargo sandbox --shot ID` | A contact sheet to `shots/ID.png`, no window. `--view quarter\|side\|front`, `--cols N`, `--clips a,b` |
| `cargo sandbox --scene FILE --spawn all` | A picture of the test world with creatures in it, no window |
| `cargo test --workspace` | Unit tests: voxel world, meshing and rays, body movement, skeleton roles and turns |
| `cargo clippy --workspace -- -D warnings` | Lints |
| `node --test tools/tripo/` | The Tripo CLI's own tests, against a fake Studio |

Both `forge` and `sandbox` build in release: Bevy unoptimised is too slow to judge motion by.

## Windows, from WSL

`make windows` builds the game as a Windows program inside WSL and starts it as the player (`ARGS="--spawn all"` for the plain sandbox), so it opens as an ordinary Windows window on the real GPU (DirectX 12 or Vulkan). The code, the agent and every other command stay in WSL. `SPAWN=wolf,player` picks the creatures, as with `make run`.

- **Once:** `sudo apt install mingw-w64` for the Windows linker. The Rust half (`x86_64-pc-windows-gnu`) comes with the pinned toolchain. `make setup` says if the linker is missing.
- **Why not run the Linux build through WSLg:** `make run` does open a window that way, but WSL reaches the GPU through a Vulkan-on-DirectX layer, which is slower than the native path and a known source of trouble for wgpu. Try it if you like; `make windows` is the supported way.
- **How it finds its files:** the program looks for `assets/` in the folder it was started from and then above where it sits, so it runs from the repository in WSL with nothing copied to the Windows side.
- The first build compiles Bevy a second time, for Windows. Outside WSL the target only builds the `.exe` (`target/x86_64-pc-windows-gnu/release/cube_game.exe`).

`make cross`, part of `make check`, type-checks every crate for Windows without needing the linker. macOS has no such check yet; it is expected to work and has not been built. The Windows program builds and links here but has not yet been started on a Windows machine.

## The sandbox

A panel on the left: spawn any creature by kind, or one of each; pick one; play a clip, set its speed, hold and scrub a frame; send it wandering or running at its measured speed; draw skeletons. Right-drag orbits, the wheel zooms, WASD and QE move the focus, F looks at the picked creature.

**Playing.** Tab takes control of the picked creature (the player, with `--play`, from the start; `--as wolf` for another) and Tab again lets go. Controlled: WASD runs relative to the camera, Shift sprints, Ctrl walks, Space jumps and swims up, the mouse turns the camera, the wheel moves it in and out, V switches to first person and back. The mouse is locked to the window while controlling, so Tab is also how to reach the panel. Any creature can be controlled, which shows its gaits at real speeds. Creatures cannot be walked through; wandering ones use the same movement and bump the same way.

A name that is not a creature, or no display, stops the sandbox with one line before the engine starts.

Creatures are found by scanning `assets/creatures/*/` for a `creature.toml` beside a `model.glb`, so a creature forge has just built appears on the next start.

## Reading Bevy

Bevy 0.20's source and examples are on disk once built: `~/.cargo/registry/src/*/bevy-0.20.0/examples/`. Names changed in recent releases (`WorldAssetRoot`, `WorldInstanceReady`, `GlobalAmbientLight`, `RenderTarget` as a component, egui's `Panel`), so read the example for what you are about to write. The ones this code was written from: `animation/animated_mesh_control.rs`, `3d/render_to_texture.rs`, `window/screenshot.rs`, `app/headless_renderer.rs`.
