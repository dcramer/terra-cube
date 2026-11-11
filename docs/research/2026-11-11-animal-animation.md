# Where good animation comes from (2026-11-11)

The question: the creatures' motion was called bad twice by the one person who plays the game,
while every forge check passed. What is the source of a clip that looks right?

## What was found, each by one sheet set side by side

- **Tripo's human presets are right as delivered.** Its `walk` and `run`, untouched, are a person
  walking and running. What the game played was not that: forge locked the feet, stretched the
  stride, and replaced the walk with a made one because its `slide` measure read 88% on the
  recorded clip. The measure was wrong for a person (an ankle does not move at one speed
  while its foot is down). Fixed the same day: recorded gaits are kept as recorded
  ([pipeline.md](../pipeline.md#gaits)).
- **Putting other capture on Tripo's skeleton was worse.** CMU takes carried over in Blender
  by turn-from-rest came out with the trunk twisted and leaning back. It passed every check.
  Dropped.
- **Tripo's other human presets:** `fall` is a good death (used); `slash` an attack (used,
  trimmed); `dive` a roll and `climb` a vault (not used yet); `jump` is skipping on the spot
  and `hurt` is sitting wounded (no use).
- **Tripo's motions from a description are good for a person, some of them.** A jump and a
  flinch came back as a person would do them and are used; a breaststroke came back lying
  still. 20 credits each, once: the asset goes on any humanoid free. Swim is still made.
- **Tripo has one animal clip, and it does not travel.** `quadruped:walk` on the wolf lifts
  its legs well but its feet cover 0.14 to 0.27 m/s: marching on the spot.
- **Our made animal gaits are stiff, and tuning them did not help.** A longer stride with more
  back, head and tail motion changed the bear and little else; most animals have their stride
  cut per creature (`gait_gain` 0.4 on wolf and deer) because Tripo's animal skinning tears
  at full reach. The limit is the rig, not the numbers.
- **An animal animated by hand is plainly better.** Quaternius's wolf (CC0, 1,962 triangles,
  51 joints; clips Walk, Gallop, Gallop_Jump, Attack, Death, Eating, Idle, Idle_2,
  Idle_2_HeadLow, Idle_HitReact_Left/Right, Jump_ToIdle) drawn in our engine beside ours:
  full strides, a gallop that gathers and reaches, a lunge, a fall, a head-down feed. Its
  look is plainer than Tripo's (flat colours, no texture).

## What it would take to use the hand-animated animals

Tried as a trial creature (the changes are kept in `.work/wolf-authored/trial`):

- `Skeleton::read` must take a skeleton with helper bones beside the body's (IK targets): the
  root is the one with the most under it. Done in the trial, six lines.
- Roles cannot be read from this skeleton's shape (the reader took an ear for the head and two
  hind legs for all the legs, and so got the size wrong). These rigs have honest names, so a
  plan for them would name its bones; the rule against matching names stays for Tripo's.
- It has no trot: `run` would be the walk quickened or the gallop slowed, or made.
- The look: either the pack's own flat colours, or its motion carried to Tripo's models,
  which is the retargeting that failed for people and is harder for animals.

## The lesson for the pipeline

A number that passes is not a clip that looks right. Before anything is done to a clip, and
after, it is drawn from the side at eight moments beside what it came from, large enough to
see a spine (`cargo sandbox --shot ID --clips walk,run --cols 8 --view side`).

## Sources

- Quaternius, Ultimate Animated Animals, CC0: https://quaternius.com/packs/ultimateanimatedanimals.html
- The wolf used in the trial, CC0: https://poly.pizza/m/P1gU3Qkr9r
- CMU Graphics Lab Motion Capture Database: http://mocap.cs.cmu.edu/
