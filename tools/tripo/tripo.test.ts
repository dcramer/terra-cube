import assert from "node:assert/strict";
import { test } from "node:test";
import { presignPut, Studio } from "./studio.ts";
import { ourEndpoints, ourNames, versionEnums } from "./bundle.ts";
import { projectIdOf, slug } from "./cli.ts";
import { gltfOf, triangles } from "./glb.ts";
import { chargesIn } from "./spending.ts";
import { textureSize } from "./steps.ts";

test("signs an upload as the client that worked against Tripo's bucket did", () => {
  const url = presignPut("tripo-data", "tripo-studio/20261012/5e0c347f-eaa0/input.png", "us-west-2", "ASIAEXAMPLE", "secret/key+x", "tok/en+==", new Date("2026-10-11T22:45:56Z"));
  assert.equal(url, "https://tripo-data.s3.us-west-2.amazonaws.com/tripo-studio/20261012/5e0c347f-eaa0/input.png?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=ASIAEXAMPLE%2F20261011%2Fus-west-2%2Fs3%2Faws4_request&X-Amz-Date=20261011T224556Z&X-Amz-Expires=900&X-Amz-Security-Token=tok%2Fen%2B%3D%3D&X-Amz-SignedHeaders=host&X-Amz-Signature=9446ed7050f3412913b1f0c32fe44292164974c92ca7e22809a7d81f4d482b11");
});

test("counts fulfilled charges only, an operation billed in parts once per record", () => {
  const charge = (operator_id: string, amount: number, status: string, txn_type = "charge") =>
    ({ operator_id, amount, status, txn_type, create_time: "2026-10-11T22:17:58Z" });
  const got = chargesIn([
    charge("02a7ec14-a07f-4445-aa4c-a9d743574f4f", 5, "fulfilled"),
    charge("02a7ec14-a07f-4445-aa4c-a9d743574f4f", 30, "fulfilled"),
    charge("6e7d9e9c-2a91-4289-a725-34ae7e5ccb35", 10, "pending"),
    charge("3c2bce2b-c710-4d71-89fe-2437d43ea455", 25, "cancelled"),
    charge("00000000-0000-0000-0000-000000000000", 3000, "fulfilled", "payment"),
  ]);
  assert.deepEqual(got, [
    { operation: "02a7ec14-a07f-4445-aa4c-a9d743574f4f", credits: 5, day: "2026-10-11" },
    { operation: "02a7ec14-a07f-4445-aa4c-a9d743574f4f", credits: 30, day: "2026-10-11" },
  ]);
});

test("takes a project from its id or any Studio URL", () => {
  const id = "758cdffb-92b7-4818-a89e-f54ed4fc664b";
  assert.equal(projectIdOf(id), id);
  assert.equal(projectIdOf(`https://studio.tripo3d.ai/workspace/rigging/${id}`), id);
  assert.throws(() => projectIdOf("crate.glb"), /not a project id/);
  assert.throws(() => projectIdOf(undefined), /give a project/);
});

test("names a file after the first words of a prompt", () => {
  assert.equal(slug("A weathered supply crate, plain grey background!"), "a-weathered-supply-crate-plain-grey");
  assert.equal(slug("!!!"), "tripo");
});

test("counts a glTF's triangles, indexed or not", () => {
  const doc = {
    accessors: [{ count: 300 }, { count: 90 }, { count: 12 }],
    meshes: [{ primitives: [{ attributes: { POSITION: 2 }, indices: 0 }, { attributes: { POSITION: 1 } }] }, { primitives: [{ attributes: { POSITION: 2 }, mode: 1 }] }],
  };
  assert.equal(triangles(doc), 130);
  assert.equal(triangles({}), 0);
});

test("reads the JSON of a GLB and nothing else", () => {
  const json = Buffer.from(JSON.stringify({ asset: { version: "2.0" } }));
  const header = Buffer.alloc(20);
  header.write("glTF", 0, "latin1");
  header.writeUInt32LE(12 + 8 + json.length, 8);
  header.writeUInt32LE(json.length, 12);
  assert.deepEqual(gltfOf(Buffer.concat([header, json])), { asset: { version: "2.0" } });
  assert.equal(gltfOf(Buffer.from("not a glb at all, no")), undefined);
});

test("exports at the texture size a model was made at", () => {
  assert.equal(textureSize({ is_textured: true }), 2048);
  assert.equal(textureSize({ is_hd_textured: true }), 4096);
  assert.equal(textureSize({ is_hd_textured: true, is_ultra_textured: true }), 8192);
  assert.equal(textureSize(undefined), 2048);
});

test("finds the app's model-version enums in minified code", () => {
  const js = "x=function(e){return e.Default=`default`,e}({}),ow=function(e){return e.v1_0=`v1.0-20240301`,e.v3_0=`v3.0-20260909`,e}({}),nw=function(e){return e.Nexus_v2_0=`Nexus-v2.0-20260801`,e.tripo_h3_1=`Tripo H3.1`,e}({})";
  assert.deepEqual(versionEnums(js), [["v1.0-20240301", "v3.0-20260909"], ["Nexus-v2.0-20260801", "Tripo H3.1"]]);
});

test("reads the endpoints the CLI calls from its source", () => {
  const src = 'client.call("POST", "/v2/studio/progress", x); client.start("export", b); apply(client, a, pid, "remesh", body); endpoint = "text_to_model"; `/v2/studio/operation/${name}`';
  assert.deepEqual([...ourEndpoints(src)].sort(), ["/v2/studio/operation/export", "/v2/studio/operation/remesh", "/v2/studio/operation/text_to_model", "/v2/studio/progress"]);
});

test("takes the names the CLI sends from versions.ts", () => {
  const names = ourNames();
  for (const name of ["v3.1-20260211", "v3.0-20260909", "gemini_3.1_flash_image_preview", "unreal", "gltf"]) assert.ok(names.has(name), name);
  assert.ok(!names.has("h3.1"), "our own names for them are not sent");
});

test("takes the session cookie bare, padded or not, or as a whole cookie header", () => {
  const session = (c: string) => new Studio(c).cookie;
  assert.equal(session("abc"), "ory_kratos_session=abc");
  assert.equal(session("MTc5-k="), "ory_kratos_session=MTc5-k=");
  assert.equal(session("ory_kratos_session=MTc5-k="), "ory_kratos_session=MTc5-k=");
  assert.equal(session("tripo_device_id=d; ory_kratos_session=MTc5-k="), "tripo_device_id=d; ory_kratos_session=MTc5-k=");
  assert.throws(() => session("tripo_device_id=d; other=x"));
});
