// What each command sends to Studio, checked against a fake that answers as Studio does. The
// bodies expected here are the ones the web app sends (MAINTAINING.md), so a change that alters
// a request shows up here before it costs credits.

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

// The spending ledger lives under XDG_STATE_HOME; keep the tests' charges out of the real one.
const DIR = mkdtempSync(join(tmpdir(), "tripo-test-"));
process.env.XDG_STATE_HOME = join(DIR, "state");
mkdirSync(process.env.XDG_STATE_HOME);
const { Studio } = await import("./studio.ts");
const { main } = await import("./tripo.ts");

const PID = "11111111-2222-3333-4444-555555555555";
const REF = { bucket: "tripo-data", key: "upload/ref.png", image_source: "upload", image_audit_result: "pass" };

// A GLB holding only its JSON: two named meshes of 12 triangles in all, with texture coordinates.
function glb(doc: object = {
  accessors: [{ count: 30 }, { count: 6 }],
  meshes: [{ primitives: [{ attributes: { POSITION: 0, TEXCOORD_0: 0 }, indices: 0 }] }, { primitives: [{ attributes: { POSITION: 1 } }] }],
  nodes: [{ name: "body", mesh: 0 }, { name: "hat", mesh: 1 }],
}): Buffer {
  const json = Buffer.from(JSON.stringify(doc));
  const header = Buffer.alloc(20);
  header.write("glTF", 0, "latin1");
  header.writeUInt32LE(2, 4);
  header.writeUInt32LE(20 + json.length, 8);
  header.writeUInt32LE(json.length, 12);
  header.write("JSON", 16, "latin1");
  return Buffer.concat([header, json]);
}
const MODEL = join(DIR, "model.glb");
writeFileSync(MODEL, glb());

type Request = { method: string; path: string; body?: any; query?: any };

// Studio as far as the commands can tell: every request is recorded and answered.
class FakeStudio extends Studio {
  requests: Request[] = [];
  detail: any = {
    id: PID,
    model_url: "https://fake/model.glb",
    operator: { operator_id: "head", is_textured: true, rigging: { status: "success", type: "biped" }, retarget: [] },
    running_operator: null,
  };
  rigCheck: any = { riggable: true, rig_type: "biped" };
  charges: any[] = [];
  replies: Record<string, any> = {};
  started = 0;

  constructor() {
    super("ory_kratos_session=fake");
  }

  // The operations started, as [name, body]: what costs credits or changes a project.
  get operations(): [string, any][] {
    const reads = ["progress", "pre_rig_check", "symmetry_check", "uv_edit/context", "download_with_name"];
    return this.requests
      .filter((r) => r.method === "POST" && /\/v2\/studio\/(operation|image\/gen|motion\/generate)/.test(r.path))
      .map((r) => [r.path.replace(/^\/v2\/studio\/(operation\/)?/, ""), r.body] as [string, any])
      .filter(([name]) => !reads.includes(name));
  }

  override async sendOnce(method: string, path: string, body: unknown, query: Record<string, unknown> | undefined, auth: boolean): Promise<[number, any]> {
    if (path.endsWith("/whoami")) return [200, { tokenized: `x.${Buffer.from(JSON.stringify({ exp: 4e9 })).toString("base64url")}.x` }];
    // As it goes over the wire, where fields left undefined are dropped.
    this.requests.push({ method, path, body: body === undefined ? undefined : JSON.parse(JSON.stringify(body)), query });
    return [200, { code: 0, data: this.answer(path) }];
  }

