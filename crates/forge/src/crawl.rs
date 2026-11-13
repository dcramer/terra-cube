//! What goes on six or eight legs, made here of boxes with a skeleton and motion of its own:
//! a beetle, a spider, a scorpion, a crab. `[built] form = "crawler"` in its `creature.toml`
//! (`built`): no Tripo, no credits. (Tripo's rig of a crab came back with no legs found and
//! a clip that tore to nothing; the rest were never bought.)
//!
//! And what goes on two: a bird on the ground (`form = "fowl"`: a heron, a duck, a pheasant, an
//! owl, a hawk at rest, a crow), the same way: a body, a head on a neck, a tail, folded wings,
//! and two legs under it that swing fore and aft and bend at the knee to lift.
//!
//! - **Model**: a body, a head (in the mark's colour, with jaws) and an abdomen behind, and
//!   each leg two boxes: one out sideways from the body at knee height, one down from its
//!   end to the ground. Legs are spread evenly along the body, the fore ones reaching a
//!   little ahead and the hind ones behind.
//! - **Skeleton**: body (the root), head, abdomen, and two bones a leg (where it leaves the
//!   body; its knee). Every box is held whole by one bone, so nothing stretches.
//! - **Motion**: each leg swings about the upright where it leaves the body and lifts about
//!   the body's length. Walking, the legs go in two sets, every other leg of a side with
//!   the ones between on the other side, as an insect's do: a set on the ground goes back
//!   at the body's own speed (so no foot slides), the other lifts and comes forward.

use crate::built::Built;
use crate::clips::{Clip, FPS};
use crate::glb::Glb;
use crate::rig::{Leg, Pose, Roles, Skeleton};
use anyhow::{Context, Result};
use glam::{Mat4, Quat, Vec3};
use serde_json::json;
use std::f32::consts::{PI, TAU};

/// A crawler made here: how many legs a side, how far a foot stands out from where its leg
/// leaves the body, and its length (metres; the file is in metres).
pub struct Crawler {
    side: usize,
    reach: f32,
    long: f32,
    high: f32,
    /// On two legs, under it: a bird. (Its `reach` is its legs' length.) And how wide it is.
    fowl: bool,
    /// The first of the five bones of a sting's tail, each bent 0.6 radians on from the
    /// last as it stands: what lets it straighten when the animal is dead.
    sting: Option<usize>,
    /// Its hindmost legs are a leaper's, long and folded high (a grasshopper's).
    leaps: bool,
    /// A bird's neck and head, from where its neck leaves its body to its beak's root.
    neck: f32,
    wide: f32,
}

/// Boxes, each held by one bone, and the bones: the model with its skeleton.
fn assemble(boxes: &[(Vec3, Vec3, [f32; 3], usize)], places: &[Vec3], parents: &[Option<usize>], names: &[String]) -> Result<Glb> {
    let plain: Vec<(Vec3, Vec3, [f32; 3])> = boxes.iter().map(|b| (b.0, b.1, b.2)).collect();
    let mut glb = crate::built::mesh(&plain);
    // (Twenty-four corners a box, all its bone's.)
    let joints: Vec<u8> = boxes.iter().flat_map(|b| std::iter::repeat_n([b.3 as u8, 0, 0, 0], 24)).flatten().collect();
    let weights: Vec<f32> = boxes.iter().flat_map(|_| std::iter::repeat_n([1.0f32, 0.0, 0.0, 0.0], 24)).flatten().collect();
    let j = glb.add_raw(&joints, 5121, "VEC4", boxes.len() * 24);
    let w = glb.add_floats(&weights, "VEC4", false);
    glb.json["meshes"][0]["primitives"][0]["attributes"]["JOINTS_0"] = json!(j);
    glb.json["meshes"][0]["primitives"][0]["attributes"]["WEIGHTS_0"] = json!(w);
    let first = glb.array("nodes").len();
    let list = glb.json["nodes"].as_array_mut().context("no nodes")?;
    for (i, name) in names.iter().enumerate() {
        let from = parents[i].map_or(Vec3::ZERO, |p| places[p]);
        let children: Vec<usize> = (0..names.len()).filter(|c| parents[*c] == Some(i)).map(|c| first + c).collect();
        let mut node = json!({ "name": name, "translation": (places[i] - from).to_array() });
        if !children.is_empty() {
            node["children"] = json!(children);
        }
        list.push(node);
    }
    list[0]["skin"] = json!(0);
    glb.json["scenes"][0]["nodes"].as_array_mut().context("no scene")?.push(json!(first));
    let binds: Vec<f32> = places.iter().flat_map(|at| Mat4::from_translation(-*at).to_cols_array()).collect();
    let inverse = glb.add_floats(&binds, "MAT4", false);
    glb.json["skins"] = json!([{ "joints": (first..first + names.len()).collect::<Vec<_>>(), "inverseBindMatrices": inverse, "skeleton": first }]);
    Ok(glb)
}

