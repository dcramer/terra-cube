# Art guide

How things in terra-cube should look, and what we tell the generator so they do. The rules here are enforced by `assets/creatures/style.toml`, which forge adds to every prompt: change the look there, once, never per creature. Sources: [research/2026-11-09-art-guides.md](research/2026-11-09-art-guides.md) (Trove's, Veloren's and Hytale's guides). Those guides are thin on creatures, so most numbers below are ours and are starting points.

Status: first draft, 2026-11-09, written from eight generated creatures. The developer has not yet ruled on the look.

## Pillars

1. **Reads as a toy.** Small, bulky, a slightly big head, thick limbs. If it would look right as a painted wooden figure, it fits.
2. **Shape before detail.** Recognisable as a black silhouette. A few large faceted forms, nothing fiddly.
3. **Few colours, strong values.** Two to four saturated hues and one accent, with clear light and dark. Painted, never photographic.
4. **The world is blocks; what lives in it is not.** Creatures are chunky low-poly meshes. Never ask for "voxel", "blocky" or "Minecraft".

## The prompt

`<subject>, <plan pose>, <style>`. Forge assembles it; `cargo forge prompt ID` prints it.

**Subject** (per creature, in `creature.toml`): `<size word> <what it is> with <one defining feature>, <up to two colours>`.

- Good: `a red deer stag with antlers` · `a green goblin raider with big pointed ears, a leather loincloth and shoulder strap` · `a small brown rabbit with long upright ears`.
- One defining feature. More and the model spends its triangles on clutter.
- Nothing held, worn loose or trailing: no weapons, capes, ropes, long hair. Gear is added in the game, on bones.
- No style words and no pose words. They are in the style and the plan.
- Name a glow only if the creature should have one, and only one (`glowing purple eyes`).

**Plan pose** (per body plan): the pose that rigs. A biped is a T-pose from the front, hands open and empty. A quadruped stands on four straight, separated legs, seen from the side and slightly in front, tail clear of the legs, mouth closed. Limbs that touch get fused into one mesh and cannot be rigged.

**Style** (shared): see `style.toml`. In short: stylised low-poly, proportions true to the animal (a head and eyes of natural size), lean and wary and never cute, a dangerous thing looking dangerous, faceted planar forms, bold silhouette, flat hand-painted colour in a few muted natural hues with one accent, soft stepped shading, no noise or grain or fur strands, no pure black or white, one character, plain grey background, even light, no ground shadow.

## Sizes and proportions

Heights are metres in the game (a cell is 0.5 m). Set `height` in `creature.toml`.

| Kind | Height | Proportions |
|---|---|---|
| Player | 1.75 | About 3 to 4 heads tall, hands and feet a little large, neutral build |
| Humanoid monster | 1.0 to 2.6 | One part exaggerated (ears, arms, jaw); same eye style as the player |
| Small wildlife | 0.2 to 0.6 | Head near half the mass, stubby legs, one colour and a belly tone |
| Large wildlife | 1.0 to 2.0 | Barrel body, thick legs, no thin ankles |
| Boss | 3 to 6 | The same rules larger, not more detail; widest silhouette; the one allowed glow |

Detail follows size, not importance: the smallest feature should be at least a sixteenth of the creature's height so it survives distance.

## Materials

- Base colour only. No normal, roughness or metallic maps (`--no-pbr`); roughness is one flat value. It must look right with plain lighting and no post-processing.
- 1024² texture, 8,000 triangles asked, 12,000 allowed.
- Not built yet: a master palette that textures are snapped to, and per-biome colour ramps. Worth doing once there are biomes to match.

## Judging a concept

Look at `cargo forge concepts` (every concept in one picture) before paying for a model. Regenerate on any "no":

1. Exactly one creature and nothing else: no base, prop, text or scenery?
2. In the plan's pose, with every limb separate from the body and from the others?
3. Recognisable as its subject if filled solid black?
4. Thick limbs, nothing thin or trailing that weights will tear?
5. Four or fewer main colours plus at most one accent, no noise, no fur strands?
6. Next to the player, would it look like the same game?

## What we have seen

- Quadrupeds come out faceted and painterly, as wanted. Bipeds drawn with Tripo's T-pose option come out smoother and more "mobile game". If the developer wants them matched, the biped pose line is the place to push facets harder.
- The style line holds across subjects: eight creatures from one line look like one set.
- Tripo's textures come back paler than the concept. A palette snap would fix both that and drift.

**Not cute.** The first roster of 29 came back as a shelf of toys: big heads, big round eyes, smiles. The style used to ask for "small bulky cartoony proportions with a slightly oversized head", and that is what it got. A creature may be appealing; it may not be a mascot. The style text now says so in as many words (2026-11-10), and the seven creatures built before it are the old proportions.

**Sizes are real, and big things are big.** `height` in a `creature.toml` is metres to the top of the creature as it rests, antlers and ears included, beside a player of 1.75 m. An animal is the size of the animal (a moose 2.6 m to its antlers, a fox half a metre to its ears). A monster meant to frighten stands well over the player: the troll 3.0 m, the ogre 3.4, the golem 3.6. Changing a height costs nothing: `cargo forge finish ID` scales the model and measures its speeds again. The simulation follows the size: a body as wide as it is measured (up to 2.4 m across), and a creature over two metres steps up more than half a metre without a jump.