  answer(path: string): any {
    const name = path.replace(/^\/v2\/studio\//, "");
    if (name in this.replies) return this.replies[name];
    if (name === "user/profile/payment") return { wallet: { total_credit: 100 } };
    if (name === "txn/records") return { records: this.charges, have_more: false };
    if (name.startsWith("project/detail/v3/")) return this.detail;
    if (name.startsWith("project/history/")) return { history: [{ operator_id: "head", type: "upload", created_at: 1791821715 }] };
    if (name === "progress") return [{ status: "success" }];
    if (name === "audit/image") return { result: "pass" };
    if (name === "image/gen_image_v2" || name === "image/gen_multiview") return { asset_id: "asset", remaining_free_count: 99 };
    if (name === "image/get_image_asset") return { status: "success", output: { data: [0, 1, 2, 3].map((i) => ({ url: `https://fake/${i}`, key: `out/${i}.png` })) } };
    if (name === "operation/pre_rig_check") return this.rigCheck;
    if (name === "operation/symmetry_check") return { symmetry: true };
    if (name === "operation/uv_edit/context") return { next_action: "generate", running_task: null };
    if (name === "operation/download_with_name") return { model_url: "https://fake/export.glb" };
    if (name === "operation/save_as_new") return { project_id: "copy" };
    if (name === "motion/generate") return { task_id: "task" };
    if (name === "motion/get_task") return { status: "success", asset_id: "motion" };
    return { project_id: PID, operator_id: `op${++this.started}` };
  }

  override async put(): Promise<any> {
    return { bucket: REF.bucket, key: REF.key };
  }

  override async download(_url: string, path: string): Promise<string> {
    writeFileSync(path, glb());
    return path;
  }
}

// Runs one command line on a fake Studio; returns it with the JSON lines printed.
async function run(argv: string[], setup: (s: FakeStudio) => void = () => {}): Promise<{ studio: FakeStudio; printed: any[]; failed: boolean }> {
  const studio = new FakeStudio();
  setup(studio);
  const printed: any[] = [];
  const [log, error, code] = [console.log, console.error, process.exitCode];
  console.log = (line: string) => printed.push(JSON.parse(line));
  console.error = () => {};
  try {
    await main(argv, () => studio);
    return { studio, printed, failed: process.exitCode === 1 };
  } finally {
    [console.log, console.error, process.exitCode] = [log, error, code];
  }
}
const out = (name: string) => ["-o", join(DIR, name)];
const parts = ["body", "hat"];

test("image: the app's default model, a square image, or the reference's own shape", async () => {
  const plain = await run(["image", "a crate", ...out("crate.png")]);
  assert.deepEqual(plain.studio.operations, [
    ["image/gen_image_v2", { if_upload: true, sketch_to_render: false, t_pose: false, amount: 1, model_version: "gemini_3.1_flash_image_preview", prompt: "a crate", resolution: "1K", scale: "1:1" }],
  ]);
  const like = await run(["image", "a crate", "--ref", "ref.png", "--model", "gpt-image-2.5", ...out("like.png")]);
  assert.deepEqual(like.studio.operations[0][1], {
    if_upload: true, sketch_to_render: false, t_pose: false, amount: 1, model_version: "gpt_image_2.5_sunburst", prompt: "a crate", resolution: "1K", image: REF,
  });
});

test("views: one image in, four views out", async () => {
  const { studio, printed } = await run(["views", "ref.png", ...out("crate")]);
  assert.deepEqual(studio.operations, [["image/gen_multiview", { if_upload: true, image: REF }]]);
  assert.deepEqual(printed[0].files.map((f: string) => f.split("-").at(-1)), ["front.png", "left.png", "back.png", "right.png"]);
});

test("model: H3.1 at the app's defaults, our face limit", async () => {
  const { studio } = await run(["model", "ref.png", ...out("m.glb")]);
  assert.deepEqual(studio.operations, [
    ["image_to_model", {
      model_version: "v3.1-20260211", face_limit: 10000, quad: false, visibility: "private",
      generate_parts: false, smart_poly: false, texture: true,
      delight: false, pbr: true, texture_alignment: "original_image", texture_quality: "detailed",
      geometry_quality: "detailed", image: REF, enable_image_autofix: false,
    }],
  ]);
});

test("model: turned down to the cheapest H3.1", async () => {
  const { studio } = await run(["model", "ref.png", "--geometry", "standard", "--texture-quality", "standard", "--no-pbr", "--no-quad", ...out("m.glb")]);
  const body = studio.operations[0][1];
  assert.equal(body.geometry_quality, "standard");
  assert.equal(body.texture_quality, "standard");
  assert.equal(body.pbr, false);
});

test("model: from a prompt, as the app sends it", async () => {
  const { studio } = await run(["model", "--text", "a red crate", ...out("m.glb")]);
  const [name, body] = studio.operations[0];
  assert.equal(name, "text_to_model");
  assert.equal(body.prompt, "a red crate");
  assert.equal(body.gen_image_model_version, "flux.1_dev");
  assert.equal(body.sketch_to_render, false);
  assert.equal(body.t_pose, false);
  assert.equal(body.image, undefined);
  assert.equal(body.enable_image_autofix, undefined);
});

test("model: from views, front and one more, in Studio's order", async () => {
  const { studio } = await run(["model", "--views", "f.png", "-", "b.png", ...out("m.glb")]);
  const [name, body] = studio.operations[0];
  assert.equal(name, "multiview_to_model");
  assert.deepEqual(body.image, [REF, null, REF, null]);
  assert.equal(body.enable_image_autofix, undefined);
});

test("model: Smart Mesh P2 as the app makes and textures it", async () => {
  const { studio } = await run(["model", "ref.png", "--model", "p2", ...out("m.glb")]);
  assert.deepEqual(studio.operations, [
    ["image_to_model", { model_version: "Nexus-v2.0-20260801", face_limit: 5000, quad: true, visibility: "private", image: REF, symmetry: true }],
    ["texture_model", { image: REF, project_id: PID, part_names: parts, texture_quality: "detailed", texture_alignment: "original_image", delight: true }],
  ]);
});

test("model: P2 variations, each its own project", async () => {
  const { studio, printed } = await run(["model", "ref.png", "--model", "p2", "--variations", "2", "--no-texture", ...out("m.glb")], (s) => {
    s.replies["operation/image_to_model"] = { variations: [{ accepted: true, project_id: "pa", operator_id: "oa" }, { accepted: true, project_id: "pb", operator_id: "ob" }] };
  });
  assert.deepEqual(studio.operations[0][1].variations, [{ face_limit: 5000 }, { face_limit: 5000 }]);
  assert.equal(studio.operations[0][1].face_limit, undefined);
  assert.deepEqual(printed.map((p) => p.project), ["pa", "pb"]);
  assert.deepEqual(printed.map((p) => p.files[0].split("/").at(-1)), ["m-1.glb", "m-2.glb"]);
});

test("model: variations only for P2", async () => {
  const { studio, failed } = await run(["model", "ref.png", "--variations", "2", ...out("m.glb")]);
  assert.ok(failed);
  assert.deepEqual(studio.operations, []);
});

test("import: our own model, keeping its UVs only when it has them", async () => {
  const { studio } = await run(["import", MODEL, ...out("i.glb")]);
  assert.deepEqual(studio.operations, [
    ["import_user_model", { format: "glb", model: { bucket: REF.bucket, key: REF.key }, name: "model.glb", transform_matrix: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1], use_original_uv: true }],
  ]);
  const bare = join(DIR, "bare.glb");
  writeFileSync(bare, glb({ meshes: [{ primitives: [{ attributes: { POSITION: 0 } }] }] }));
  assert.equal((await run(["import", bare, ...out("i.glb")])).studio.operations[0][1].use_original_uv, false);
});

