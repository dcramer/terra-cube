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
- **Any cell can break into 5 cm bits** (`SPLIT` 5, so 125 to a cell), and only broken cells cost more. Tried 2026-11-10 to answer whether a fine grid could be the rule without the whole world being stored that fine: it can. A cell is whole until something smaller than a cell happens to it (a hole dug in it, something built on it), then it holds its own 125 bits; left all one thing again, it mends. Bodies collide with bits, rays stop at them, and a hole is a rounded bowl, not a missing cube. So 5 cm is the smallest thing in the world and the unit things are placed on, while hills are stored a hundred and twenty-five times cheaper. Measured (`cargo world dig`): a hole 0.3 to 1.6 m across breaks about 35 cells and costs 5 kB; 120 of them add 7% to the faces drawn; a changed chunk is drawn again in under 2 ms; creatures live among them without ending up in the ground. The cost that grows is faces: a broken cell's flat side is 25 little squares where a whole cell's is one, so 5,000 holes on this island triple what is drawn. Merging flat faces (greedy meshing) is the answer and is not built.
- **Meshing today**: one quad per visible face, ambient occlusion baked into vertex colour, slight per-cell tint. No textures. About the simplest thing that does not look like one flat tile.
- **The Eco look is shaped blocks, not smooth isosurfaces**: blocks keep the grid (so building and simulation stay simple) but draw as mesh pieces chosen by their neighbours (bevelled edges, slopes, rounded corners). That is the main lever for "much better than Minecraft" and the next terrain job. Marching cubes or surface nets would look smoother but lose the block grid that building and simulation want.
- **Deep water is harder to see through**: a water face's alpha grows with the water under it, so a ford shows its bed and the sea does not. The sea beyond the island is four plain sheets out past the fog.
- **Not yet**: greedy meshing (measured elsewhere at 50 to 350 µs a chunk for plain meshing, so not urgent), meshing off the main thread, level of detail, chunk streaming, saving.

### The ground's blocks

- **The land is drawn in blocks of the size its slope calls for** (`mesh::Eased::step`, 2026-11-11; `mesh::Ground::Stepped`, the default). Nearly level ground is cut in steps of a bit (5 cm), so a meadow rolls; a slope is whole cells; steep ground is blocks half a metre a side and a rock face blocks of a metre. The land gets blockier as it gets wilder, and no two hillsides are the same staircase, which is what every slope was when each was drawn as its 0.25 m cubes. It is voxels throughout: only the size varies.
- **The cells are untouched.** Digging, building and what a body stands on are the same 0.25 m cells and bits; this is how the top cell of each column of land (`Block::ground`: soil, sand, snow, stone) is drawn. Its height is the mean height of the ground in the sixteen columns round it, rounded to the block's size, with a riser down to each neighbour that stands lower. Ground cover, the bits of a broken cell and a creature's feet (`mesh::eased_under`) are drawn at that height.
- **Stone that is not land is drawn as its cubes**: a column whose top would be drawn more than a few cells from where it is (a block standing alone, something built) falls back to plain cubes.
- **A low riser is lit and coloured as the ground it is a fold of.** Lit as a wall it took the warm light of the horizon while the ground beside it took the blue of the sky overhead, and a forest floor was a grid of orange lines.
- **Tried and set aside: the ground eased fully smooth** (`--smooth`). It was built first, from a misreading of what was asked. Smooth land under cube trees was two games in one picture. `--cubes` draws the old plain cubes.
- **Not done**: the far levels of detail still draw plain cubes of their own size, so the ground changes character at 40 m; a cube-drawn neighbour of a stepped column can leave a gap.

### Light

