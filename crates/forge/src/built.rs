//! Models made here, of boxes: the small life of the air and the water. A songbird is the
//! length of a hand and is seen for a second as it crosses; it does not want forty-five
//! credits of sculpture, and the world it flies in is cuboids. So forge builds it: a body,
//! a head, a tail and wings of flat boxes, each box its own shade, at its true size, with the
//! wings spread so the rig made here (`shaped`) can take them. A creature asks for one with
//! `[built]` in its `creature.toml`: a form (`bird`, `insect`, `fish`), its length, its span
//! and its colours.

use crate::glb::Glb;
use anyhow::{Result, bail};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// What `[built]` says. Lengths in metres; colours linear red, green, blue.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Built {
    /// `bird`, `insect` (two pairs of wings) or `fish`.
    pub form: String,
    /// Beak to the root of the tail (a fish: nose to the root of its tail fin).
    pub length: f32,
    /// Wingtip to wingtip (a fish: across its side fins).
    pub span: f32,
    /// How deep a wing is front to back at its root, of the body's length.
    #[serde(default = "half")]
    pub chord: f32,
    /// Its tail's length, of the body's; and whether it is forked.
    #[serde(default = "half")]
    pub tail: f32,
    #[serde(default)]
    pub forked: bool,
    pub back: [f32; 3],
    pub belly: [f32; 3],
    pub wing: [f32; 3],
    /// The wingtips, a bird's beak, a butterfly's wing edge, a fish's fins.
    pub mark: [f32; 3],
}

fn half() -> f32 {
    0.5
}

struct Boxes(Vec<(Vec3, Vec3, [f32; 3])>);

impl Boxes {
    /// A box by its middle and its size.
    fn put(&mut self, middle: Vec3, size: Vec3, colour: [f32; 3]) {
        self.0.push((middle - size / 2.0, middle + size / 2.0, colour));
    }

    /// A box and its mirror across the body.
    fn pair(&mut self, middle: Vec3, size: Vec3, colour: [f32; 3]) {
        self.put(middle, size, colour);
        self.put(middle * Vec3::new(-1.0, 1.0, 1.0), size, colour);
    }

    /// A wing each side: flat boxes one after another from `from` out to `to` along the
    /// span, the chord and the thickness narrowing, swept back by `sweep` at the tip, the
    /// outer part in the mark's colour. Many short boxes, so the wing bends where it is held
    /// by two bones.
    #[allow(clippy::too_many_arguments)]
    fn wings(&mut self, from: f32, to: f32, at: Vec3, chord: (f32, f32), thick: (f32, f32), sweep: f32, colour: [f32; 3], tip: [f32; 3], pieces: usize) {
        for i in 0..pieces {
            let (a, b) = (i as f32 / pieces as f32, (i + 1) as f32 / pieces as f32);
            let mid = (a + b) / 2.0;
            let size = Vec3::new((to - from) * (b - a), thick.0 + (thick.1 - thick.0) * mid, chord.0 + (chord.1 - chord.0) * mid);
            let shade = 0.9 + 0.2 * ((i * 7 % 5) as f32 / 4.0);
            let colour = if mid > 0.72 { tip } else { colour }.map(|c| c * shade);
            self.pair(Vec3::new(from + (to - from) * mid, at.y, at.z - sweep * mid * mid), size, colour);
        }
    }
}

