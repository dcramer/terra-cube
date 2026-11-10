// Every model version and name the web app sends, in one place, so following Tripo is a
// change to this file. `bundle.ts` checks these against the app's own code (MAINTAINING.md).

// Generators of 3D models, by the names the web app shows.
export const MODELS: Record<string, string> = {
  "h3.1": "v3.1-20260211",
  "h3.0": "v3.0-20250812",
  "h2.5": "v2.5-20250123",
  p1: "Nexus-v1.0-20260214",
  p2: "Nexus-v2.0-20260801",
};

// Image generators: our names, then what the app sends.
export const IMAGE_MODELS: Record<string, string> = {
  "gpt-image-2.5": "gpt_image_2.5_sunburst",
  "gpt-image-2": "gpt_image_2",
  "nano-banana-pro": "gemini_3_pro_image_preview",
  "nano-banana-2": "gemini_3.1_flash_image_preview",
  "nano-banana": "gemini_2.5_flash_image_preview",
  midjourney: "midjourney",
};
// What a model from a prompt draws its image with.
export const PROMPT_IMAGE_MODEL = "flux.1_dev";

// The V3 rig the app's Auto Rig sends, and the older V1 rig in Tripo's own skeleton, which the
// app no longer offers but our character converter takes (README, Rigs).
export const RIG_V3 = "v3.0-20260909";
export const RIG_V1 = "v1.0-20240301";
// The bones a V3 humanoid is named for; "tripo" asks for the V1 rig instead.
export const SKELETONS = ["unreal", "mixamo", "actorcore", "unity", "vrm", "tripo"];

// The other operations' versions; "default" lets Studio choose.
export const TEXTURE = "v3.0-20250812"; // upscale and PBR
export const SEGMENTATION = "v2.0-20260430";
export const COMPLETION = "v1.0-20250506"; // rebuilding hidden parts
export const DEFAULT = "default";

// Export formats: ours, then the name on the wire.
export const EXPORT_FORMATS: Record<string, string> = { glb: "gltf", fbx: "fbx", obj: "obj", stl: "stl", usdz: "usdz", "3mf": "3mf" };
// The order Studio gives and takes views in everywhere.
export const VIEWS = ["front", "left", "back", "right"];
