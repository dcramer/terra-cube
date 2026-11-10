# Engine design

Decided 2026-11-09, the first night. Each choice gives its reason so it can be argued with. Evidence: [research/2026-11-09-engine-prior-art.md](research/2026-11-09-engine-prior-art.md).

## The choice: Bevy for the frame, our own crates for the world

**Bevy 0.20, pinned, for windowing, rendering, assets, glTF, skinned animation, input, UI and its ECS. Our own code for everything that is the game: voxels, meshing, simulation, creatures' behaviour.**

Why not our own engine on wgpu:

- We are one person and an agent, and not game developers. A renderer, an asset system, skinned animation with blending and a glTF loader are months of work that Bevy has, and none of it is what makes this game different.
- Every maintained Rust voxel library found targets Bevy. The Rust voxel games that wrote their own renderer (Veloren, Tiny Glade) had rendering specialists or years of volunteers.
- The thing we had to prove first, generated models that animate, needed exactly the parts Bevy ships. It took one evening, not a quarter.

What Bevy costs, and how we contain it:

- **API churn**: a release every three to five months, each breaking. We pin the version, upgrade deliberately at most once a release, and agents read the local examples instead of trusting memory ([AGENTS.md](../AGENTS.md), Rules).
- **Lock-in**: kept shallow. Voxel data and meshing (`cube_voxel`) use no Bevy types, and the simulation will not either. If Bevy's renderer ever becomes the limit for terrain, we replace how chunks are drawn (a custom material or render pass) and keep the rest. Tiny Glade did exactly that: Bevy's ECS and app, its own renderer.
- **Custom shaders** break on upgrades (0.20 moved WGSL to WESL). So shaders are for environment effects only, written late, kept few.

## Crates

| Crate | Depends on | Holds |
|---|---|---|
| `cube_voxel` | glam | Blocks, chunks (32³), the world map, test terrain, meshing to plain vertex arrays |
| `cube_game` | bevy, cube_voxel | Drawing and input. Today the sandbox |
| `forge` | glam, serde, image | The creature pipeline; never linked into the game |
| `cube_sim` | cube_voxel | The simulation at a fixed 60 ticks a second, runnable and testable with no window. Today only `body`, how a creature moves through voxels. To come: world generation, block changes, creatures' needs and behaviour, seeded randomness |

The rule this protects: **the simulation never needs a window.** A big sandbox with heavy simulation will be server-authoritative sooner or later, and a simulation that runs headless is also one an agent can test in milliseconds.

## Voxels

- **A cell is 0.5 m**, a chunk 32³ cells (16 m). Half-metre cells are the cheapest step up from Minecraft's 1 m: slopes and silhouettes read finer and a 1.75 m player is 3.5 cells tall. Costs 8× the cells per cubic metre; storage is the thing to watch (palette-compress chunks when it matters).
- **Meshing today**: one quad per visible face, ambient occlusion baked into vertex colour, slight per-cell tint. No textures. About the simplest thing that does not look like one flat tile.
- **The Eco look is shaped blocks, not smooth isosurfaces**: blocks keep the grid (so building and simulation stay simple) but draw as mesh pieces chosen by their neighbours (bevelled edges, slopes, rounded corners). That is the main lever for "much better than Minecraft" and the next terrain job. Marching cubes or surface nets would look smoother but lose the block grid that building and simulation want.
- **Not yet**: greedy meshing (measured elsewhere at 50 to 350 µs a chunk for plain meshing, so not urgent), meshing off the main thread, level of detail, chunk streaming, saving.

## Creatures

Full detail in [pipeline.md](pipeline.md). The design choices:

- **Creatures are stylised low-poly skinned meshes, not voxel models.** Eco does the same. The world is blocks; what lives in it is not.
- **Tripo makes mesh, texture, skeleton and skin weights. We make most of the motion.** Tripo has a dozen preset clips for bipeds and exactly one (a walk) for anything else, so animals' other clips are made in code against bone *roles* read from the skeleton's shape. One recipe serves every species; Spore did the same.
- **Clips are baked into the model file by forge, not computed in the game.** A baked clip can be checked by numbers and seen on a sheet before it ships, and the game just plays glTF. The same Rust recipes can move into the game later for what must react at runtime (feet on slopes, looking at things).
- **One skeleton for every biped** (Tripo's Mixamo-named rig, 65 joints). The player and humanoid monsters share bone names, so clips and gear are shared.
- **Player customisation, in order of cost** (none built yet): tint and texture swaps on the one base body; rigid gear (helmet, pauldrons, weapons) parented to a single bone, which needs no skin weights and so suits generated meshes; body-part swaps on the shared skeleton only if those are not enough. Generated *skinned* clothing is avoided: fitting it to a body is the hard problem terra spent weeks on.

## Moving through the world

Built 2026-11-10. The rules are `cube_sim::body`; the game only feeds it keys and draws the result.

- **Our own box-against-voxels movement, not a physics engine.** A body is an upright box moved one axis at a time and put back against the cell face it crossed. A block world needs nothing more, it is about 150 lines, it runs headless, and it will behave the same on a server. A physics engine (Avian, Rapier) would bring its own world to keep in step with ours and is not deterministic across machines by default.
- **A one-cell step is walked up, and down, without leaving the ground.** With half-metre cells every hill is a staircase; needing a jump for each would be unplayable. The model eases after the body over a step so the half-metre does not pop.
- **Fixed tick, drawn in between.** Bodies step 60 times a second whatever the frame rate; the game draws them between the last two ticks.
- **Third person, camera-relative.** The models are the point of the game, so the player sees theirs. WASD is relative to the camera, the character turns to face where it goes, and ground between camera and character pulls the camera in (a ray through the voxels).
- **Every moving creature has the same body.** The keys drive one; a wander drives the rest; the player is not special. So an animal steps, falls and swims by the same rules, and any creature can be controlled (`--as wolf`).
- **Creatures do not pass through each other.** Each tick, before bodies step, overlapping ones are pushed apart over the ground (`body::separate`): two movers share the push, one standing still gives none. A creature is met as a capsule sized from its measured standing width and length (`meta.toml`): round for an upright figure, a fattened line along the body for an animal, so a wolf is bumped at its nose and not only its middle. Rounded, so they slide past instead of locking corners. `--bodies`, or the panel's tick box, draws the shapes. Against the voxel world a body is still a square box of the capsule's radius, so a long animal's head can poke into a wall.
- **The gait is asked for, and moves at its clip's speed.** Walk, run or sprint is the driver's choice (Ctrl, nothing, Shift) and the body moves at the speed forge built or measured that clip for, so feet hold: 1.2, 4.0 and 5.6 m/s for the player. While speeding up or slowing the clip's rate follows the body's speed.
- **In the air and in water the body picks the clip:** `jump` on the way up from a jump, `fall` otherwise, `land` after a drop of more than about a cell if it is not running on, `swim` when the water is over its middle. All are made by forge ([pipeline.md](pipeline.md)).
- **Feel, in numbers** (`Tuning`): a run is reached in under a fifth of a second (18 m/s²) and stopped in an eighth (26); gravity 22 m/s²; a held jump clears two cells (1.08 m) and a tapped one is a hop of a third of that; a jump pressed up to 0.12 s before landing still happens, and up to 0.1 s after walking off an edge; falls are capped at 25 m/s so a tick never crosses a cell. The model leans a few degrees into turns and into speeding up.
- **Water:** a body floats with the surface at its middle, swims up with the jump key and down with Ctrl. Water is drawn as its own see-through mesh.
- **First person is the same camera at the eyes** with the model hidden and turned to face where the camera looks (V). Third person stays the default.
- **Feet are not placed on steps.** A body eases over a half-metre step in about a fifth of a second and its feet do not seek the riser. Other block games do the same; leg IK against voxels in the game is real work for little gain while every hill is a staircase. Shaped blocks (slopes) change that sum, so it is decided again then.
- **Not yet:** arms or body visible in first person, a camera and tint for under water, turning on the spot (the model only turns as it moves, or with the camera in first person).

## The sandbox and agent tooling

- `cargo sandbox` is the debug level: spawn any creature, play any clip, scrub, wander, show skeletons.
- `cargo sandbox --shot ID` draws offscreen to a PNG with no window, so an agent on a server can look. A sheet is one image: rows are clips, columns are moments.
- `--scene FILE --play --script '…' --strip 10` runs a scripted route with no window and saves it as a strip of pictures: how movement is judged without a screen. `--trace` prints the body.
- The real window runs on a machine with no GPU screen through the software Vulkan driver, and `tools/drive/drive.py` presses keys and moves the mouse in it. That is how the keyboard, mouse look and cursor lock were tested ([development.md](development.md)).
- Wanted next: a remote port (Bevy Remote Protocol) so an agent can drive a running sandbox, and fixed-seed simulation tests.

## Order of work

1. ~~Creature pipeline and sandbox~~ (this night).
2. ~~Player control in the sandbox~~ (2026-11-10; see Moving through the world).
3. `cube_sim`, the rest: seeded randomness; move terrain generation into it; block edits (the ray is in `cube_voxel`), then placing and digging from the player.
4. Shaped blocks (the Eco look), then textures or material colour by block.
5. More body plans (bird, spider, serpent) and their clips; a real gallop.
6. Player gear: one rigid helmet and one weapon through the same forge steps.
7. Chunk streaming, meshing off-thread, saving.

## Open questions

- **Binary assets in git.** Models are 0.5 to 1 MB each today. At some dozens of creatures this wants git-lfs or an asset store rebuilt from `forge.lock` (the Tripo projects are the source).
- **Biped style drift.** T-pose concept images come out smoother and less faceted than the animals. See [art-guide.md](art-guide.md).
- **Where the developer plays.** This machine has no screen. The window runs here on a software renderer and is driven by script; it has not been run on Windows through `make windows`.