/// Whether a model is one of these.
pub fn made(glb: &Glb) -> bool {
    glb.array("nodes").iter().any(|n| n["name"] == "leg_l0_knee")
}

impl Crawler {
    /// The model with its bones: boxes, each held by one bone.
    pub fn model(b: &Built) -> Result<(Glb, Self)> {
        let (l, side) = (b.length, (b.legs.max(4) / 2) as usize);
        let (wide, thick, clear) = (l * 0.42, l * 0.26, l * 0.16);
        let knee = clear + thick * 0.75;
        let reach = (b.span / 2.0 - wide / 2.0).max(l * 0.2);
        let leg = l * 0.07;
        let dark = |c: [f32; 3], k: f32| c.map(|v| v * k);
        // Each box with the bone that holds it; and where each bone is.
        let mut boxes: Vec<(Vec3, Vec3, [f32; 3], usize)> = Vec::new();
        let mut put = |middle: Vec3, size: Vec3, colour: [f32; 3], bone: usize| boxes.push((middle - size / 2.0, middle + size / 2.0, colour, bone));
        let mid = clear + thick / 2.0;
        put(Vec3::new(0.0, mid, 0.0), Vec3::new(wide, thick, l * 0.5), b.back, 0);
        put(Vec3::new(0.0, clear + thick * 0.2, 0.0), Vec3::new(wide * 0.8, thick * 0.4, l * 0.44), b.belly, 0);
        // Head, and jaws ahead of it.
        put(Vec3::new(0.0, mid, l * 0.36), Vec3::new(wide * 0.6, thick * 0.7, l * 0.22), b.mark, 1);
        for x in [-1.0, 1.0] {
            put(Vec3::new(x * wide * 0.18, mid - thick * 0.1, l * 0.53), Vec3::new(l * 0.05, l * 0.04, l * 0.14), dark(b.mark, 0.7), 1);
        }
        // Pincers: an arm out ahead and to each side from the head, and a claw of two fingers.
        if b.claws {
            for x in [-1.0f32, 1.0] {
                let (from, to) = (Vec3::new(x * wide * 0.3, mid, l * 0.4), Vec3::new(x * wide * 0.75, mid, l * 0.72));
                for i in 0..3 {
                    put(from.lerp(to, (i as f32 + 0.5) / 3.0), Vec3::new(l * 0.11, l * 0.07, l * 0.14), b.wing, 1);
                }
                put(to + Vec3::new(x * l * 0.03, 0.0, l * 0.12), Vec3::new(l * 0.07, l * 0.08, l * 0.2), b.mark, 1);
                put(to + Vec3::new(-x * l * 0.06, 0.0, l * 0.1), Vec3::new(l * 0.04, l * 0.05, l * 0.15), dark(b.mark, 0.8), 1);
            }
        }
        // Abdomen, in two pieces, the hinder smaller.
        let tail = l * b.tail;
        // A sting: the tail goes on from the abdomen in pieces, up and back over the body.
        if b.sting {
            let root = Vec3::new(0.0, mid, -l * 0.25 - tail);
            // (Round an arc as wide as a third of its body, each piece into the next.)
            let r = l * 0.34;
            for i in 0..10 {
                let a = i as f32 / 9.0 * 2.7;
                let at = root + Vec3::new(0.0, r * (1.0 - a.cos()), -r * a.sin());
                // (Two pieces to a bone of its own, after the legs': `sting`.)
                put(at, Vec3::splat(r * (0.5 - 0.02 * i as f32)), if i == 9 { b.mark } else { dark(b.back, 0.8 + 0.03 * i as f32) }, 3 + side * 6 + i / 2);
            }
        }
        put(Vec3::new(0.0, mid + thick * 0.08, -l * 0.25 - tail * 0.3), Vec3::new(wide * 1.05, thick * 1.1, tail * 0.6), dark(b.back, 0.85), 2);
        put(Vec3::new(0.0, mid + thick * 0.05, -l * 0.25 - tail * 0.8), Vec3::new(wide * 0.75, thick * 0.8, tail * 0.4), dark(b.back, 0.7), 2);
        let mut places = vec![Vec3::new(0.0, mid, 0.0), Vec3::new(0.0, mid, l * 0.25), Vec3::new(0.0, mid, -l * 0.25)];
        let mut parents: Vec<Option<usize>> = vec![None, Some(0), Some(0)];
        let mut names: Vec<String> = vec!["body".into(), "head".into(), "abdomen".into()];
        for (s, x) in [(0usize, -1.0f32), (1, 1.0)] {
            for k in 0..side {
                // (Along the body from fore to hind, and splayed: the fore reach ahead.)
                let t = if side > 1 { k as f32 / (side - 1) as f32 } else { 0.5 };
                let (z, splay) = (l * (0.2 - 0.4 * t), l * (0.16 - 0.32 * t));
                // (A leaper's hind legs: folded high over its back, and set down well behind.)
                let leaper = b.leaps && k == side - 1;
                let knee = if leaper { knee * 1.9 } else { knee };
                let hip = Vec3::new(x * wide / 2.0, mid, z);
                let hip = if leaper { hip } else { hip.with_y(knee) };
                let end = Vec3::new(x * (wide / 2.0 + reach * if leaper { 0.6 } else { 1.0 }), knee, z + splay * if leaper { 2.0 } else { 1.0 });
                let bone = places.len();
                // The upper part, out to the knee (in three short boxes along its slant); the lower, down to the ground.
                for i in 0..if leaper { 3 } else { 0 } {
                    let at = hip.lerp(end, (i as f32 + 0.5) / 3.0);
                    // (Each as long as its share of the way, whichever way that runs.)
                    put(at, (end - hip).abs() / 3.0 + Vec3::splat(leg * 1.5), b.wing, bone);
                }
                // (Not a leaper's: its leg rises from its side to a knee over its back, half
                // way out, and comes down from there to a foot at its full reach. Until
                // 2026-11-13 it went out level and straight down, a table's leg.)
                let foot = end.with_y(0.0);
                let end = if leaper {
                    put(Vec3::new(end.x, knee / 2.0, end.z), Vec3::new(leg * 0.8, knee, leg * 0.8), dark(b.wing, 0.8), bone + 1);
                    end
                } else {
                    let hip = Vec3::new(x * wide / 2.0, mid, z);
                    let top = hip.lerp(foot, 0.5).with_y(knee * 1.45);
                    // (Small cubes one after another along each part: a box as long as its
                    // share of a slant is a slab, and eight of those were a wall.)
                    for (from, to, thick, colour, bone) in [(hip, top, leg, b.wing, bone), (top, foot, leg * 0.8, dark(b.wing, 0.8), bone + 1)] {
                        let n = (from.distance(to) / (thick * 0.9)).ceil().max(1.0) as usize;
                        for i in 0..n {
                            put(from.lerp(to, (i as f32 + 0.5) / n as f32), Vec3::splat(thick), colour, bone);
                        }
                    }
                    top
                };
                let hip = if leaper { hip } else { hip.with_y(mid) };
                let name = format!("leg_{}{k}", if s == 0 { 'l' } else { 'r' });
                // (And a bone where its foot is, holding nothing: what is judged to stand and not slide.)
                places.extend([hip, end, foot]);
                parents.extend([Some(0), Some(bone), Some(bone + 1)]);
                names.extend([name.clone(), format!("{name}_knee"), format!("{name}_foot")]);
            }
        }
        let mut sting = None;
        if b.sting {
            let (first, root, r) = (places.len(), Vec3::new(0.0, mid, -l * 0.25 - l * b.tail), l * 0.34);
            for j in 0..5 {
                let a = j as f32 * 0.6;
                places.push(root + Vec3::new(0.0, r * (1.0 - a.cos()), -r * a.sin()));
                parents.push(Some(if j == 0 { 2 } else { first + j - 1 }));
                names.push(format!("sting_{j}"));
            }
            sting = Some(first);
        }
        Ok((assemble(&boxes, &places, &parents, &names)?, Self { side, reach, long: l, high: mid, fowl: false, sting, leaps: b.leaps, neck: 0.0, wide }))
    }