test("texture, upscale and PBR send the app's versions and qualities", async () => {
  assert.deepEqual((await run(["texture", PID, "--prompt", "rusty iron", ...out("t.glb")])).studio.operations, [
    ["texture_model", { prompt_text: "rusty iron", project_id: PID, part_names: parts, texture_quality: "detailed", texture_alignment: "original_image", delight: false }],
  ]);
  assert.deepEqual((await run(["upscale", PID, "--8k", ...out("u.glb")])).studio.operations, [
    ["texture_upscaler", { model_version: "v3.0-20250812", project_id: PID, texture_quality: "extreme" }],
  ]);
  assert.deepEqual((await run(["pbr", PID, ...out("p.glb")])).studio.operations, [["pbr_generate", { model_version: "v3.0-20250812", project_id: PID }]]);
});

test("retopology, segmenting and filling", async () => {
  assert.deepEqual((await run(["retopo", PID, ...out("r.glb")])).studio.operations, [
    ["remesh", { bake: true, face_limit: 10000, model_version: "default", part_name_list: parts, project_id: PID, quad: true, smart_poly: false }],
  ]);
  assert.deepEqual((await run(["segment", PID, ...out("s.glb")])).studio.operations, [
    ["ai_segmentation", { model_version: "v2.0-20260430", project_id: PID, segmentation_granularity: "balanced" }],
  ]);
  assert.deepEqual((await run(["fill", PID, ...out("f.glb")])).studio.operations, [["mesh_fill", { model_version: "default", part_names: parts, project_id: PID }]]);
  assert.deepEqual((await run(["fill", PID, "--ai", ...out("f.glb")])).studio.operations, [
    ["ai_completion", { model_version: "v1.0-20250506", part_names: parts, project_id: PID }],
  ]);
});

