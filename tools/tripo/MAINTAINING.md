# Keeping the Tripo CLI current

The CLI talks to Tripo Studio's private backend, the one its web app uses, so it spends our subscription's credits rather than API credits. Nothing documents that backend, and Tripo changes it when they ship the app. This explains how the requests were found, how to check them against the app today, and what to change when they drift.

The rule is simple: **the CLI sends what the app sends**, with the same model versions and the same defaults, except where this file or the README says otherwise and why.

## How the requests were found

- **Captured from the web app** (2026-10-11). A browser signed in as us drove the app, image to views to model, then rig, animate and export, recording every request and reply. Details and pitfalls are in [research/tripo-studio-api.md](../../research/tripo-studio-api.md).
- **Read from the app's JavaScript.** The production bundle names every endpoint and builds each request body in the open, so operations we had not run could be read rather than guessed. This is now how we check the CLI, with `bundle.ts`.
- **Checked against other people's clients**, listed in the research note. Where they disagreed, what the app sends won.
- **Run.** Every command was run against the live service before it was called done.

## Checking the CLI against the app

The check needs no login. Start with the drift report:

1. **Find the app's entry script.** Open <https://studio.tripo3d.ai> in any browser and view the page source. Find `<script type="module" src="https://tripo-webapp-assets.tripo3d.ai/studio-prod/_nuxt/XXXX.js">`; `XXXX.js` is the entry. The page sits behind Cloudflare's challenge, so `curl` gets a 403, but a browser is enough. With agent-browser:

   ```bash
   agent-browser --headed open https://studio.tripo3d.ai/ && agent-browser eval 'document.querySelector("script[type=module][src]").src'
   ```

2. **Run the report.**

   ```bash
   node tools/tripo/bundle.ts XXXX.js
   ```

   It downloads every script the app can load (about 180, from an open CDN, cached under the system temp folder by build id). It prints:
   - every model-version enum in the app;
   - the versions and endpoints the CLI sends that the app no longer has (it exits 1 if there are any);
   - the endpoints the app has that the CLI does not call.

