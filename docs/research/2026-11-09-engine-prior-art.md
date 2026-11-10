# Engine prior art: Bevy vs DIY, voxel tech, agent-driven workflow

Researched 2026-11-09. Web sources plus the local Bevy 0.20.0 source in `~/.cargo/registry`.
"Unverified" means I could not confirm it from a primary source in this pass.

## In plain words

1. **Use Bevy 0.20, pinned exactly (`bevy = "=0.20.x"`). Do not build on raw wgpu + winit.** Every maintained Rust voxel crate found targets Bevy; the DIY examples (Veloren, Tiny Glade) had rendering specialists or years of volunteer time.
2. **Upgrade Bevy at most once per release, and only when a feature is needed.** Releases land every 3 to 5 months (0.17 Sep 2025, 0.18 Jan 2026, 0.19 Jun 2026, 0.20 Oct 2026). A Bevy collaborator puts a migration at "20 minutes to a couple hours"; waiting for third-party crates is the slow part.
3. **Split the workspace like Veloren: a `sim` crate with no rendering, a headless `server`, and a `client` that only draws.** The sim should run and be tested with no window and no GPU.
4. **Chunk size: 32x32x32 blocks.** bevy_voxel_world and (per a secondary source) Vintage Story use 32; the fastest mesher uses 62 (64 with padding). 32 fits both.
5. **Meshing a chunk costs about 50 to 350 microseconds on one core**, so meshing is not the bottleneck. In bevy_voxel_world the measured bottlenecks were a lock (74% of main-thread time) and chunk scheduling.
6. **For the Eco look, use shaped block pieces picked by neighbours, not marching cubes.** Eco's mod kit describes block sets plus a "custom builder" that changes a block's mesh next to others (corners). Internals unverified.
7. **Start from `bevy_voxel_world` 0.19 (supports Bevy 0.20, released 2026-11-08) or copy its design.** It has 32-block chunks, LOD levels and a hook for a custom mesher. Do not depend on `block-mesh` (last release April 2022).
8. **Skip LOD at first.** When needed, use chunks that double in size per level; bevy_voxel_world covered an 800-chunk view distance with 64,400 chunks instead of 537,000.
9. **Use stock Bevy for characters and animals**: glTF loading, skinned animation and PBR are built in. Do not write a custom render pipeline early; Bevy 0.20 just changed its shader language (WGSL to WESL), which breaks custom shaders.
10. **Tell the agent the Bevy version and point it at the local source** (`~/.cargo/registry/src/*/bevy-0.20.0/examples`). First-hand reports say agents invent APIs from older Bevy versions.
11. **Give the agent ways to check its own work**: headless sim tests with a fixed tick and seeded randomness, screenshots written to PNG, and a remote-control port (Bevy Remote Protocol) to read state and send input.
12. **No 2025-2026 postmortem of a shipped Bevy voxel game was found.** The evidence here is libraries, repos and maintainers' statements, not a finished comparable game.

## Sources

