# Model pipeline prior art: generate, rig, animate, validate (2026-11-09)

Scope: web research for a Tripo-driven, agent-run pipeline for a stylized (Eco-like) Rust/Bevy voxel sandbox.
Confidence labels: **[doc]** read in a primary doc this session, **[2nd]** secondary source or search summary only,
**[mem]** background knowledge not re-verified this session, **[unverified]** could not confirm.

## In plain words

1. **Tripo's API gives quadrupeds exactly one animation: walk.** The public preset list is 11 generic/biped clips plus one
   locomotion clip per creature type. There is no documented quadruped idle, run, attack or death, and no avian preset
   at all. Plan for this before generating any wildlife.
2. **So treat Tripo as "mesh + skeleton + weights", and own the animation yourself** for everything that is not a
   biped. Use Tripo presets for the player and humanoid monsters; drive animals with code (item 4).
3. **Always call rig-check first.** It is free, returns a recommended rig type, and is the cheapest gate in the
   pipeline. Rig costs 25 credits, each animation 10.
4. **Animate creatures in Rust per body plan, Veloren-style.** One small procedural animation module per plan
   (quadruped, bird, fish, snake) that sets bone rotations from sin waves and speed. It works on any model whose
   bones you can map to a short canonical list, and needs no clip library.
5. **Add a "bone map" step per model.** Whether Tripo emits the same bone names for every quadruped is
   **unverified**. Have the agent write a tiny sidecar file (canonical name -> actual bone name) and fail the asset if
   a required bone is missing. Bevy matches clips to bones by name path, so this map is what makes reuse possible.
6. **Keep skin weights for bodies, use rigid attachment for gear.** Smooth low-poly creatures (the Eco look) need
   skinning at joints. Helmets, pauldrons, weapons, backpacks should be rigid meshes parented to one bone: no weights,
   nothing to get wrong, and it is what Valheim/Vintage Story/Hytale-style games do for hard gear.
7. **One fixed player body, generated once and hand-checked.** Generate gear against that body (image-to-3D of the
   body wearing the item, or the item alone), then fit by bone-local bounding box in a script. Do not try to
   AI-generate skinned clothing that shares the skeleton; start with texture swaps + tint masks for soft clothing.
8. **Rigid-segment bodies are the fallback, not the default.** If a creature fails skinning QA twice, split it into
   parts (Tripo has segmentation endpoints) and parent each part to a bone. It is more robust but shows seams at
   joints unless the design hides them (overlapping ball joints, chunky proportions).
9. **Prompt for riggability**: T-pose or A-pose for bipeds, "standing, four legs apart, side view, neutral pose" for
   animals, limbs clearly separated, no capes/long hair/held items, plain background. Thin parts (fins, tails, wings)
   are where weights fail.
10. **Normalize every asset in one script before the engine sees it**: scale to a target height, feet on ground,
    pivot at bottom centre, forward axis fixed, one skin, merged clips with canonical names. Tripo's convert step
    already has `flatten_bottom`, `pivot_to_center_bottom`, `scale_factor`, `face_limit`, `animate_in_place`.
11. **Validate with cheap numeric checks first, pictures second.** Khronos validator + `gltf-transform inspect`, then
    scripted checks (bone count, weights sum to 1, no vertex bound only to root, bounding box per frame, foot height
    per frame). Only assets that pass get a VLM look.
12. **Render QA images in your own Bevy binary, not Blender.** It is the ground truth for what the player sees and
    avoids Blender headless GPU problems. Six fixed views for the static mesh; a 4x2 grid of evenly spaced frames from
    a side view per animation clip.
13. **Ask the VLM narrow yes/no questions with a fixed answer format**, never "does this look good?". Six RGB views are
    enough per published benchmarks; VLM judges still trail humans, so keep a human spot check for hero assets.
14. **Keep a failure log (symptom, cause, fix, body type) the agent reads before each run.** The one detailed
    first-hand account found says this log is what took a model from about 2 hours to about 10 minutes.

## Sources

