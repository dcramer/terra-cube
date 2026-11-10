# Art guides digest: what to tell the mob generator (2026-11-09)

Question: what do published voxel/stylized art guides actually require, and what of it can we
enforce on Tripo-generated low-poly creatures via prompt + automated QA?

Read status: Trove PDF read in full (7 pages, as images). Both Veloren pages read via a
summarising fetch (not verbatim). Hytale modelling post read the same way. NOT read / nothing
found: Eco, Cube World, Vintage Story (search found only technical entity tutorials, no art rules).
Tags: [T] transfers to AI meshes, [V] voxel-editor specific, [mine] my inference, not in a source.

## In plain words

1. The guides are thinner than their reputation. Trove's is weapons-only (no creature rules);
   Veloren's pages give scale and file naming only (no palette or shading rules). Hytale's 2025
   post is the only one with real style rules. Most creature rules below are therefore ours to set.
2. Scale is fixed to the world block, with numbers. Trove: player 23 voxels tall, 24 = one world
   block. Veloren: block = 11x11x11 small voxels, character ~22 tall x 10 wide = 2 blocks x 1.
   [T] as "height in blocks per category"; the voxel counts are [V].
3. Characters are short and bulky. Hytale: "small, bulky, cartoony"; props "toylike", pure iconic
   shapes. Trove's knight reads as roughly 2.5-3 heads tall [mine, from the picture]. [T]
4. Silhouette first, simplest geometry that carries it. Hytale: cubes and quads only, "no
   spheres", add geometry only if the silhouette needs it; must read in a cluttered scene. [T] as
   "chunky faceted forms, no smooth organic sculpt"; the cube-only rule is [V].
5. Palette: bold, saturated, few hues that complement each other, one accent. Trove's example is
   two hues from a triad (purple body, green accent). No colour-count number is given anywhere. [T]
6. Value matters more than hue. Trove rejects saturated colours with no light/dark range.
   Check the render in greyscale. [T]
7. Shading is NOT "flat, no gradients". Trove wants smooth stepped value ramps (dark base to
   light tip), abrupt jumps only on tiny parts. Hytale: paint shadow/AO/highlight into the
   texture; avoid perfectly flat surfaces. [T] as "soft hand-painted value ramps per part".
8. No noise or grain (Hytale), no photo texture. [T], and it is the commonest AI failure.
9. No pure white, no pure black; shadows keep colour (Hytale suggests a hint of purple). [T]
10. No PBR: no roughness/normal/displacement maps; model must look right with no post-processing
    (Hytale). [T]: use base colour only, drop Tripo's PBR maps. No source covers emissive.
11. Faces get the detail budget. Hytale gives characters 64 px/unit vs 32 for props so eyes and
    mouths read; stretch stays within 0.7-1.3x. [T] as "simple large eyes, detail on the head".
12. Conventions are machine-checkable and violations are rejected outright (Trove). Trove: one
    attachment voxel, RGB 255,0,255; handle 1 voxel thick, >= 3 long; front axis fixed; weapon
    box 35x10x10. Veloren: `snake_case-0.vox`, numbered from zero; sprite <= 33 voxels (3 blocks)
    tall. [T] as fixed facing/pose/pivot/naming; the magic-pink voxel is [V].

## Sources