    /// A bird on the ground: `length` its body, `span` how long its legs are, `chord` its
    /// neck (of the body's length), `tail` its tail; `wing` its wings' colour, `mark` its
    /// beak's and feet's.
    pub fn fowl(b: &Built) -> Result<(Glb, Self)> {
        let (l, legs, neck) = (b.length, b.span, b.length * b.chord);
        let (wide, thick, leg) = (l * 0.45, l * 0.42, l * 0.05);
        let dark = |c: [f32; 3], k: f32| c.map(|v| v * k);
        let mut boxes: Vec<(Vec3, Vec3, [f32; 3], usize)> = Vec::new();
        let mut put = |middle: Vec3, size: Vec3, colour: [f32; 3], bone: usize| boxes.push((middle - size / 2.0, middle + size / 2.0, colour, bone));
        let mid = legs + thick / 2.0;
        put(Vec3::new(0.0, mid + thick * 0.12, 0.0), Vec3::new(wide, thick * 0.76, l * 0.75), b.back, 0);
        put(Vec3::new(0.0, mid - thick * 0.3, l * 0.02), Vec3::new(wide * 0.85, thick * 0.4, l * 0.62), b.belly, 0);
        // Its wings, folded: each a slab down its side from the shoulder, and a second folded
        // up over that from its lower edge, which opens out to twice the reach (bones 9 to
        // 12, after the legs: `wings`).
        for (n, x) in [-1.0f32, 1.0].into_iter().enumerate() {
            put(Vec3::new(x * wide * 0.5, mid + thick * 0.1, -l * 0.05), Vec3::new(l * 0.05, thick * 0.6, l * 0.62), b.wing, 9 + n * 2);
            put(Vec3::new(x * (wide * 0.5 + l * 0.04), mid + thick * 0.08, -l * 0.09), Vec3::new(l * 0.03, thick * 0.56, l * 0.5), dark(b.wing, 0.8), 10 + n * 2);
        }
        // Neck and head, the beak ahead of it.
        let base = Vec3::new(0.0, mid + thick * 0.3, l * 0.3);
        put(base + Vec3::Y * neck / 2.0, Vec3::new(wide * 0.34, neck, wide * 0.34), b.belly, 1);
        let head = base + Vec3::new(0.0, neck + l * 0.1, l * 0.04);
        put(head, Vec3::splat(l * 0.26), dark(b.back, 1.1), 1);
        put(head + Vec3::new(0.0, -l * 0.02, l * 0.13 + l * 0.11), Vec3::new(l * 0.07, l * 0.06, l * 0.22), b.mark, 1);
        let rump = Vec3::new(0.0, mid + thick * 0.2, -l * 0.375);
        put(rump - Vec3::Z * l * b.tail / 2.0, Vec3::new(wide * 0.6, l * 0.04, l * b.tail), dark(b.wing, 0.85), 2);
        let mut places = vec![Vec3::new(0.0, mid, 0.0), base, rump];
        let mut parents: Vec<Option<usize>> = vec![None, Some(0), Some(0)];
        let mut names: Vec<String> = vec!["body".into(), "head".into(), "abdomen".into()];
        for (s, x) in [('l', -1.0f32), ('r', 1.0)] {
            let (hip, knee, bone) = (Vec3::new(x * wide * 0.25, legs, 0.0), Vec3::new(x * wide * 0.25, legs / 2.0, 0.0), places.len());
            put(hip.lerp(knee, 0.5), Vec3::new(leg * 1.3, legs / 2.0, leg * 1.3), dark(b.belly, 0.8), bone);
            put(knee.with_y(legs / 4.0), Vec3::new(leg, legs / 2.0, leg), b.mark, bone + 1);
            // (And its foot, a bone of its own on the ground: what is judged to stand and not slide.)
            put(Vec3::new(knee.x, l * 0.012, l * 0.05), Vec3::new(l * 0.1, l * 0.024, l * 0.18), dark(b.mark, 0.85), bone + 2);
            places.extend([hip, knee, knee.with_y(0.0)]);
            parents.extend([Some(0), Some(bone), Some(bone + 1)]);
            names.extend([format!("leg_{s}0"), format!("leg_{s}0_knee"), format!("leg_{s}0_foot")]);
        }
        for (s, x) in [('l', -1.0f32), ('r', 1.0)] {
            let bone = places.len();
            places.extend([Vec3::new(x * wide * 0.5, mid + thick * 0.4, -l * 0.05), Vec3::new(x * (wide * 0.5 + l * 0.025), mid - thick * 0.2, -l * 0.05)]);
            parents.extend([Some(0), Some(bone)]);
            names.extend([format!("wing_{s}"), format!("wing_{s}_tip")]);
        }
        Ok((assemble(&boxes, &places, &parents, &names)?, Self { side: 1, reach: legs, long: l, high: mid, fowl: true, sting: None, leaps: false, neck: neck + l * 0.2, wide }))
    }