| Source | Date | What it says |
|---|---|---|
| [Tripo rig docs](https://developers.tripo3d.com/en/models/rig) | 2026, undated page | **[doc]** `v3/animations/rig-check` (0 credits, returns recommended rig type), `v3/animations/rig` (25 credits). `rig_type`: biped, quadruped, hexapod, octopod, avian, serpentine, aquatic; `others` with v3.0. Models: `v1.0-20240301` (default, biped only), `v2.5-20260210` (creatures), `v3.0-20260909` (humanoid). `spec`: tripo, mixamo (+ actorcore, unity, unreal, vrm on v3.0), biped only. Out: glb/fbx. |
| [Tripo animation docs](https://developers.tripo3d.com/en/models/animation) | 2026, undated | **[doc]** `v3/animations/retarget`, 10 credits per clip, `animations` batch of up to 5, `export_with_geometry`, `animate_in_place`, 10 parallel tasks. Claims "100+ presets (incl. 90+ biped-exclusive)" but names only three. |
| [tripo-python-sdk models.py](https://github.com/VAST-AI-Research/tripo-python-sdk/blob/master/tripo3d/models.py) | fetched 2026-11-09 | **[doc]** The `Animation` enum: 16 values (listed below). `PostStyle`: lego, voxel, voronoi, minecraft. |
| [ComfyUI TripoRetargetNode docs](https://docs.comfy.org/built-in-nodes/TripoRetargetNode) | 2026 | **[doc]** Same 16 presets. `preset:biped:*` extras are for v1.0 rigs and are not enumerated. v2.5 rig accepts only: chop, climb, dive, fall, hurt, idle, jump, run, shoot, slash, turn, walk. Mixamo-spec v1.0 rigs fail retarget. |
| [Tripo v2->v3 migration skill](https://developers.tripo3d.com/assets/developer/tripo-v2-to-v3-migration/SKILL.md) | verified 2026-07 | **[doc]** Full v3 endpoint list incl. `/v3/mesh/segment`, `/v3/mesh/smartsegment`, `/v3/mesh/decimate`, `/v3/models/convert`, `/v3/models/stylize`. `model_version` is required on generation (400 otherwise). Generation versions `v3.1-20260211`, `v3.0-20250812`, `v2.5-20250123`, `P1-20260311`. `generate_parts` requires `texture:false`. Download URLs live about 24 h. No websocket; poll or webhook. |
| [Tripo ComfyUI nodes reference](https://developers.tripo3d.com/en/docs/comfyui-nodes.md) | 2026 | **[doc]** Convert options: flatten bottom, pivot to centre-bottom, scale factor, face limit, force symmetry, preserve animation, animate in place, texture size. Smart low poly face limit up to 20000. Stylize and Refine nodes are marked deprecated there (conflicts with migration skill listing `/v3/models/stylize`; treat stylize as unreliable). |
| [Meshy rigging API](https://docs.meshy.ai/en/api/rigging-and-animation), [animation API](https://docs.meshy.ai/en/api/animation) | 2026 | **[doc]** Rigging is humanoid only; non-humanoid and unclear-limb models unsupported; face must point +Z; max 380k tris; `height_meters` for scale; returns walk + run. Animation: up to 10 `action_ids` merged into one GLB as separate clips; `change_fps` post-process. |
| [Kossarev, AI-orchestrated 3D asset pipeline (DEV)](https://dev.to/aleksandr_kossarev_e23623/ai-orchestrated-3d-asset-pipeline-from-jpeg-to-game-ready-glb-without-touching-blender-1akf) | May 27, year not shown (likely 2026) | **[doc]** First-hand: Meshy -> agent-scripted Blender (BlenderMCP) -> Godot GLB for fish. Blender auto-weights failed on thin fins; root/utility bones absorbed weights; rest-pose and axis mismatches; clip leakage between models. Fix pattern: one action per step, screenshot + VLM "assert" with fixed answer format, forced camera angles, failure log. |
| [Fuwa Run dog game write-up](https://next-blog-9qg.pages.dev/en/blog/fuwa-run-2025) | 2025-04-14 | **[doc]** First-hand: generated dog mesh; Anything World quadruped auto-rig errored; rigged manually with Rigify. "Generation took moments; remesh, retopo, bake, rig took the time." |
| [UniRig paper](https://arxiv.org/abs/2504.12451) | 2025-04 | **[2nd]** Open-source autoregressive skeleton + skin weights for diverse shapes (same lab as Tripo). Later papers list missing bones as a failure mode. |
| [Rigel3D](https://arxiv.org/abs/2605.13129) | 2026-05-13 | **[doc, abstract]** Argues for generating geometry and rig jointly rather than attaching rigs after the fact. Research only. |
| [3D-DefectBench (Roblox)](https://arxiv.org/abs/2607.10826) | 2026-07-12, rev. 2026-10-06 | **[doc, abstract]** VLM judges for 9 binary defect types. Model choice matters most; six-view RGB is about as good as denser views or added depth/normals; best VLM still trails trained humans; binary judges catch most severe defects. |
| [Gen3DEval](https://arxiv.org/abs/2504.08125) | 2025-04 | **[2nd]** Up to 8 multi-view renders (RGB + normals) to a vision LLM; general GPT-4-class models are weaker at 3D quality than tuned ones. |
| [Veloren devblog 13](https://veloren.net/blog/devblog-13/) | 2019-04-29 | **[doc]** Per-category skeletons; each bone gets one rigid VOX model; animations are code: `next.l_hand.offset = Vec3::new(2.0 - wavecos * 2.5, 7.5, 12.0 + wave * 1.5);`. |
| [Veloren `Animation` trait](https://docs.veloren.net/veloren_voxygen_anim/trait.Animation.html) | current | **[2nd]** Each animation is a Rust type bound to a skeleton type; still code-driven. |
| [Hecker et al., Spore retargeting](https://chrishecker.com/Real-time_Motion_Retargeting_to_Highly_Varied_User-Created_Morphologies) | SIGGRAPH 2008 | **[2nd]** Animations authored against body-part roles ("grasper in front"), resolved at runtime with IK. Took the team weeks to learn which motions generalize. |
| [Bevy `AnimationTargetId` docs](https://dev-docs.bevyengine.org/bevy/animation/struct.AnimationTargetId.html) | current | **[2nd]** "Any animation is playable on any armature as long as the bone names match." IDs hash the name path from the root. |
| [glTF Transform CLI](https://gltf-transform.dev/cli) | current | **[doc]** `inspect`, `validate`, `merge`, `prune`, `dedup`, `resample`, `simplify`, `weld`, `center`, `flatten`, `join`, `palette`, `resize`, `meshopt`. |
| [Eco avatar dev post](https://devtrackers.gg/eco/p/8675f93a-new-avatars-in-eco-10) | Eco Update 10 era | **[2nd]** Avatars rebuilt on a Blender rig using Unity's UMA system; multiple tints per clothing asset. |
| [Jotunn (Valheim modding lib)](https://github.com/Valheim-Modding/Jotunn) | current | **[2nd]** Ships a "bone reorderer" because custom skinned armor deforms wrongly when its bone order differs from the player's. |
| [Hytale Blockbench plugin notes](https://cdn.jsdelivr.net/gh/JannisX11/blockbench-plugins@master/plugins/hytale_plugin/about.md) | 2026 | **[2nd]** Models are node trees of cubes/quads (max 255 nodes); attachments (hair, clothing, weapons) are separate pieces bound to named bones of the base model. |

## Tripo preset animation names (exact strings)

From the official Python SDK enum, matching the ComfyUI node docs:

- Generic / biped: `preset:idle`, `preset:walk`, `preset:run`, `preset:dive`, `preset:climb`, `preset:jump`,
  `preset:slash`, `preset:shoot`, `preset:hurt`, `preset:fall`, `preset:turn`
- Quadruped: `preset:quadruped:walk`
- Hexapod: `preset:hexapod:walk`
- Octopod: `preset:octopod:walk`
- Serpentine: `preset:serpentine:march`
- Aquatic: `preset:aquatic:march`
- Avian: none listed. `others`: none listed.
- `preset:biped:*`: 90+ further clips exist for v1.0 biped rigs; names are **not published** in any doc found
  (ComfyUI docs mention `chop` as accepted on v2.5 rigs, so `preset:biped:chop` or similar probably exists;
  **[unverified]**). `preset:biped:walk` appears as an example in the animation doc.
- **[unverified]** whether Tripo Studio's UI offers more creature clips than the API. Check the Studio UI by hand once.

## 1. How people run generate -> rig -> animate pipelines

- Tripo is the only mainstream hosted API found that documents non-biped rig types. Meshy's API docs say rigging is
  humanoid only (its marketing mentions quadrupeds; the API page does not). Mixamo is biped only and has no official
  API **[mem]**. AccuRig is humanoid, GUI only **[mem]**. Anything World supports quadrupeds but the one first-hand
  report found had it error out. UniRig is the open-source option if hosted rigging fails (needs a GPU).
- No independent test of Tripo quadruped weight quality was found. Everything positive about it is vendor text.
  **Budget a one-day spike**: rig 5 animals (deer, wolf, bear, rabbit, bird), apply walk, and inspect.
- Doc inconsistencies to expect: rig model versions differ between the rig page (v3.0 exists) and the migration skill
  (v1.0 default, v2.5); `bake_animation` is ignored in v3; stylize is listed in one doc and deprecated in another.
  Pin every `model_version` explicitly in the CLI.
- Tripo post-styles (lego, voxel, voronoi, minecraft, plus keyring/fridge_magnet/keycap) are novelty conversions, not
  an "Eco look" switch. The stylized look has to come from the prompt or, better, a consistent concept image fed to
  image-to-model (`style_image` also exists). Tripo's own guides recommend stating T-pose/A-pose in the prompt
  **[2nd, vendor]**. No vendor guidance on "low poly stylized" wording was found; `smart_low_poly` and the P1/P2
  low-poly models with `face_limit` are the documented controls.
- Retarget returns clips in batches of up to 5; whether each batch is one GLB with several clips or one per clip is
  **[unverified]**. Either way plan a merge step (`gltf-transform merge` or a small script) that strips duplicate
  geometry (`export_with_geometry:false`) and renames clips to canonical names.
- Reported failure modes (first-hand accounts above plus vendor prep guides): weights failing on thin geometry;
  a root or helper bone taking most weights; limbs fused to the torso in the generated mesh so no rig can separate
  them; non-identity rest pose; axis and scale differences between tool and engine; clips referring to bones that do
  not exist in the target; dense unorganized triangle soup hurting deformation. Feet sliding is a known retarget
  artifact **[mem]**; the practical fix is `animate_in_place` plus driving playback speed from actual move speed.
- Automatic checks people use: structural asserts in scripts, then screenshot + VLM with fixed-format answers after
  each step (Kossarev). No published, tested checker for skin weight quality was found; the per-bone "rotate it and
  see what moves" test is the closest and is easy to script numerically (rotate bone 30 degrees, measure which
  vertices moved, compare to the bone's expected region).

## 2. Player customization on a shared skeleton

| Approach | Used by | Fit for AI-generated assets |
|---|---|---|
| Rigid mesh parented to one bone | Hytale attachments **[2nd]**, Vintage Story wearables via `stepParentName` **[2nd]**, Veloren armor pieces as per-bone voxel models **[mem]**, weapons everywhere | Best. No weights. Fit = transform in bone space. |
| Swap whole body segment | Veloren, Cube World **[mem]** | Good if the body is already segmented. |
| Skinned modular mesh sharing the skeleton | Valheim, Eco (UMA) **[2nd]** | Fragile: bone order/name/bind pose must match exactly (Jotunn needs a fixer even for human-made mods). |
| Texture swap + tint masks | Eco (multi-tint), Valheim skin/hair colour | Easy and safe; gives skins and cloth variety. |

- Recommendation: fixed base body (skinned, one skeleton, bone names frozen). Skins = texture variants and tint
  masks. Armor = rigid pieces on head, chest, upper arms, forearms, thighs, shins, plus body-part hiding flags so the
  body does not poke through (Valheim mods and Vintage Story `disableElements` both do hiding **[2nd]**).
- Making generated gear fit: generate each piece alone on a plain background, then in a script (a) compute the
  bone-local bounding box of the body region it covers, (b) scale and translate the piece to that box plus a small
  margin, (c) render front/side with the body and let the VLM answer "does skin show through? is it floating?".
  Store the final transform per piece in a data file so a human can nudge it. This procedure is a design
  suggestion, not something found documented elsewhere.
- Chest armor is the hard case because the spine bends. Chunky, short torso pieces attached to one chest bone hide it;
  stylized games accept this.

## 3. Animating many creature kinds cheaply

- **Clip library per body plan**: works only if every model of a plan has the same bone names and similar rest pose.
  Bevy supports name-path matching but has no built-in retargeting for differing proportions. Tripo skeleton
  consistency across models is **[unverified]**; test in the spike. With only one creature preset per plan from
  Tripo, a clip library would have to be hand-made or bought anyway.
- **Code-driven animation (recommended for wildlife)**: Veloren has shipped this in Rust for years across many body
  plans; each animation is a function of time, velocity and state writing bone transforms. Cost is one module per
  body plan (idle, walk, run, attack, hurt, death as parameterized functions), shared by every species through the
  bone map. Add two-bone IK for foot placement later if wanted. Spore shows the same idea at full generality but took
  a specialist team; Rain World and Overgrowth are well-known talks on procedural animation layered over few
  keyframes **[mem]**. Stay at the Veloren level of ambition.
- **Rigid segments vs skin weights**: no source found that tests this on AI-generated meshes, so this is reasoning
  plus analogy. For: Veloren, Hytale, Vintage Story, Minecraft all animate rigid parts with no weights, and weights
  are the step with the most reported failures. Against: those games design models as separate blocks; an AI mesh is
  one fused surface, so cutting it leaves open holes and visible seams when joints bend, and segmentation is itself
  an AI step that can fail. Tripo offers `/v3/mesh/segment`, `/v3/mesh/smartsegment`, `/v3/mesh/complete`
  (hole fill) and `generate_parts` (needs `texture:false`), so the route exists. Verdict: keep as fallback and as the
  approach for gear; the code-driven animation layer is identical for both, so the choice can be made per asset.

## 4. Scriptable glTF tooling

- **glTF Transform** (CLI + JS API): the default tool. `inspect` gives a compact table of meshes, skins, animations
  and sizes (good for agent context), `merge` to combine files, `resample`/`prune`/`dedup` for cleanup, `simplify`,
  `resize`. Use the JS API for custom steps (rename bones/clips, check weights). Verify `merge` behaviour on skins
  with a real file; the docs do not spell it out.
- **Khronos glTF-Validator** **[mem]**: `gltf_validator -o file.glb` gives JSON errors/warnings; gate on zero errors.
- **gltfpack / meshoptimizer** **[mem]**: one-shot simplify + compress with skin and animation support. Run last,
  and only if Bevy is built to read the chosen compression; otherwise skip.
- **Blender headless (`blender -b -P script.py`)**: needed only for jobs glTF Transform cannot do (re-weighting,
  splitting meshes by bone, re-baking). EEVEE/Workbench headless without a GPU or display has been unsupported or
  needs xvfb + software GL, and the engine identifier changed across 4.2 to 5.0 **[2nd]**; Cycles on CPU works
  headless. Clean the scene between models (clip/bone leakage is a reported bug source).
- **Contact sheets**: render in a small headless Bevy binary (same shaders and import path as the game), fixed
  cameras, write PNGs, tile them with ImageMagick `montage` **[mem]**. One sheet per clip, 8 frames, side view,
  ground line and a 1 m grid visible so sliding, sinking and scale errors can be seen. Also emit numbers next to the
  image (min foot height per frame, bounding box per frame) so the VLM is not asked to measure.
- **VLM QA advice from the sources**: six views is enough; ask binary, defect-specific questions; force known camera
  angles; do not ask the model to count; use the same judge model consistently; expect it to miss subtle texture
  problems; treat the judge as a filter that removes obvious failures, not as approval.

## Open items to verify by hand

1. Tripo quadruped/avian rig quality and bone-name consistency on 5 test animals.
2. Whether Studio exposes more non-biped presets than the 5 in the API, and the full `preset:biped:*` list.
3. Output shape of a batched retarget (one GLB or several) and whether `export_with_geometry:false` output merges
   cleanly onto the rigged GLB.
4. Whether the `others` rig type on v3.0 produces usable skeletons for odd shapes.