| Source | Date | What it says |
|---|---|---|
| [Bevy news index](https://bevy.org/news/) | 2026-10-08 | 0.16 Apr 2025, 0.17 Sep 2025, 0.18 Jan 2026, 0.19 Jun 2026, 0.20 Oct 2026 |
| [Bevy 0.20 release notes](https://bevy.org/news/bevy-0-20/) | 2026-10-08 | WESL replaces Bevy's WGSL dialect; mesh shaders; BSN syntax changes; 132x faster GPU mesh extraction via per-column change ticks; remote inspector still future work |
| [Bevy discussion #21838](https://github.com/bevyengine/bevy/discussions/21838) | 2025-11-14 | A user leaves Bevy over version chasing. Collaborator: migrations take "20 minutes to a couple hours", "you don't have to update unless you want to"; another user stays on 0.15 |
| [80.lv Tiny Glade interview](https://80.lv/articles/exclusive-tiny-glade-developers-discuss-bevy-proceduralism-publishers-cozy-games) | 2024-05-30 | Prototype used Bevy for everything; "our needs outscaled what Bevy could provide at the time"; moved rendering to custom Vulkan; "still using a modified version of Bevy" (the ECS layer) |
| [Veloren codebase structure](https://book.veloren.net/contributors/developers/codebase-structure.html) | undated | `common`, `world`, `server`, `client` are headless libraries; `voxygen` (graphical client) and `server-cli` are frontends |
| [Veloren devblog 125, via planet.freegamedev](https://planet.freegamedev.net/category/rpg/page/44) | 2021 | wgpu port called "a long journey of refactoring ... rebasing ... updating the code across wgpu versions" (search snippet, not read in full) |
| [bevy_voxel_world on lib.rs](https://lib.rs/crates/bevy_voxel_world) | 2026-11-08 | 0.19.2; Bevy 0.20 needs ^0.18; 32-block chunks, level L chunk is `32 << L`; custom meshing delegate; LOD seams are the user's job (skirts); AI used for features from 0.19.0 |
| [bevy_voxel_world PR #125](https://github.com/splashdust/bevy_voxel_world/pull/125) | 2026-11-02 | Lock wait was 74% of main-thread time; own binary mesher 77 to 52 microseconds per chunk, 41% fewer quads; LOD 64,400 vs 537,000 chunks; headless benchmark |
| [binary-greedy-meshing on lib.rs](https://lib.rs/crates/binary-greedy-meshing) | 2026-11-05 | 0.6.1; chunks fixed at 62 (64 padded); 65 to 340 microseconds per chunk on a Ryzen 5 5500; experimental LOD about 1 ms; used by Riverbed |
| [block-mesh on lib.rs](https://lib.rs/crates/block-mesh) | 2022-04-17 | Last release 0.2.0; no later activity shown |
| [fast-surface-nets on lib.rs](https://lib.rs/crates/fast-surface-nets) | 2025-01-03 | Last release 0.2.1; about 20 million triangles per second on one core |
| [vx_bevy](https://github.com/Game4all/vx_bevy) | stopped at Bevy 0.14 | "won't receive further updates to newer bevy versions"; about 100 fps on a GTX 1060 at 16 chunks; used block-mesh |
| [Riverbed](https://github.com/Inspirateur/riverbed) | active, date not shown | Bevy voxel game, 2 km render distance claimed; details not in README |
| [isomesh](https://github.com/Ladvien/isomesh) | 2026 | Bevy 0.20 smooth-surface mesher "written by AI agents ... covered by tests"; debug builds mesh 37 to 62 times slower than release; notes Bevy 0.20 extracts only RENDER_WORLD assets |
| [Eco wiki search snippets](https://wiki.play.eco/en/Creating_Custom_Blocks) | undated | Blocks live in BlockSets; a "custom builder" changes how blocks react next to each other, such as corners. Page returned 403; snippet only |
| [Toby Hede, "hard mode"](https://tobyhede.com/blog/hard-mode/) | 2025-11-23 | Claude Code on Bevy 0.17: suggests removed APIs, invented a non-existent Bevy version even after reading docs |
| [Ardan Labs video, via daily.dev](https://daily.dev/posts/can-ai-really-build-games-in-rust-agentic-ai-vs-bevy-engine--txjound5e) | 2026-10-05 | Agents erred more with Bevy than macroquad; pasting compiler errors back fixed it; detailed prompts mattered |
| [BeautifulRust benchmark](https://github.com/morganlinton/BeautifulRust) | 2026-10 | Rust skills raised Claude Code 85% to 94% overall, but Claude did worse on the Bevy feature task with them; one run per setup |
| [OpenSacred PR #193](https://github.com/zarcsis/OpenSacred/pull/193) | 2026-10-20 | Game exposes Bevy Remote Protocol on localhost with custom `exec`, `state`, `input`, `screenshot`, `logs` methods; Claude Code drives it; windowless HTTP tests |
| [bevy-agent-feedback-plugin](https://github.com/drsh4dow/bevy-agent-feedback-plugin) | 2026, Bevy 0.19 | Input, screenshots and frozen time for agents; "protocol readiness only means the socket exists" |
| [godogen HN thread](https://astro-hackernews.netlify.app/stories/47400868) | about 2026-03 | Godot, not Bevy: two-tier API reference, screenshot QA by a vision model, explicit engine version |
| Local Bevy 0.20.0 source | 2026-10 | `examples/app/headless_renderer.rs`, `no_renderer.rs`, `examples/remote/integration_test.rs`; `bevy_dev_tools/src/ci_testing`, `easy_screenshot.rs`; `RenderAssetUsages` docs |

## 1. Bevy vs DIY

What projects chose:

- **Veloren**: own wgpu renderer, `specs` ECS, strict headless-library split. Large volunteer team over many years; not a solo model.
- **Tiny Glade**: Bevy's ECS (a modified fork) with a custom Vulkan renderer, written by a senior rendering engineer because the game needed compute-shader features Bevy lacked then.
- **Bevy voxel projects**: Riverbed, bevy_voxel_world and isomesh are active on current Bevy. vx_bevy stopped at 0.14.
- **Lay of the Land** is Unreal Engine 5 (80.lv), so it is not Rust evidence.
- No Vintage-Story-like Rust project with a postmortem was found.

Where Bevy hurts:

- Breaking release every 3 to 5 months. Third-party crates lag each release.
- Custom shaders and render code break most. 0.20 moved shaders to WESL.
- Mesh data is copied between the main world and the render world. Bevy's docs say to mark unchanging meshes `RENDER_WORLD` only so the CPU copy is dropped. A 2025 issue (#18864) reported sporadic updates when editing such meshes; status in 0.20 unverified.
- No finished editor. Scene format (BSN) still changing in 0.20.

Where Bevy saves time (all present as crates in the local 0.20 source):

- glTF loading, skinned animation, PBR lighting and shadows, UI widgets, input, audio.
- ECS with scheduling, plus a headless mode with no renderer.
- Remote protocol for inspecting and changing a running game over HTTP.
- Screenshot API and a CI testing helper in `bevy_dev_tools`.

Not measured here: the per-frame cost of uploading many chunk meshes in Bevy 0.20. No source gave a number.

## 2. Voxel tech

Chunk size:

- 16 (Minecraft sections), 32 (bevy_voxel_world; Vintage Story per vkguide, secondary), 62+2 (binary greedy meshing, because a padded row must fit one 64-bit integer).
- Bigger chunks mean fewer draw calls but worse culling and bigger remeshes after an edit.
- Veloren's chunk dimensions: unverified.

Meshers and status (November 2026):

| Crate | Status | Note |
|---|---|---|
| `bevy_voxel_world` 0.19.2 | Active, Bevy 0.20 | Chunk streaming, LOD, own binary mesher, custom mesher hook |
| `binary-greedy-meshing` 0.6.1 | Active | Fastest; fixed 62-block chunks; full cubes only |
| `block-mesh` 0.2.0 | Dormant since 2022 | Still works as plain code; no fixes coming |
| `fast-surface-nets` 0.2.1 | Slow-moving | Smooth blobs, not block shapes |
| `transvoxel` 2.0.0 | In local registry; date unverified | Smooth terrain with LOD seams |
| `isomesh` 0.0.13 | Experimental, Bevy 0.20 | Seven smooth-surface methods, agent-written |

Eco-like shaped blocks:

- Evidence (weak, snippet only): Eco's mod kit uses BlockSets and a builder that picks a mesh by neighbours.
- That is the "block shape variants" approach: each block type has a small set of authored pieces (flat, edge, outer corner, inner corner) chosen from a neighbour mask.
- Marching cubes, surface nets and dual contouring make smooth blobs from density values. They do not keep a clean block grid, and they round off built structures.
- My inference, not sourced: greedy merging and beveled edges conflict, since merged faces lose the per-block bevel. Use face culling for hidden faces and emit shaped pieces only on exposed edges. Expect more triangles than Minecraft-style meshing.

LOD:

- bevy_voxel_world: chunk key is position plus level; level L covers `32 << L` blocks and samples every `1 << L`. Seams hidden with skirts.
- binary-greedy-meshing has an experimental LOD mesh at about 1 ms per chunk.

Keeping simulation separate:

- Veloren's layout is the reference: shared `common`, world generation, server and client as libraries with no display code; the renderer is one frontend.
- bevy_voxel_world's 2026 refactor moved chunk state changes and streaming decisions into pure functions tested without an app.
- Tick rate, networking crate and determinism strategy were not researched in this pass.

## 3. LLM agents building Rust games

What first-hand reports say goes wrong:

- Stale API knowledge. Agents write code for removed Bevy APIs and can invent version numbers (Hede, 2025; Ardan Labs, 2026).
- Compile errors are cheap to fix by feeding them back. Runtime errors are not caught by the compiler, for example conflicting queries that panic on the first frame (from a Bevy skill write-up, secondary).
- Generic Rust skill packs helped overall but made Claude worse on a Bevy feature task in one benchmark (one run, weak evidence).

Practices with evidence:

- **Pin and name the version.** Stated in every report. Take examples from the matching version's source, not the web.
- **Local source as the reference.** The godogen author used a two-tier API reference: common classes always loaded, full docs on demand.
- **Remote control port.** OpenSacred adds custom methods on Bevy Remote Protocol for state, input, screenshots and logs, off unless a flag is passed.
- **Semantic readiness, not sleeps.** Wait for a named game state before capturing; a socket being open proves nothing.
- **Frozen or stepped time** for repeatable captures. It only controls Bevy's clock, not `Instant::now()` or unseeded randomness.
- **Headless benchmarks and pure-function tests** (bevy_voxel_world PR #125, isomesh).
- **Release-mode checks for meshing.** isomesh measured debug builds 37 to 62 times slower.
- Windowed screenshots need a display; CI needs `xvfb-run` or the headless renderer example.

Not found: a detailed first-hand write-up of an agent-built Bevy game that reached release.
