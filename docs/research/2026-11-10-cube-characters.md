# Characters made of cubes: who does it, and how

2026-11-10. Asked after `cargo forge cubes` turned the player into 5 cm cubes: the body and head read well, the legs did not. Is that the cube size, or the method?

## What we measured

`cargo forge cubes player --size S --head 1.0`, looked at from the front (`shots/sizes.png`):

| Cube | Cubes | Triangles | Legs |
|---|---|---|---|
| 5 cm | 1,732 | 6,760 | One brown mass, a stray lump at the hem |
| 3.5 cm | 4,360 | 13,140 | The same mass, finer; over the 12,000 triangle bar |
| 2.5 cm | 10,742 | 25,448 | Still one mass; twice the bar |

Smaller cubes do not fix the legs. The cause is the source: a model of natural proportions stands with its legs a few centimetres apart, so on any grid coarse enough to look like cubes they fuse, and forty-six bones each claiming cubes leaves ragged seams between thigh, shin, foot and toes.

## How shipped games do it

- **Veloren** (Rust, open source). Characters are drawn by hand in MagicaVoxel, about 22 voxels tall and 10 wide, which is about two terrain blocks: a figure's voxels are much finer than the land's. The model is cut into separate parts that match the skeleton's bones, a body plan has few bones (a medium quadruped has 11), and animation is code moving each rigid part. Artists are told gaps show at joints if a part ends in a flat face. [Contributing voxel models](https://book.veloren.net/contributors/artists/voxel-models.html), [devblog 88](https://veloren.net/blog/devblog-88/), [devblog 13](https://veloren.net/blog/devblog-13/).
- **Trove and most voxel games**: voxels rigidly fixed to the joints of an ordinary skeleton, chunks of the model snapped to bones. [Working with voxels in gamedev, 80.lv](https://80.lv/articles/working-with-voxels-in-gamedev).
- **Fugl**: every creature but the player is animated by swapping whole voxel models, one a frame. [Art design deep dive, Game Developer](https://www.gamedeveloper.com/art/art-design-deep-dive-the-gorgeous-voxel-creatures-of-i-fugl-i-).
- **Hytale** (and Minecraft before it): not voxels for characters at all, but a few cuboids and flat quads with pixel-art textures, made in Blockbench, about 64 pixels to a block for creatures. [Hytale modding: art assets](https://hytalemodding.dev/en/docs/established-information/server/content-categories/art-assets), [Hytale art style explained](https://allthings.how/hytale-texture-packs-and-art-style-explained).

Not found in this search: how Cube World or Stonehearth build theirs. From memory, unverified: Eco and Dragon Quest Builders put smooth characters in a block world, which is what terra-cube did before this trial.

## What their characters look like

Looked at 2026-11-10 in each game's own store screenshots (not kept in the repository: they are theirs).

| Game | World | Character |
|---|---|---|
| [Lay of the Land](https://store.steampowered.com/app/2776090/) (in development) | Fine voxels, dug and built | Built of the same fine voxels; limbs are thin separate stacks with clear air between the legs |
| [Voxile](https://store.steampowered.com/app/2283580/) | Fine voxels | Chunky body, big head, legs two separate blocks |
| [Teardown](https://store.steampowered.com/app/1167630/) | 10 cm voxels | The nearest to natural proportions; legs still plainly two |
| [The Touryst](https://store.steampowered.com/app/1796410/) | Voxel sets | Head about a third of the height, stubby separate legs |
| [Stonehearth](https://store.steampowered.com/app/253250/) | Blocks | Huge head, small body, tiny separate legs |
| [Trove](https://store.steampowered.com/app/304050/) | Blocks | Cube head, limbs straight box shapes held apart |
| [Cube World](https://store.steampowered.com/app/1128000/) | Blocks | Big head and hair, small body |
| [Staxel](https://store.steampowered.com/app/405710/) | Blocks with fine detail | Big head, straight limbs, arms held out from the body |
| [Everwind](https://store.steampowered.com/app/2253100/) | Blocks | Not voxels: a few textured boxes, as Minecraft and Hytale |

Every one of them, whatever its voxel size: legs are two straight pieces with a gap you can see, arms stand off the body, the head is big for the body, and a figure is a handful of parts. None has the thing ours has, two legs fused into one mass.

## What follows for us

1. **It is the method, not the size.** Every game above that makes cube characters work designs them as a handful of clean rigid parts, with proportions chosen for the grid (big head, thick separate legs, mitten hands). None converts a natural-proportioned smooth model bone by bone.
2. **Our 5 cm is already finer than Veloren's figures** (35 cubes tall against 22). Resolution is not what is missing.
3. **Two honest ways on.** Keep smooth creatures in the cube world (cheapest, proven, less of one piece). Or make cube creatures the way the games do: forge builds each body part as its own block shape from a dozen parts, not forty-six bones, legs kept apart, and the style prompt asks Tripo for chunky toy proportions so there is something clean to convert.
4. **A character's cubes need not be the land's.** Veloren's are about eleven to a block. Our bit (5 cm, five to a cell) is the same idea and can stay the one small size.
