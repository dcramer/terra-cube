# Tripo CLI

Makes, textures, rigs and animates 3D models with [Tripo](https://studio.tripo3d.ai), spending our Studio subscription's credits rather than API credits. It does from the command line what the Studio web app does, so a person or an agent can script it. TypeScript, run directly by Node 22.18 or later, with no dependencies and no build step.

## Setup

The CLI logs in as you, with the session cookie of a browser signed in to Studio. It lasts about 30 days.

1. Sign in at <https://studio.tripo3d.ai>.
2. Open developer tools, then **Application → Cookies → https://studio.tripo3d.ai**, and copy the value of `ory_kratos_session`.
3. Put it in `.env` at the repository root (worktrees read the main checkout's): `TRIPO_STUDIO_COOKIE=<value>`. The whole `cookie` header of any request to `api.tripo3d.ai` works too.

Check it with `node tools/tripo/tripo.ts balance`. When it says the login failed, copy the cookie again.

## Use

```bash
tripo=tools/tripo/tripo.ts

node $tripo image "a weathered supply crate, plain grey background" -o crate.png
node $tripo model crate.png -o crate.glb              # prints {"project": "…", "credits": 25, …}
node $tripo rig <project>                              # a humanoid, in a T- or A-pose
node $tripo animate <project> walk run idle -o crate-anim.glb
```

Every command that makes or changes a model prints one line of JSON with the `project` it worked on, the `credits` it spent and the `files` it wrote, so the next command can take the project. A project is the id or the project's Studio URL. A project keeps every step done to it; each command works on its latest. `node $tripo <command> --help` lists a command's options.

| Command | What it does | Credits |
|---|---|---|
| `balance` | Credits left | |
| `list` | Your projects, newest first | |
| `show PROJECT` | What has been done to a project: textured, rigged, its animations, and its steps, newest first | |
| `image PROMPT [--ref IMG …]` | Images from a prompt, optionally like up to ten reference images. `--t-pose` for a figure to rig, `--count`, `--aspect` (with references, theirs unless given), `--4k`, `--model` (default `nano-banana-2`, as the app's) | free (100 a month), then 10 to 20 by model |
| `views IMAGE` | Front, left, back and right views of the subject, for `model --views` | 20 in the app, 0 from the CLI |
| `model IMAGE` | A model from one image | 55 by default; 25 with `--geometry standard --texture-quality standard --no-pbr` |
| `model --text PROMPT` | A model from a prompt | as above |
| `model --views F L B R` | A model from views; `-` for a missing one, front and one more needed | as above |
| `import FILE` | Brings in our own GLB, FBX, OBJ or STL to work on | free |
| `texture PROJECT --image IMG \| --views F L B R \| --prompt TEXT` | Textures a model; `--quality standard\|detailed\|extreme` is 2K, 4K (default) or 8K | 20 (10 at 2K) |
| `upscale PROJECT [--8k]` | Raises the texture to 4K or 8K | 10, 20 for 8K |
| `pbr PROJECT` | Adds normal, roughness and metallic maps | 5 |
| `retopo PROJECT [--faces 10000] [--no-quad]` | Rebuilds the mesh at a face budget, in quads unless `--no-quad` | 10 (5 in triangles) |
| `segment PROJECT` | Splits the model into parts | 40 |
| `fill PROJECT [--ai]` | Closes the holes segmenting leaves, or rebuilds hidden parts | free |
| `uv PROJECT` | Unwraps the UVs again (Smart UV) | 20 (free during a trial) |
| `edit PROJECT --box X0,Y0,Z0,X1,Y1,Z1` | Remakes what is inside a box on an untextured P2 Smart Mesh (Mesh Edit); the box is in the model's coordinates as `export` gives them (not yet confirmed; MAINTAINING.md), each -1 to 1 | 50 (free on our plan so far) |
| `rig PROJECT [--skeleton …] [--type humanoid\|other]` | Auto-rig, with fingers; see Rigs below | 20 |
| `animate PROJECT NAME … [--prompt TEXT]` | Puts preset animations (`walk`, `run`, `idle`, `jump`, …) or ones made from a description (humanoids only) on a rigged model, and exports them in one GLB or FBX | presets free, `--prompt` 20 |
| `unanimate PROJECT NAME …` | Removes animations from a rigged model | free |
| `export PROJECT --format glb\|fbx\|obj\|stl\|usdz\|3mf` | Exports in a format; `--animations` to include animations, `--texture-size` (default the model's own). OBJ comes zipped with its textures | free |
| `download PROJECT` | The model as Studio keeps it | free |
| `cancel PROJECT` | Cancels the operation running on a project | |
| `restore PROJECT OPERATOR` | Rolls a project back to an earlier step, from `show`'s steps | free |
| `fork PROJECT [OPERATOR]` | Copies a project, at its latest step or an earlier one, into a new project | 5 by the app's price list |
| `rename PROJECT NAME` | Renames a project | free |
| `delete PROJECT …` | Deletes projects for good | free |
| `report [--all]` | Notes the account's spending for the token stats; every command does this too | |

Credits are from Tripo's price list in the app's own code, checked against what runs were charged; Tripo can change them. This table is the one place they are written down; the skills and docs point here.

`model` options, defaulting to what the app does: `--model` picks the generator: `h3.1` (the default, the best quality), `h3.0`, `h2.5`, or Smart Mesh `p1` (35) and `p2` (100), which make cleaner low-poly meshes and are textured in a second step as the app does (20 more). `--geometry detailed` (h3.1), `--texture-quality detailed` (4K) and `--pbr` are on unless turned down (`--geometry standard`, `--texture-quality standard`, `--no-pbr`). `--faces` (10,000, or 5,000 for Smart Mesh), `--quad` (on for p2; `--no-quad`), `--no-texture`, `--parts`, `--variations 2|4` (p2: several models from one input, each its own project), `--public`.

Where the CLI's defaults differ from the app's, on purpose: a model's face limit is 10,000 where the app's is one or two million, since our models go in a game; `image` makes one image where the app makes four; `export` gives GLB where the app offers FBX for a quad model; `rig` names its bones for `unreal` where the app picks `mixamo`.

## Rigs

The app's Auto Rig is Tripo's V3 rig, and `rig` sends what it does. A humanoid (`--type humanoid`, the default) is rigged as a biped with its bones named for `--skeleton`: `unreal` (default), `mixamo` (65 joints), `actorcore`, `unity` or `vrm`. Only `unreal` weights its twist bones (upper arm, forearm, thigh, calf); `actorcore`'s twist and share bones move nothing, and `mixamo` and `unity` have none on the limbs (checked on our base body, 2026-10-12). When Tripo's check says a humanoid is not ready, the app offers to try anyway, and so does `--anyway`. Anything else (`--type other`) is rigged as Tripo reads its shape (quadruped, avian, serpentine…), with no choice of bones; generated motions need a humanoid.

`--skeleton tripo` is the older V1 rig in Tripo's own skeleton (78 joints, `BoneRoot`, `Hip`, `L_Thigh`…), which the app no longer offers; its twist and share bones move nothing. Our character pipeline uses `unreal` instead (`tools/characters`, research/skin-deformation.md): with plain skinning V1 had pinched less at the knees, but with the game's skinning `unreal` passes our rig check, and only its twist bones carry skin.

A command saves its result as Studio keeps it, as `download` does: a meshopt-compressed GLB, which some tools cannot open, or an FBX for a quad Smart Mesh, named `.fbx` whatever `-o` says. Studio keeps models facing +X. `export --format glb` gives a plain GLB facing the standard glTF way, as our skeleton does, and a rigged model exports with its skeleton and the animations asked for.

A project runs one operation at a time, so a command on a project that is busy waits for what is running, then starts. A command stopped with Ctrl-C leaves its operation running on Studio; the next command on that project waits for it. The `credits` a command prints are what Tripo charged for the operations it started, read from the account's own history, so they are exact even when other runs spend at the same time.

## Credits in the token stats

The [token stats](../token-stats/README.md) count Tripo's credits beside Claude's tokens, priced at what they cost on our plan. After every command the CLI reads the account's own charge history and notes each charge once, by its operation, in the ledger `report.mjs` sends from (`~/.local/state/terra-token-stats/spends.jsonl`), so work done by hand in the web app counts too, the next time anyone on that account runs the CLI. `report --all` notes the whole history. It needs nothing beyond the token stats' own setup; without it, the ledger simply waits.

## The code

| File | What it holds |
|---|---|
| `tripo.ts` | The command line: runs one command |
| `commands/` | The commands, by what they do: `make.ts` (image, views, model, import), `edit.ts` (texture to Mesh Edit), `rig.ts` (rig, animate), `project.ts` (the account, a project's history, export, deleting) |
| `steps.ts` | What commands share: waiting on a project, running an operation and saving its result, texturing, animating, exporting |
| `studio.ts` | Studio's backend: login, requests, uploads, waiting, what a run was charged |
| `versions.ts` | Every model version and name the app sends |
| `cli.ts`, `glb.ts`, `spending.ts` | Arguments and help; reading a GLB; noting spending for the token stats |
| `bundle.ts` | Compares the CLI with the app's own code ([MAINTAINING.md](MAINTAINING.md)) |
| `requests.test.ts` | What every command sends, against a fake Studio, as the app sends it |

## When it breaks

This is not a public API: it makes the requests the web app makes, and Tripo can change them without notice. When a command fails where it used to work, or to check the CLI still matches the app, see [MAINTAINING.md](MAINTAINING.md): `bundle.ts` reads the app's own code and reports what changed, and it says where in the code to fix it. `node --test` in this folder runs the tests that need no Tripo.
