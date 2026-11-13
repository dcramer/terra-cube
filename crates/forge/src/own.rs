//! A four-legged model given our own skeleton: the one the box creature of `cube_anim` has,
//! sized to this mesh and put inside it, with the skin bound to it here. Nothing of Tripo's
//! rig is used; a bare mesh is all that is asked of whoever made the model. Since the bones
//! are the walker's own, what moves the boxes moves the model joint for joint: its gaits and
//! its lying down are the walker's, written down, with nothing solved again on the model.
//!
//! `[plans.walked]` (2026-11-12; the wolf), after a week in which
//! nearly every fault traced to a rig read by its shape: bones named anyhow, a hind leg a
//! bone short, legs hung from stubs of the spine, skin that tore.

use crate::clips::{self, Clip, FPS};
use crate::glb::Glb;
use crate::rig::{Leg, Pose, Roles, Skeleton};
use anyhow::{Context, Result, bail};
use cube_anim::{Build, Shape, Walker};
use glam::{Mat3, Mat4, Quat, Vec3};
use serde_json::json;

/// The bones, in the order the skin lists them: the trunk (the root), neck, head and two of
/// tail; then each leg in the walker's order (fore left, fore right, hind left, hind right),
/// a fore leg's three and a hind leg's four.
pub const BONES: [&str; 21] = [
    "trunk", "neck", "head", "tail_1", "tail_2", "fl_upper", "fl_fore", "fl_paw", "fr_upper", "fr_fore", "fr_paw", "hl_thigh", "hl_shank", "hl_lower", "hl_paw", "hr_thigh", "hr_shank", "hr_lower", "hr_paw", "tail_3", "tail_4",
];
/// The tail, root to end: four bones (the last two added after the rest, 2026-11-12: on two,
/// a pony's long tail swung clear of the ground was a hinge, and its hair was pulled to
/// more than twice its length).
const TAIL: [usize; 4] = [3, 4, 19, 20];
const PARENTS: [Option<usize>; 21] = [None, Some(0), Some(1), Some(0), Some(3), Some(0), Some(5), Some(6), Some(0), Some(8), Some(9), Some(0), Some(11), Some(12), Some(13), Some(0), Some(15), Some(16), Some(17), Some(4), Some(19)];
/// The clips that are the walker's own: the rest are made or carried as any four-legged animal's.
pub const WALKED: [&str; 5] = ["idle", "walk", "run", "sprint", "lie"];
/// Each leg's bones, from where it leaves the trunk down.
const LEGS: [&[usize]; 4] = [&[5, 6, 7], &[8, 9, 10], &[11, 12, 13, 14], &[15, 16, 17, 18]];

/// A model rigged here: the walker's build of it in metres, and how the walker's frame (x
/// its right, y up, z ahead, from the ground under its middle) lies in the file's.
pub struct Own {
    build: Build,
    /// Its legs as the mesh has them.
    shape: Shape,
    /// Metres to a unit of the file.
    metres: f32,
    origin: Vec3,
    right: Vec3,
    ahead: Vec3,
    /// Where the tail ends, from its last bone, and how thick it is; the file's units.
    tip: Vec3,
    thick: f32,
}

/// Whether a model was rigged here: it has our bones.
pub fn made(glb: &Glb) -> bool {
    glb.array("nodes").iter().any(|n| n["name"] == "hl_shank")
}

fn to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let along = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
    p.distance(a + (b - a) * along)
}

impl Own {
    /// A point of the walker's, in the file.
    fn file(&self, w: Vec3) -> Vec3 {
        self.origin + (self.right * w.x + Vec3::Y * w.y + self.ahead * w.z) / self.metres
    }

