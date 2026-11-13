# Creatures built and animated wholly in code: what another project does

Source: https://github.com/mitsuhiko/dungeon-crawler/tree/828080efca5c08808f8c32d8c4db2585d6c15755/blender (`creature.py`, `ik.py`, `moose.py`), read 2026-11-12 with the author's permission. Blender scripts: each creature's mesh, skeleton, skin and every clip are made by a Python file and baked to plain keys.

What it does that bears on ours:

| Theirs | Ours | Status |
|---|---|---|
| A limb that touches something is posed backwards from the point it must reach (two-bone, by the law of cosines); angles alone let feet skate | `cube_anim::two_bone`, feet planted in the world | same |
| `step(phase, duty, stride, lift)`: a foot is down for `duty` of the loop and slides back at the body's speed, then lifts and swings | `biped` (stride clock, duty 0.62 walking, 0.35 running) | same idea, arrived at separately |
| A walk sets down one foot at a time (hind left 0, fore right 0.25, hind right 0.5, fore left 0.75; duty 0.62); a run is diagonal pairs with duty 0.32 | `Walker` trots at every speed below a gallop | **to do**: a true walk for four legs |
| Soft reach: the target is eased in as a limb nears full stretch, so a knee does not snap straight | reach capped at 98.5% of the leg | **to try** if a knee pops |
| The bend side of a knee is kept from flipping between frames by blending toward a fallback as the knee nears the straight line | fixed pole per leg | not needed yet |
| Every clip is lifted, frame by frame, until no skin is under the ground; a death is let down onto it | `check::sunk` for carried clips, `check::off_floor` for `die` | same; could cover every clip |
| Head and neck move against the body; the tail sways with a lag along its bones; idle breathes | `Own::lively`, `biped` idle | adopted 2026-11-12 |
| Skin weights: soft by distance to each part, then smoothed over the mesh (6 passes), top four | by distance only | tried 2026-11-12 on the wolf: stretch went from 66% to 111% in two passes, taken out. Their bodies are smooth blobs made for it; Tripo's mesh has a thin leg beside a thick trunk |
| A validator: weights sum to 1, nothing under the ground, a loop closes | `cargo forge check` | same |
| Review: a contact sheet and side-by-side clips | `cargo forge sheet`, `--strip`, `make beside` | same |
