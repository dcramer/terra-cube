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
| `cube_voxel` | glam | Blocks, chunks (32³), the world map, meshing to plain vertex arrays |
| `cube_sim` | cube_voxel, serde | The simulation at a fixed 60 ticks a second, runnable and testable with no window: making the world from a seed (`terrain`), measuring it (`survey`), how a body moves through voxels (`body`), the creatures and one tick of all of them (`life`), what each decides (`mind`), seeded randomness (`rng`). To come: block changes, needs, saving |
| `cube_game` | bevy, cube_sim, cube_voxel | Drawing and input: it draws what the simulation holds and feeds the player's keys in |
| `world` | cube_sim, image | `cargo world`: the world and its creatures judged with no window, in numbers and one map |
| `forge` | glam, serde, image | The creature pipeline; never linked into the game |

The rule this protects: **the simulation never needs a window.** A big sandbox with heavy simulation will be server-authoritative sooner or later, and a simulation that runs headless is also one an agent can test in milliseconds.

## Voxels

- **A cell is 0.25 m**, a chunk 32³ cells (8 m): a quarter of Minecraft's 1 m, so a hillside reads as a slope and a tree's crown as round, and a 1.75 m player is seven cells tall. Chosen 2026-11-10 from a side-by-side picture against 0.5 m, before block editing exists, when it was cheapest to change. It costs 64 cells a cubic metre against Minecraft's one: the island is 1,500 chunks and 2.1 million faces, about 900 MB at its peak while starting. So storage and meshing are what to watch (palette-compress chunks, mesh greedily), and digging and building will want to work in pieces bigger than a cell. Everything that means a distance (terrain shapes, tree sizes, how high a body steps) is written in metres and turned to cells with `CELL`, so it can change again.
- **Meshing today**: one quad per visible face, ambient occlusion baked into vertex colour, slight per-cell tint. No textures. About the simplest thing that does not look like one flat tile.
- **The Eco look is shaped blocks, not smooth isosurfaces**: blocks keep the grid (so building and simulation stay simple) but draw as mesh pieces chosen by their neighbours (bevelled edges, slopes, rounded corners). That is the main lever for "much better than Minecraft" and the next terrain job. Marching cubes or surface nets would look smoother but lose the block grid that building and simulation want.
- **Deep water is harder to see through**: a water face's alpha grows with the water under it, so a ford shows its bed and the sea does not. The sea beyond the island is four plain sheets out past the fog.
- **Not yet**: greedy meshing (measured elsewhere at 50 to 350 µs a chunk for plain meshing, so not urgent), meshing off the main thread, level of detail, chunk streaming, saving.

## The world

Built 2026-11-10. `cube_sim::terrain` makes it; `cargo world survey` and `map` judge it.

- **An island from a seed**, 192 m across (`SEED`, `RADIUS`). The same seed is always the same world, in the game, in tests and in `cargo world`. An island because a finite world needs an edge, and sea to the horizon is an edge nobody questions.
- **Height is layered noise with four shapes cut into it**: rolling lowland; a mountain mask that raises ridges and steps them into ledges (cliffs read well in blocks; smooth cones do not); a river where a slow noise crosses its middle, deep in places and a ford in others; and the shore falling into the sea. The start is a flat clearing and mountains are kept 25 m off it: they are what is seen from the start, not where it is.
- **Biomes come from height, slope and a moisture noise**: shore, meadow, forest, rock (high or steep), snow. A biome decides the surface block, what is planted (broadleaf trees, pines higher up, bushes, boulders) and who lives there.
- **`Land` is kept beside the voxels**: every column's height and biome, and the trees. Placing creatures and measuring the world read it, not the cells.
- **Trees and bushes are solid blocks.** A crown starts 3 m up so a troll walks under it. Creatures will not climb onto leaves; the player can.
- **Judged by numbers first** (`cargo world survey`): at most 4 million faces to draw, water 20% to 60%, each of meadow, forest and rock at least 6%, 150 trees, ground at least 15 m high, cliffs on 3% to 35% of the land, half the land reached from the start on foot and nine tenths by swimming. A seed that fails one is not a world to ship; seed 6 passes and is the default.
- **Not yet:** caves and overhangs (the land is a height map), shaped blocks, grass and flowers, streaming (the whole island is meshed at the start, about three seconds).