    /// Bones a leg: three, its foot one of them.
    fn per(&self) -> usize {
        3
    }

    /// Its roles, which are known: the bones are ours.
    pub fn roles(&self, sk: &Skeleton) -> Roles {
        let per = self.per();
        let legs = (0..self.side * 2).map(|n| Leg { chain: (0..per).map(|k| 3 + n * per + k).collect(), front: n % self.side < self.side.div_ceil(2), left: n >= self.side }).collect();
        let (lo, hi) = sk.joints.iter().fold((f32::MAX, f32::MIN), |(lo, hi), j| (lo.min(j.world.w_axis.y), hi.max(j.world.w_axis.y)));
        Roles { forward: Vec3::Z, side: Vec3::X, legs, head: vec![1], tail: vec![2], spine: vec![0], others: Vec::new(), arms: Vec::new(), floor: 0.0, height: hi.max(lo), length: self.long, nose: None, sinks: 0.0, curls: false }
    }

    /// A pose: the body moved and turned so, and each leg swung fore or aft by so far along
    /// the ground (metres) and lifted by so much (radians).
    fn pose(&self, sk: &Skeleton, moved: Vec3, turned: Quat, legs: impl Fn(usize) -> (f32, f32)) -> Pose {
        let mut pose = sk.rest();
        pose.rotations[0] = turned;
        pose.root_t = sk.joints[0].rest_t + moved;
        for n in 0..self.side * 2 {
            let (fore, lift) = legs(n);
            // (The side it is on: its lift is a roll that way up.)
            let x = if n < self.side { -1.0 } else { 1.0 };
            let swing = (fore / self.reach).clamp(-0.9, 0.9).asin();
            if self.fowl {
                // (Under it: the thigh swings fore and aft, and to lift, the knee bends.)
                // (And its foot stays flat whatever the leg over it does.)
                pose.rotations[3 + n * 3] = Quat::from_rotation_x(-swing - lift);
                pose.rotations[4 + n * 3] = Quat::from_rotation_x(2.0 * lift);
                pose.rotations[5 + n * 3] = (turned * Quat::from_rotation_x(-swing + lift)).inverse();
                continue;
            }
            pose.rotations[3 + n * 3] = Quat::from_rotation_y(-x * swing) * Quat::from_rotation_z(x * lift);
        }
        pose
    }

