# Is there open work that has solved models and animation? (2026-11-12)

The developer, after the jump and the gaits still read as not quite natural: "someone must
have open sourced some work that they have solved animations and model generation?"

Not as one thing. It is three problems, solved to different degrees. Found by search on
2026-11-12; none of it was run here, and each licence wants reading at its source before use.

## Making the model: solved well enough

Tripo (what forge uses), and open models of the same kind. Not our problem.

## Rigging it: solved for people, not for animals

- **UniRig** (VAST, who make Tripo; MIT code): skeletons and skin for anything. Probably what
  Tripo's own rigging is. The public weights are said to be weaker than the paper's.
  https://github.com/VAST-AI-Research/UniRig
- **Puppeteer** (Apache-2.0 per its repository): rigs, then animates from a video.
  https://github.com/Seed3D/Puppeteer

Our own finding stands: a person comes back on Mixamo's skeleton and is dependable; an
animal's rig shares no names and is read by shape (`rig.rs`).

## Moving it: three kinds of open work, none of them a whole answer

- **Clips a person made, carried over.** What forge does since today (`library.rs`):
  Quaternius's libraries, CC0. The motion is right because a person drew it; what is not
  natural is everything between clips.
- **Motion from a description.** NVIDIA's Kimodo (March 2026; code Apache-2.0, weights under
  NVIDIA's own licences, which differ by checkpoint) and ARDY (July 2026, streams frame by
  frame); Tencent's HY-Motion. People only, on their own skeletons (SMPL-X and the like), so
  the result is carried over as a library clip is. Tripo's own described motions are this
  kind: single actions came back well, cycles and lying poses did not (pipeline.md).
  https://arxiv.org/pdf/2603.15546
- **Choosing the next frame while playing (motion matching).** What makes a shipped game's
  character feel natural: no clips and no state machine, the next pose is looked up in hours
  of capture by where the body is going. Open implementations exist; the data they are
  shown with (Ubisoft's LAFAN1) is non-commercial and no-derivatives, and nothing of the
  kind was found for animals. https://github.com/ubisoft/ubisoft-laforge-animation-dataset

## What this says for us

The clips are no longer the weak part. What still reads as unnatural is how they are played:
one clip at a time with a fifth of a second's fade, no starting or stopping, no turning on
the spot, a gait's speed set by playing it faster, a leap cut from three pieces. That is
ours to write, with the clips we have: blend walk into run by speed, play a start and a stop,
lean into a turn. The library has more to take (Quaternius's second library adds starts,
stops, turns and combat). A model that writes motion is worth a trial for one-off actions
the library lacks, not for gaits.

## Resting clips for animals: none found free (2026-11-12)

Looked for a lying, sleeping, sitting or curled clip to carry onto the animals whose legs
will not fold (fox, lynx, deer, rabbit, hedgehog). All twelve animals of Quaternius's
Animated Animal Pack (CC0, https://poly.pizza/bundle/Animated-Animal-Pack-ILAPXeUYiS) were
downloaded and their clips listed: every one has Attack (or Attack_Headbutt and Attack_Kick),
Death, Eating, Gallop, Gallop_Jump, Idle, Idle_2, Idle_Headlow, two hit reactions, a jump
and Walk. None lies, sleeps or sits. What search found with such clips is sold (a fox with
Sit and Sleep, a forest kit with Sleep, about $10 to $15 each on CGTrader) or wants its
author named (CC-BY). Of use from the pack and not yet taken: the hoofed animals' headbutt
and kick, for a deer, goat or pony that strikes.
