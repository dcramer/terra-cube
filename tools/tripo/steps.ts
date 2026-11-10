// The steps commands share: waiting on a project, running an operation and saving its result,
// texturing, animating, exporting, and reading a project's model.

import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { extname, join } from "node:path";
import { type Args, output, report, stem } from "./cli.ts";
import { gltfOf, meshNames } from "./glb.ts";
import { type ImageRef, type Operator, type Project, type Studio, StudioError, log, sleep } from "./studio.ts";
import { DEFAULT, EXPORT_FORMATS } from "./versions.ts";

// Waits out an operation already running on a project: Studio refuses another until it ends.
export async function ready(client: Studio, pid: string): Promise<void> {
  const running = (await client.project(pid)).running_operator;
  if (running?.operator_id) {
    log(`waiting for the ${running.type ?? "operation"} already running on this project`);
    await client.wait(running.operator_id, running.type);
  }
}

// Runs one operation on a project, waits for it and saves the result.
export async function apply(client: Studio, a: Args, pid: string, name: string, body: unknown, suffix: string): Promise<void> {
  await ready(client, pid);
  const receipt = await client.start(name, body);
  await client.wait(receipt.operator_id, name);
  await finish(client, pid, receipt.operator_id, output(a, pid, suffix));
}

// Saves the model an operation made, as Studio keeps it, and reports it with what it was
// charged. A quad Smart Mesh is an FBX, so it is saved under that extension.
export async function finish(client: Studio, pid: string, op: string | undefined, path: string): Promise<void> {
  const detail = await client.project(pid, op);
  const format = storedFormat(detail);
  if (extname(path).slice(1).toLowerCase() !== format) path = `${path.slice(0, path.length - extname(path).length)}.${format}`;
  await client.download(detail.model_url, path);
  report({ project: pid, operator: detail.operator.operator_id, credits: await client.charged(), files: [path] });
}

// The format Studio keeps a model in: GLB, or FBX for a quad Smart Mesh.
export function storedFormat(detail: Project): string {
  return extname(new URL(detail.model_url).pathname).slice(1).toLowerCase() || "glb";
}

// A project's model as a GLB, to read: Studio's own file when it is one, else exported as one
// (free). An export faces the standard glTF way, not as Studio keeps models, so it is only read.
async function saveGlb(client: Studio, detail: Project, path: string): Promise<void> {
  if (storedFormat(detail) === "glb") await client.download(detail.model_url, path);
  else await exportFile(client, detail.id, path, "glb", Boolean(detail.operator.is_rigged), [], false, textureSize(detail.operator));
}

export async function textureModel(client: Studio, pid: string, source: Record<string, unknown>, quality: string, align: string, delight: boolean): Promise<string> {
  await ready(client, pid);
  const body = { ...source, project_id: pid, part_names: await partNames(client, pid), texture_quality: quality, texture_alignment: align, delight };
  const receipt = await client.start("texture_model", body);
  await client.wait(receipt.operator_id, "texture");
  return receipt.operator_id;
}

// Puts one animation on a rigged model, or reuses it if it is already there; returns its id.
// Studio runs one at a time per project and answers 406 to another while one runs.
export async function retarget(client: Studio, pid: string, rigType: string, what: { animation?: string; motion?: string }): Promise<string> {
  const existing = (await client.project(pid)).operator.retarget ?? [];
  const found = existing.find((r) => r.status === "success" && ((what.animation && r.name === what.animation) || (what.motion && r.motion_asset_id === what.motion)));
  if (found) {
    log(`${what.animation ?? what.motion}: already on the model`);
    return found.operator_id;
  }
  const body = { model_version: DEFAULT, project_id: pid, rig_type: rigType, ...(what.animation ? { animations: [what.animation] } : { motion_asset_id: what.motion }) };
  for (let i = 0; ; i++) {
    const [status, reply] = await client.send("POST", "/v2/studio/operation/retarget_model", body);
    if (status === 406 && i < 60) {
      await sleep(5000);
      continue;
    }
    if (reply.code !== 0) throw new StudioError(`animating ${what.animation ?? what.motion} failed (${status}): ${reply.message || JSON.stringify(reply)}`);
    client.bill(reply.data.operator_id);
    await client.wait(reply.data.operator_id, what.animation ?? "motion");
    return reply.data.operator_id;
  }
}