## What lives in it

Built 2026-11-10. `cube_sim::life` and `mind`; `cargo world run` judges it.

- **The simulation owns every creature.** `Sim` holds an agent for each: its body, who drives it (the keys, its own mind, nobody) and what it is doing (`Act`, which is a clip's name). The game keeps a model in step with each agent and never moves one itself. So the same tick runs in the window, in a test and in `cargo world run`, and a server later.
- **A mind is a temper and a handful of states**, not a behaviour tree. Tempers, set in `creature.toml` `[life]`: *timid* (rabbit, deer) grazes and runs from people, hunters and monsters; *hunter* (wolf) runs timid creatures down, then is spent for a while; *wary* (bear, boar) faces whatever comes within 7 m and hits it if it comes closer; *hostile* (goblin, troll) goes for people on sight. A deer bolts from a wolf at 12 m and lets a bear come to 5. Enough for a world that looks alive and for the player to have something to run from; more is added when a need (hunger, sleep) asks for it.
- **No path planning.** A creature heads for a point and looks a step ahead: it turns aside from walls, drops and water it would have to swim, trying ways further round until one is open, and its own box must be able to move that way. At its ease it takes half-metre steps and drops of a metre, so it stays on the ground it knows; fleeing or chasing it jumps a metre and drops three. Good enough on open ground; a creature can still be walled in, and then it waits.
- **Roaming keeps to a home**: a group shares one, placed in the kind's biome. A creature that flees settles where it stops.
- **Nothing dies yet.** A blow is a shove and a flinch (`hurt`). Health, death and what the player hits back with come with combat.
- **A swimmer climbs out over a bank** up to half a metre above its waist. Without it a steep-sided river was a trap, for the player too; the headless run found it.
- **Random choices come from one seeded generator** in the `Sim`, and agents are visited in id order, so a seed replays exactly (tested).
- **Judged by numbers** (`cargo world run`): no creature ends a tick inside the ground or falls out of the world, each kind walks into something for at most 8% of the time, is walled in for at most 10%, swims for at most 5%, and goes at least 3 m.

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
- **A ledge of half a metre is walked up, and down, without leaving the ground**: two cells. Every hill is a staircase of cells; needing a jump for each would be unplayable. The model eases after the body over a step so it does not pop.
- **Fixed tick, drawn in between.** Bodies step 60 times a second whatever the frame rate; the game draws them between the last two ticks.
- **Third person, camera-relative.** The models are the point of the game, so the player sees theirs. WASD is relative to the camera, the character turns to face where it goes, and ground between camera and character pulls the camera in (a ray through the voxels).
- **Every moving creature has the same body.** The keys drive one; a wander drives the rest; the player is not special. So an animal steps, falls and swims by the same rules, and any creature can be controlled (`--as wolf`).
- **Creatures do not pass through each other.** Each tick (`Sim::step`), before bodies step, overlapping ones are pushed apart over the ground (`body::separate`): two movers share the push, one standing still gives none. A creature is met as a capsule sized from its measured standing width and length (`meta.toml`): round for an upright figure, a fattened line along the body for an animal, so a wolf is bumped at its nose and not only its middle. Rounded, so they slide past instead of locking corners. `--bodies`, or the panel's tick box, draws the shapes. Against the voxel world a body is still a square box of the capsule's radius, so a long animal's head can poke into a wall.
- **The gait is asked for, and moves at its clip's speed.** Walk, run or sprint is the driver's choice (Ctrl, nothing, Shift) and the body moves at the speed forge built or measured that clip for, so feet hold: 1.2, 4.0 and 5.6 m/s for the player. While speeding up or slowing the clip's rate follows the body's speed.
- **In the air and in water the body picks the clip:** `jump` on the way up from a jump, `fall` otherwise, `land` after a drop of more than about half a metre if it is not running on, `swim` when the water is over its middle. All are made by forge ([pipeline.md](pipeline.md)).
- **Feel, in numbers** (`Tuning`): a run is reached in under a fifth of a second (18 m/s²) and stopped in an eighth (26); gravity 22 m/s²; a held jump clears a metre (1.08 m) and a tapped one is a hop of a third of that; a jump pressed up to 0.12 s before landing still happens, and up to 0.1 s after walking off an edge; falls are capped at 25 m/s, and a body that ends a tick inside the floor is put back on top of it. The model leans a few degrees into turns and into speeding up.
- **Water:** a body floats with the surface at its middle, swims up with the jump key and down with Ctrl. Water is drawn as its own see-through mesh.
- **First person is the same camera at the eyes** with the model hidden and turned to face where the camera looks (V). Third person stays the default.
- **Feet are not placed on steps.** A body eases over a step in about a fifth of a second and its feet do not seek the riser. Other block games do the same; leg IK against voxels in the game is real work for little gain while every hill is a staircase. Shaped blocks (slopes) change that sum, so it is decided again then.
- **Not yet:** arms or body visible in first person, a camera and tint for under water, turning on the spot (the model only turns as it moves, or with the camera in first person).

## The sandbox and agent tooling

- `cargo world` judges the world and its creatures with no window, in under two seconds: `survey` (the land), `run` (five minutes of life, a line a kind, then `ok` or `FAIL` lines), `map` (one picture from above with where everything went), `who` (where each group lives). `WORLD_TRACE=deer cargo world run` follows the deer that ends up most stuck, a line a second, with the ground round it when it is caught. This is where behaviour and terrain are changed and checked; the window is for how it looks.
- `cargo sandbox` is the debug level: spawn any creature, play any clip, scrub, wander, give it its own mind, show skeletons. `--wild` starts the living world instead.
- `--scene FILE --wild --at X,Z` pictures any place on the island (`--view high` from the air), so a herd or a hillside can be looked at without walking there.
- `cargo sandbox --shot ID` draws offscreen to a PNG with no window, so an agent on a server can look. A sheet is one image: rows are clips, columns are moments.
- `--scene FILE --play --script '…' --strip 10` runs a scripted route with no window and saves it as a strip of pictures: how movement is judged without a screen. `--trace` prints the body.
- The real window runs on a machine with no GPU screen through the software Vulkan driver, and `tools/drive/drive.py` presses keys and moves the mouse in it. That is how the keyboard, mouse look and cursor lock were tested ([development.md](development.md)).
- Wanted next: a remote port (Bevy Remote Protocol) so an agent can drive a running sandbox.

## Order of work

1. ~~Creature pipeline and sandbox~~ (this night).
2. ~~Player control in the sandbox~~ (2026-11-10; see Moving through the world).
3. ~~A world worth walking in~~ (2026-11-10): terrain from a seed, creatures living in it, `cargo world` to judge both.
4. Block edits (the ray is in `cube_voxel`), then placing and digging from the player; remeshing a changed chunk.
5. Combat: health, the player's attack, death. With it, **hit shapes**: a box for the head, the trunk and each limb, fitted by forge to each bone's share of the skin, written to `meta.toml` and drawn by `--bodies`. The one capsule stays for walking and bumping; these answer what a blow or an arrow struck.
6. Shaped blocks (the Eco look), then textures or material colour by block.
7. Needs for creatures (hunger, sleep, a day and night), so behaviour has reasons.
8. More body plans (bird, spider, serpent) and their clips; a real gallop.
9. Player gear: one rigid helmet and one weapon through the same forge steps.
10. Chunk streaming, meshing off-thread, saving.

## Open questions

- **Binary assets in git.** Models are 0.5 to 1 MB each today. At some dozens of creatures this wants git-lfs or an asset store rebuilt from `forge.lock` (the Tripo projects are the source).
- **Biped style drift.** T-pose concept images come out smoother and less faceted than the animals. See [art-guide.md](art-guide.md).
- **Where the developer plays.** This machine has no screen. The window runs here on a software renderer and is driven by script; it has not been run on Windows through `make windows`.