    /// Gives a bare mesh that stands on four legs its skeleton and skin. `height` is how tall
    /// the creature is, metres.
    pub fn rig(glb: &mut Glb, height: f32) -> Result<Self> {
        // Whatever rig it came with is taken off: its skin, what moved it, and the say its
        // bones had over each point. (Its bones stay in the file as nodes that hold nothing.)
        let rigged = !glb.array("skins").is_empty();
        if rigged {
            glb.json.as_object_mut().context("no model")?.remove("animations");
            for node in glb.json["nodes"].as_array_mut().context("no nodes")? {
                node.as_object_mut().map(|n| n.remove("skin"));
            }
            for mesh in glb.json["meshes"].as_array_mut().context("no meshes")? {
                for p in mesh["primitives"].as_array_mut().into_iter().flatten() {
                    for key in ["JOINTS_0", "WEIGHTS_0"] {
                        p["attributes"].as_object_mut().map(|a| a.remove(key));
                    }
                }
            }
        }
        let nodes = glb.array("nodes").to_vec();
        let holder = nodes.iter().position(|n| n.get("mesh").is_some()).context("no node carries a mesh")?;
        let local = |n: &serde_json::Value| -> Mat4 {
            if let Some(m) = n.get("matrix").and_then(|m| m.as_array()) {
                return Mat4::from_cols_slice(&m.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect::<Vec<_>>());
            }
            let f = |key: &str, default: &[f32]| -> Vec<f32> { n.get(key).and_then(|a| a.as_array()).map_or(default.to_vec(), |a| a.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect()) };
            Mat4::from_scale_rotation_translation(Vec3::from_slice(&f("scale", &[1.0; 3])), Quat::from_slice(&f("rotation", &[0.0, 0.0, 0.0, 1.0])).normalize(), Vec3::from_slice(&f("translation", &[0.0; 3])))
        };
        let mut placed = if rigged { Mat4::IDENTITY } else { local(&nodes[holder]) };
        let mut at = holder;
        while !rigged && let Some(parent) = nodes.iter().position(|n| n["children"].as_array().is_some_and(|c| c.iter().any(|c| c.as_u64() == Some(at as u64)))) {
            placed = local(&nodes[parent]) * placed;
            at = parent;
        }
        let mesh = nodes[holder]["mesh"].as_u64().unwrap() as usize;
        let primitives = glb.array("meshes")[mesh]["primitives"].as_array().cloned().unwrap_or_default();
        let mut points: Vec<Vec<Vec3>> = Vec::new();
        for p in &primitives {
            let (data, width) = glb.floats(p["attributes"]["POSITION"].as_u64().context("a primitive without POSITION")? as usize)?;
            points.push(data.chunks(width).map(|c| placed.transform_point3(Vec3::new(c[0], c[1], c[2]))).collect());
        }
        let all: Vec<Vec3> = points.iter().flatten().copied().collect();
        let (lo, hi) = all.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let size = hi - lo;
        // It is long the way it is longer, and its head is at the end that stands higher.
        let (mut ahead, side) = if size.z >= size.x { (Vec3::Z, Vec3::X) } else { (Vec3::X, Vec3::Z) };
        let mid = (lo + hi) / 2.0;
        let high: Vec<Vec3> = all.iter().copied().filter(|p| p.y > lo.y + size.y * 0.8).collect();
        if high.iter().map(|p| (*p - mid).dot(ahead)).sum::<f32>() < 0.0 {
            ahead = -ahead;
        }
        let right = Vec3::Y.cross(ahead);
        let _ = side;
        let (z, x) = (|p: Vec3| (p - mid).dot(ahead), |p: Vec3| (p - mid).dot(right));
        // Its feet: what of it is near the ground, fore and hind, and where the middle of each lot is.
        let low: Vec<Vec3> = all.iter().copied().filter(|p| p.y < lo.y + size.y * 0.06).collect();
        let mean = |of: &dyn Fn(&Vec3) -> bool, f: &dyn Fn(Vec3) -> f32| {
            let some: Vec<f32> = low.iter().filter(|p| of(p)).map(|p| f(*p)).collect();
            (!some.is_empty()).then(|| some.iter().sum::<f32>() / some.len() as f32)
        };
        let split = low.iter().map(|p| z(*p)).sum::<f32>() / low.len().max(1) as f32;
        let (Some(fore), Some(hind)) = (mean(&|p| z(*p) > split, &|p| z(p)), mean(&|p| z(*p) <= split, &|p| z(p))) else { bail!("no feet found near the ground: is this a model standing on four legs?") };
        let wide = mean(&|_| true, &|p| x(p).abs()).unwrap_or(size.dot(right.abs()) * 0.3) * 2.0;
        // Its trunk, between its legs: how low its belly is and how high its back.
        let between: Vec<Vec3> = all.iter().copied().filter(|p| z(*p) > hind + (fore - hind) * 0.3 && z(*p) < hind + (fore - hind) * 0.7).collect();
        let (belly, back) = between.iter().fold((f32::MAX, f32::MIN), |(b, t), p| (b.min(p.y), t.max(p.y)));
        if between.is_empty() || belly - lo.y < size.y * 0.15 {
            bail!("no trunk found standing over its legs");
        }
        let metres = height / size.y;
        let m = |file: f32| file * metres;
        let (front, rear) = (all.iter().map(|p| z(*p)).fold(f32::MIN, f32::max), all.iter().map(|p| z(*p)).fold(f32::MAX, f32::min));
        let deep = m(back - belly);
        let build = Build { long: m(fore - hind), wide: m(wide), tall: m((back + belly) / 2.0 - lo.y), deep, leg: m(belly - lo.y) * 0.13, neck: m(front - fore) * 0.6, head: m(front - fore) * 0.5, tail: m(hind - rear) };
        let origin = mid.with_y(lo.y) + ahead * (fore + hind) / 2.0;
        // Its legs, read off the mesh: each leg's skin cut into slices from the ground to
        // the belly, and the middle of each slice. Its paw is the lowest; a fore leg's elbow
        // is where it meets the chest; a hind leg's hock is where the leg is furthest back
        // below its middle and its knee where it is furthest ahead above that. Where a leg
        // leaves the trunk is inside the body and cannot be seen: it is put over the elbow
        // or knee as an animal's is. (Until this the joints were a dog's of the same size,
        // and the model stood crouched on them: its own joints were somewhere else.)
        let walk = |p: Vec3| Vec3::new((p - origin).dot(right), p.y - lo.y, (p - origin).dot(ahead)) * metres;
        let under = m(belly - lo.y);
        // (Read off the skin's faces, not its points alone: a leg drawn as a few long faces
        // has points only at its ends, and a bear's legs were not found for want of them.)
        let mut sampled = all.clone();
        for (p, pts) in primitives.iter().zip(&points) {
            let corners: Vec<usize> = match p["indices"].as_u64() {
                Some(accessor) => glb.floats(accessor as usize)?.0.iter().map(|i| *i as usize).collect(),
                None => (0..pts.len()).collect(),
            };
            for t in corners.chunks_exact(3) {
                let (a, b, c) = (pts[t[0]], pts[t[1]], pts[t[2]]);
                for i in 0..=4 {
                    for j in 0..=4 - i {
                        let (u, v) = (i as f32 / 4.0, j as f32 / 4.0);
                        sampled.push(a * (1.0 - u - v) + b * u + c * v);
                    }
                }
            }
        }
        let centres = |n: usize| -> Vec<Vec3> {
            let (front, left) = (n < 2, n.is_multiple_of(2));
            // (Only what is over its own foot, within a quarter of the way to the other pair: a
            // fox's brush hangs behind its hind legs and was read as part of them, its hock
            // put half a body behind it.)
            let over = if front { fore } else { hind };
            let own: Vec<Vec3> = sampled.iter().copied().filter(|p| (x(*p) < 0.0) == left && x(*p).abs() > wide * 0.1 && (z(*p) - over).abs() < (fore - hind) * 0.28 && p.y < belly).map(walk).collect();
            // (Ten slices, and three points make one: a bear's legs, drawn with few points,
            // had too few in each of fourteen to be found at all.)
            (0..10).filter_map(|k| {
                let (from, to) = (under * k as f32 / 10.0, under * (k + 1) as f32 / 10.0);
                let slice: Vec<Vec3> = own.iter().copied().filter(|p| p.y >= from && p.y < to).collect();
                (slice.len() >= 3).then(|| (slice.iter().sum::<Vec3>() / slice.len() as f32).with_y((from + to) / 2.0))
            }).collect()
        };
        let middle = Vec3::new(0.0, build.tall, 0.0);
        let dog = Walker::new(build, Vec3::ZERO, 0.0).limbs();
        let (mut hips, mut bone, mut lower, mut stand, mut shank) = ([Vec3::ZERO; 4], [(0.0, 0.0); 4], [0.0; 4], [glam::Vec2::ZERO; 4], 0.24f32);
        let mut read = Vec::new();
        for n in 0..4 {
            let line = centres(n);
            if std::env::var_os("FORGE_TRACE").is_some() {
                println!("own: leg {n}: {} of 10 slices found · belly {:.2} m up · feet {:.2} m apart · {:.2} m wide", line.len(), under, m(fore - hind), m(wide));
            }
            // (Not found: a dog's, of its size.)
            let fallback = |hips: &mut [Vec3; 4], bone: &mut [(f32, f32); 4], lower: &mut [f32; 4]| {
                (hips[n], bone[n]) = (dog[n][0] - middle, (dog[n][0].distance(dog[n][1]), dog[n][1].distance(dog[n][2])));
                lower[n] = if n < 2 { 0.0 } else { dog[n][2].distance(dog[n][3]) };
            };
            let (Some(first), Some(last)) = (line.first().filter(|_| line.len() >= 6), line.last()) else {
                fallback(&mut hips, &mut bone, &mut lower);
                continue;
            };
            let (paw, top) = (first.with_y(0.0), last.with_y(under));
            if n < 2 {
                let shoulder = top + Vec3::new(0.0, under * 0.3, under * 0.1);
                (hips[n], bone[n], stand[n]) = (shoulder - middle, (shoulder.distance(top), top.distance(paw)), glam::Vec2::new(paw.x - shoulder.x, paw.z - shoulder.z));
                read.push(format!("elbow {:.2}", top.z));
            } else {
                let low: Vec<&Vec3> = line.iter().filter(|p| p.y > under * 0.2 && p.y < under * 0.55).collect();
                let Some(hock) = low.iter().copied().copied().reduce(|a, b| if b.z < a.z { b } else { a }).filter(|h| paw.z - h.z > under * 0.02) else {
                    fallback(&mut hips, &mut bone, &mut lower);
                    continue;
                };
                let knee = line.iter().copied().filter(|p| p.y > hock.y + under * 0.2).reduce(|a, b| if b.z > a.z { b } else { a }).unwrap_or(top);
                let hip = knee + Vec3::new(0.0, under * 0.34, -under * 0.26);
                (hips[n], bone[n], lower[n], stand[n]) = (hip - middle, (hip.distance(knee), knee.distance(hock)), hock.distance(paw), glam::Vec2::new(paw.x - hip.x, paw.z - hip.z));
                // (No further forward than two parts in five of its height: a lynx's broad hind
                // paw put the middle of its foot far ahead of a low hock, 0.68, and the part
                // from hock to foot was laid so far over that the leg folded at every step.)
                shank = ((paw.z - hock.z) / hock.y.max(1e-3)).clamp(0.0, 0.4);
                read.push(format!("knee {:.2},{:.2} hock {:.2},{:.2}", knee.z, knee.y, hock.z, hock.y));
            }
        }
        // (The two of a pair are one leg seen twice: made alike, so it goes straight.)
        for pair in [[0, 1], [2, 3]] {
            let (a, b) = (pair[0], pair[1]);
            let both = |p: Vec3, q: Vec3| Vec3::new((q.x - p.x) / 2.0, (p.y + q.y) / 2.0, (p.z + q.z) / 2.0);
            let h = both(hips[a], hips[b]);
            (hips[a], hips[b]) = (h.with_x(-h.x.abs()), h.with_x(h.x.abs()));
            bone[a] = ((bone[a].0 + bone[b].0) / 2.0, (bone[a].1 + bone[b].1) / 2.0);
            (bone[b], lower[a]) = (bone[a], (lower[a] + lower[b]) / 2.0);
            lower[b] = lower[a];
            let st = glam::Vec2::new((stand[b].x - stand[a].x) / 2.0, (stand[a].y + stand[b].y) / 2.0);
            (stand[a], stand[b]) = (glam::Vec2::new(-st.x, st.y), st);
        }
        let shape = Shape { hips, bone, lower, stand, shank, lain: build.deep / 2.0 };
        if std::env::var_os("FORGE_TRACE").is_some() {
            println!("own: legs read off the mesh: {} · {shape:.2?}", read.join(" · "));
        }
        let mut own = Self { build, shape, metres, origin, right, ahead, tip: Vec3::ZERO, thick: 0.0 };

        // Where each bone is as it stands: the walker's own joints, stood.
        let stood = Walker::shaped(build, shape, Vec3::ZERO, 0.0);
        let limbs = stood.limbs();
        let top = Vec3::new(0.0, build.tall + build.deep * 0.2, 0.0);
        let part = |of: &dyn Fn(f32) -> bool, or: Vec3| {
            let some: Vec<Vec3> = all.iter().copied().filter(|p| of(z(*p))).collect();
            if some.is_empty() { or } else { some.iter().sum::<Vec3>() / some.len() as f32 }
        };
        let head = part(&|at| at > front - (front - fore) * 0.35, own.file(top + Vec3::Z * (build.long / 2.0 + build.neck)));
        let tip = part(&|at| at < rear + (hind - rear) * 0.2, own.file(top - Vec3::Z * (build.long / 2.0 + build.tail)));
        let tail = own.file(top - Vec3::Z * build.long / 2.0);
        let along = |t: f32| tail.lerp(tip, t);
        (own.tip, own.thick) = (tip + (tip - tail) * 0.1 - along(0.75), build.leg / metres);
        let mut places = vec![own.file(stood.trunk().0), own.file(top + Vec3::Z * build.long / 2.0), head, tail, along(0.25)];
        for limb in &limbs {
            places.extend(limb.iter().map(|p| own.file(*p)));
        }
        places.extend([along(0.5), along(0.75)]);
        // What holds each point of the skin: each bone is a line with a thickness, and a point
        // is held most by what it is nearest. A leg's bones hold nothing on the other side.
        let leg = build.leg / metres;
        let mut lines: Vec<(usize, Vec3, Vec3, f32, f32)> = vec![
            (0, own.file(Vec3::new(0.0, build.tall, -build.long / 2.0)), own.file(Vec3::new(0.0, build.tall, build.long / 2.0)), deep / metres * 0.45, 0.0),
            (1, places[1], head, deep / metres * 0.3, 0.0),
            (2, head, head + ahead * (front - z(head)).max(leg), deep / metres * 0.3, 0.0),
            (3, tail, along(0.25), leg, 0.0),
            (4, along(0.25), along(0.5), leg, 0.0),
            (19, along(0.5), along(0.75), leg, 0.0),
            (20, along(0.75), tip, leg, 0.0),
        ];
        for (n, bones) in LEGS.iter().enumerate() {
            let sided = if n % 2 == 0 { -1.0 } else { 1.0 };
            for (k, &bone) in bones.iter().enumerate() {
                let to = bones.get(k + 1).map_or(places[bone] + ahead * leg, |next| places[*next]);
                lines.push((bone, places[bone], to, leg * 0.6, sided));
            }
        }
        let soft = (belly - lo.y) * 0.04;
        for (p, pts) in points.iter().enumerate() {
            // Every point's hold by every bone, by how near it is. Points at one place are one
            // point: the skin is cut along the seams of its picture and must not come apart
            // there. (Tried 2026-11-12 and taken out: smoothing the hold over the mesh, each
            // point taking half of its neighbours', as is done for bodies modelled in code.
            // On this mesh two passes took a walk's stretch from 66% to 111% and lying from
            // 81% to 224%: a wide blend between a leg and the trunk drags the skin.)
            let mut place: std::collections::HashMap<[i32; 3], usize> = std::collections::HashMap::new();
            let grain = size.y / 20000.0;
            let welded: Vec<usize> = pts.iter().map(|at| { let n = place.len(); *place.entry([(at.x / grain).round() as i32, (at.y / grain).round() as i32, (at.z / grain).round() as i32]).or_insert(n) }).collect();
            let mut hold = vec![[0.0f32; BONES.len()]; place.len()];
            for (at, w) in pts.iter().zip(&welded) {
                // (A leg holds what is over its own foot and no more, and the tail what is
                // behind the hind feet: a pony's tail hangs down its hind legs to the hocks,
                // each held some of the other, and lying or dying tore both.)
                let reach = (fore - hind) * 0.33;
                // (And what hangs on the body's middle line behind the hind feet is tail, not
                // leg: the legs stand either side of it.)
                let between = z(*at) < hind && x(*at).abs() < wide * 0.2;
                let mine = |bone: usize| match bone {
                    3 | 4 | 19 | 20 => z(*at) < hind - reach * 0.3 || between,
                    5..=10 => (z(*at) - fore).abs() < reach,
                    11..=18 => (z(*at) - hind).abs() < reach && !between,
                    _ => true,
                };
                for l in lines.iter().filter(|l| mine(l.0) && (l.4 == 0.0 || x(*at) * l.4 > -wide * 0.05)) {
                    hold[*w][l.0] = hold[*w][l.0].max(1.0 / ((to_segment(*at, l.1, l.2) - l.3).max(0.0) + soft).powi(4));
                }
            }
            for h in &mut hold {
                let sum = h.iter().sum::<f32>().max(1e-9);
                h.iter_mut().for_each(|v| *v /= sum);
            }
            let (mut joints, mut weights) = (Vec::with_capacity(pts.len() * 4), Vec::with_capacity(pts.len() * 4));
            for w in &welded {
                let mut held: Vec<(usize, f32)> = hold[*w].iter().copied().enumerate().collect();
                held.sort_by(|a, b| b.1.total_cmp(&a.1));
                held.truncate(4);
                let sum: f32 = held.iter().map(|h| h.1).sum::<f32>().max(1e-9);
                joints.extend(held.iter().map(|h| h.0 as u8));
                weights.extend(held.iter().map(|h| h.1 / sum));
            }
            let j = glb.add_raw(&joints, 5121, "VEC4", pts.len());
            let w = glb.add_floats(&weights, "VEC4", false);
            glb.json["meshes"][mesh]["primitives"][p]["attributes"]["JOINTS_0"] = json!(j);
            glb.json["meshes"][mesh]["primitives"][p]["attributes"]["WEIGHTS_0"] = json!(w);
        }
        // The bones as nodes under the scene, none of them turned as it stands; the mesh is
        // drawn by them.
        let first = glb.array("nodes").len();
        let list = glb.json["nodes"].as_array_mut().context("no nodes")?;
        for (i, name) in BONES.iter().enumerate() {
            let from = PARENTS[i].map_or(Vec3::ZERO, |p| places[p]);
            let children: Vec<usize> = (0..BONES.len()).filter(|c| PARENTS[*c] == Some(i)).map(|c| first + c).collect();
            let mut node = json!({ "name": name, "translation": (places[i] - from).to_array() });
            if !children.is_empty() {
                node["children"] = json!(children);
            }
            list.push(node);
        }
        list[holder]["skin"] = json!(0);
        let scene = glb.json["scene"].as_u64().unwrap_or(0) as usize;
        glb.json["scenes"][scene]["nodes"].as_array_mut().context("no scene")?.push(json!(first));
        let binds: Vec<f32> = places.iter().flat_map(|at| (Mat4::from_translation(-*at) * placed).to_cols_array()).collect();
        let inverse = glb.add_floats(&binds, "MAT4", false);
        glb.json["skins"] = json!([{ "joints": (first..first + BONES.len()).collect::<Vec<_>>(), "inverseBindMatrices": inverse, "skeleton": first }]);
        Ok(own)
    }

