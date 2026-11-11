# What makes cube worlds look good (2026-11-11)

Asked: how do the best-looking strictly cube-based games get detail and realism, and how do they build foliage from cubes? Evidence is strong for light, material and tree skeletons and thin for foliage from cubes; where a recipe below is ours and not a source's, it says so.

## What the sources say, by how much it buys

1. **Realistic light on abstract cubes** (Teardown) is the largest lever against the Minecraft look. Voxels of 10 cm; shadows softened by jittering the light, wider at sunrise and sunset; ambient occlusion traced over a long range; emissive voxels as area lights; volumetric light and height fog. It has no true global illumination: occlusion and area lights carry it.
2. **A fake bevel by blurring normals** (Teardown): a full-screen blur of the normal buffer makes cube edges catch light as if bevelled, for no geometry.
3. **Detail under the size of a cube goes in the material**: Teardown lays albedo and normal maps on at one texel a voxel; the labPBR standard for Minecraft packs carries normal, occlusion, height (parallax), smoothness, porosity (darkens when wet) and subsurface scattering per texel.
4. **Trees are a tapering skeleton with leaf only on thin tips** (Vintage Story): width lost per block, children at set spacing, a gravity drag on level limbs; leaf kind by branch width (thicker shows its twigs); moss and vines by chance; the ground under a tree blurred into forest floor.
5. **Break the leaf cube** (Hytale): leaf texture cut through the block at angles, each block turned at random, grass and leaves swaying and bending round what walks through.
6. **Colour discipline** (Trove, Hytale): mostly solid colour with low-contrast noise, nothing near black, no lighting painted in; one block tinted by area for many variants; a transition between ground types.
7. **Level of detail for cubes** (0fps): round vertices to powers of two per level and morph between levels by distance.

## What we took (ours where no source is named)

- Wood in cubes of 5 and 10 cm near to, each its own shade and a line of them along the grain sharing one (furrows); trunks swelling into stepped roots; moss on the foot (4, and ours).
- Leaf as boxes of many sizes, each its own green, lit on top and dark under, over a dark heart, with gaps (5, 6, and ours). A conifer's spray as thin plates, with dark between the tiers (ours).
- Grass as columns of bits one to five high in clumps, dark at the foot; ferns as stepped arcs of bits; a flower one bit of colour on a stem (ours).
- Stone in beds of differing shade with moss and lichen on what looks up (ours).
- Under a tree, that tree's ground (4).
- Not yet taken: the normal-blur bevel (2), occlusion baked per cube over a long range (1), parallax and porosity in the material (3), morphing between levels (7), foliage bending round bodies (5).

## Not verified

Nothing technical was found for Lay of the Land, Cube World, Dragon Quest Builders 2 or Teardown's vegetation; no written method from MagicaVoxel artists. Claims about Hytale's "no PBR" style and about what the Complementary shader pack adds came from search summaries, not pages read.

## Sources read

- https://juandiegomontoya.github.io/teardown_breakdown.html : Teardown's buffers, occlusion, soft shadows, normal-blur bevel, per-voxel texels, no GI.
- https://blog.voxagon.se/2018/10/17/from-screen-space-to-voxel-space.html : shadow volume at 5/10 cm, light jitter.
- https://softwareengineeringdaily.com/wp-content/uploads/2024/12/SED1772-Teardown.txt : 10 cm voxels.
- https://www.hytale.com/news/2019/2/exploring-hytale-s-block-tech : leaf geometry and rotation, tints, transitions, sway.
- https://wiki.vintagestory.at/index.php/Modding:Trees : tree generator, leaf by branch width, moss, forest floor.
- https://wiki.vintagestory.at/Settings : what each effect costs.
- https://shaderlabs.org/wiki/LabPBR_Material_Standard : what PBR packs carry per texel.
- https://trovesaurus.com/page=5624 : Trove's colour rules.
- https://0fps.net/2018/03/03/a-level-of-detail-method-for-blocky-voxels/ : level of detail with morphing.
