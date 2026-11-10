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
| `biped` | humanoid, Mixamo names | idle, walk, run, attack (slash) | sprint, jump, fall, land, swim, hurt, die; and walk or run where Tripo's slides |
| `quadruped` | as Tripo reads it | nothing | every clip |

A plan can also cut a preset that runs long (`trim`: clip → from, to, speed): the biped's `attack` is 2.4 s of Tripo's 6.6 s overhead chop at double speed. Tripo's biped `hurt` is a figure sat wounded for 14 s and its `jump` is three hops on the spot, so ours are made. Its `fall`, `dive` and `climb` presets are free and have not been looked at.

Tripo's one animal clip, a walk, is not used: measured, its forefeet never reach the ground and the feet that do slide at 90% to 180% of the body's speed. `made = ["attack"]` in a `creature.toml` makes a clip for that creature where its plan would take the preset (the troll: the overhead chop tore its skin at 94%).

## Gaits

A gait is right when a foot that is down stays where it was put while the body goes over it. Forge holds every walk, run and sprint to that, and `check` measures it (`slide`, bar 25%).

- **Four legs: made** (`gait` in clips.rs). Each foot is carried straight back along the ground at the gait's speed while it is down and swung forward in an arc while it is up, and the leg is bent to reach it (a two-bone solve). Walk, trot (`run`) and gallop (`sprint`) differ only in numbers: how long a foot is down, the order the feet land, stride, lift, and for the gallop a rocking trunk. Speed comes from the leg's length by Froude number, v² / (g × leg), which animals of every size share at each gait: a wolf walks at 1.0 m/s, trots at 2.0, gallops at 3.6; a rabbit 0.5, 0.9, 1.6. Slide is 0% to 6%.
- **Two legs: Tripo's, with the feet locked** (`planted`). The speed is the middle backward speed of feet while they are down; then each foot, while down, is held to a straight line at that speed and the leg bent to keep it there. Everything else in the recording is left alone. The player's run goes from 28% slide to 10%.
- **Two legs, where that is not enough: made.** A preset is fitted to other proportions, and on a short or bulky body the feet wander too much to lock (the troll's walk slid at 140%, the goblin's at 111%). A recorded gait still over the bar after locking is replaced by a made stride: the same planted-foot generator on two legs, carried on the ankles with the feet flat, the arms swinging against the legs and the trunk leaning in. Today that is every biped's walk and all the troll's gaits.
- **`meta.toml` speeds are the speeds the game moves at**, so a clip always plays at its own rate. To make a creature faster, change the gait, not a number in the game.
- `gait_gain` in `creature.toml` (under 1) shortens stride, lift, rock and arm swing for a body whose skin tears at full reach: bear 0.6, boar 0.7, deer 0.4, troll 0.6.

## Roles and made clips

Tripo's bone names cannot be trusted (`bone_7` beside `tripo::0_Right_Limb_0`; a tail bone named `1_Left_Limb_0`). So `rig.rs` reads roles from shape:

- A **chain** runs from a leaf bone up to the first bone that branches.
- Chains whose tip is near the floor are **legs**, front or hind and left or right by position.
- The highest other chain is the **head**; one trailing behind the feet is the **tail**; the trunk from the root to the head is the **spine**; the rest (fingers, wings, ears) are **others**.
- A limb that leaves the trunk and ends in a branching hand is an **arm** (shoulder to hand); its fingers are the others.
- **Forward** is toward the head when it sits ahead of the feet, else +Z as Tripo exports an upright figure. `facing = "+x"` in `creature.toml` overrides a wrong guess.

`cargo forge inspect ID` prints the roles. Look at it first when a creature moves wrongly.

Clip recipes (`clips.rs`) are written against roles, so each serves every plan: `idle` (breathing, a look about, tail sway), `sprint` for a biped whose run is Tripo's (that run, quicker and leant forward), `jump` (into an airborne pose and held: one knee up and arms out for a biped, forelegs reaching for an animal), `fall` (that pose, limbs stirring), `swim` (a biped lies forward, kicks and reaches its arms over in turn; an animal holds its head up and paddles), `attack` (a lunge), `hurt` (a flinch), `die` (falls and is settled on the floor each frame), `eat` (neck and head down, nibbling), and `land` (sinks onto bent legs and rises, feet held where they were), and a plain `walk` for a body that is neither four-legged nor upright. They are plain and readable on purpose; they are the place to add craft.

What finish does to every clip, so no creature needs hand fixes:

- A made clip is laid over how the creature **stands** (the first frame of Tripo's idle), not its rest pose, so a biped does not snap to a T-pose.
- A recipe that cares which way a limb really points (arms in `jump`, `fall`, `swim`) gives its turns **as the creature stands** (`Stance` in clips.rs): on a T-pose rig "swing the arm forward" is a different turn at rest than with the arm hanging.
- A looping preset is **closed**: its last fifth eases into its first frame (Tripo's biped run was 19° open).
- Tripo's gaits have their feet locked (see Gaits).

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
| feet off floor | ≤ 8% of height | Feet sinking or floating in idle, walk, run, sprint, eat |
| slide | ≤ 25% of the gait's speed | Feet that skate: how fast a foot moves over the ground while it is down, with the gait played at its speed |

The bars are first guesses calibrated on eight creatures. Terra measured a hand-rigged deer at 10% stretch in a gallop and bad auto-weights at 118% to 194%; ours sit at 8% to 41%.

## Looking

`cargo forge sheet ID` renders `shots/ID.png` offscreen: a row a clip, six moments each, a slab under each row to read feet against. `--view side` or `front` turns the creature; `--clips walk,run` limits the rows. One sheet is about 1,500 tokens. Ask of it: does each row read as its name, do feet stay on the slab, does anything tear or fold.

`cargo sandbox --scene shots/scene.png --spawn all` draws the test world with one of each creature in it, offscreen.

In the sandbox (`cargo sandbox --spawn ID`) the panel plays clips, scrubs a held frame, sets speed, sends the creature wandering at its measured walk or run speed (so sliding feet show), and draws skeletons.

## Known limits

- Made gaits are regular: every step is the same, and a made biped walk is stiffer than a recorded one. They are the place to add craft (a head that leads a turn, a gallop's gathered and stretched phases held longer).
- The gallop is one kind (transverse). A rabbit gallops like a small deer; it does not hop.
- `die` leaves legs stiff. The rabbit's `eat` tips its ears more than its head: its head chain is read as the ears.
- A biped's `die` and `hurt`, and `attack` for animals, have been judged on sheets, not in play.
- The Tripo CLI drives the web app's private backend with a browser cookie that lasts about 30 days ([tools/tripo/README.md](../tools/tripo/README.md)). When builds fail with a login error, the cookie in `.env` needs replacing.