    /// Its roles, which are known and not read: the bones are ours.
    pub fn roles(&self, sk: &Skeleton) -> Roles {
        let (lo, hi) = sk.joints.iter().fold((f32::MAX, f32::MIN), |(lo, hi), j| (lo.min(j.world.w_axis.y), hi.max(j.world.w_axis.y)));
        let along: Vec<f32> = sk.joints.iter().map(|j| j.world.w_axis.truncate().dot(self.ahead)).collect();
        Roles {
            forward: self.ahead,
            // (As `Roles::read` has them: its side is up crossed with ahead, and a leg on that
            // side is its left. Given the other way round, what is made by roles went the
            // wrong way: a head raised to eat, a death rolled against its own legs.)
            side: self.right,
            legs: LEGS.iter().enumerate().map(|(n, bones)| Leg { chain: bones.to_vec(), front: n < 2, left: n % 2 == 1 }).collect(),
            head: vec![1, 2],
            tail: TAIL.to_vec(),
            spine: vec![0],
            others: Vec::new(),
            arms: Vec::new(),
            floor: lo,
            height: hi - lo,
            length: along.iter().copied().fold(f32::MIN, f32::max) - along.iter().copied().fold(f32::MAX, f32::min),
            nose: None,
            sinks: 0.0,
            curls: false,
        }
    }

