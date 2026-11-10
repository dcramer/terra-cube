// Making things: images, views, models, and bringing in our own.

import { basename, extname } from "node:path";
import { type Args, type Command, int, numbered, one, out, report, slug, stem } from "../cli.ts";
import { hasUVs } from "../glb.ts";
import { finish, textureModel, uploadViews } from "../steps.ts";
import { type Studio, StudioError, log } from "../studio.ts";
import { IMAGE_MODELS, MODELS, PROMPT_IMAGE_MODEL, VIEWS } from "../versions.ts";

async function image(client: Studio, a: Args): Promise<void> {
  const prompt = one(a, "a prompt");
  const v = a.values;
  const refs: string[] = v.ref ?? [];
  if (refs.length > 10) throw new StudioError("at most 10 reference images");
  if (refs.length && v.model === "midjourney") throw new StudioError("Midjourney takes no reference images");
  const body: Record<string, unknown> = {
    if_upload: true,
    sketch_to_render: false,
    t_pose: Boolean(v["t-pose"]),
    amount: int(v.count, "--count"),
    model_version: IMAGE_MODELS[v.model],
    prompt,
    resolution: v["4k"] ? "4K" : "1K",
    // As the app does: with references and no ratio chosen, the result keeps theirs.
    scale: v.aspect ?? (refs.length ? undefined : "1:1"),
  };
  const uploaded = [];
  for (const path of refs) uploaded.push(await client.uploadImage(path));
  if (uploaded.length === 1) body.image = uploaded[0];
  else if (uploaded.length) body.images = uploaded;
  const started = await client.start("/v2/studio/image/gen_image_v2", body);
  log(`image ${started.asset_id}, ${started.remaining_free_count} free images left this month`);
  const asset = await client.waitImage(started.asset_id, "image");
  const outputs = asset.output.data;
  const base = v.out ?? `${slug(prompt)}.png`;
  const files = [];
  for (const [i, item] of outputs.entries()) files.push(await client.download(item.url, outputs.length === 1 ? base : numbered(base, i + 1)));
  report({ asset: started.asset_id, credits: await client.charged(), files });
}

async function views(client: Studio, a: Args): Promise<void> {
  const path = one(a, "an image");
  const source = await client.uploadImage(path);
  const started = await client.start("/v2/studio/image/gen_multiview", { if_upload: true, image: source });
  const asset = await client.waitImage(started.asset_id, "views");
  const prefix = a.values.out ?? path.slice(0, path.length - extname(path).length);
  const files = [];
  for (const [i, item] of asset.output.data.slice(0, 4).entries()) {
    files.push(await client.download(item.url, `${prefix}-${VIEWS[i]}${extname(item.key) || ".jpeg"}`));
  }
  report({ asset: started.asset_id, credits: await client.charged(), files });
}

async function model(client: Studio, a: Args): Promise<void> {
  const v = a.values;
  const version = MODELS[String(v.model).toLowerCase()] ?? v.model;
  const smartMesh = version.startsWith("Nexus");
  const textured = Boolean(v.texture);
  const variations = int(v.variations, "--variations");
  if (variations > 1 && version !== MODELS.p2) throw new StudioError("--variations needs --model p2");
  const body: Record<string, any> = {
    model_version: version,
    face_limit: int(v.faces ?? (smartMesh ? 5000 : 10000), "--faces"),
    quad: v.quad ?? version === MODELS.p2,
    visibility: v.public ? "shareable" : "private",
  };
  if (!smartMesh) {
    // Smart Mesh refuses every one of these, even when false.
    Object.assign(body, { generate_parts: Boolean(v.parts), smart_poly: Boolean(v["smart-poly"]), texture: textured });
    if (v.parts) body.segmentation_granularity = v.parts;
    if (textured) {
      Object.assign(body, { delight: false, pbr: Boolean(v.pbr), texture_alignment: "original_image", texture_quality: v["texture-quality"] });
    }
    if (version === MODELS["h3.1"]) body.geometry_quality = v.geometry;
  }

  let endpoint: string;
  let name: string;
  let textureFrom: Record<string, unknown>;
  if (v.text) {
    if (a.positionals.length) throw new StudioError("give a prompt with --text or an image, not both");
    endpoint = "text_to_model";
    name = slug(v.text);
    Object.assign(body, { prompt: v.text, gen_image_model_version: PROMPT_IMAGE_MODEL, sketch_to_render: false, t_pose: Boolean(v["t-pose"]) });
    textureFrom = { prompt_text: v.text };
  } else if (v.views) {
    endpoint = "multiview_to_model";
    name = stem(a.positionals.find((p) => p !== "-") ?? "views");
    body.image = await uploadViews(client, a.positionals);
    textureFrom = { images: body.image };
  } else {
    const path = one(a, "an image, --text or --views");
    endpoint = "image_to_model";
    name = stem(path);
    body.image = await client.uploadImage(path);
    if (!smartMesh) body.enable_image_autofix = false;
    textureFrom = { image: body.image };
  }
  if (version === MODELS.p2) {
    // Whether the subject is symmetric, from the main view; the app takes a failed check as no.
    const front = Array.isArray(body.image) ? body.image[0] : body.image;
    body.symmetry = false;
    if (front) {
      try {
        const check = await client.call("POST", "/v2/studio/operation/symmetry_check", { image: { bucket: front.bucket, key: front.key } });
        body.symmetry = Boolean(check.symmetry);
      } catch (err) {
        log(`symmetry check failed, so none assumed: ${(err as Error).message}`);
      }
    }
  }
  if (variations > 1) {
    body.variations = Array.from({ length: variations }, () => ({ face_limit: body.face_limit }));
    delete body.face_limit;
  }

  const reply = await client.start(endpoint, body);
  const receipts: any[] = reply.variations ? reply.variations.filter((r: any) => r.accepted !== false) : [reply];
  for (const [i, receipt] of receipts.entries()) {
    log(`project ${receipt.project_id}`);
    await client.wait(receipt.operator_id, "model");
    let op = receipt.operator_id;
    if (smartMesh && textured) {
      // Smart Mesh comes untextured; the app textures it in a second step, as we do, with
      // the app's settings for it.
      log("Smart Mesh comes untextured; texturing it");
      op = await textureModel(client, receipt.project_id, textureFrom, v["texture-quality"], "original_image", true);
    }
    const path = v.out ?? `${name}.glb`;
    await finish(client, receipt.project_id, op, receipts.length > 1 ? numbered(path, i + 1) : path);
  }
}