    /// A bird's wings: `open` from folded (0) to spread level (1), and `beat` how far above
    /// level they are (radians; under it, less than nothing). The outer half follows a
    /// little behind the inner, as a wing bends.
    fn wings(&self, mut pose: Pose, open: f32, beat: f32, after: f32) -> Pose {
        for (n, x) in [-1.0f32, 1.0].into_iter().enumerate() {
            pose.rotations[9 + n * 2] = Quat::from_rotation_z(x * (PI / 2.0 * open + beat * open));
            pose.rotations[10 + n * 2] = Quat::from_rotation_z(-x * PI * open + x * (after - beat) * 0.6 * open);
        }
        pose
    }

    /// Which of the two sets a leg steps with: every other leg of a side, with the ones
    /// between on the other side.
    fn set(&self, n: usize) -> usize {
        (n % self.side + n / self.side) % 2
    }

    /// One of its clips.
    pub fn clip(&self, name: &str, sk: &Skeleton) -> Result<Clip> {
        let still = |_: usize| (0.0, 0.0);
        let frames_of = |seconds: f32, f: &dyn Fn(f32) -> Pose| -> Vec<Pose> {
            let n = (seconds * FPS).round() as usize;
            (0..=n).map(|i| f(i as f32 / n as f32)).collect()
        };
        let ease = |t: f32| t * t * (3.0 - 2.0 * t);
        let (frames, looping, speed) = match name {
            "walk" | "run" | "sprint" => {
                // A stride as long as its legs let it be, taken quicker to go faster.
                let pace = self.long * match name { "walk" => 2.5, "run" => 6.0, _ => 9.0 };
                let stride = self.reach * if self.fowl { 0.7 } else { 0.9 };
                // (A bird's legs are short for its body: it steps at a bird's quick rate and goes as far as that takes it.)
                let pace = if self.fowl { 2.0 * stride / match name { "walk" => 0.5, "run" => 0.3, _ => 0.24 } } else { pace };
                let seconds = 2.0 * stride / pace;
                let frames = frames_of(seconds, &|t| {
                    self.pose(sk, Vec3::Y * self.long * 0.006 * (2.0 * TAU * t).sin(), Quat::from_rotation_z(0.012 * (TAU * t).sin()), |n| {
                        let p = (t + self.set(n) as f32 * 0.5).fract();
                        // Down for half its turn, going back as the body goes on; up for the other, coming forward.
                        if p < 0.5 { (stride * (0.5 - 2.0 * p), 0.0) } else { (stride * (2.0 * ease((p - 0.5) * 2.0) - 1.0) * 0.5, 0.3 * (PI * (p - 0.5) * 2.0).sin()) }
                    })
                });
                (frames, true, Some(pace))
            }
            "idle" => (frames_of(4.0, &|t| self.pose(sk, Vec3::Y * self.long * 0.01 * (TAU * t).sin(), Quat::from_rotation_y(0.04 * (TAU * t).sin()), still)), true, None),
            // Gathered, and sprung: its legs pressed down, then drawn up under it.
            // Up: its wings thrown open and brought down hard, its legs pushing off.
            "jump" if self.fowl => (frames_of(0.3, &|t| self.wings(self.pose(sk, Vec3::ZERO, Quat::from_rotation_x(-0.25 * t), |_| (0.0, -0.2 + 0.7 * t)), ease((t / 0.4).min(1.0)), 0.9 * (TAU * t / 0.6).cos(), 0.9 * (TAU * (t - 0.05) / 0.6).cos())), false, None),
            // On the wing: three beats a second, its legs trailed behind it (hung straight down
            // they were a heron standing in the air), its body lifting on each.
            "fall" if self.fowl => (frames_of(1.0, &|t| {
                let (beat, after) = (0.75 * (TAU * 3.0 * t).sin(), 0.75 * (TAU * 3.0 * (t - 0.05)).sin());
                self.wings(self.pose(sk, Vec3::Y * self.long * 0.05 * -beat, Quat::from_rotation_x(-0.12), |_| (-self.reach * 0.88, 0.08)), 1.0, beat, after)
            }), true, None),
            // Down: its wings held high as its feet take it, then folded.
            "land" if self.fowl => (frames_of(0.37, &|t| self.wings(self.pose(sk, Vec3::NEG_Y * self.high * 0.08 * (1.0 - ease(t)), Quat::from_rotation_x(-0.2 * (1.0 - t)), |_| (0.0, 0.4 * (1.0 - ease(t)))), 1.0 - ease(t), 0.6, 0.6)), false, None),
            "jump" => (frames_of(0.3, &|t| self.pose(sk, Vec3::Y * self.high * if self.fowl { 0.0 } else { -0.35 * (1.0 - t) + 0.2 * t }, Quat::IDENTITY, |_| (0.0, -0.2 + 0.6 * t))), false, None),
            "fall" => (frames_of(0.9, &|t| self.pose(sk, Vec3::ZERO, Quat::from_rotation_x(0.05 * (TAU * t).sin()), |n| (0.0, 0.4 + 0.08 * (TAU * t + n as f32).sin()))), true, None),
            "land" => (frames_of(0.37, &|t| self.pose(sk, Vec3::NEG_Y * self.high * if self.fowl { 0.08 } else { 0.4 } * (1.0 - ease(t)), Quat::IDENTITY, |_| (0.0, 0.4 * (1.0 - ease(t))))), false, None),
            // Reared on its hind legs, and struck forward and down.
            "attack" => (frames_of(0.8, &|t| {
                let (up, out) = ((PI * (t / 0.45).min(1.0)).sin() * if t < 0.45 { 1.0 } else { 0.0 }, ease(((t - 0.3) / 0.2).clamp(0.0, 1.0)) * (1.0 - ease(((t - 0.55) / 0.25).clamp(0.0, 1.0))));
                self.pose(sk, Vec3::Z * self.long * 0.3 * out + Vec3::Y * self.high * 0.2 * up, Quat::from_rotation_x(-0.45 * up + 0.2 * out), |n| if n % self.side == 0 && !self.fowl { (self.reach * 0.3 * out, 0.7 * up) } else { (0.0, 0.0) })
            }), false, None),
            "hurt" => (frames_of(0.5, &|t| {
                let k = (PI * t).sin();
                self.pose(sk, Vec3::NEG_Z * self.long * 0.15 * k, Quat::from_rotation_z(0.25 * k), |_| (0.0, 0.2 * k))
            }), false, None),
            // Over onto its back, its legs drawn in over it.
            "die" => (frames_of(1.4, &|t| {
                let k = ease((t / 0.6).min(1.0));
                // (A bird goes over onto its side, its legs drawn up.)
                if self.fowl {
                    // (And goes slack once it is down: its neck lets its head to the ground,
                    // the wing that is uppermost sags, its legs trail and its tail
                    // drops. Stiff, it lay as it had stood, turned a quarter over.)
                    let limp = ease(((t - 0.4) / 0.45).clamp(0.0, 1.0));
                    let mut pose = self.pose(sk, Vec3::Y * (self.wide / 2.0 - self.high) * k, Quat::from_rotation_z(PI / 2.0 * k), |_| (0.0, 0.7 * k * (1.0 - limp) + 0.12 * limp));
                    // (No further than the ground its body lies on: let right down, a heron's long
                    // neck held its body up off the ground.)
                    let droop = (self.wide * 0.3 / self.neck.max(1e-3)).min(0.7).asin();
                    pose.rotations[1] = Quat::from_rotation_z(droop * limp) * Quat::from_rotation_x(0.4 * limp);
                    pose.rotations[2] = Quat::from_rotation_z(0.3 * limp);
                    // (The wing that is uppermost sags a little open, no more: let fall open
                    // it stood straight up from the bird like a flag. It lies closed on it.)
                    pose.rotations[11] = Quat::from_rotation_z(0.18 * limp);
                    return pose;
                }
                // What is dead is limp (the developer, 2026-11-13: "dead things dont stay
                // stiff"): its legs fold in over it at the knee, as a dead spider's do, and a
                // sting's tail straightens and lies along the ground behind it. Stiff, a dead
                // scorpion stood on its curled tail with its body in the air.
                let limp = ease(((t - 0.35) / 0.5).clamp(0.0, 1.0));
                // (A sting's tail lets go as the body goes over, not after: still curled when
                // it was on its back, it held the scorpion up in the air for a moment. The
                // developer, 2026-11-13: "one frame of scorpion still has it floating".)
                let slack = ease((t / 0.42).clamp(0.0, 1.0));
                // How its legs come to rest (at the hip, at the knee): a scorpion's fallen flat
                // beside it; a spider's drawn tight in over its belly and crossed, as a dead
                // spider's are; a beetle's, a crab's and a grasshopper's folded in over it more
                // loosely. (Four pairs tried on the spider and looked at, 2026-11-13.)
                // (A grasshopper's fall flat too, and its long hind legs fold down over it:
                // curled, or flat like the rest, they stood under it and held it up.)
                let (hip, knee) = if self.sting.is_some() || self.leaps { (-0.44, 1.2) } else if self.side >= 4 { (-1.6, -1.2) } else { (-1.1, -1.5) };
                let mut pose = self.pose(sk, Vec3::Y * (self.high * 0.5 * (PI * k).sin() - self.high * 0.35 * k), Quat::from_rotation_z(PI * k), |n| (0.0, -1.1 * k * (1.0 - limp) + if self.leaps && n % self.side == self.side - 1 { -1.4 } else { hip } * limp));
                for n in 0..self.side * 2 {
                    let x = if n < self.side { -1.0 } else { 1.0 };
                    pose.rotations[4 + n * 3] = Quat::from_rotation_z(x * if self.leaps && n % self.side == self.side - 1 { 0.0 } else { knee } * limp);
                }
                if let Some(first) = self.sting {
                    for j in 1..5 {
                        pose.rotations[first + j] = Quat::from_rotation_x(-0.74 * slack);
                    }
                }
                pose
            }), false, None),
            "swim" => (frames_of(1.2, &|t| self.pose(sk, Vec3::ZERO, Quat::IDENTITY, |n| (self.reach * 0.4 * (TAU * t + self.set(n) as f32 * PI).sin(), 0.25))), true, None),
            // (A bird pecks: its head goes down on its neck, and its body stays on its feet.)
            "eat" if self.fowl => (frames_of(4.0, &|t| {
                let mut pose = self.pose(sk, Vec3::ZERO, Quat::IDENTITY, still);
                pose.rotations[1] = Quat::from_rotation_x(1.1 + 0.25 * (TAU * 4.0 * t).sin());
                pose
            }), true, None),
            "eat" => (frames_of(4.0, &|t| self.pose(sk, Vec3::NEG_Y * self.high * 0.12, Quat::from_rotation_x(0.2 + 0.05 * (TAU * 4.0 * t).sin()), still)), true, None),
            _ => (frames_of(1.0, &|_| self.pose(sk, Vec3::ZERO, Quat::IDENTITY, still)), true, None),
        };
        Ok(Clip { name: name.into(), looping, frames, source: "made", speed })
    }
}
