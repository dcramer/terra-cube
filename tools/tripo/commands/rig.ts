// Rigging and animating.

import { type Args, type Command, int, out, output, projectId, report } from "../cli.ts";
import { aiMotion, apply, exportFile, ready, retarget, textureSize } from "../steps.ts";
import { type Retarget, type Studio, StudioError, log } from "../studio.ts";
import { RIG_V1, RIG_V3, SKELETONS } from "../versions.ts";

// As the app's Auto Rig: a humanoid is rigged as a biped on the skeleton chosen; anything
// else as the free pre-check reads it (quadruped, avian, serpentine…), with no choice of bones.
async function rig(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const { skeleton, type } = a.values;
  const version = skeleton === "tripo" ? RIG_V1 : RIG_V3;
  await ready(client, pid);
  const check = await client.call("POST", "/v2/studio/operation/pre_rig_check", { model_version: version, project_id: pid });
  if (type === "humanoid" && !check.riggable && !a.values.anyway) {
    throw new StudioError(`Tripo reads this as not ready to rig as a humanoid (${JSON.stringify(check)}). A figure rigs best in a T-pose or A-pose; --anyway tries regardless, as the app offers.`);
  }
  const rigType = type === "humanoid" ? "biped" : check.rig_type || "others";
  const body: Record<string, unknown> = { model_version: version, project_id: pid, rig_type: rigType };
  if (version === RIG_V3 && type === "humanoid") body.spec = skeleton;
  log(`rigging a ${rigType}`);
  await apply(client, a, pid, "rigging_model", body, "rigged");
}

async function animate(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const v = a.values;
  const motions = a.positionals.slice(1);
  const prompts: string[] = v.prompt ?? [];
  if (!motions.length && !prompts.length) throw new StudioError("name at least one motion (walk, run, idle…) or give --prompt");
  await ready(client, pid);
  const rigging = (await client.project(pid)).operator.rigging;
  if (rigging?.status !== "success") throw new StudioError("the model is not rigged yet: run `tripo.ts rig` first");
  if (prompts.length && rigging.type !== "biped") throw new StudioError(`motions made from a description need a humanoid rig; this one is ${rigging.type}`);
  const ids = [];
  for (const m of motions) ids.push(await retarget(client, pid, rigging.type, { animation: `preset:${rigging.type}:${m}` }));
  for (const p of prompts) ids.push(await retarget(client, pid, rigging.type, { motion: await aiMotion(client, p, int(v.seconds, "--seconds")) }));
  const size = v["texture-size"] ? int(v["texture-size"], "--texture-size") : textureSize((await client.project(pid)).operator);
  const path = await exportFile(client, pid, output(a, pid, "animated", `.${v.format}`), v.format, true, ids, Boolean(v["in-place"]), size);
  report({ project: pid, credits: await client.charged(), files: [path] });
}

async function unanimate(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const wanted = a.positionals.slice(1);
  if (!wanted.length) throw new StudioError("name the animations to remove");
  await ready(client, pid);
  const detail = await client.project(pid);
  const on = (detail.operator.retarget ?? []).filter((r) => r.status === "success");
  const missing = wanted.filter((w) => !on.some((r) => motionName(r) === w));
  if (missing.length) throw new StudioError(`${missing.join(", ")} not on the model; it has ${on.map(motionName).join(", ") || "none"}`);
  for (const r of on.filter((r) => wanted.includes(motionName(r)))) {
    await client.start("remove_retarget", { model_operator_id: detail.operator.operator_id, project_id: pid, retarget_operator_id: r.operator_id });
  }
  report({ project: pid, removed: wanted });
}

// An animation's short name: "walk" for "preset:biped:walk".
export function motionName(r: Retarget): string {
  return r.name.split(":").at(-1)!;
}

export const rigging: Record<string, Command> = {
  rig: {
    usage: "rig PROJECT [--skeleton unreal] [--type humanoid|other]",
    help: "rig a model with Tripo's auto-rig",
    options: {
      skeleton: { type: "string", default: "unreal", choices: SKELETONS, help: "a humanoid's bone names; unreal is the one our characters use, tripo the older V1 rig" },
      type: { type: "string", default: "humanoid", choices: ["humanoid", "other"], help: "other rigs an animal or creature as Tripo reads its shape" },
      anyway: { type: "boolean", help: "rig a humanoid even when Tripo's check says it is not ready" },
      ...out,
    },
    run: rig,
  },
  animate: {
    usage: 'animate PROJECT [MOTION …] [--prompt "TEXT" …] [-o out.glb]',
    help: "put preset animations (walk, run, idle, jump…) or ones made from a description on a rigged model, and export them in one file",
    options: {
      prompt: { type: "string", multiple: true, help: "a motion made from a description (repeatable; humanoids only)" },
      seconds: { type: "string", default: "5", help: "length of a --prompt motion, 1 to 10" },
      format: { type: "string", default: "glb", choices: ["glb", "fbx"], help: "file format" },
      "texture-size": { type: "string", choices: ["512", "1024", "2048", "4096", "8192"], help: "texture size (default the model's own)" },
      "in-place": { type: "boolean", help: "keep the root still instead of moving through the world" },
      ...out,
    },
    run: animate,
  },
  unanimate: {
    usage: "unanimate PROJECT MOTION …",
    help: "remove animations from a rigged model",
    free: true,
    run: unanimate,
  },
};
