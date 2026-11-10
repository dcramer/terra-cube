// Reading models: what the CLI needs to know of a GLB without a 3D library.

import { readFileSync } from "node:fs";

// A GLB's JSON chunk, or undefined if the bytes are not a GLB.
export function gltfOf(glb: Buffer): any {
  if (glb.length < 20 || glb.toString("latin1", 0, 4) !== "glTF") return undefined;
  return JSON.parse(glb.toString("utf8", 20, 20 + glb.readUInt32LE(12)));
}

// The names of a glTF's meshes, which texture, retopology and filling act on.
export function meshNames(doc: any): string[] {
  return (doc.nodes ?? []).filter((n: any) => n.mesh !== undefined && n.name).map((n: any) => n.name);
}

// How many triangles a glTF's meshes have.
export function triangles(doc: any): number {
  let n = 0;
  for (const mesh of doc.meshes ?? []) {
    for (const p of mesh.primitives ?? []) {
      const counted = p.indices !== undefined ? doc.accessors[p.indices] : doc.accessors[p.attributes?.POSITION];
      if ((p.mode ?? 4) === 4 && counted) n += Math.floor(counted.count / 3);
    }
  }
  return n;
}

// Whether a model file has texture coordinates, for Studio to keep on import.
export function hasUVs(path: string, format: string): boolean {
  if (format === "stl") return false;
  if (format === "obj") return /^vt\s/m.test(readFileSync(path, "latin1"));
  if (format === "glb") return Boolean(gltfOf(readFileSync(path))?.meshes?.some((m: any) => m.primitives?.some((p: any) => p.attributes?.TEXCOORD_0 !== undefined)));
  return true; // FBX: too costly to read here; Studio copes when there are none
}