async function importModel(client: Studio, a: Args): Promise<void> {
  const path = one(a, "a model file");
  const format = extname(path).slice(1).toLowerCase();
  if (!["glb", "fbx", "obj", "stl"].includes(format)) throw new StudioError("Studio imports GLB, FBX, OBJ or STL");
  const place = await client.put(path, format);
  const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]; // placed as it is, column-major
  const receipt = await client.start("import_user_model", { format, model: place, name: basename(path), transform_matrix: identity, use_original_uv: hasUVs(path, format) });
  await client.wait(receipt.operator_id, "import");
  await finish(client, receipt.project_id, receipt.operator_id, a.values.out ?? `${stem(path)}-imported.glb`);
}

export const make: Record<string, Command> = {
  image: {
    usage: 'image "PROMPT" [--ref IMAGE …] [-o out.png]',
    help: "generate images from a prompt, optionally like up to ten reference images",
    options: {
      ref: { type: "string", multiple: true, help: "a reference image (repeatable, up to 10)" },
      model: { type: "string", default: "nano-banana-2", choices: Object.keys(IMAGE_MODELS), help: "the image model (the app's default)" },
      aspect: { type: "string", choices: ["1:1", "16:9", "9:16", "4:3", "3:4"], help: "aspect ratio (default 1:1, or the reference's own)" },
      count: { type: "string", default: "1", choices: ["1", "2", "3", "4"], help: "how many images" },
      "4k": { type: "boolean", help: "4K instead of 1K (costs extra)" },
      "t-pose": { type: "boolean", help: "draw a figure in a T-pose, ready to rig" },
      ...out,
    },
    run: image,
  },
  views: {
    usage: "views IMAGE [-o PREFIX]",
    help: "four views (front, left, back, right) of the subject of one image; writes PREFIX-front.jpeg and so on",
    options: out,
    run: views,
  },
  model: {
    usage: "model IMAGE | --text PROMPT | --views FRONT LEFT BACK RIGHT  [options]",
    help: "make a model from an image, a prompt or several views ('-' for a missing view)",
    options: {
      text: { type: "string", help: "a prompt instead of an image" },
      views: { type: "boolean", help: "the images given are front, left, back and right views" },
      model: { type: "string", default: "h3.1", help: `${Object.keys(MODELS).join(", ")} or a dated version` },
      faces: { type: "string", help: "face limit (default 10000, or 5000 for p1 and p2 as the app has it)" },
      quad: { type: "boolean", help: "quads instead of triangles (default for p2, as the app has it; --no-quad for triangles)" },
      variations: { type: "string", default: "1", choices: ["1", "2", "4"], help: "p2: several models from one input, each its own project" },
      texture: { type: "boolean", default: true, help: "texture the model (--no-texture for geometry only)" },
      geometry: { type: "string", default: "detailed", choices: ["standard", "detailed"], help: "h3.1's geometry; standard costs less" },
      "texture-quality": { type: "string", default: "detailed", choices: ["standard", "detailed", "extreme"], help: "2K, 4K or 8K" },
      pbr: { type: "boolean", default: true, help: "PBR maps as well as colour (H models; --no-pbr for colour only)" },
      parts: { type: "string", choices: ["simple", "balanced", "detailed"], help: "generate in separate parts (H models)" },
      "smart-poly": { type: "boolean", help: "low-poly topology (H models)" },
      "t-pose": { type: "boolean", help: "with --text: model a figure in a T-pose" },
      public: { type: "boolean", help: "share the project instead of keeping it private" },
      ...out,
    },
    run: model,
  },
  import: {
    usage: "import FILE [-o out.glb]",
    help: "bring in a model of our own (GLB, FBX, OBJ or STL) to texture, rig or animate it",
    options: out,
    run: importModel,
  },
};