| Source | Date | What it says |
|---|---|---|
| [Trove Art Guide (PDF)](https://webcdn.triongames.com/trove/pdf/Trove_Art_Guide.pdf) | undated, c. 2014 [mine] | Weapon submissions. Style goal: "if it makes you smile or laugh". Vibrant complementary palette, value ramps, 35x10x10 box, player 23 voxels, pink attachment point, front-axis facing, 7-item checklist, rejection on any miss. |
| [Veloren: Voxel Models](https://veloren.gitlab.io/book/contributors/artists/voxel-models.html) | undated | MagicaVoxel; block = 11^3 voxels; character 22 x 10 voxels = 1x2 blocks; "study the asset repo" for style; models go through approval. No palette/shading rules. |
| [Veloren: Adding Sprites](https://veloren.gitlab.io/book/contributors/guides/adding-sprites/guide.html) | undated | Sprite = one column up to 3 blocks (33 voxels); `solid_height = voxels / 11`, max 3.0; naming `word_word-0`; manifest with offset (half size), wind_sway 0-1, variations. |
| [Hytale: An Introduction to Making Models](https://hytale.com/news/2025/12/an-introduction-to-making-models-for-hytale) | Dec 2025 | Pillars: immersive, fantasy, stylized, flexible. Cubes + quads only, no spheres. 64 px/unit characters, 32 props, sizes multiples of 32. Painted light, no noise, no pure white/black, coloured shadows, no PBR, stretch 0.7-1.3x. No creature-specific ratios. |
| Eco, Cube World, Vintage Story | - | Not read. No published art guide found in a quick search. |

## What a generator playbook should contain

Everything in this section is a proposal [mine]; numbers are starting points to tune on renders.

### Sections

1. Style pillars (3-4 words each, like Hytale's) and the one-line test ("reads as a toy").
2. Shared style suffix (positive) and shared negative list. Never edited per creature.
3. Body-plan pose hints (biped, quadruped, bird, serpent, insect, blob).
4. Category proportion table (below), expressed in world blocks and head counts.
5. Palette system: named biome ramps + role slots.
6. Subject-line grammar: `<size adj> <animal/archetype>, <one defining feature>, <primary colour>
   with <accent colour> <accent location>`. Max one feature, two named colours.
7. Post-processing contract: base colour only, triangle budget per category, facing +Z,
   feet on ground plane, pivot at feet centre, uniform scale to category height.
8. QA checklist and reject/regenerate policy (max retries, then flag for a human).
9. Naming: `<category>_<name>-<variant>` numbered from zero (Veloren convention).

### Prompt phrases

Positive (shared suffix): "stylized low-poly game creature, chunky faceted forms, thick limbs,
oversized head, simple large eyes, bold readable silhouette, hand-painted flat colour with soft
value shading, saturated complementary palette, toylike, single character, neutral grey
background, even studio lighting".

Avoid (negative prompt, or omit from subject lines): "photorealistic, realistic fur, hair
strands, feathers detail, skin pores, noise, grain, gritty, fine detail, thin spindly limbs,
smooth subdivision sculpt, glossy, metallic PBR, pure black, pure white, voxel, minecraft,
blocky cubes, pixel art, text, base, pedestal, multiple characters, weapon, scenery".
"Voxel" is in the avoid list on purpose: the world is blocks, the creatures are not.

Pose hints: biped "A-pose, arms away from body, legs apart, facing camera"; quadruped "standing
on four legs, side-on three-quarter view, legs separated, tail clear of legs"; bird "wings
half open, standing"; serpent "single S-curve, head raised". Separated limbs matter for rigging.

### Proportions per category

| Category | Height (blocks) | Heads tall | Rules |
|---|---|---|---|
| Player | 1.8-2.0 | 3 | Limb width >= 1/3 head width; hands/feet oversized; neutral build |
| Humanoid monster | 1.5-2.5 | 2.5-3.5 | One exaggerated part (jaw, arms or shoulders); same eye style as player |
| Small wildlife | 0.4-0.8 | 1.5-2 | Head ~ half the mass; stubby legs; one colour + belly tone |
| Large wildlife | 1.5-3 | 4-5 (body lengths for quadrupeds) | Barrel torso, short thick legs, small head allowed; no thin ankles |
| Boss | 3-6 | 3-4 | Same rules scaled up, not more detail; one emissive accent; widest silhouette |

Level of detail tracks size class, not importance: small = 2 colours + eyes, medium = 3,
large/boss = up to 4 plus one accent. Smallest feature should be >= 1/16 of creature height so
it survives distance (the equivalent of "one voxel" in Trove/Veloren).

### Palette approach

- One master palette of ~32 swatches: 8 hue ramps x 4 values, none pure white/black
  (clamp value to roughly 10-92%), shadows shifted toward purple/blue, highlights toward yellow.
- Per creature pick roles: primary (60%), secondary (30%), accent (10%), plus eye colour.
  Accent is the complement or triad of the primary (Trove's purple/green example).
- Each biome owns 2-3 ramps so fauna matches its terrain; hostile mobs take the biome's accent.
- Enforce after generation: quantise the albedo texture to the master palette, flatten roughness
  to one constant, discard normal/metallic maps. Emissive only where the subject line names a
  glowing part, at most one per creature.

### QA checklist (yes/no, from a front + side + greyscale render)

1. Is there exactly one creature and nothing else (no base, props, scenery)?
2. Is the silhouette recognisable as the named subject when filled solid black?
3. Is the head at least the stated fraction of total height for its category?
4. Are all limbs thick (no limb thinner than a third of the head width)?
5. Are all limbs separated from the body and from each other?
6. Are there four or fewer dominant colours, plus at most one accent?
7. Do the primary and accent colours match the requested ones?
8. Is the surface free of noise, grain, fur strands and photo texture?
9. Is the model free of pure black and pure white areas?
10. In greyscale, are there clearly distinct light and dark regions?
11. Are surfaces faceted/chunky rather than smooth realistic or cube-blocky?
12. Are two simple eyes visible and symmetric from the front?
13. Is it free of glossy or metallic highlights?
14. Is it facing forward, upright, with feet on the ground plane?
15. Placed next to the player reference, would it look like the same game?

Any "no" on 1-5 or 14 = regenerate; "no" on 6-13 = try texture fix (palette quantise) first.
