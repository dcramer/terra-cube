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
- **How it lives** is a `[life]` table added to `creature.toml` by hand: `temper` (`timid`, `hunter`, `wary`, `hostile`; see [engine.md](engine.md#what-lives-in-it)), `habitat` (`meadow`, `forest`, `rock`, `shore`, `snow`), `group` (how many keep together) and `groups` (how many such groups a world starts with). Without it the creature can be spawned in the sandbox but no world has any. `cargo world run` shows what the new creature does.
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
- **A recorded gait can be given a longer stride** (`[plans.biped.stride]`: run 1.25, sprint 1.12): its feet are locked to a line that much faster, so they travel further while down at the cadence recorded. Tripo's run is a 3.2 m/s jog; this makes it 4.0, and the sprint 5.6. Making a gait faster by only quickening it spins the legs without covering ground, which is how the first sprint read.
- **Two legs, where that is not enough: made.** A preset is fitted to other proportions, and on a short or bulky body the feet wander too much to lock (the troll's walk slid at 140%, the goblin's at 111%). A recorded gait still over the bar after locking is replaced by a made stride: the same planted-foot generator on two legs, carried on the ankles with the feet flat, the arms swinging against the legs and the trunk leaning in. Today that is every biped's walk and all the troll's gaits.
- **`meta.toml` speeds are the speeds the game moves at**, so a clip always plays at its own rate. To make a creature faster, change the gait, not a number in the game.
- `gait_gain` in `creature.toml` (under 1) shortens stride, lift, rock and arm swing for a body whose skin tears at full reach: bear 0.6, boar 0.7, deer 0.4, troll 0.6.

## The roster, and two plans that do not work yet

2026-11-10: 36 creatures besides the player are wanted first (100 in the end). The 29 new ones have a `creature.toml` and a concept each (58 credits); `cargo forge status` lists them. The 13 four-legged and 3 two-legged ones use plans that work and can be bought as they are. The bird and crawler plans were tried on one creature each before buying the rest, and neither works:

- **`bird` (the crow, built, failing):** Tripo's rig for it reads as one leg, with a wing and the spine taken for arms; clips made on that tear the skin by up to 560%. The role reader needs to know a bird: two legs under a body that lies level, and wings that are not arms.
- **`crawler` (the crab, rigged, not finished):** six legs are found of its eight and its claws, and making its run crashes forge (`quicker` is handed a clip with no frames: there is no gait for more than four legs).

Until those are fixed, no more birds or crawlers are bought, and the crow is kept out of the world (`groups = 0`). Nothing flies in the simulation either: a bird that worked would still only walk.

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

## Trying a look

`cargo forge looks` draws the creatures named in `assets/creatures/looks.toml` in each look listed there and lays them out as `shots/looks.jpg`, a row a look under the house style's own row. A look is the text that stands where the house style does in a concept prompt. Only concept images are made, 2 credits each, once: they are kept under `assets/creatures/looks/`. A look is judged for more than how it draws: limbs of natural length and set apart (so the rig and clips fit), a biped in a true T-pose, an animal square on and solid (a flat drawing makes a poor model). The one that wins is copied into `style.toml` by hand, and one creature is built in it and held to the checks before the rest are bought.

**Building a creature in a look on trial:** a `creature.toml` with `look = "painted"` takes that look's style in its prompt, and with `look_of = "player"` its concept is the picture `cargo forge looks` already drew of that creature, so the model is made from the image that was chosen and it is not paid for twice. Then judge it as any creature: `cargo forge check`, and `make gif ID=…` for a person to watch.

Built 2026-11-10 (65 credits each): `player-carved` fails skin stretch in every clip (101% to 126% against a bar of 80%: squared limbs crease at every joint) and `wolf-carved` in five; its flat browns were also the least liked. **Carved is dropped.** `player-painted` passes everything and is the only biped whose recorded walk held its feet well enough to keep. `wolf-painted` passes once its stride is shortened (`gait_gain = 0.55`). Its concept was a three-quarter view and the model came back standing at 45° to the axes, walking sideways: forge now reads a diagonal facing from the way a four-legged body's feet are spread, and measures length along it. House and painted were the two left, and **the house style was chosen** (2026-11-10): it is what the eight creatures are already in, so nothing is bought again. The trial creatures (`player-carved`, `player-painted`, `wolf-carved`, `wolf-painted`, and `adventurer` from the cubes trial) were taken out of `assets/creatures/` afterwards; they are in git history at commit `0b541c3` with their locks, should one be wanted back.

Tried 2026-11-10 on the player, wolf and troll (18 credits): `carved` (squared, chamfered block shapes: the nearest to the world), `flat` (strict flat-shaded low-poly; its wolf came back a flat drawing and its troll out of the T-pose) and `painted` (hand-painted and smooth, as Eco; its troll also out of the T-pose).

## Cubes

**Parked 2026-11-10: the game keeps its smooth models** ([engine.md](engine.md#creatures) says why). What follows is kept so the trial need not be repeated.

`cargo forge cubes ID` rebuilds a finished model from 5 cm cubes, the size of a bit of the world, as `assets/creatures/ID-cubes/` with the same skeleton and clips. Its output is not committed. This section is the routine for getting a good one, written after getting several bad ones. Why it is built this way: [research/2026-11-10-cube-characters.md](research/2026-11-10-cube-characters.md).

### The routine

1. **Is it worth converting?** A figure wants to be at least 10 cubes tall (half a metre) with limbs two cubes thick (10 cm). The rabbit is neither. Smaller things are drawn for the grid by hand or left smooth; no setting rescues them.
2. **Write the subject for cubes** before any credits are spent, if the creature is new or being rebuilt. In the subject or the plan's pose: thick straight arms and legs; legs set apart with a clear gap; mitten hands, no separate fingers; big plain areas of one colour; no fine detail (straps, laces, fur tufts, thin horns); hair as one solid mass that leaves the face clear; nothing hanging over the top of the legs. Look at the concept (2 credits) for exactly those things before paying for a model (65).
3. **Run it:** `cargo forge cubes ID`. One line, `ok` or the failing checks; `FORGE_VERBOSE=1` shows every number. It exits 1 on a failed check.
4. **Fix by the table below, not by eye.** Change the code or the source, run again.
5. **Then look once:** `cargo sandbox --scene shots/ID.png --spawn ID-cubes --dist 4 --yaw 0.8` (a lone creature is framed, stood on the ground, at about the distance it is seen in play), and `cargo forge sheet ID-cubes` for the clips. Judge it at that distance: a close-up makes any cube figure look crude.
6. **A defect seen that no check caught becomes a check** in `judge` (cubes.rs) with a bar in `style.toml` `[cubes]`, before it is fixed.

| FAIL | It means | Do |
|---|---|---|
| `tall` | Too small to be made of 5 cm cubes | Leave it smooth, or draw it for the grid |
| `triangles` | Too many faces: a big body | Bigger cubes for it (`--size 0.1`), until flat faces are merged |
| `legs-apart` | Two legs touch somewhere | The source stands with its legs together or crossed: ask for legs apart; check the idle's first frame |
| `limbs` | An arm or leg is one cube thick | Ask for thicker limbs; a deer's shins cannot be saved by settings |
| `loose` | Fingers, wisps of hair, crumbs of cloth | Ask for mitten hands and solid hair; it is the source's fine detail |
| `speckle` | One-cube spots of colour | The texture is busy: ask for plain colour areas |
| `face` | No room for eyes, nose and mouth | A bigger head (`--head 1.4`), or hair that leaves the face clear |

| `limbs-shown` | Most of an arm or leg piece is inside another part | A clip made for a slim figure sinks a stout one's arms in its sides; arms are moved out of the trunk, so what is left is a limb folded into itself |
| `face-framed` | The face is the colour of the hair round it | The skin colour was read off hair: look at where the hair falls |

**The first model made to these rules** (`adventurer`, 67 credits, 2026-11-10): the concept came back as asked but for fingers where mittens were wanted. Stood still and walking it is the first cube figure that reads right, with a clear face. It was also the one that showed the checks were not enough: it passed them all with a hair-coloured face and no arms to be seen, which is where the two checks above came from. It still fails `limbs-shown`, and in its run and attack the arms cut through the chest: a part turns about its bone's joint, which is no longer where the cubes' joint was put. As a smooth model it fails the stretch check in every clip; a stout body on the slim skeleton's clips is a problem either way.

Where the eight stand, 2026-11-10: player and wolf pass. Goblin fails `loose` and `speckle` (ragged cloth, busy skin). Deer fails `limbs`. Bear, boar and troll fail `triangles`. Rabbit fails nearly everything, being 8 cubes tall.

### How it is cut

- **A dozen parts, not every bone.** Games that draw characters in cubes build them from a few clean rigid pieces; cutting ours bone by bone (46 of the player's 65) left ragged seams and fused the legs into one mass, at any cube size. So the figure is cut into hips, chest, head and each limb in up to three pieces, read from the skeleton's roles: 15 parts for the player. Fingers, toes, ears and collar bones go with what they hang from.
- **Each part is cut on its own.** The model is stood as in its idle; a part within 25° of an axis is straightened onto it (a leg that leans stands straight, a wolf's sloping neck is left); its surface is sampled into the 5 cm grid and its inside filled. Then each pair of legs is moved apart a cube at a time until there is a column of air between them. A cell two parts both fill goes to the one nearer the root. A cube is fixed to its part's bone and never bends.
- **Each part is tidied**, as someone drawing in cubes would: a cube holding on by one face or none is taken off, twice over; a cube whose colour no neighbour shares takes the colour most of them have.
- **A cube takes the colour most of the texture under it has**, not the average, which is a colour the model has nowhere.
- **Faces are flat and placed.** A head of true size is four cubes wide and an eye is smaller than a cube, so a face copied from the model is a smear. For a creature with arms the front of the head is made one plane of one skin colour (the commonest on the bottom third of the head's front; hair can be the top half), from the chin up to the highest row that is nearly half skin. On it go single cubes: two eyes on the top row either side of the middle, a nose under them one cube proud, a mouth lower. An animal keeps its muzzle and has its eyes darkened where the texture is near black.
- **Not done:** flat faces are not merged (so big bodies fail `triangles`), parts pass through each other at joints and gaps open there, and nothing checks the figure in motion.

## Known limits

- Made gaits are regular: every step is the same, and a made biped walk is stiffer than a recorded one. They are the place to add craft (a head that leads a turn, a gallop's gathered and stretched phases held longer).
- The gallop is one kind (transverse). A rabbit gallops like a small deer; it does not hop.
- `die` leaves legs stiff. The rabbit's `eat` tips its ears more than its head: its head chain is read as the ears.
- A biped's `die` and `hurt`, and `attack` for animals, have been judged on sheets, not in play.
- The Tripo CLI drives the web app's private backend with a browser cookie that lasts about 30 days ([tools/tripo/README.md](../tools/tripo/README.md)). When builds fail with a login error, the cookie in `.env` needs replacing.