test("uv: generate a candidate unwrap, then apply it", async () => {
  const { studio } = await run(["uv", PID, ...out("uv.glb")]);
  const current = { project_id: PID, current_operator_id: "head" };
  assert.deepEqual(studio.operations, [
    ["uv_edit/generate", { ...current, action: "generate" }],
    ["uv_edit/apply", { ...current, candidate_operator_id: "op1" }],
  ]);
});

test("edit: Mesh Edit on an untextured P2 model, the box clamped as the app does", async () => {
  const p2 = (s: FakeStudio) => (s.detail.operator = { operator_id: "head", is_nexus_mesh: true });
  const { studio } = await run(["edit", PID, "--box", "0.5,-2,0,-0.5,0.2,1", ...out("e.glb")], p2);
  assert.deepEqual(studio.operations, [["local_edit", { bbox: [[-0.5, -1, 0], [0.5, 0.2, 1]], face_limit: 12, project_id: PID, source_operator_id: "head" }]]);
  const textured = await run(["edit", PID, "--box", "0,0,0,1,1,1", ...out("e.glb")]);
  assert.ok(textured.failed);
  assert.deepEqual(textured.studio.operations, []);
});

test("a quad Smart Mesh, which Studio keeps as FBX, is read through a GLB export and saved as its FBX", async () => {
  const fbx = (s: FakeStudio) => {
    s.detail.model_url = "https://fake/output_mesh.fbx";
    s.detail.operator = { operator_id: "head", is_nexus_mesh: true };
  };
  const { studio, printed } = await run(["edit", PID, "--box", "0,0,0,1,1,1", ...out("e.glb")], fbx);
  assert.deepEqual(studio.operations.map(([name]) => name), ["export", "local_edit"], "read through an export, then edited");
  assert.equal(studio.operations[0][1].format, "gltf");
  assert.equal(studio.operations[1][1].face_limit, 6, "a quad is one face");
  assert.equal(printed[0].files[0].split("/").at(-1), "e.fbx");
});

test("rig: the V3 rig on the unreal skeleton by default", async () => {
  const { studio } = await run(["rig", PID, ...out("r.glb")]);
  assert.deepEqual(studio.requests.find((r) => r.path.endsWith("pre_rig_check"))?.body, { model_version: "v3.0-20260909", project_id: PID });
  assert.deepEqual(studio.operations, [["rigging_model", { model_version: "v3.0-20260909", project_id: PID, rig_type: "biped", spec: "unreal" }]]);
});

test("rig: the V1 rig in Tripo's own skeleton names no spec", async () => {
  const { studio } = await run(["rig", PID, "--skeleton", "tripo", ...out("r.glb")]);
  assert.deepEqual(studio.operations, [["rigging_model", { model_version: "v1.0-20240301", project_id: PID, rig_type: "biped" }]]);
});

test("rig: a creature as Tripo reads it", async () => {
  const { studio } = await run(["rig", PID, "--type", "other", ...out("r.glb")], (s) => (s.rigCheck = { riggable: false, rig_type: "quadruped" }));
  assert.deepEqual(studio.operations, [["rigging_model", { model_version: "v3.0-20260909", project_id: PID, rig_type: "quadruped" }]]);
});

test("rig: a humanoid Tripo says is not ready, only when asked to try anyway", async () => {
  const notReady = (s: FakeStudio) => (s.rigCheck = { riggable: false, rig_type: "biped" });
  const refused = await run(["rig", PID, ...out("r.glb")], notReady);
  assert.ok(refused.failed);
  assert.deepEqual(refused.studio.operations, []);
  assert.equal((await run(["rig", PID, "--anyway", ...out("r.glb")], notReady)).studio.operations.length, 1);
});

test("animate: presets and a described motion on a humanoid, exported together", async () => {
  const { studio } = await run(["animate", PID, "walk", "--prompt", "a bow", ...out("a.glb")]);
  assert.deepEqual(studio.operations, [
    ["retarget_model", { model_version: "default", project_id: PID, rig_type: "biped", animations: ["preset:biped:walk"] }],
    ["motion/generate", { segments: [{ prompt: "a bow", duration_seconds: 5 }] }],
    ["retarget_model", { model_version: "default", project_id: PID, rig_type: "biped", motion_asset_id: "motion" }],
    ["export", {
      animate_in_place: false, animations: ["op1", "op2"], bake_animation_frame: 0, enable_bake_animation: false, export_orientation: "-y",
      export_vertex_colors: false, fbx_preset: "blender", format: "gltf", model_version: "default", name: "a", pack_uv: false, project_id: PID,
      texture_packaging: "zip", texture_size: 2048, with_animation: true,
    }],
  ]);
});