/// The boxes of a model, wings along x, facing +z, its belly at y = 0.
fn boxes(b: &Built) -> Result<Boxes> {
    let (l, half_span) = (b.length, b.span / 2.0);
    let mut out = Boxes(Vec::new());
    let dark = |c: [f32; 3], k: f32| c.map(|v| v * k);
    match b.form.as_str() {
        "bird" => {
            let (wide, deep) = (l * 0.3, l * 0.3);
            // Body: back over belly, narrowing toward the tail.
            out.put(Vec3::new(0.0, deep * 0.7, l * 0.05), Vec3::new(wide, deep * 0.6, l * 0.55), b.back);
            out.put(Vec3::new(0.0, deep * 0.25, l * 0.05), Vec3::new(wide * 0.9, deep * 0.5, l * 0.5), b.belly);
            out.put(Vec3::new(0.0, deep * 0.55, -l * 0.28), Vec3::new(wide * 0.7, deep * 0.6, l * 0.2), dark(b.back, 0.9));
            // Head and beak.
            out.put(Vec3::new(0.0, deep * 0.8, l * 0.4), Vec3::splat(l * 0.24), dark(b.back, 1.1));
            out.put(Vec3::new(0.0, deep * 0.72, l * 0.57), Vec3::new(l * 0.07, l * 0.06, l * 0.12), b.mark);
            // Tail: one fan, or two prongs.
            let long = l * b.tail;
            if b.forked {
                out.pair(Vec3::new(wide * 0.28, deep * 0.6, -l * 0.38 - long / 2.0), Vec3::new(wide * 0.22, l * 0.03, long), dark(b.wing, 0.85));
            } else {
                out.put(Vec3::new(0.0, deep * 0.6, -l * 0.38 - long / 2.0), Vec3::new(wide * 0.75, l * 0.035, long), dark(b.wing, 0.85));
            }
            let chord = l * b.chord;
            out.wings(wide * 0.4, half_span, Vec3::new(0.0, deep * 0.75, l * 0.08), (chord, chord * 0.45), (l * 0.06, l * 0.025), chord * 0.5, b.wing, b.mark, 7);
        }
        "insect" => {
            let thick = l * 0.14;
            // Head, thorax, and a long abdomen in pieces.
            out.put(Vec3::new(0.0, thick * 0.6, l * 0.44), Vec3::splat(thick * 1.1), dark(b.back, 0.8));
            out.put(Vec3::new(0.0, thick * 0.6, l * 0.25), Vec3::new(thick * 1.2, thick * 1.2, l * 0.26), b.back);
            for i in 0..4 {
                out.put(Vec3::new(0.0, thick * 0.55, l * 0.05 - l * 0.14 * i as f32), Vec3::new(thick * (0.9 - 0.12 * i as f32), thick * (0.9 - 0.12 * i as f32), l * 0.14), if i % 2 == 0 { b.belly } else { b.back });
            }
            // Two pairs of wings: the fore pair the larger. A broad chord is a butterfly's, a
            // narrow one a dragonfly's.
            let chord = l * b.chord;
            out.wings(thick * 0.5, half_span, Vec3::new(0.0, thick * 1.0, l * 0.3), (chord, chord * 0.75), (l * 0.012, l * 0.008), -chord * 0.25, b.wing, b.mark, 4);
            out.wings(thick * 0.5, half_span * 0.82, Vec3::new(0.0, thick * 0.95, l * 0.3 - chord * 0.95), (chord * 0.9, chord * 0.6), (l * 0.012, l * 0.008), chord * 0.2, dark(b.wing, 0.9), b.mark, 4);
        }
        "fish" => {
            let (deep, wide) = (l * 0.28, l * 0.14);
            // A body of slices, deepest a third of the way back, back over belly.
            let slices = 7;
            for i in 0..slices {
                let t = (i as f32 + 0.5) / slices as f32;
                let fat = (1.0 - (t - 0.35).abs() * 1.5).clamp(0.25, 1.0);
                let z = l * (0.5 - t);
                out.put(Vec3::new(0.0, deep * 0.5 + deep * 0.22 * fat, z), Vec3::new(wide * fat, deep * 0.5 * fat, l / slices as f32), dark(b.back, 0.9 + 0.2 * ((i % 2) as f32)));
                out.put(Vec3::new(0.0, deep * 0.5 - deep * 0.2 * fat, z), Vec3::new(wide * fat * 0.9, deep * 0.45 * fat, l / slices as f32), b.belly);
            }
            // The tail fin, upright; a fin on the back; and one each side (its `wings`).
            let long = l * b.tail;
            out.put(Vec3::new(0.0, deep * 0.5, -l * 0.5 - long * 0.3), Vec3::new(wide * 0.12, deep * 0.5, long * 0.6), b.mark);
            out.put(Vec3::new(0.0, deep * 0.5, -l * 0.5 - long * 0.8), Vec3::new(wide * 0.1, deep * 0.95, long * 0.4), b.mark);
            out.put(Vec3::new(0.0, deep * 0.95, l * 0.02), Vec3::new(wide * 0.1, deep * 0.22, l * 0.28), b.mark);
            out.wings(wide * 0.4, half_span, Vec3::new(0.0, deep * 0.35, l * 0.2), (l * 0.12, l * 0.06), (l * 0.02, l * 0.012), l * 0.08, b.wing, b.mark, 2);
        }
        other => bail!("no built form `{other}`: bird, insect or fish"),
    }
    Ok(out)
}

/// The model as a GLB with no skeleton: one mesh, a colour at every corner.
pub fn model(b: &Built) -> Result<Glb> {
    let boxes = boxes(b)?;
    let (mut positions, mut normals, mut colours, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::<u8>::new());
    let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
    let mut count = 0u32;
    for (from, to, colour) in &boxes.0 {
        for axis in 0..3 {
            for sign in [-1.0f32, 1.0] {
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let mut normal = Vec3::ZERO;
                normal[axis] = sign;
                // Lit a little from above even before the light: tops paler, undersides darker.
                let shade = 1.0 + 0.12 * normal.y;
                for (a, b) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                    let mut p = Vec3::ZERO;
                    p[axis] = if sign > 0.0 { to[axis] } else { from[axis] };
                    p[u] = if a == 1 { to[u] } else { from[u] };
                    p[v] = if b == 1 { to[v] } else { from[v] };
                    (lo, hi) = (lo.min(p), hi.max(p));
                    positions.extend(p.to_array());
                    normals.extend(normal.to_array());
                    colours.extend([colour[0] * shade, colour[1] * shade, colour[2] * shade, 1.0]);
                }
                let order: [u32; 6] = if sign > 0.0 { [0, 1, 2, 0, 2, 3] } else { [0, 2, 1, 0, 3, 2] };
                for i in order {
                    indices.extend((count + i).to_le_bytes());
                }
                count += 4;
            }
        }
    }
    let mut glb = Glb { json: json!({ "asset": { "version": "2.0", "generator": "terra-cube forge" }, "scene": 0, "scenes": [{ "nodes": [0] }], "nodes": [{ "name": "built", "mesh": 0 }] }), bin: Vec::new() };
    let position = glb.add_floats(&positions, "VEC3", false);
    glb.json["accessors"][position]["min"] = json!(lo.to_array());
    glb.json["accessors"][position]["max"] = json!(hi.to_array());
    let normal = glb.add_floats(&normals, "VEC3", false);
    let colour = glb.add_floats(&colours, "VEC4", false);
    let index = glb.add_raw(&indices, 5125, "SCALAR", count as usize / 4 * 6);
    glb.json["materials"] = json!([{ "name": "built", "pbrMetallicRoughness": { "baseColorFactor": [1.0, 1.0, 1.0, 1.0], "metallicFactor": 0.0, "roughnessFactor": 0.85 } }]);
    glb.json["meshes"] = json!([{ "primitives": [{ "attributes": { "POSITION": position, "NORMAL": normal, "COLOR_0": colour }, "indices": index, "material": 0 }] }]);
    Ok(glb)
}