    /// The skeleton as the walker has it at one moment: the trunk where the walker's is
    /// (less how far it has gone, so that it stays over its place) and every leg's bone
    /// pointed from its joint to the next of the walker's.
    fn pose(&self, sk: &Skeleton, walker: &Walker, gone: f32) -> Pose {
        let mut pose = sk.rest();
        let basis = Quat::from_mat3(&Mat3::from_cols(self.right, Vec3::Y, self.ahead));
        let (trunk, turned) = walker.trunk();
        clips::turn_now(sk, &mut pose, 0, (basis * turned * basis.inverse()).normalize());
        pose.root_t = sk.base.inverse().transform_point3(self.file(trunk - Vec3::Z * gone));
        for (bones, limb) in LEGS.iter().zip(walker.limbs()) {
            for k in 0..bones.len() - 1 {
                let world = sk.pose_world(&pose);
                let now = world[bones[k + 1]].w_axis.truncate() - world[bones[k]].w_axis.truncate();
                let want = self.file(limb[k + 1]) - self.file(limb[k]);
                if let (Some(now), Some(want)) = (now.try_normalize(), want.try_normalize()) {
                    clips::turn_now(sk, &mut pose, bones[k], Quat::from_rotation_arc(now, want));
                }
            }
            // (A paw stays flat, as it stood, whatever the leg over it does, while it is on
            // the ground; lifted, its toes hang, as a foot's do that is carried forward. Held
            // flat in the air a lynx's long hind foot made a square corner of its leg at
            // every step.)
            let paw = bones[bones.len() - 1];
            let turned = sk.pose_world(&pose)[paw].to_scale_rotation_translation().1;
            clips::turn_now(sk, &mut pose, paw, turned.inverse());
            let lifted = (limb[limb.len() - 1].y / (self.build.tall * 0.12).max(1e-3)).clamp(0.0, 1.0);
            if lifted > 0.0 {
                clips::turn_now(sk, &mut pose, paw, Quat::from_axis_angle(self.right, 1.0 * 0.75 * lifted));
            }
        }
        // A tail that hangs as it stands would go into the ground as it lies: it is swung
        // out behind until its end is on the ground and not in it.
        // (Bent along its length, a little at its root and as much again half way: all at
        // the root, a pony's long tail was a hinge and the skin there was pulled to three
        // times its length.)
        for turn in 0..180 {
            if sk.pose_world(&pose)[20].transform_point3(self.tip).y >= self.origin.y + self.thick {
                break;
            }
            clips::turn_now(sk, &mut pose, TAIL[turn % 4], Quat::from_axis_angle(self.right, 1f32.to_radians()));
        }
        pose
    }

