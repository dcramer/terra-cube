# Art guide

How terra-cube should look, what was decided and why, and what we tell the generator so it comes out that way. Two things enforce it: `assets/creatures/style.toml` (forge adds its style to every creature prompt: change the look there, once, never per creature) and `crates/cube_sim/src/plants.rs` and `terrain.rs` (what the land and what grows on it are made of). How a creature is made is [pipeline.md](pipeline.md); why the engine draws as it does is [engine.md](engine.md). Sources for the first draft: [research/2026-11-09-art-guides.md](research/2026-11-09-art-guides.md).

Status: the developer ruled on the look on 2026-11-10. What follows is decided unless it says otherwise.

## Pillars

1. **The world is cubes; what lives in it is not.** The land is 0.25 m cubes that break into 5 cm bits. Creatures are stylised low-poly meshes that belong to it by style, not by being made of cubes. Never ask the generator for "voxel", "blocky" or "Minecraft".
2. **Real sizes.** A tree is the size of a tree, a moose of a moose, beside a player of 1.75 m. Things are drawn in cubes; they are not shrunk to suit them.
3. **Wild, not cute.** Proportions true to the animal, a head and eyes of natural size, lean and wary. A dangerous thing looks dangerous. Appealing is fine; a mascot is not.
4. **Shape before detail.** Recognisable as a black silhouette. A few large faceted forms, nothing fiddly.
5. **Few colours, strong values.** A few muted natural hues and one accent, with clear light and dark. Painted, never photographic.

## What was tried and dropped

Each of these was built and looked at, not argued. Do not try them again without a new reason.

