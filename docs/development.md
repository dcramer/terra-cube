# Development

## Setup

1. **Rust**: `rustup` installs the toolchain pinned in `rust-toolchain.toml` on first build.
2. **Node 22.18 or newer**, for the Tripo CLI (no packages to install).
3. **A GPU with Vulkan** (or Metal, or DX12), and Bevy's Linux libraries (`libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`).
4. **A display.** winit wants one even to draw offscreen. A server with no screen runs `Xvfb :1 -screen 0 1920x1080x24` and sets `DISPLAY=:1`; contact sheets then draw on the real GPU into an image. The sandbox window itself needs a real screen.
5. **`.env`** at the root of the main checkout with `TRIPO_STUDIO_COOKIE=…` ([tools/tripo/README.md](../tools/tripo/README.md)). Worktrees read the main checkout's. `node tools/tripo/tripo.ts balance` checks it.

The first build compiles Bevy: several minutes. After that a change to the game links in seconds.

## Commands

| Command | What it does |
|---|---|
| `cargo forge …` | The creature pipeline; `cargo forge help` lists it ([pipeline.md](pipeline.md)) |
| `cargo sandbox` | The sandbox window. `--spawn a,b` starts with creatures in it |
| `cargo sandbox --shot ID` | A contact sheet to `shots/ID.png`, no window. `--view quarter\|side\|front`, `--cols N`, `--clips a,b` |
| `cargo sandbox --scene FILE --spawn all` | A picture of the test world with creatures in it, no window |
| `cargo test --workspace` | Unit tests: voxel world and meshing, skeleton roles and turns |
| `cargo clippy --workspace -- -D warnings` | Lints |
| `node --test tools/tripo/` | The Tripo CLI's own tests, against a fake Studio |

Both `forge` and `sandbox` build in release: Bevy unoptimised is too slow to judge motion by.

## The sandbox

A panel on the left: spawn any creature by kind, or one of each; pick one; play a clip, set its speed, hold and scrub a frame; send it wandering or running at its measured speed; draw skeletons. Right-drag orbits, the wheel zooms, WASD and QE move the focus, F looks at the picked creature.

Creatures are found by scanning `assets/creatures/*/` for a `creature.toml` beside a `model.glb`, so a creature forge has just built appears on the next start.

## Reading Bevy

Bevy 0.20's source and examples are on disk once built: `~/.cargo/registry/src/*/bevy-0.20.0/examples/`. Names changed in recent releases (`WorldAssetRoot`, `WorldInstanceReady`, `GlobalAmbientLight`, `RenderTarget` as a component, egui's `Panel`), so read the example for what you are about to write. The ones this code was written from: `animation/animated_mesh_control.rs`, `3d/render_to_texture.rs`, `window/screenshot.rs`, `app/headless_renderer.rs`.
