// Changing a model: its texture, its mesh, its parts and its UVs.

import { type Args, type Command, int, out, output, projectId } from "../cli.ts";
import { triangles } from "../glb.ts";
import { apply, finish, modelGltf, partNames, ready, storedFormat, textureModel, uploadViews } from "../steps.ts";
import { type Studio, StudioError, log } from "../studio.ts";
import { COMPLETION, DEFAULT, SEGMENTATION, TEXTURE } from "../versions.ts";

async function texture(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const v = a.values;
  const given = [v.image, v.views, v.prompt].filter(Boolean).length;
  if (given !== 1) throw new StudioError("give one of --image, --views or --prompt");
  const source = v.image
    ? { image: await client.uploadImage(v.image) }
    : v.views
      ? { images: await uploadViews(client, a.positionals.slice(1)) }
      : { prompt_text: v.prompt };
  const op = await textureModel(client, pid, source, v.quality, v.align, Boolean(v.delight));
  await finish(client, pid, op, output(a, pid, "textured"));
}

async function upscale(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  await apply(client, a, pid, "texture_upscaler", { model_version: TEXTURE, project_id: pid, texture_quality: a.values["8k"] ? "extreme" : "detailed" }, "upscaled");
}

async function pbr(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  await apply(client, a, pid, "pbr_generate", { model_version: TEXTURE, project_id: pid }, "pbr");
}

async function retopo(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const body = {
    bake: true,
    face_limit: int(a.values.faces, "--faces"),
    model_version: DEFAULT,
    part_name_list: await partNames(client, pid),
    project_id: pid,
    quad: Boolean(a.values.quad),
    smart_poly: Boolean(a.values["smart-poly"]),
  };
  await apply(client, a, pid, "remesh", body, "retopo");
}

async function segment(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  await apply(client, a, pid, "ai_segmentation", { model_version: SEGMENTATION, project_id: pid, segmentation_granularity: a.values.detail }, "segmented");
}

async function fill(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const names = await partNames(client, pid);
  if (a.values.ai) await apply(client, a, pid, "ai_completion", { model_version: COMPLETION, part_names: names, project_id: pid }, "completed");
  else await apply(client, a, pid, "mesh_fill", { model_version: DEFAULT, part_names: names, project_id: pid }, "filled");
}

// Smart UV makes a candidate unwrap beside the model, which then replaces the model's own.
async function uv(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  await ready(client, pid);
  const head = (await client.project(pid)).operator.operator_id;
  const current = { project_id: pid, current_operator_id: head };
  let context = await client.call("POST", "/v2/studio/operation/uv_edit/context", current);
  if (context.running_task?.operator_id) {
    log("waiting for the unwrap already running on this project");
    await client.wait(context.running_task.operator_id, "uv");
    context = await client.call("POST", "/v2/studio/operation/uv_edit/context", current);
  }
  const started = await client.start("uv_edit/generate", { ...current, action: context.next_action || "generate" });
  await client.wait(started.operator_id, "uv");
  await client.start("uv_edit/apply", { ...current, candidate_operator_id: started.operator_id });
  await finish(client, pid, undefined, output(a, pid, "uv"));
}

// Mesh Edit: the app lets a box be drawn on an untextured P2 model that is the project's
// latest step, and remakes what is inside it.
async function edit(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const box = String(a.values.box ?? "").split(",").map(Number);
  if (box.length !== 6 || box.some((n) => !Number.isFinite(n))) throw new StudioError("give the box as --box X0,Y0,Z0,X1,Y1,Z1");
  const clamp = (n: number) => Math.min(1, Math.max(-1, n));
  const lo = box.slice(0, 3).map((n, i) => clamp(Math.min(n, box[i + 3])));
  const hi = box.slice(0, 3).map((n, i) => clamp(Math.max(n, box[i + 3])));
  await ready(client, pid);
  const op = (await client.project(pid)).operator;
  if (!op.is_nexus_mesh || op.is_textured || op.is_rigged || op.is_segmented || op.is_pbr) {
    throw new StudioError("Mesh Edit works on an untextured P2 Smart Mesh only (model --model p2 --no-texture)");
  }
  // As the app: the model's own faces, at most 25,000 for a quad model, which Studio keeps as
  // FBX, or 50,000 for a GLB. A quad is one face, two triangles in the GLB we read.
  const quads = storedFormat(await client.project(pid)) === "fbx";
  const own = triangles(await modelGltf(client, pid)) / (quads ? 2 : 1);
  const faces = a.values.faces ? int(a.values.faces, "--faces") : Math.min(Math.round(own), quads ? 25000 : 50000);
  await apply(client, a, pid, "local_edit", { bbox: [lo, hi], face_limit: faces, project_id: pid, source_operator_id: op.operator_id }, "edited");
}

export const editing: Record<string, Command> = {
  texture: {
    usage: "texture PROJECT --image IMAGE | --views FRONT LEFT BACK RIGHT | --prompt TEXT  [options]",
    help: "texture a model from an image, views or a prompt",
    options: {
      image: { type: "string", help: "an image to texture from" },
      views: { type: "boolean", help: "texture from the views given after the project" },
      prompt: { type: "string", help: "describe the texture instead" },
      quality: { type: "string", default: "detailed", choices: ["standard", "detailed", "extreme"], help: "2K, 4K or 8K (4K is the app's default)" },
      align: { type: "string", default: "original_image", choices: ["original_image", "geometry"], help: "follow the image or the shape" },
      delight: { type: "boolean", help: "remove the lighting baked into the image" },
      ...out,
    },
    run: texture,
  },
  upscale: {
    usage: "upscale PROJECT [--8k]",
    help: "raise a textured model's texture to 4K or 8K",
    options: { "8k": { type: "boolean", help: "8K instead of 4K" }, ...out },
    run: upscale,
  },
  pbr: { usage: "pbr PROJECT", help: "add normal, roughness and metallic maps to a textured model", options: out, run: pbr },
  retopo: {
    usage: "retopo PROJECT [--faces 10000] [--no-quad] [--smart-poly]",
    help: "rebuild a model's mesh at a face budget",
    options: {
      faces: { type: "string", default: "10000", help: "face limit" },
      quad: { type: "boolean", default: true, help: "quads, as the app's default (--no-quad for triangles)" },
      "smart-poly": { type: "boolean", help: "hand-made-looking low-poly topology" },
      ...out,
    },
    run: retopo,
  },
  segment: {
    usage: "segment PROJECT [--detail balanced]",
    help: "split a model into parts",
    options: { detail: { type: "string", default: "balanced", choices: ["simple", "balanced", "detailed"], help: "how finely" }, ...out },
    run: segment,
  },
  fill: {
    usage: "fill PROJECT [--ai]",
    help: "close the holes left by segmenting, or with --ai rebuild the hidden parts",
    options: { ai: { type: "boolean", help: "AI completion instead of a quick cap" }, ...out },
    run: fill,
  },
  uv: { usage: "uv PROJECT", help: "unwrap a model's UVs again (Smart UV)", options: out, run: uv },
  edit: {
    usage: "edit PROJECT --box X0,Y0,Z0,X1,Y1,Z1 [--faces N]",
    help: "remake the part of an untextured P2 Smart Mesh inside a box (Mesh Edit)",
    options: {
      box: { type: "string", help: "the box's corners in the model's coordinates as Studio stores it, each -1 to 1 (required)" },
      faces: { type: "string", help: "face limit (default the model's own, at most 50,000, as the app does)" },
      ...out,
    },
    run: edit,
  },
};