    /// What the walker does not move, moved: over one turn of the frames given, the head
    /// held level against the trunk's rocking with a small nod, and the tail swung from side
    /// to side, its end after its root. (A trunk and four legs going under a head and tail
    /// held as if cast was half an animal.)
    fn lively(&self, sk: &Skeleton, frames: &mut [Pose], much: f32) {
        let n = frames.len() as f32;
        for (i, pose) in frames.iter_mut().enumerate() {
            let turn = i as f32 / n * std::f32::consts::TAU;
            let nose = (sk.pose_world(pose)[0].to_scale_rotation_translation().1 * (sk.joints[0].world.to_scale_rotation_translation().1.inverse() * self.ahead)).y.clamp(-1.0, 1.0).asin();
            clips::turn_now(sk, pose, 1, Quat::from_axis_angle(self.right, nose * 0.7 + 0.03 * much * (2.0 * turn).sin()));
            for (k, bone) in TAIL.into_iter().enumerate() {
                clips::turn_now(sk, pose, bone, Quat::from_axis_angle(Vec3::Y, 0.12 * much * (turn - 0.6 * k as f32).sin()));
            }
        }
    }

    /// One of its clips, made by the walker: standing, a stride of a gait at the speed its
    /// legs' length gives it, or lying down.
    /// Lying down is fitted to this body: how high its trunk lies is tried
    /// over a hand's breadth either way and the height kept that `judge` likes best (what
    /// it gives: how far the pose is from lying as `poses.toml` has it, and whether it
    /// passes). A chest is deeper than a belly by an amount no two animals share.
    pub fn clip_judged(&self, name: &str, sk: &Skeleton, judge: &dyn Fn(&Pose) -> Option<(f32, bool)>) -> Result<Clip> {
        if name == "lie" {
            // (And how far its tail is lifted beyond where its bones clear the ground: a brush
            // is thicker than the bones in it.)
            let lain = |by: f32, lift: f32| {
                let mut walker = Walker::shaped(self.build, Shape { lain: self.shape.lain + by, ..self.shape }, Vec3::ZERO, 0.0);
                for _ in 0..(5.0 * FPS) as usize {
                    walker.step(1.0 / FPS, Vec3::ZERO, 0.0, 1.0, |_, _| 0.0);
                }
                let mut pose = self.pose(sk, &walker, 0.0);
                if lift < 0.0 {
                    // (So much of the way back to how it hangs.)
                    let rest = sk.rest();
                    for bone in TAIL {
                        pose.rotations[bone] = pose.rotations[bone].slerp(rest.rotations[bone], -lift);
                    }
                } else {
                    clips::turn_now(sk, &mut pose, 3, Quat::from_axis_angle(self.right, lift));
                }
                pose
            };
            let under = self.build.tall - self.build.deep / 2.0;
            // (Or let back down: a long tail of hair joined to the legs it hangs between tears if
            // it is swung clear of them, and lies better left where it hangs.)
            let tried: Vec<(f32, Pose, Option<(f32, bool)>)> = (-16..=6).flat_map(|k| [-1.0f32, -0.5, 0.0, 0.15, 0.3, 0.5, 0.8].map(|lift| (k, lift))).map(|(k, lift)| { let pose = lain(k as f32 * 0.02 * under, lift); let said = judge(&pose); (k as f32, pose, said) }).collect();
            let best = tried.iter().filter(|t| t.2.is_some_and(|s| s.1)).chain(tried.iter()).min_by(|a, b| (!a.2.is_some_and(|s| s.1), a.2.map_or(a.0.abs(), |s| s.0)).partial_cmp(&(!b.2.is_some_and(|s| s.1), b.2.map_or(b.0.abs(), |s| s.0))).unwrap_or(std::cmp::Ordering::Equal));
            let pose = best.map_or_else(|| lain(0.0, 0.0), |b| b.1.clone());
            return Ok(Clip { name: name.into(), looping: true, frames: vec![pose; (FPS * 4.0) as usize + 1], source: "walker", speed: None });
        }
        let mut walker = Walker::shaped(self.build, self.shape, Vec3::ZERO, 0.0);
        let legs = self.build.tall - self.build.deep / 2.0;
        let pace = (9.81 * legs).sqrt() * match name {
            // (The speeds the wolf has gone at in the world since its gaits were first walked.)
            "walk" => 0.63,
            "run" => 1.36,
            "sprint" => 2.43,
            _ => 0.0,
        };
        let dt = 1.0 / (FPS * 4.0);
        let mut frames = Vec::new();
        if pace > 0.0 {
            // Walked until it is in its stride, then one stride of it kept.
            let (mut gone, mut began) = (0.0f32, None);
            for tick in 0..(14.0 / dt) as usize {
                gone += pace * dt;
                let before = walker.steps[0];
                walker.step(dt, Vec3::Z * gone, 0.0, 0.0, |_, _| 0.0);
                let stepped = walker.steps[0] != before;
                if began.is_none() && stepped && tick as f32 * dt > 3.0 {
                    began = Some(tick);
                } else if began.is_some() && stepped {
                    break;
                }
                if let Some(from) = began && (tick - from).is_multiple_of(4) {
                    frames.push(self.pose(sk, &walker, gone));
                }
            }
            if frames.len() < 4 {
                bail!("the walker took no stride at {pace:.1} m/s");
            }
            self.lively(sk, &mut frames, if name == "walk" { 1.0 } else { 0.6 });
            frames.push(frames[0].clone());
            clips::close_loop(&mut frames);
        } else {
            // Standing; or lain down, and held there.
            let rest = if name == "lie" { 1.0 } else { 0.0 };
            for _ in 0..(5.0 / dt) as usize {
                walker.step(dt, Vec3::ZERO, 0.0, rest, |_, _| 0.0);
            }
            frames = vec![self.pose(sk, &walker, 0.0); (FPS * 4.0) as usize];
            if name == "idle" {
                self.lively(sk, &mut frames, 0.35);
            }
            frames.push(frames[0].clone());
        }
        Ok(Clip { name: name.into(), looping: true, frames, source: "walker", speed: (pace > 0.0).then_some(pace / self.metres) })
    }
}
