//! A creature rebuilt from cubes: `cargo forge cubes ID` takes a finished model and makes
//! `assets/creatures/ID-cubes/`, the same skeleton and clips with the skin replaced by little
//! cubes, so it is made of the same stuff as the world (a bit is 5 cm).
//!
//! The model is stood as it stands in its idle, so the cubes line up with the world's when it
//! is at ease. Its surface is sampled into a grid, the inside filled, and each cube given to
//! the one bone that pulls on it most and the colour most of the texture there has. A cube
//! moves with its bone and never bends: limbs turn as rigid pieces, like a jointed toy.
//!
//! A face needs help: a head of true size is four cubes wide, and eyes are smaller than a
//! cube, so they vanish. The head is made bigger first (`--head`, 1.4 times: a toy's
//! proportions), and the two cubes on its surface where the texture is darkest against its
//! surroundings, one each side, are made eyes.

use crate::glb::Glb;
use crate::rig::Skeleton;
use crate::{clips, spec};
use anyhow::{Context, Result, bail, ensure};
use glam::{IVec3, Mat4, Vec2, Vec3};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};

struct Cube {
    colour: Vec3,
    bone: usize,
}

/// What the surface samples that fall in a cube say of it.
#[derive(Default)]
struct Tally {
    colours: Vec<Vec3>,
    pull: HashMap<usize, f32>,
}