test("animate: described motions only on a humanoid, refused before spending", async () => {
  const { studio, failed } = await run(["animate", PID, "--prompt", "a gallop", ...out("a.glb")], (s) => (s.detail.operator.rigging.type = "quadruped"));
  assert.ok(failed);
  assert.deepEqual(studio.operations, []);
});

test("export: at the model's own texture size, an FBX in one file", async () => {
  const hd = (s: FakeStudio) => (s.detail.operator.is_hd_textured = true);
  const glbExport = (await run(["export", PID, ...out("x.glb")], hd)).studio.operations[0][1];
  assert.equal(glbExport.texture_size, 4096);
  assert.equal(glbExport.texture_packaging, "zip");
  const fbx = (await run(["export", PID, "--format", "fbx", "--texture-size", "1024", ...out("x.fbx")], hd)).studio.operations[0][1];
  assert.equal(fbx.format, "fbx");
  assert.equal(fbx.texture_size, 1024);
  assert.equal(fbx.texture_packaging, "embedded");
  assert.equal(fbx.with_animation, false, "an unrigged model has no skeleton to export");
});

test("export: a rigged model keeps its skeleton, with only the animations asked for", async () => {
  const rigged = (s: FakeStudio) => {
    s.detail.operator.is_rigged = true;
    s.detail.operator.retarget = [{ name: "preset:biped:walk", status: "success", operator_id: "walk-op" }, { name: "preset:biped:run", status: "success", operator_id: "run-op" }];
  };
  const plain = (await run(["export", PID, ...out("x.glb")], rigged)).studio.operations[0][1];
  assert.equal(plain.with_animation, true);
  assert.deepEqual(plain.animations, []);
  assert.deepEqual((await run(["export", PID, "--animations", "run", ...out("x.glb")], rigged)).studio.operations[0][1].animations, ["run-op"]);
});

test("project steps: unanimate, cancel, restore, fork, rename and delete", async () => {
  const withWalk = (s: FakeStudio) => (s.detail.operator.retarget = [{ name: "preset:biped:walk", status: "success", operator_id: "walk-op" }]);
  assert.deepEqual((await run(["unanimate", PID, "walk"], withWalk)).studio.operations, [
    ["remove_retarget", { model_operator_id: "head", project_id: PID, retarget_operator_id: "walk-op" }],
  ]);
  const running = (s: FakeStudio) => (s.detail.running_operator = { operator_id: "busy", type: "texture_generation" });
  assert.deepEqual((await run(["cancel", PID], running)).studio.operations, [["cancel", { operation_type: "texture_generation", operator_id: "busy", scene: "workspace" }]]);
  assert.deepEqual((await run(["restore", PID, "old-op", ...out("r.glb")])).studio.operations, [["restore", { operator_id: "old-op", project_id: PID }]]);
  assert.deepEqual((await run(["fork", PID])).studio.operations, [["save_as_new", { operator_id: "head", project_id: PID }]]);
  assert.deepEqual((await run(["rename", PID, "a crate"])).studio.operations, [["update_name", { name: "a crate", project_id: PID }]]);
  const other = "66666666-7777-8888-9999-000000000000";
  const deleted = await run(["delete", PID, `https://studio.tripo3d.ai/workspace/${other}`]);
  assert.deepEqual(deleted.studio.requests.find((r) => r.method === "DELETE"), { method: "DELETE", path: "/v2/studio/assets", body: { project_ids: [PID, other] }, query: undefined });
});

test("credits: what Tripo charged for this run's operations, not other runs'", async () => {
  const now = new Date().toISOString();
  const charge = (operator_id: string, amount: number, status = "fulfilled") => ({ operator_id, amount, status, txn_type: "charge", create_time: now });
  const { printed } = await run(["model", "ref.png", "--model", "p2", ...out("m.glb")], (s) => {
    // op1 made the model, op2 textured it; another run's charge lands between them.
    s.charges = [charge("op2", 20, "pending"), charge("someone-else", 55), charge("op1", 100), charge("op1", 5, "cancelled")];
  });
  assert.equal(printed[0].credits, 120);
});