- **The sky is air scattering the sun** (Bevy's `Atmosphere`, 2026-11-11), so the colour of the sunlight, of the sky and of the light that fills shadows all come from where the sun is, and will follow it when there is a time of day. Before, a white sun and a flat blue fill lit everything evenly and nothing had depth.
- **The sun stands at 35 degrees**: everything throws a shadow longer than itself.
- **On every camera that looks at the world** (`world::look`): exposure for full sun, a filmic roll-off (TonyMcMapface), a little more saturation and contrast, bloom, screen-space ambient occlusion for the depth between one thing and another, and haze that warms toward the sun.
- **Tried and backed out the same hour**: lit air (volumetric fog) over the whole island turned the sky purple and the grass yellow; a stronger grade made shade black. Light shafts want a volume placed where they are wanted (a wood, a morning), not one over everything.
- **Not done**: a time of day, weather, wind, anything in the air.

### Paint

- **One atlas of grains, made in code at start** (`cube_voxel::texture`, 2026-11-11): fifteen tiles of 64 texels with a border of 8 that repeats the tile's far side, and three smaller copies each half the last. The border is what lets the picture be drawn smooth and shrunk with distance without a tile taking colour from its neighbour; a tile and its border halve cleanly three times. No art files, no engine types: the game wraps the bytes in an image.
- **The mesher gives every corner a place in it** from where the corner is in the world, not in its cube: tops lie flat, sides stand upright, and the tile repeats every eight cells, so bits, whole cells and the far levels of detail all show the same picture at the same place.
- **A grain holds brightness only and every tile averages 0.8**; the mesher divides the block's colour by that. So the far levels need no other colours, and grain can be changed without touching a block.
- **Still one material and one draw a chunk.** Not done: grain that differs by biome or wetness, normal or roughness maps (the art guide says base colour only), and merging faces (the per-cube shade is small now but still in the vertices).
- **Every scene shot says what it cost** (`pace`): the middle and worst frame of the last sixty, the terrain faces in view, and what drew them. Frame times compare only on the same machine, and a scene may be held to the screen's 60 a second; faces in view compare anywhere.

## The world

Built 2026-11-10. `cube_sim::terrain` makes it; `cargo world survey` and `map` judge it.

- **An island from a seed**, 384 m across (`SEED`, `RADIUS`). The same seed is always the same world, in the game, in tests and in `cargo world`. An island because a finite world needs an edge, and sea to the horizon is an edge nobody questions. It is a stand-in: the world is to be procedural and far larger, a small version of a real one (see Where the world goes).
- **Every size comes from one table, in metres** (`cube_sim::scale`), measured against a person of 1.75 m: the landforms (how far across, how tall, how rough), how tall each kind of tree stands, how much room it has, the classes of rock, the heights where rock and snow begin. `terrain` and `plants` carry no sizes of their own, and `cargo world survey` holds what was made to bars in the same file (the middle tree at least 8 people tall, the largest rock ten times the smallest, 65% of lowland meadow and forest easy ground). The reason: the world was scaled up three times by hand, a number at a time, and each time something was left behind and the person looked like a giant on a model. Scale is a ratio between things, so it is kept in one place and measured.
- **The height is those landforms added up** (`terrain::column`, a pure function of a point and the seed): a lowland; smooth hills over it (two layers of noise, since every layer added makes the ground as steep again, and lumps every few metres are what made it a model); one mountain and one lesser height, placed by the seed well out from the start and each a bell, so the outer third is foothills and the middle is climbing; ridges and ledges on the mountain only, the ledges wandering and missing from whole faces so they are not rings; a river in a broad valley; the shore. The first mountains were wherever a noise ran high: on this island that was most of the rim, as flat-topped walls standing straight up from the plain (seen with `cargo world grade`, which was written to find out why only a third of the lowland was easy ground).
- **Biomes come from height, slope and a moisture noise**: shore, meadow, forest, rock (high or steep), snow. A biome decides the surface block, what is planted (broadleaf trees, pines higher up, bushes, boulders) and who lives there.
- **`Land` is kept beside the voxels**: every column's height and biome, and the trees. Placing creatures and measuring the world read it, not the cells.
- **Things are their real size, drawn in cubes.** An oak stands 13 to 24 m, a birch 11 to 19, a pine 17 to 34, a willow 9 to 14, six and a half metres apart in a forest; every other measure of a tree (trunk, limbs, crown) is a part of its height. The first trees were 5 to 8 m and read as saplings; the second were 9 to 18 m and still made the person a giant.
- **A tree is a tree, not a pole with a ball on it** (`cube_sim::plants`). A trunk flares into roots at the ground and tapers upward with a lean; limbs leave it, bend and fork; foliage is separate clumps at the ends of the limbs, so sky shows through and no two trees are alike. Five kinds: oak (broad, heavy-limbed), birch (slender, pale bark, light and high), pine (whorls of drooping boughs under a point, dead stubs below the green), willow (short and thick by water, hanging in curtains), and the dead snag. Which grows where follows the land: conifers high and on rock, willows within a few metres of water, the rest mixed. The first trees were a square column and one blob, and from the ground a forest was a field of sticks.
- **What is made by the generator is shaped to the bit where a cell is too coarse.** A rock is stone with seven to ten flat faces struck off it, cut at 5 cm. Rocks come in three classes (stones underfoot 0.3 to 0.8 m, boulders 1 to 2.6 m, the rare great one 3.2 to 6 m that is a landmark) and four forms (a block, a slab lying flat, a tooth standing up, a hump mostly buried), each sat on the ground under it with smaller pieces against it and stones fallen round a large one. Built from whole cells a boulder was a pile of crates; as rounded lumps of one size they were uniform orbs. Terrain, trunks and foliage stay whole cells: a face of a broken cell is 25 squares where a whole one is one.
- **A wood has an understory.** Between the trees, on a finer grid of their own, stand young ones three to seven metres tall with their crowns at eye level, bushes from knee to head high, fallen logs and stumps. Without them everything green was overhead.
- **Ground cover is drawn as blades, not cubes, and is walked through.** Long grass, flowers and reeds are cells that are not solid: the mesher draws each as a few stems a bit (5 cm) across, a flower with a head in one of three colours. They grow in patches (a slow noise says where), not sprinkled evenly.
- **Trees and bushes are solid blocks.** A crown starts 4.5 m up so the golem walks under it. Creatures will not climb onto leaves; the player can.
- **Judged by numbers first** (`cargo world survey`): water 20% to 60%, each of meadow, forest and rock at least 6%, 400 trees, ground at least 40 m high, cliffs on 3% to 35% of the land, half the land reached from the start on foot and nine tenths by swimming, at most 2 million faces drawn from the start, and the scale bars above. `cargo world grade` says where the ground is easy, steep and cliff by height, and draws it. A seed that fails one is not a world to ship; seed 6 passes and is the default.
- **Not yet:** caves and overhangs (the land is a height map), shaped blocks, textures.
- **Where the world goes** (decided 2026-11-10): procedural and much larger, a small version of a real world, not one island held whole in memory. What that needs, in order: `Land` answering for any column from the seed instead of holding an array of all of them; cells made a chunk at a time near the player and dropped behind, with what was changed kept; trees and rocks placed by a rule that gives the same answer whichever chunk asks first (a tree's crown crosses chunks); where mountains, plains, forests and seas are coming from slow maps (height of the land, warmth, wet) instead of one placed mountain; creatures placed and put away as the player moves; and beyond the last chunks one coarse mesh from the height map. The scale table is what such a world is generated from, and `column` is already a pure function of a point.

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

## Seeing far

Started 2026-11-10, when the island was drawn whole at full detail and hazed out from 80 m.

- **Three levels of detail a chunk, chosen by distance** (`world::LEVELS`): every cell to 40 m; cells taken two a side to 104 m (a quarter of the faces); four a side beyond (a sixteenth). `mesh_chunk_far` makes the coarse ones: a group of cells is solid if any is, and is the block highest in it, so distant ground has no holes and trees keep their bulk; ground cover and the bits of broken cells are left out.
- **Bevy's `VisibilityRange` does the switching**, with a fade of 8 m between levels so nothing pops. Every level of a chunk is drawn from an entity at the chunk's middle, with its vertices given from there: the fade is computed from the entity's origin, and with all chunks at the world's origin every one faded as if it stood there (the first try: half the island ghosted, the rest missing). `use_aabb` is not for crossfades.
- **Haze from 180 m to 700 m**, so the island is clear from end to end and the sea has no edge.
- **A chunk's level is meshed when the camera first comes near enough to need it** (`world::stream`), nearest first, in a slice of each frame (4 ms; 50 ms for the first three seconds and for scenes), and dropped when far behind. The island doubled in width on that. Still to do, in order:
  1. Meshing off the main thread.
  2. Merge flat faces of one colour (greedy meshing). The per-cell tint has to move out of the vertices first (a small noise texture or a shader), or nothing merges.
  3. Make and keep chunks only near the player, the rest as the seed and what has been changed: then the world is as big as wanted.
  4. Beyond the last chunks, one coarse mesh drawn from `Land`'s height map and biome colours, to the horizon: mountains seen from kilometres off cost almost nothing.
- **Judged by** `cargo world survey`: the faces drawn standing at the start, each chunk at the level its distance gives it (bar: 2 million).

## Creatures

Full detail in [pipeline.md](pipeline.md). The design choices:

- **Creatures are stylised low-poly skinned meshes, not voxel models.** Eco does the same. The world is blocks; what lives in it is not. Tried the other way on 2026-11-10 and decided against: creatures rebuilt from the world's 5 cm cubes (`cargo forge cubes`) can look right standing still when the model is drawn for it, but took a dozen rounds to get one figure there, come apart at the joints in motion, lose small animals altogether, and would mean buying every creature again. A creature should belong to the world by its style (chunky shapes, flat colour, a shared palette), not by being made of the same cubes. The trial, its checks and what it showed are kept in [pipeline.md](pipeline.md#cubes). Three other looks were then drawn and two built beside the house style (carved from blocks, flat-shaded, hand-painted; [pipeline.md](pipeline.md#trying-a-look)), and the house style was kept: faceted, bulky, flat colour.
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
- **Feel, in numbers** (`Tuning`): a run is reached in under a fifth of a second (18 m/s²) and stopped in an eighth (26); gravity 22 m/s²; a held jump clears a metre and a half and a tapped one is a hop of a third of that; a body turns to face the way it is going at ten radians a second, not in one tick; a jump pressed up to 0.12 s before landing still happens, and up to 0.1 s after walking off an edge; falls are capped at 25 m/s, and a body that ends a tick inside the floor is put back on top of it. The model leans a few degrees into turns and into speeding up.
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
- `--gif 12` saves the same run as a moving picture. A strip is for the agent, which reads numbers and one still; a GIF is for the person, who can see in a second whether a change of clip pops or a gait feels wrong, and the agent does not have to look at it.
- The real window runs on a machine with no GPU screen through the software Vulkan driver, and `tools/drive/drive.py` presses keys and moves the mouse in it. That is how the keyboard, mouse look and cursor lock were tested ([development.md](development.md)).
- Wanted next: a remote port (Bevy Remote Protocol) so an agent can drive a running sandbox.

## Order of work

1. ~~Creature pipeline and sandbox~~ (this night).
2. ~~Player control in the sandbox~~ (2026-11-10; see Moving through the world).
3. ~~A world worth walking in~~ (2026-11-10): terrain from a seed, creatures living in it, `cargo world` to judge both.
4. Digging and building, the rest: a first cut works (left and right click cut or raise a ball to the bit, the chunk is drawn again that frame). Still to do: tools and what they cut, what is dug going somewhere, bits of anything but stone, water flowing into holes, merging the faces of broken cells.
5. Combat: health, the player's attack, death. With it, **hit shapes**: a box for the head, the trunk and each limb, fitted by forge to each bone's share of the skin, written to `meta.toml` and drawn by `--bodies`. The one capsule stays for walking and bumping; these answer what a blow or an arrow struck.
6. Shaped blocks (the Eco look), then textures or material colour by block.
7. Needs for creatures (hunger, sleep, a day and night), so behaviour has reasons.
8. More body plans (bird, spider, serpent) and their clips; a real gallop.
9. Player gear: one rigid helmet and one weapon through the same forge steps.
10. The large procedural world (see Where the world goes): chunks made and kept near the player, meshing off-thread, saving.

## Open questions

- **Binary assets in git.** Models are 0.5 to 1 MB each today. At some dozens of creatures this wants git-lfs or an asset store rebuilt from `forge.lock` (the Tripo projects are the source).
- **Biped style drift.** T-pose concept images come out smoother and less faceted than the animals. See [art-guide.md](art-guide.md).
- **Where the developer plays.** This machine has no screen. The window runs here on a software renderer and is driven by script; it has not been run on Windows through `make windows`.
