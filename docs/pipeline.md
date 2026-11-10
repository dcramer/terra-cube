# Making creatures

`cargo forge` takes a creature from a line of text to an animated model in the game. The paid steps run once; everything after them is free, local and repeatable. Evidence for the design: [research/2026-11-09-model-pipeline-prior-art.md](research/2026-11-09-model-pipeline-prior-art.md).

## A new creature

```bash
cargo forge new fox --kind wildlife --plan quadruped --height 0.45 "a red fox with a bushy white-tipped tail"
cargo forge build fox --to concept     # 2 credits: draws assets/creatures/fox/concept.jpg
cargo forge concepts fox player        # shots/concepts.jpg: look at it beside the player
cargo forge build fox                  # 65 credits: model, rig, export; then finish and check
cargo forge sheet fox                  # shots/fox.png: look once
```

`build` prints one line when the creature is good, and the failing checks when it is not. It exits 1 on a failed check and 2 on an error.

- **The subject** says what the creature is, with at most one defining feature and two named colours. No style words: the style comes from `assets/creatures/style.toml` ([art-guide.md](art-guide.md)). `cargo forge prompt ID` shows the whole prompt.
- **The height** is in metres to the top of the rest pose (antlers and ears count).
- **A bad concept** (wrong pose, joined legs, props): delete `concept` from `steps` in `assets/creatures/ID/forge.lock`, adjust the subject, build `--to concept` again. Each try is 2 credits; a model is 45, so look first.

## What a build does

| Step | Where | Cost | What happens |
|---|---|---|---|
| concept | Tripo | 2 | An image from "subject, plan pose, style" |
| model | Tripo | 45 | A textured mesh from the image, 8,000 triangles, colour only |
| rig | Tripo | 20 | Skeleton and skin weights: a biped on the Mixamo-named skeleton, anything else as Tripo reads its shape |
| export | Tripo | 0 | Tripo's preset clips put on, the file saved to `.work/ID/rigged.glb` |
| finish | local | 0 | Reads the skeleton's roles, stands the model at the origin facing +Z at its height, names Tripo's clips, makes the rest, measures speeds, writes `model.glb` and `meta.toml` |
| check | local | 0 | Holds the model to the bars in `style.toml` `[checks]` |

`forge.lock` records the Tripo project and the steps done, so a build run again spends nothing. `.work/` is scratch: if `rigged.glb` is gone, `build` fetches it again for free.

## Body plans

A plan (`[plans.*]` in `style.toml`) says how the concept is posed, how Tripo rigs it, which of Tripo's presets to take, and which clips every creature of the plan must have.

| Plan | Rig | From Tripo | Made by forge |
|---|---|---|---|
| `biped` | humanoid, Mixamo names | idle, walk, run, attack (slash), hurt, jump | die |
| `quadruped` | as Tripo reads it | walk | idle, run, attack, hurt, die, eat |

A plan can also cut a preset that runs long (`trim`: clip → from, to, speed). The biped's `attack` is 2.4 s of Tripo's 6.6 s overhead chop at double speed; the animals' walk plays at 1.8×. Tripo's biped `hurt` is a figure sat wounded for 14 s, so ours is made.

Tripo has no other presets for animals (one walk or march per rig type, none for birds), which is why the rest are made.

## Roles and made clips

Tripo's bone names cannot be trusted (`bone_7` beside `tripo::0_Right_Limb_0`; a tail bone named `1_Left_Limb_0`). So `rig.rs` reads roles from shape:

- A **chain** runs from a leaf bone up to the first bone that branches.
- Chains whose tip is near the floor are **legs**, front or hind and left or right by position.
- The highest other chain is the **head**; one trailing behind the feet is the **tail**; the trunk from the root to the head is the **spine**; the rest (arms, wings, ears) are **others**.
- **Forward** is toward the head when it sits ahead of the feet, else +Z as Tripo exports an upright figure. `facing = "+x"` in `creature.toml` overrides a wrong guess.

`cargo forge inspect ID` prints the roles. Look at it first when a creature moves wrongly.

Clip recipes (`clips.rs`) are written against roles, so each serves every plan: `idle` (breathing, a look about, tail sway), `run` (the walk quicker and wider with the trunk pumping), `attack` (a lunge), `hurt` (a flinch), `die` (falls and is settled on the floor each frame), `eat` (neck and head down, nibbling), and `walk` as a fallback for a rig with no preset. They are plain and readable on purpose; they are the place to add craft.

What finish does to every clip, so no creature needs hand fixes:

- A made clip is laid over how the creature **stands** (the first frame of Tripo's idle), not its rest pose, so a biped does not snap to a T-pose.
- A looping preset is **closed**: its last fifth eases into its first frame (Tripo's biped run was 19° open).
- Walk and run are **grounded**: the body is lifted on any frame where a foot would be under the floor.
- `gait_gain` in `creature.toml` scales how far Tripo's walk swings the limbs. The bear needs 0.6: the preset is fitted to long legs and folded its short ones (73% stretch, feet 9% under the floor, both caught by the checks).

## Checks

One line each, `ok` or `FAIL name value (bar)`; passing lines print only when something failed, or with `FORGE_VERBOSE=1`.

| Check | Bar | Catches |
|---|---|---|
| triangles, joints | ≤ 12,000, ≤ 80 | A model too heavy for a crowd |
| roles | ≥ 2 legs and a head | A skeleton the recipes cannot use |
| clips | all the plan's | A preset Tripo did not deliver |
| weights | 0 unweighted vertices | Skin left behind |
| stretch, per clip | 99th-percentile edge stretch ≤ 60% | Bad weights or a recipe that tears the skin: the skin is posed on the CPU through each clip |
| loop gap | ≤ 5° | A looping clip that pops |
| feet off floor | ≤ 8% of height | Feet sinking or floating in idle, walk, run, eat |

The bars are first guesses calibrated on eight creatures. Terra measured a hand-rigged deer at 10% stretch in a gallop and bad auto-weights at 118% to 194%; ours sit at 8% to 41%.

## Looking

`cargo forge sheet ID` renders `shots/ID.png` offscreen: a row a clip, six moments each, a slab under each row to read feet against. `--view side` or `front` turns the creature; `--clips walk,run` limits the rows. One sheet is about 1,500 tokens. Ask of it: does each row read as its name, do feet stay on the slab, does anything tear or fold.

`cargo sandbox --scene shots/scene.png --spawn all` draws the test world with one of each creature in it, offscreen.

In the sandbox (`cargo sandbox --spawn ID`) the panel plays clips, scrubs a held frame, sets speed, sends the creature wandering at its measured walk or run speed (so sliding feet show), and draws skeletons.

## Known limits

- Animals are slow: Tripo's walk is an amble, and `run` is that walk quicker and wider, not a gallop (the wolf runs at 1.6 m/s). The game can play clips faster; a real gallop recipe is the fix.
- `die` leaves legs stiff.
- Walk speed is measured from the feet of the in-place clip; if it reads wrong the creature slides when wandering.
- A biped's `die` and an animal's whole set have only been judged on sheets, not in play.
- The Tripo CLI drives the web app's private backend with a browser cookie that lasts about 30 days ([tools/tripo/README.md](../tools/tripo/README.md)). When builds fail with a login error, the cookie in `.env` needs replacing.