fn light(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

impl Tally {
    /// The colour most of its samples have, not their average: a cube half tunic and half
    /// belt is tunic, where the average is a colour the model has nowhere.
    fn colour(&self) -> Vec3 {
        let bucket = |c: &Vec3| (c.powf(1.0 / 2.2) * 5.0).round().as_ivec3();
        let mut counts: HashMap<IVec3, (Vec3, f32)> = HashMap::new();
        for c in &self.colours {
            let entry = counts.entry(bucket(c)).or_default();
            *entry = (entry.0 + *c, entry.1 + 1.0);
        }
        counts.into_iter().max_by(|a, b| a.1.1.total_cmp(&b.1.1).then(a.0.to_array().cmp(&b.0.to_array()))).map_or(Vec3::splat(0.5), |(_, (sum, n))| sum / n)
    }

    /// How much of it is near black: a pupil is, and hair, fur and shadow are not.
    fn dark(&self) -> f32 {
        self.colours.iter().filter(|c| light(**c) < 0.03).count() as f32 / self.colours.len().max(1) as f32
    }
}

/// A face: its normal, then its two in-plane axes (u, v) such that u × v = normal.
const FACES: [(IVec3, IVec3, IVec3); 6] = [
    (IVec3::X, IVec3::Y, IVec3::Z),
    (IVec3::NEG_X, IVec3::Z, IVec3::Y),
    (IVec3::Y, IVec3::Z, IVec3::X),
    (IVec3::NEG_Y, IVec3::X, IVec3::Z),
    (IVec3::Z, IVec3::X, IVec3::Y),
    (IVec3::NEG_Z, IVec3::Y, IVec3::X),
];

fn linear(srgb: u8) -> f32 {
    let c = srgb as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// The model's colour texture, if it has one.
fn texture(glb: &Glb, primitive: &Value) -> Option<image::RgbImage> {
    let material = &glb.array("materials")[primitive["material"].as_u64()? as usize];
    let texture = &glb.array("textures")[material["pbrMetallicRoughness"]["baseColorTexture"]["index"].as_u64()? as usize];
    let view = &glb.array("bufferViews")[glb.array("images")[texture["source"].as_u64()? as usize]["bufferView"].as_u64()? as usize];
    let from = view["byteOffset"].as_u64().unwrap_or(0) as usize;
    image::load_from_memory(&glb.bin[from..from + view["byteLength"].as_u64()? as usize]).ok().map(|i| i.to_rgb8())
}

/// The cubes of a model stood in a pose: its surface sampled, its inside filled.
fn cubes(glb: &Glb, sk: &Skeleton, world: &[Mat4], size: f32, head: &[usize]) -> Result<HashMap<IVec3, Cube>> {
    let skin: Vec<Mat4> = world.iter().zip(&sk.inverse_bind).map(|(w, ib)| *w * *ib).collect();
    let mut shell: HashMap<IVec3, Tally> = HashMap::new();
    for mesh in glb.array("meshes") {
        for p in mesh["primitives"].as_array().into_iter().flatten() {
            let attr = |name: &str| p["attributes"][name].as_u64().map(|a| a as usize);
            let (Some(pos), Some(j), Some(w)) = (attr("POSITION"), attr("JOINTS_0"), attr("WEIGHTS_0")) else { continue };
            let rest: Vec<Vec3> = glb.floats(pos)?.0.chunks(3).map(Vec3::from_slice).collect();
            let joints: Vec<[usize; 4]> = glb.floats(j)?.0.chunks(4).map(|c| [c[0] as usize, c[1] as usize, c[2] as usize, c[3] as usize]).collect();
            let weights: Vec<[f32; 4]> = glb.floats(w)?.0.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
            let uvs: Vec<Vec2> = match attr("TEXCOORD_0") {
                Some(a) => glb.floats(a)?.0.chunks(2).map(Vec2::from_slice).collect(),
                None => vec![Vec2::ZERO; rest.len()],
            };
            let picture = texture(glb, p);
            let stood: Vec<Vec3> = (0..rest.len()).map(|i| (0..4).map(|k| skin[joints[i][k]].transform_point3(rest[i]) * weights[i][k]).sum::<Vec3>() / weights[i].iter().sum::<f32>().max(1e-6)).collect();
            let index: Vec<usize> = match p["indices"].as_u64() {
                Some(a) => glb.floats(a as usize)?.0.iter().map(|&i| i as usize).collect(),
                None => (0..rest.len()).collect(),
            };
            for t in index.chunks_exact(3) {
                let [a, b, c] = [stood[t[0]], stood[t[1]], stood[t[2]]];
                // Points across the triangle, closer together than a third of a cube.
                let n = ((a.distance(b).max(b.distance(c)).max(c.distance(a)) / (size / 3.0)).ceil() as usize).max(1);
                for i in 0..=n {
                    for k in 0..=n - i {
                        let (u, v) = (i as f32 / n as f32, k as f32 / n as f32);
                        let part = [1.0 - u - v, u, v];
                        let point = a * part[0] + b * part[1] + c * part[2];
                        let uv = uvs[t[0]] * part[0] + uvs[t[1]] * part[1] + uvs[t[2]] * part[2];
                        let colour = picture.as_ref().map_or(Vec3::splat(0.5), |img| {
                            let (x, y) = ((uv.x.rem_euclid(1.0) * img.width() as f32) as u32, (uv.y.rem_euclid(1.0) * img.height() as f32) as u32);
                            let px = img.get_pixel(x.min(img.width() - 1), y.min(img.height() - 1));
                            Vec3::new(linear(px[0]), linear(px[1]), linear(px[2]))
                        });
                        let tally = shell.entry((point / size).floor().as_ivec3()).or_default();
                        tally.colours.push(colour);
                        for (corner, share) in t.iter().zip(part) {
                            for k in 0..4 {
                                *tally.pull.entry(joints[*corner][k]).or_default() += weights[*corner][k] * share;
                            }
                        }
                    }
                }
            }
        }
    }
    ensure!(!shell.is_empty(), "no skinned mesh to make cubes of");
    let mut out: HashMap<IVec3, Cube> = shell
        .iter()
        .map(|(at, t)| (*at, Cube { colour: t.colour(), bone: t.pull.iter().max_by(|a, b| a.1.total_cmp(b.1).then(a.0.cmp(b.0))).map_or(0, |b| *b.0) }))
        .collect();

    // Eyes: on the front of the head (a finished model faces +Z), the pair of cubes mirrored
    // across its middle, at least a cube apart, with the most black in them. Each becomes
    // one dark cube.
    let of_head = |at: &IVec3| out.get(at).is_some_and(|c| head.contains(&c.bone));
    let on_head: Vec<IVec3> = out.keys().filter(|at| of_head(at) && !(1..4).any(|ahead| of_head(&(**at + IVec3::Z * ahead)))).copied().collect();
    let mirrored = |at: &IVec3| IVec3::new(-1 - at.x, at.y, at.z);
    let pair = on_head
        .iter()
        .filter(|at| at.x >= 1 && on_head.contains(&mirrored(at)))
        .map(|at| (*at, shell[at].dark().min(shell[&mirrored(at)].dark())))
        .filter(|(_, dark)| *dark > 0.04)
        .max_by(|a, b| a.1.total_cmp(&b.1).then(a.0.to_array().cmp(&b.0.to_array())));
    if let Some((at, _)) = pair {
        for eye in [at, mirrored(&at)] {
            out.get_mut(&eye).unwrap().colour = Vec3::splat(0.012);
        }
    }

    // The inside: whatever the outside cannot reach from a corner of the box round it. Each
    // inside cube takes after the nearest surface one, so a limb that turns shows no hollow.
    let (lo, hi) = out.keys().fold((IVec3::MAX, IVec3::MIN), |(lo, hi), k| (lo.min(*k), hi.max(*k)));
    let (lo, hi) = (lo - 1, hi + 1);
    let inside_box = |p: IVec3| p.cmpge(lo).all() && p.cmple(hi).all();
    let mut outside = std::collections::HashSet::from([lo]);
    let mut queue = VecDeque::from([lo]);
    while let Some(at) = queue.pop_front() {
        for (n, ..) in FACES {
            let next = at + n;
            if inside_box(next) && !out.contains_key(&next) && outside.insert(next) {
                queue.push_back(next);
            }
        }
    }
    let mut queue: VecDeque<IVec3> = out.keys().copied().collect();
    while let Some(at) = queue.pop_front() {
        let (colour, bone) = (out[&at].colour, out[&at].bone);
        for (n, ..) in FACES {
            let next = at + n;
            if inside_box(next) && !outside.contains(&next) && !out.contains_key(&next) {
                out.insert(next, Cube { colour, bone });
                queue.push_back(next);
            }
        }
    }
    Ok(out)
}

/// Makes `ID-cubes` from the finished model of `id`, its head `head_scale` times its size.
/// Returns one line of what it made.
pub fn run(id: &str, size: f32, head_scale: f32) -> Result<String> {
    let from = spec::dir(id);
    let mut glb = Glb::read(&from.join("model.glb"))?;
    let sk = Skeleton::read(&glb)?;
    let clips = clips::read(&glb, &sk)?;
    let stand = clips.iter().find(|c| c.name == "idle").map_or(sk.rest(), |c| c.frames[0].clone());
    let world = sk.pose_world(&stand);
    // The head is everything past the neck, and grows about where it joins it.
    let mut meta: spec::Meta = spec::load(&from.join("meta.toml"))?;
    let neck = meta.bones.get("neck").and_then(|name| sk.joints.iter().position(|j| &j.name == name));
    let mut head: Vec<usize> = neck.map_or(Vec::new(), |n| sk.joints[n].children.clone());
    let mut next = 0;
    while next < head.len() {
        head.extend(sk.joints[head[next]].children.clone());
        next += 1;
    }
    let mut bigger = world.clone();
    if let Some(&first) = head.first() {
        let pivot = world[first].w_axis.truncate();
        let grow = Mat4::from_translation(pivot) * Mat4::from_scale(Vec3::splat(head_scale)) * Mat4::from_translation(-pivot);
        for &j in &head {
            bigger[j] = grow * world[j];
        }
    }
    let made = cubes(&glb, &sk, &bigger, size, &head)?;

    // Every face of a cube that is not against another cube of the same bone, taken back to
    // where its bone has it at rest, which is where a skin's vertices are kept.
    let back: Vec<Mat4> = world.iter().zip(&sk.inverse_bind).map(|(w, ib)| (*w * *ib).inverse()).collect();
    let mut keys: Vec<IVec3> = made.keys().copied().collect();
    keys.sort_by_key(|k| (k.y, k.z, k.x));
    let (mut positions, mut normals, mut colours, mut weights) = (Vec::<f32>::new(), Vec::<f32>::new(), Vec::<f32>::new(), Vec::<f32>::new());
    let (mut joints, mut indices) = (Vec::<u8>::new(), Vec::<u8>::new());
    let (mut lo, mut hi, mut count) = (Vec3::MAX, Vec3::MIN, 0u32);
    for at in keys {
        let cube = &made[&at];
        // A little lighter or darker a cube, as the ground is, so a flat colour has grain.
        let grain = 0.92 + 0.16 * ((at.x.wrapping_mul(73856093) ^ at.y.wrapping_mul(19349663) ^ at.z.wrapping_mul(83492791)) as u32 % 1000) as f32 / 1000.0;
        for (n, u, v) in FACES {
            if made.get(&(at + n)).is_some_and(|other| other.bone == cube.bone) {
                continue;
            }
            let normal = back[cube.bone].transform_vector3(n.as_vec3()).normalize();
            for (su, sv) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                let corner = back[cube.bone].transform_point3((at + n.max(IVec3::ZERO) + u * su + v * sv).as_vec3() * size);
                (lo, hi) = (lo.min(corner), hi.max(corner));
                positions.extend(corner.to_array());
                normals.extend(normal.to_array());
                colours.extend([cube.colour.x * grain, cube.colour.y * grain, cube.colour.z * grain, 1.0]);
                weights.extend([1.0, 0.0, 0.0, 0.0]);
                joints.extend([cube.bone as u16, 0, 0, 0].iter().flat_map(|j| j.to_le_bytes()));
            }
            indices.extend([count, count + 1, count + 2, count, count + 2, count + 3].iter().flat_map(|i| i.to_le_bytes()));
            count += 4;
        }
    }

    let meshes = glb.json["meshes"].as_array().map_or(0, Vec::len);
    if meshes != 1 {
        bail!("{id} has {meshes} meshes; cubes are made of a model with one");
    }
    let position = glb.add_floats(&positions, "VEC3", false);
    glb.json["accessors"][position]["min"] = json!(lo.to_array());
    glb.json["accessors"][position]["max"] = json!(hi.to_array());
    let attributes = json!({
        "POSITION": position,
        "NORMAL": glb.add_floats(&normals, "VEC3", false),
        "COLOR_0": glb.add_floats(&colours, "VEC4", false),
        "WEIGHTS_0": glb.add_floats(&weights, "VEC4", false),
        "JOINTS_0": glb.add_raw(&joints, 5123, "VEC4", count as usize),
    });
    let index = glb.add_raw(&indices, 5125, "SCALAR", indices.len() / 4);
    // Its colour is in its cubes: no texture, so none is kept.
    glb.json["materials"] = json!([{ "name": "cubes", "pbrMetallicRoughness": { "metallicFactor": 0.0, "roughnessFactor": 0.9 } }]);
    glb.json["meshes"][0]["primitives"] = json!([{ "attributes": attributes, "indices": index, "material": 0 }]);
    for unused in ["textures", "images", "samplers"] {
        glb.json.as_object_mut().context("a GLB's JSON is an object")?.remove(unused);
    }
    glb.drop_unused();

    let to = spec::dir(&format!("{id}-cubes"));
    glb.write(&to.join("model.glb"))?;
    meta.triangles = glb.triangles();
    spec::save(&to.join("meta.toml"), &meta, &format!("# Made by `cargo forge cubes {id}` from that model; do not edit.\n"))?;
    // The same creature, marked as made from another so forge does not try to build it, and
    // with no place in the world of its own.
    let mut text = String::new();
    for line in std::fs::read_to_string(from.join("creature.toml"))?.lines() {
        if line.trim() == "[life]" {
            break;
        }
        text += &match line.strip_prefix("name = \"") {
            Some(name) => format!("name = \"{} (cubes)\"\n", name.trim_end_matches('"')),
            None => format!("{line}\n"),
        };
    }
    std::fs::write(to.join("creature.toml"), format!("{}\ncubes_of = \"{id}\"\n", text.trim_end()))?;
    let bones: std::collections::HashSet<usize> = made.values().map(|c| c.bone).collect();
    Ok(format!("{id}-cubes: {} cubes of {:.0} cm on {} of {} bones · {} triangles (was {}) · {} kB", made.len(), size * 100.0, bones.len(), sk.joints.len(), meta.triangles, spec::load::<spec::Meta>(&from.join("meta.toml"))?.triangles, std::fs::metadata(to.join("model.glb"))?.len() / 1000))
}
