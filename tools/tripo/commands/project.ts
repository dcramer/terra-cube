// The account and its projects: what is there, a project's history, and its files.

import { type Args, type Command, int, out, output, projectId, projectIdOf, report } from "../cli.ts";
import { exportFile, finish, ready, storedFormat, textureSize } from "../steps.ts";
import { noteSpending } from "../spending.ts";
import { type Studio, StudioError } from "../studio.ts";
import { EXPORT_FORMATS } from "../versions.ts";
import { motionName } from "./rig.ts";

async function balance(client: Studio): Promise<void> {
  report({ credits: await client.credits() });
}

async function list(client: Studio, a: Args): Promise<void> {
  const n = int(a.values.n, "-n");
  const data = await client.call("GET", "/v2/studio/assets/v2", undefined, { asset_type: "mine", type: "all", offset: 0, size: n, locale: "en" });
  for (const p of (data.projects ?? []).slice(0, n)) console.log(`${p.id}  ${String(p.type ?? "").padEnd(10)} ${p.project_name ?? ""}`);
}

async function show(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const d = await client.project(pid);
  const history: any[] = (await client.call("GET", `/v2/studio/project/history/${pid}`)).history ?? [];
  const op = d.operator;
  const flags = Object.entries(op)
    .filter(([k, v]) => k.startsWith("is_") && v === true && k !== "is_owner" && k !== "is_latest_operator")
    .map(([k]) => k.slice(3));
  const shown = {
    project: d.id,
    name: d.project_name,
    head: op.operator_id,
    made_with: op.model_version,
    flags,
    running: d.running_operator?.type,
    rig: op.rigging?.type ? { type: op.rigging.type, status: op.rigging.status } : undefined,
    animations: (op.retarget ?? []).map((r) => ({ name: motionName(r), status: r.status })),
    steps: history.map((h) => ({ operator: h.operator_id, type: h.type, at: new Date(h.created_at * 1000).toISOString() })),
  };
  console.log(JSON.stringify(shown, null, 2));
}

async function exportModel(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const v = a.values;
  const detail = await client.project(pid);
  let ids: string[] = [];
  if (v.animations) {
    const wanted = a.positionals.slice(1);
    const done = (detail.operator.retarget ?? []).filter((r) => r.status === "success");
    ids = done.filter((r) => !wanted.length || wanted.includes(motionName(r))).map((r) => r.operator_id);
    if (wanted.length && ids.length < wanted.length) throw new StudioError(`animations ${wanted.join(", ")} asked for; the project has ${done.map(motionName).join(", ") || "none"}`);
  }
  const ext = v.format === "obj" ? ".zip" : `.${v.format}`; // OBJ comes zipped with its textures
  const size = v["texture-size"] ? int(v["texture-size"], "--texture-size") : textureSize(detail.operator);
  const path = await exportFile(client, pid, output(a, pid, "export", ext), v.format, Boolean(detail.operator.is_rigged), ids, Boolean(v["in-place"]), size, v["fbx-for"]);
  report({ project: pid, credits: await client.charged(), files: [path] });
}

async function download(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const detail = await client.project(pid);
  const path = await client.download(detail.model_url, output(a, pid, "model", `.${storedFormat(detail)}`));
  report({ project: pid, operator: detail.operator.operator_id, files: [path] });
}

async function cancel(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const running = (await client.project(pid)).running_operator;
  if (!running?.operator_id) throw new StudioError("nothing is running on this project");
  await client.start("cancel", { operation_type: running.type, operator_id: running.operator_id, scene: "workspace" });
  report({ project: pid, cancelled: running.type ?? running.operator_id });
}

async function restore(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const op = a.positionals[1];
  if (!op) throw new StudioError("give the operator id of the step to go back to");
  await ready(client, pid);
  await client.start("restore", { operator_id: op, project_id: pid });
  await finish(client, pid, undefined, output(a, pid, "restored"));
}

async function fork(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const op = a.positionals[1] ?? (await client.project(pid)).operator.operator_id;
  const made = await client.start("save_as_new", { operator_id: op, project_id: pid });
  report({ project: made.project_id, from: pid, operator: op, credits: await client.charged() });
}

// Deletes projects for good, as the app's asset list does.
async function remove(client: Studio, a: Args): Promise<void> {
  if (!a.positionals.length) throw new StudioError("name the projects to delete");
  const ids = a.positionals.map(projectIdOf);
  await client.call("DELETE", "/v2/studio/assets", { project_ids: ids });
  report({ deleted: ids });
}

async function rename(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const name = a.positionals[1];
  if (!name || a.positionals.length > 2) throw new StudioError('give the new name, quoted: rename PROJECT "NAME"');
  await client.start("update_name", { name, project_id: pid });
  report({ project: pid, name });
}

export const projects: Record<string, Command> = {
  balance: { usage: "balance", help: "credits left on the subscription", free: true, run: balance },
  list: {
    usage: "list [-n 20]",
    help: "your projects, newest first",
    options: { n: { type: "string", short: "n", default: "20", help: "how many" } },
    free: true,
    run: list,
  },
  show: { usage: "show PROJECT", help: "a project's state: what has been done to it, its rig and animations, and its steps, newest first", free: true, run: show },
  export: {
    usage: "export PROJECT [--format glb] [--animations [NAME …]]",
    help: "export a model in another format, with its animations if asked",
    options: {
      format: { type: "string", default: "glb", choices: Object.keys(EXPORT_FORMATS), help: "file format; OBJ comes zipped with its textures" },
      "texture-size": { type: "string", choices: ["512", "1024", "2048", "4096", "8192"], help: "texture size (default the model's own: 2048, or 4096 or 8192 when upscaled)" },
      "fbx-for": { type: "string", default: "blender", choices: ["blender", "3dsmax", "mixamo"], help: "the FBX flavour" },
      animations: { type: "boolean", help: "include the animations named after the project, or all of them" },
      "in-place": { type: "boolean", help: "keep the root still" },
      ...out,
    },
    run: exportModel,
  },
  download: {
    usage: "download PROJECT [-o out.glb]",
    help: "download a project's model as Studio keeps it: a meshopt-compressed GLB, or FBX for a quad Smart Mesh",
    options: out,
    run: download,
  },
  cancel: { usage: "cancel PROJECT", help: "cancel the operation running on a project", free: true, run: cancel },
  restore: {
    usage: "restore PROJECT OPERATOR",
    help: "roll a project back to an earlier step (an operator id from `show`'s steps)",
    options: out,
    run: restore,
  },
  fork: {
    usage: "fork PROJECT [OPERATOR]",
    help: "copy a project, at its latest step or an earlier one, into a new project",
    run: fork,
  },
  rename: { usage: 'rename PROJECT "NAME"', help: "rename a project", free: true, run: rename },
  delete: { usage: "delete PROJECT …", help: "delete projects for good", free: true, run: remove },
  report: {
    usage: "report [--all]",
    help: "note this account's Tripo spending for the token stats (every command does this too)",
    options: { all: { type: "boolean", help: "the whole history, not only the latest 100 charges" } },
    free: true,
    run: async (client, a) => noteSpending(client, a.values.all, false),
  },
};