// A motion made from a description, ready to put on a rig.
export async function aiMotion(client: Studio, prompt: string, seconds: number): Promise<string> {
  const task = await client.start("/v2/studio/motion/generate", { segments: [{ prompt, duration_seconds: seconds }] });
  log(`motion task ${task.task_id}`);
  for (;;) {
    const state = await client.call("POST", "/v2/studio/motion/get_task", { task_id: task.task_id });
    if (state.status === "success") {
      client.bill(state.asset_id);
      return state.asset_id;
    }
    if (["failed", "error", "cancelled", "expired"].includes(state.status)) throw new StudioError(`motion "${prompt}" ${state.status}`);
    await sleep(4000);
  }
}

// Exports a project's model. A rigged model keeps its skeleton, as the app exports it, with
// the animations given.
export async function exportFile(client: Studio, pid: string, path: string, format: string, rigged: boolean, animations: string[] = [], inPlace = false, textureSize = 2048, fbxFor = "blender"): Promise<string> {
  const body = {
    animate_in_place: inPlace,
    animations,
    bake_animation_frame: 0,
    enable_bake_animation: false,
    export_orientation: "-y",
    export_vertex_colors: false,
    fbx_preset: fbxFor,
    format: EXPORT_FORMATS[format],
    model_version: DEFAULT,
    name: stem(path),
    pack_uv: false,
    project_id: pid,
    // The app's panel always sends zip: a GLB still comes back as one file and an OBJ zipped
    // with its textures, but an FBX as a zip too. Its Blender, Max and Maya bridges send
    // embedded, which gives one FBX, so we do for every other format.
    texture_packaging: format === "glb" || format === "obj" ? "zip" : "embedded",
    texture_size: textureSize,
    with_animation: rigged || animations.length > 0,
  };
  const receipt = await client.start("export", body);
  let url = receipt.model_url;
  if (!url) {
    await client.wait(receipt.operator_id, "export");
    url = (await client.call("POST", "/v2/studio/operation/download_with_name", { file_name: stem(path), operator_id: receipt.operator_id })).model_url;
  }
  return client.download(url, path);
}

// The texture size a model was made at, which the app exports at unless asked otherwise.
export function textureSize(op: Operator | undefined): number {
  return op?.is_ultra_textured ? 8192 : op?.is_hd_textured ? 4096 : 2048;
}

// The glTF of a project's latest model: its JSON, without the binary.
export async function modelGltf(client: Studio, pid: string): Promise<any> {
  const dir = mkdtempSync(join(tmpdir(), "tripo-"));
  try {
    const path = join(dir, "model.glb");
    await saveGlb(client, await client.project(pid), path);
    const doc = gltfOf(readFileSync(path));
    if (!doc) throw new StudioError("the project's model is not a GLB; cannot read it");
    return doc;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// The names of a project's meshes, which texture, retopology and filling act on.
export async function partNames(client: Studio, pid: string): Promise<string[]> {
  const names = meshNames(await modelGltf(client, pid));
  if (!names.length) throw new StudioError("the project's model has no named meshes");
  return names;
}

// Views uploaded in Studio's order, '-' for a missing one.
export async function uploadViews(client: Studio, paths: string[]): Promise<(ImageRef | null)[]> {
  if (paths.length > 4 || paths[0] === undefined || paths[0] === "-" || paths.filter((p) => p !== "-").length < 2) {
    throw new StudioError("give the front view and at least one more, in the order front left back right ('-' for a missing one)");
  }
  const refs: (ImageRef | null)[] = [];
  for (const p of paths) refs.push(p === "-" ? null : await client.uploadImage(p));
  while (refs.length < 4) refs.push(null);
  return refs;
}