| Tried | When | Outcome |
|---|---|---|
| Creatures rebuilt from 5 cm cubes (`cargo forge cubes`) | 2026-11-10 | Parked. One figure drawn for it looked right standing still after a dozen rounds; it came apart at the joints in motion, small animals were lost, and every creature would be bought again. [pipeline.md](pipeline.md#cubes) |
| "Carved" look: squared limbs, chamfered edges | 2026-11-10 | Dropped. Skin tore in every clip (101% to 126% stretch against a bar of 80%) and its flat browns were the least liked. |
| "Flat" look: strict flat-shaded triangles | 2026-11-10 | Dropped at the concept: the generator drew flat pictures, not solid animals. |
| "Painted" look: smooth, hand-painted, as Eco | 2026-11-10 | Passed its checks and animated well; the house style was preferred. |
| The player as a castaway with a bandana; then in a tucked-in work shirt | 2026-11-10, 11 | Replaced twice: a pirate, then a man in a dress outfit. |
| Bulky cartoony proportions, oversized head | to 2026-11-10 | Replaced. It was the house style's own wording and it made a shelf of toys. |
| Trees as a square column and one blob | to 2026-11-10 | Replaced: from the ground a forest was a field of sticks. |
| Boulders of whole 0.25 m cubes | to 2026-11-10 | Replaced: a pile of crates, worse than a stone the player can build. |
| Cubes of flat colour, each a tenth lighter or darker than the next | to 2026-11-11 | Replaced by grain: every surface was a chequer of cubes. |
| Grain as brightness alone, in broad soft shapes | 2026-11-11 | Replaced the same day: it read as no texture at all. |
| Grain as square texels the size of a bit (5 cm), drawn sharp | 2026-11-11 | Dropped the same hour: it is Minecraft's look exactly. |
| Every slope drawn as its 0.25 m cubes | to 2026-11-11 | Replaced by blocks sized to the slope: every hillside was the same staircase. |
| The ground eased fully smooth | 2026-11-11 | Set aside: smooth land under cube trees was two games in one picture. The world stays voxels; their size varies. |
| A white sun and a flat blue fill light | to 2026-11-11 | Replaced by a scattering sky and a lower sun: nothing had depth or mood. |
| Boulders as rounded lumps, all about a metre | to 2026-11-10 | Replaced: uniform orbs. |
| Sizes scaled up a number at a time (trees, then the island, then the hills) | to 2026-11-10 | Replaced by one table of sizes that is measured: each time something was left behind and the person stood like a giant on a model. |
| Mountains wherever a noise ran high | to 2026-11-10 | Replaced: flat-topped walls round most of the island's rim, straight up from the plain. |

## Creatures

### The prompt

`<subject>, <plan pose>, <style>`. Forge assembles it; `cargo forge prompt ID` prints it.

**Subject** (per creature, in `creature.toml`): `<what it is> with <one defining feature>, <up to two colours>`.

- Good: `a red deer stag with antlers` · `a lynx with black-tufted ears, a short bobbed tail and a spotted tawny coat` · `a golem of stacked grey boulders with glowing blue cracks, huge forearms and a small head sunk between the shoulders`.
- One defining feature. More and the model spends its triangles on clutter.
- Nothing held, worn loose or trailing: no weapons, capes, ropes, long hair. Gear is added in the game, on bones.
- No style words. A pose is said in the subject only to repeat the T-pose for a biped that keeps coming back with its arms down (`both arms held straight out sideways at shoulder height`); even so the frost ogre came back arms-down three times, and rigged anyway.
- Name a glow only if the creature should have one, and only one.

**Plan pose** (per body plan, in `style.toml`): the pose that rigs. A biped is a T-pose from the front, hands open and empty. A quadruped stands on four straight, separated legs, seen from the side and slightly in front. Limbs that touch get fused into one mesh and cannot be rigged. A three-quarter view can give a model standing at 45°; forge reads that and turns it.

**Style** (shared, in `style.toml`), in short: stylised low-poly; proportions true to the real animal with a head and eyes of natural size; lean and angular, alert and wary, never cute; a dangerous creature looks dangerous; chunky faceted planar forms; bold readable silhouette; simple large shapes; hand-painted flat colour in a few muted natural hues with one accent; soft stepped shading; not cute, not chibi, not a toy, not a mascot, no big round eyes, no smile; no noise, grain, photorealism or fur strands; no pure black or white; one character, full body, plain light grey background, even light, no ground shadow.

Nothing in the game is in the older, cuter proportions any more: the seven creatures built before the change (bear, boar beast, deer, goblin, rabbit, troll, wolf) were drawn and built again on 2026-11-10, and the player became a castaway (chosen from six subjects drawn with `cargo forge ideas player`; the first was a stock medieval adventurer), then on 2026-11-11 an ordinary man in a work shirt and canvas trousers (which read as a man in a dress outfit), and the same day the same man in homespun: a rough undyed tunic tied with a leather strip, loose dark trousers bound at the ankle, stitched leather shoes. **The world is fantasy and the player wears no costume**: nothing modern (no collars, buttons, zips, printed cloth), and not a pirate or an adventurer either; someone ordinary who made his own clothes out here. Of nine plain men drawn, three came back as photographs (one with a wolf beside him): the style words do not always hold for a subject this ordinary, so look before building.

### Sizes

`height` in `creature.toml` is metres to the top of the creature as it rests, antlers and ears included. Changing it costs nothing: `cargo forge finish ID` scales the model and measures its speeds again, and the simulation follows (a body as wide as it measures, up to 2.4 m; a creature over two metres steps up more than half a metre without a jump).

| Kind | Height | Examples |
|---|---|---|
| Player | 1.75 | |
| Small wildlife | 0.15 to 0.5 | hedgehog 0.15, squirrel 0.2, rabbit 0.3, badger 0.3, fox 0.5 |
| Medium wildlife | 0.7 to 1.5 | lynx 0.7, wolf 0.85, mountain goat 1.1, bear 1.3 at the shoulder, pony 1.45 |
| Large wildlife | 1.9 to 2.6 | deer 1.9 and moose 2.6, both to the antlers |
| Humanoid monster | 1.2 to 2.4 | goblin 1.2, moss shambler 2.4 |
| Giant | 3.0 to 3.6 | troll 3.0, frost ogre 3.4, stone golem 3.6 |

A monster meant to frighten stands well over the player. Nothing smaller than about 15 cm is made as a creature: it would be a few pixels.

### Materials

- Base colour only. No normal, roughness or metallic maps (`--no-pbr`); roughness is one flat value. It must look right with plain lighting and no post-processing.
- 1024² texture, 8,000 triangles asked, 12,000 allowed.
- Not built yet: a master palette that textures are snapped to. Tripo's textures come back paler than the concept; a palette snap would fix that and drift between creatures.

### Judging a concept

Look at `cargo forge concepts` (every concept in one picture) before paying for a model. Regenerate on any "no":

1. Exactly one creature and nothing else: no base, prop, text or scenery?
2. In the plan's pose, with every limb separate from the body and from the others? A biped's arms straight out?
3. Recognisable as its subject if filled solid black?
4. Nothing thin or trailing that weights will tear?
5. A few main colours plus at most one accent, no noise, no fur strands?
6. **Not cute?** A head and eyes of natural size, no smile; a monster that would be run from?
7. Next to the others, would it look like the same game?

### Judging a model

Numbers, then one picture, then a person: `cargo forge check ID` (skin stretch, feet on the floor, foot slide, loops); `cargo forge sheet ID` (every clip at six moments); `make gif ID=…` (twelve seconds of every gait, a turn, a jump, a blow and a flinch, for the developer to watch). To try another subject for a creature before buying, `cargo forge ideas ID`; to try another look, `cargo forge looks`.

## The land

Made by `cube_sim::terrain` and `plants`; judged by `cargo world survey` and `map`, then looked at with `cargo sandbox --scene`.

### Scale

The measure is a person, 1.75 m. The world reads as full size when things keep the ratios to a person that they have outside, and as a model when any one is off, however good it looks alone. So every size is in one table, `crates/cube_sim/src/scale.rs`, and nothing else carries a size: change the world's scale there. `cargo world survey` measures what was made against it.

| Thing | Size | Against a person |
|---|---|---|
| Hills | 80 m from one to the next, 7 m high, smooth | a slope walked, never lumps |
| The mountain | 280 m across its foot, 64 m high, in 5 m ledges | foothills walked, the middle climbed |
| Grown trees | oak 13 to 24 m, birch 11 to 19, pine 17 to 34, willow 9 to 14 | 6 to 19 people tall; the middle one at least 8 (bar) |
| Room for a tree | 6.5 m; small things 3.5 m | |
| Under a crown | 4.5 m clear | the golem (3.6 m) walks under |
| Young trees, bushes | 3 to 7.5 m; knee to head high | what is at eye level |
| Rocks | stones 0.3 to 0.8 m (58 in 100), boulders 1 to 2.6 m (34), great ones 3.2 to 6 m (8) | stepped over, walked round, a landmark; largest at least 10 times smallest (bar) |
| Easy ground | no more than 0.75 m up or down over 2 m | at least 65% of lowland meadow and forest (bar) |

Two rules that the numbers come from:

- **Roughness is what makes a model.** Each finer layer of noise makes the ground as steep again. Lowland gets two layers and no lumps smaller than 40 m; roughness belongs to mountains.
- **A thing's parts follow its height.** A tree's trunk, limbs and crown are parts of how tall it stands, so a taller oak is a bigger oak and not a longer pole.

### Cubes

- **A cube is 0.25 m; a bit is 5 cm.** Almost everything is whole cubes. A cube breaks into 125 bits where something smaller than a cube is wanted: a hole dug, a stone built, a boulder's surface. Nothing in the world is smaller than a bit.
- **What the generator makes is shaped to the bit where a cube is too coarse** (boulders), and left as whole cubes where it is not (ground, trunks, foliage): a broken cube's face is 25 squares where a whole one's is one.
- **Colour is by block, grain is by surface.** A block has a top colour and a side colour (`Block::colors`, `cube_voxel`). Over that goes a grain, one of fifteen made in code (`cube_voxel::texture`: grass, soil, rock from above, rock in beds from the side, sand, snow, pebbles, bark, birch bark, end grain, leaves, needles, leaf litter, moss). A new block needs a colour and the name of a grain it shares, not a painting.
- **A grain has shape, hue and relief.** Broad shapes the size of brush strokes and fine ones seen from a step away; colours a surface really carries besides its own (dry blades in grass, russet and gold in fallen leaves, lichen and rust on rock, moss on bark); and relief, so light rakes across it. Painted and drawn smooth, never square pixels, on a tile two metres across that runs on from block to block.
- **Far off a surface is its block's colour**: every grain averages the same brightness, and smaller copies of the picture take over with distance, so nothing sparkles and nothing changes tone at a level of detail.
- **Blocks are sized to the land**: steps of 5 cm where it is nearly level, whole cubes on a slope, half-metre and metre blocks where it is steep and on rock faces. Still voxels, never smoothed.
- **Every hour has its light, and it is never flat.** Dawn and evening are amber, with long shafts through mist; noon is bright with drifting cloud shadow; night is dim and blue under stars, with fireflies, but always readable. Something in every view is in shade and something is lit. Judge a change to light at five hours, not one (`--hour 6.4`, `12`, `16.8`, `17.5`, `22`).
- **Nothing is still.** Grass and leaves move in the wind, their shadows with them; pollen drifts in the light; leaves fall in the woods; butterflies work the meadows; birds wheel overhead. A screenshot of this world should look like a frame of something moving.
- **Light has a time and a direction**: a sun at 35 degrees through real sky, long shadows, shade that is blue and dim but not black, corners darkened where things meet. Shade on each cube's corners is also baked (ambient occlusion in vertex colour).

| Block | Reads as |
|---|---|
| Grass, Turf | meadow; the darker ground under trees. Their sides are green, not earth, so a slope is a green hill and not brown stripes |
| Dirt, Stone, Gravel, Sand, Snow | under the soil and on steep ground; river beds; shores; high ground |
| Wood, Birch | bark, dark and pale |
| Leaves, Needles | broadleaf and conifer foliage |
| Tuft, Flower, Reed | ground cover: drawn as blades, walked through |
| Water | see-through where shallow, opaque where deep |

### Trees and what grows

- **A tree is a tree.** A trunk that flares into roots and tapers with a lean; limbs that leave it, bend and fork; foliage in separate clumps at the ends of the limbs, so sky shows through and no two are alike.
- **Five kinds**, each with its own silhouette: oak (broad, heavy-limbed), birch (slender, pale, light and high), pine (whorls of drooping boughs under a point, dead stubs below), willow (short and thick by water, hanging in curtains), snag (dead and bare). Sizes are in Scale.
- **Where each grows follows the land**: conifers high and on rock, willows within a few metres of water, the rest mixed. A grown tree has six and a half metres to itself.
- **A wood has an understory**: young trees, bushes, fallen logs, stumps. Something green is always at eye level.
- **Foliage starts 4.5 m up** on grown trees, so the largest creature walks under.
- **Ground cover grows in patches**, not sprinkled: long grass and flowers (white, yellow, violet) in meadow, sparser under trees, reeds at the water's edge.
- **Rocks vary in size and in form.** Stone with flat faces struck off it, never a ball: a block, a slab lying flat, a tooth standing up, or a hump mostly buried, in slate, chalk or grey stone, with smaller pieces against it and stones fallen round a large one.

### Distance

- **Three levels of detail.** Within 40 m every cube is drawn, with ground cover and the bits of broken cubes. To 104 m the cubes are taken two a side (half a metre), beyond that four a side (a metre). A far group of cubes is solid if any of it is, so distant ground has no holes and trees keep their bulk. Each level fades into the next across 8 m.
- **Haze starts at 180 m and is full at 700 m**: the whole island is clear from anywhere on it, and the sea fades into the sky without an edge.
- The island is 384 m across and a stand-in: the world is to be procedural and far larger ([engine.md](engine.md#the-world)).

### Judging the land

`cargo world survey` (water, biomes, trees, cliffs, what can be walked to, the faces it costs to draw, and scale), `cargo world grade FILE` when the ground is wrong (easy, steep and cliff by height, and a picture of where), then `cargo world map` (one picture from above), then `cargo sandbox --scene FILE --at X,Z` for any place at eye level and `--view high` from the air. Look at it from where the player stands: a forest that reads from the air can still be sticks from the ground.

Always judge scale with a person in the picture (`--play --view three`): a tree alone has no size.