3. **Read a request's body** when something changed or is new. Find the endpoint, then the call that builds its body:

   ```bash
   node tools/tripo/bundle.ts XXXX.js --grep 'operation/rigging_model`'
   node tools/tripo/bundle.ts XXXX.js --grep 'Es\(\{' --around 600
   ```

   Minified names differ in each script and each build, so follow them by hand.

### Patterns in the minified code

| Pattern | What it is |
|---|---|
| `Es=async e=>H(\`/v2/studio/operation/rigging_model\`,{body:e,method:\`POST\`})` | The wrapper for one endpoint. Grep its local name (`Es(`) in the same script to find where the body is built. |
| `ow=function(e){return e.v1_0=\`v1.0-20240301\`,…,e}({})` | An enum. Model versions, rig types, skeletons and qualities are all enums. |
| `modelVersion:C(()=>B.v3_0)` | Which member the app actually sends. An enum lists old versions too, so find the use and don't assume the newest is sent. |
| `getHDDefaults`, `Ut={…}`, `P(\`quad\`),P(1e4)` | The panel defaults: what a user gets without touching anything. |
| `yw={AICompletion:0,…,Rigging:20,…}` | The fallback price list, in credits. |
| `e.trace?.blocked(X.PaywallSegmentation…)`, `toast.…` | The app's own checks before it sends anything, such as face limits, plan permissions and humanoid-only motions. |

When the code cannot settle a question, for example what a reply looks like, capture the app in use as described in "Capturing again" in the research note. A capture holds live tokens: never commit one.

## What to change when the app changes

1. Fix it where it lives: a version or name in `versions.ts`, a request's body or default in its command under `commands/`, a shared step in `steps.ts`, login, uploads or waiting in `studio.ts`. Change `requests.test.ts` to expect what the app sends now; it fails until the command sends it.
2. If it was a default that changed, change ours to match, unless it is listed under "Where we differ on purpose" below.
3. Run the command live once, free commands first, and look at what it made. A check that only reads code is not done.
4. Update the README's table, its credits and this file's "Last checked".
5. Run `node --test` in this folder (`cargo xtask check` runs it too), and note anything new learned about the backend in the research note.

### Where we differ on purpose

- A model's face limit is 10,000, where the app asks for one or two million. Our models go in a game.
- `image` makes one image, where the app makes four.
- `export` gives GLB where the app offers FBX for a quad model, and an FBX in one file (`texture_packaging: embedded`, as the app's Blender, Max and Maya bridges send) rather than the panel's zip.
- `rig` defaults to the `unreal` skeleton where the app defaults to `mixamo`: on our base body it is the only one whose twist bones carry skin.
- `--skeleton tripo` sends the V1 rig (`v1.0-20240301`), which the app no longer offers. Our character pipeline uses `unreal` (research/skin-deformation.md): measured with plain skinning, V1 had pinched less at the knees, but with the game's skinning `unreal` passes our rig check and V1's twist bones carry no skin. Check again when Tripo ships a new rig: `bundle.ts` lists the rig enum.

### Not yet standard: which way models face

Results are saved as Studio keeps them, facing +X, where glTF's standard is +Z forward and +Y up (as our skeleton faces). They should be standard; they are not yet because the character tools (`tools/characters`) turn every Tripo model 90° themselves and the art in `art/` was made that way, so the CLI and those change together, in one commit. Worked out and checked on 2026-10-12, to do then:

1. **CLI:** save every result through `exportFile(…, "glb", rigged, [], false, textureSize(op))` in `steps.ts`'s `finish`, which faces +Z (`export_orientation: "-y"`, the only value the app sends), plain, with the skeleton if rigged; leave `download` as Studio's own file. Give `import` `transform_matrix: [0, 0, -1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1]` (column-major: +Z to +X about +Y) instead of the identity. A standard GLB imported with it and exported came back joint for joint as it went in; with the identity it came back turned 90°.
2. **Character tools:** drop the turn about Z in `convert_rig.py`, `morph.py`, `rig_base.py`, `shell_garment.py` and `paint_skin.py` (both ways), add 90 to each piece's turn in `fit_gear.py`'s `PIECES`, and correct the note in `compare.py`.
3. **Art:** re-save the 14 Tripo files in `art/` standard through Studio itself: import each as it is (identity matrix), export it as GLB at its own texture size (one is 4096) with its skeleton, delete the project. A round trip like that kept vertices, triangles, texture and skeleton exactly.
4. **Check:** `build_bodies.sh` before and after; the game's character files should match to rounding.

## Last checked

2026-10-12, against build `d87d2926-d393-4b74-a71f-390dba757862`.

- **Versions:** every model version we send is one the app sends: H3.1 `v3.1-20260211`, Smart Mesh `Nexus-v1.0-20260214` and `Nexus-v2.0-20260801`, rig `v3.0-20260909`, texture upscale and PBR `v3.0-20250812`, segmentation `v2.0-20260430`, AI completion `v1.0-20250506`. Every endpoint we call is still in the app.
- **Defaults brought in line with the app:**
  - H3.1 now defaults to detailed geometry, a 4K texture and PBR (55 credits rather than 25).
  - `texture` defaults to 4K.
  - `retopo` defaults to quads at 10,000.
  - A Smart Mesh is textured at 4K with delighting.
  - P2 is quads at 5,000 faces.
  - `image` defaults to Nano Banana 2 and keeps a reference's aspect ratio unless one is given.
  - `export` keeps the model's own texture size.
- **Behaviour now matching the app:**
  - `rig` takes `--type other` for animals and creatures, and `--anyway`.
  - A failed symmetry check counts as "not symmetric".
  - Generated motions are refused on non-humanoid rigs before spending.
  - `import` keeps UVs only when the file has them.
  - `uv` waits for an unwrap already running.
- **Added from the app:** `model --variations` (P2), `edit` (Mesh Edit), `unanimate`, `cancel`, `restore`, `fork`, `rename`, `delete`, and `show` now lists the project's steps.
- **Run live, and working:** `show`, `animate`, `unanimate`, `rename`, `fork`, `restore`, `image` (with and without a reference), `import`, `rig` with V3 Mixamo and with `--type other` (a wolf, rigged as a quadruped), `model` at the new defaults (charged 55, as the README says), `model --model p2 --variations 2`, `edit`, `cancel` (nothing charged, no step left), `export` of a rigged model, and `delete`.
- **Found by running:**
  - `export` left a rigged model's skeleton out; the app keeps it (`with_animation` is true for any rigged model), and so does the CLI now.
  - Studio keeps a quad Smart Mesh as FBX, which the CLI had saved under a `.glb` name and could not read parts from. It now saves it as `.fbx`, and reads parts and faces through a GLB export. An export faces the standard glTF way while Studio keeps models facing +X, which our character tools turn themselves, so results stay as Studio keeps them.
  - P2 models and Mesh Edit were not charged on our plan this time, and the CLI reported 0 for them, which the balance confirmed.
- **Not settled:** where `edit`'s box lands. Both runs remade their region almost unchanged, so the result does not show it; the app's viewer starts the box at the model's own bounds, which suggests the model's coordinates.
- **Left out on purpose:**
  - Magic Brush (`retexture_generate`), which needs a render of the app's viewport.
  - Hand-painted segmentation (`modification`) and the workspace's metadata (`meta_modification`).
  - Gallery, team, billing, survey and marketing endpoints.
  - Batch export and the image tools `upscale`, `split` and `re_gen_*`. Add them when they are needed.
