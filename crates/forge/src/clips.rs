//! Animation clips: reading Tripo's, writing ours, and making the ones Tripo has no preset for.
//! Made clips are written against roles (rig.rs), so one recipe serves every creature of any
//! body plan. All clips are in place: the game moves the creature.
//!
//! To add a clip: write a `fn(&Skeleton, &Roles) -> Clip` here, add it to `make`, and name it
//! in a plan's `clips` in assets/creatures/style.toml.

use crate::glb::Glb;
use crate::rig::{Pose, Roles, Skeleton};
use anyhow::Result;
use glam::{Quat, Vec3};
use serde_json::{Value, json};
use std::f32::consts::{PI, TAU};

pub const FPS: f32 = 30.0;

/// A clip as poses at a fixed rate. `looping` clips end on the pose they start on.
pub struct Clip {
    pub name: String,
    pub looping: bool,
    pub frames: Vec<Pose>,
    /// "tripo" or "made".
    pub source: &'static str,
}

impl Clip {
    pub fn seconds(&self) -> f32 {
        (self.frames.len().max(2) - 1) as f32 / FPS
    }
    fn made(name: &str, looping: bool, seconds: f32, sk: &Skeleton, mut pose: impl FnMut(f32, &mut Pose)) -> Self {
        let n = (seconds * FPS).round() as usize;
        let frames = (0..=n)
            .map(|i| {
                let mut p = sk.rest();
                pose(i as f32 / n as f32, &mut p);
                p
            })
            .collect();
        Clip { name: name.into(), looping, frames, source: "made" }
    }
}

/// Reads the file's animations, resampled to [`FPS`]. Only joint rotations and the root joint's
/// translation are kept: scale tracks and constant translations carry nothing.
pub fn read(glb: &Glb, sk: &Skeleton) -> Result<Vec<Clip>> {
    let mut out = Vec::new();
    for anim in glb.array("animations") {
        let samplers = anim["samplers"].as_array().cloned().unwrap_or_default();
        let mut rot: Vec<Option<(Vec<f32>, Vec<f32>)>> = (0..sk.joints.len()).map(|_| None).collect();
        let mut root_t = None;
        let mut seconds = 0.0f32;
        for ch in anim["channels"].as_array().into_iter().flatten() {
            let Some(joint) = ch["target"]["node"].as_u64().and_then(|n| sk.joints.iter().position(|j| j.node == n as usize)) else { continue };
            let s = &samplers[ch["sampler"].as_u64().unwrap() as usize];
            let path = ch["target"]["path"].as_str().unwrap_or("");
            if path != "rotation" && !(path == "translation" && joint == sk.root) {
                continue;
            }
            let times = glb.floats(s["input"].as_u64().unwrap() as usize)?.0;
            let mut values = glb.floats(s["output"].as_u64().unwrap() as usize)?.0;
            if s["interpolation"].as_str() == Some("CUBICSPLINE") {
                let w = if path == "rotation" { 4 } else { 3 };
                values = values.chunks(w * 3).flat_map(|c| c[w..w * 2].to_vec()).collect();
            }
            seconds = seconds.max(times.last().copied().unwrap_or(0.0));
            if path == "rotation" {
                rot[joint] = Some((times, values));
            } else {
                root_t = Some((times, values));
            }
        }
        let n = ((seconds * FPS).round() as usize).max(1);
        let at = |times: &[f32], t: f32| -> (usize, usize, f32) {
            let hi = times.partition_point(|&k| k < t).min(times.len() - 1);
            let lo = hi.saturating_sub(1);
            let span = times[hi] - times[lo];
            (lo, hi, if span > 0.0 { ((t - times[lo]) / span).clamp(0.0, 1.0) } else { 0.0 })
        };
        let frames = (0..=n)
            .map(|i| {
                let t = seconds * i as f32 / n as f32;
                let mut pose = sk.rest();
                for (j, track) in rot.iter().enumerate() {
                    if let Some((times, v)) = track {
                        let (lo, hi, f) = at(times, t);
                        let q = |k: usize| Quat::from_slice(&v[k * 4..k * 4 + 4]).normalize();
                        pose.rotations[j] = q(lo).slerp(q(hi), f);
                    }
                }
                if let Some((times, v)) = &root_t {
                    let (lo, hi, f) = at(times, t);
                    let p = |k: usize| Vec3::from_slice(&v[k * 3..k * 3 + 3]);
                    pose.root_t = p(lo).lerp(p(hi), f);
                }
                pose
            })
            .collect();
        out.push(Clip { name: anim["name"].as_str().unwrap_or("").to_owned(), looping: true, frames, source: "tripo" });
    }
    Ok(out)
}

/// Replaces the file's animations with these. A joint that never leaves rest gets no track.
pub fn write(glb: &mut Glb, sk: &Skeleton, clips: &[Clip]) {
    let rest = sk.rest();
    let mut anims = Vec::new();
    for clip in clips {
        let times: Vec<f32> = (0..clip.frames.len()).map(|i| i as f32 / FPS).collect();
        let input = glb.add_floats(&times, "SCALAR", true);
        let (mut samplers, mut channels): (Vec<Value>, Vec<Value>) = (Vec::new(), Vec::new());
        let mut tracks: Vec<(usize, &str, Vec<f32>, &str)> = Vec::new();
        let mut track = |_: &mut Glb, node: usize, path: &'static str, data: Vec<f32>, kind: &'static str| tracks.push((node, path, data, kind));
        for (j, joint) in sk.joints.iter().enumerate() {
            if clip.frames.iter().all(|p| p.rotations[j].angle_between(rest.rotations[j]) < 1e-4) {
                continue;
            }
            // Keep neighbouring keys in the same hemisphere so linear interpolation is short.
            let mut last = rest.rotations[j];
            let mut data = Vec::with_capacity(clip.frames.len() * 4);
            for p in &clip.frames {
                let q = if p.rotations[j].dot(last) < 0.0 { -p.rotations[j] } else { p.rotations[j] };
                data.extend(q.to_array());
                last = q;
            }
            track(glb, joint.node, "rotation", data, "VEC4");
        }
        if clip.frames.iter().any(|p| p.root_t.distance(rest.root_t) > 1e-5) {
            track(glb, sk.joints[sk.root].node, "translation", clip.frames.iter().flat_map(|p| p.root_t.to_array()).collect(), "VEC3");
        }
        if tracks.is_empty() {
            // glTF wants at least one channel: hold the root where it rests.
            tracks.push((sk.joints[sk.root].node, "rotation", clip.frames.iter().flat_map(|p| p.rotations[sk.root].to_array()).collect(), "VEC4"));
        }
        for (node, path, data, kind) in tracks {
            let output = glb.add_floats(&data, kind, false);
            samplers.push(json!({ "input": input, "output": output, "interpolation": "LINEAR" }));
            channels.push(json!({ "sampler": samplers.len() - 1, "target": { "node": node, "path": path } }));
        }
        anims.push(json!({ "name": clip.name, "samplers": samplers, "channels": channels }));
    }
    glb.json["animations"] = Value::Array(anims);
}

/// Makes the named clip, from `walk` where it is a variation on it. None for an unknown name.
pub fn make(name: &str, sk: &Skeleton, roles: &Roles, walk: Option<&Clip>, upright: bool) -> Option<Clip> {
    Some(match name {
        "idle" => idle(sk, roles),
        "walk" => stride(sk, roles),
        "run" => run(sk, roles, walk?),
        "attack" => attack(sk, roles),
        "hurt" => hurt(sk, roles),
        "die" => die(sk, roles, upright),
        "eat" => eat(sk, roles),
        _ => return None,
    })
}

fn pitch(roles: &Roles, angle: f32) -> Quat {
    // Positive tips the nose down.
    Quat::from_axis_angle(roles.side, angle)
}
fn yaw(angle: f32) -> Quat {
    Quat::from_axis_angle(Vec3::Y, angle)
}
fn roll(roles: &Roles, angle: f32) -> Quat {
    Quat::from_axis_angle(roles.forward, angle)
}
fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
/// 0 before `a`, 1 after `b`, eased between.
fn ramp(t: f32, a: f32, b: f32) -> f32 {
    smooth((t - a) / (b - a))
}
/// Rises over a..b, holds, falls over c..d.
fn pulse(t: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    ramp(t, a, b) * (1.0 - ramp(t, c, d))
}

/// Spreads a turn evenly down a chain.
fn curl(sk: &Skeleton, pose: &mut Pose, chain: &[usize], delta: impl Fn(f32) -> Quat, total: f32) {
    for &j in chain {
        pose.rotations[j] = sk.turned(j, delta(total / chain.len() as f32));
    }
}

/// Dips the front of the trunk, where the neck leaves it, keeping any legs hung from that
/// bone upright by turning them back. Tripo hangs an animal's forelegs from its neck bone, so
/// without this a lowered neck would swing them.
fn bow(sk: &Skeleton, roles: &Roles, pose: &mut Pose, angle: f32) {
    let Some(&base) = roles.spine.last() else { return };
    pose.rotations[base] = sk.turned(base, pitch(roles, angle));
    for leg in roles.legs.iter().filter(|l| sk.joints[l.chain[0]].parent == Some(base)) {
        pose.rotations[leg.chain[0]] = sk.turned(leg.chain[0], pitch(roles, -angle));
    }
}

/// Breathing, a slow look about, a swaying tail.
fn idle(sk: &Skeleton, roles: &Roles) -> Clip {
    Clip::made("idle", true, 4.0, sk, |t, pose| {
        let a = t * TAU;
        for &j in &roles.spine {
            pose.rotations[j] = sk.turned(j, pitch(roles, 0.012 * a.sin()));
        }
        for (i, &j) in roles.head.iter().enumerate() {
            let share = 1.0 / roles.head.len() as f32;
            let look = yaw(0.22 * share * a.sin()) * pitch(roles, 0.06 * share * (2.0 * a + 0.7).sin());
            pose.rotations[j] = sk.turned(j, if i == 0 || roles.head.len() <= 2 { look } else { pitch(roles, 0.0) * look });
        }
        for (i, &j) in roles.tail.iter().enumerate() {
            pose.rotations[j] = sk.turned(j, yaw(0.18 * (2.0 * a - i as f32 * 0.7).sin()));
        }
        for chain in &roles.others {
            if let Some(&j) = chain.first() {
                pose.rotations[j] = sk.turned(j, roll(roles, 0.02 * a.sin()));
            }
        }
        pose.root_t = sk.root_moved(Vec3::Y * -0.006 * roles.height * (0.5 - 0.5 * a.cos()));
    })
}

/// A plain walk for a body Tripo has no walk for: each leg swings from the hip, opposite
/// corners together, the lower leg trailing. Not a studied gait; a rig with a preset uses that.
fn stride(sk: &Skeleton, roles: &Roles) -> Clip {
    Clip::made("walk", true, 1.0, sk, |t, pose| {
        let a = t * TAU;
        for (i, leg) in roles.legs.iter().enumerate() {
            let phase = if roles.legs.len() <= 4 { if leg.front == leg.left { 0.0 } else { PI } } else { i as f32 * PI };
            let swing = 0.38 * (a + phase).sin();
            pose.rotations[leg.chain[0]] = sk.turned(leg.chain[0], pitch(roles, swing));
            if leg.chain.len() > 2 {
                pose.rotations[leg.chain[1]] = sk.turned(leg.chain[1], pitch(roles, -0.3 * (a + phase - 0.9).sin().max(0.0)));
            }
        }
        for (i, &j) in roles.tail.iter().enumerate() {
            pose.rotations[j] = sk.turned(j, yaw(0.12 * (a - i as f32 * 0.6).sin()));
        }
        if let Some(&j) = roles.head.first() {
            pose.rotations[j] = sk.turned(j, pitch(roles, 0.04 * (2.0 * a).sin()));
        }
        pose.root_t = sk.root_moved(Vec3::Y * 0.012 * roles.height * (2.0 * a).cos());
    })
}

/// The walk, quicker and wider, with the trunk pumping: a stand-in until a plan has a real run.
fn run(sk: &Skeleton, roles: &Roles, walk: &Clip) -> Clip {
    let rest = sk.rest();
    let n = ((walk.frames.len() - 1) as f32 * 0.55).round().max(4.0) as usize;
    let last = walk.frames.len() - 1;
    let frames = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let at = t * last as f32;
            let (lo, f) = ((at.floor() as usize).min(last), at.fract());
            let hi = (lo + 1).min(last);
            let mut pose = rest.clone();
            for j in 0..sk.joints.len() {
                let q = walk.frames[lo].rotations[j].slerp(walk.frames[hi].rotations[j], f);
                let (axis, angle) = (rest.rotations[j].inverse() * q).to_axis_angle();
                let angle = if angle > PI { angle - TAU } else { angle };
                pose.rotations[j] = (rest.rotations[j] * Quat::from_axis_angle(axis, angle * 1.3)).normalize();
            }
            let bob = walk.frames[lo].root_t.lerp(walk.frames[hi].root_t, f) - rest.root_t;
            pose.root_t = rest.root_t + bob * 1.5;
            // Tripo's walk leaves the trunk still; add the pump on top of whatever it has.
            for &j in &roles.spine {
                let extra = sk.turned(j, pitch(roles, 0.05 * (2.0 * t * TAU).sin()));
                pose.rotations[j] = (pose.rotations[j] * rest.rotations[j].inverse() * extra).normalize();
            }
            pose
        })
        .collect();
    Clip { name: "run".into(), looping: true, frames, source: "made" }
}

/// A lunge: draw back with the head up, snap forward and down, recover.
fn attack(sk: &Skeleton, roles: &Roles) -> Clip {
    Clip::made("attack", false, 0.8, sk, |t, pose| {
        let back = pulse(t, 0.0, 0.3, 0.3, 0.42);
        let strike = pulse(t, 0.3, 0.45, 0.55, 1.0);
        curl(sk, pose, &roles.head, |a| pitch(roles, a), -0.45 * back + 0.6 * strike);
        bow(sk, roles, pose, -0.2 * back + 0.3 * strike);
        for chain in &roles.others {
            // Arms and wings throw forward with the strike.
            curl(sk, pose, &chain[..1], |a| pitch(roles, a), 0.3 * back - 0.9 * strike);
        }
        curl(sk, pose, &roles.tail, |a| pitch(roles, a), -0.3 * strike);
        pose.root_t = sk.root_moved(roles.forward * roles.length * (-0.08 * back + 0.18 * strike) - Vec3::Y * roles.height * 0.03 * strike);
    })
}

/// A flinch away from a blow.
fn hurt(sk: &Skeleton, roles: &Roles) -> Clip {
    Clip::made("hurt", false, 0.5, sk, |t, pose| {
        let k = pulse(t, 0.0, 0.2, 0.3, 1.0);
        pose.rotations[sk.root] = sk.turned(sk.root, roll(roles, 0.14 * k) * pitch(roles, -0.08 * k));
        curl(sk, pose, &roles.head, |a| pitch(roles, a) * yaw(a * 0.6), -0.3 * k);
        curl(sk, pose, &roles.tail, |a| pitch(roles, a), 0.3 * k);
        pose.root_t = sk.root_moved(-roles.forward * roles.length * 0.05 * k - Vec3::Y * roles.height * 0.02 * k);
    })
}

/// Falls and lies still: an animal onto its side, an upright figure onto its back. The body is
/// lowered each frame so its lowest joint stays near the ground it stood on.
fn die(sk: &Skeleton, roles: &Roles, upright: bool) -> Clip {
    Clip::made("die", false, 1.4, sk, |t, pose| {
        let fall = ramp(t, 0.08, 0.6).powf(1.6);
        let limp = ramp(t, 0.3, 0.9);
        let turn = if upright { pitch(roles, -1.5 * fall) } else { roll(roles, 1.5 * fall) };
        pose.rotations[sk.root] = sk.turned(sk.root, turn);
        curl(sk, pose, &roles.head, |a| if upright { pitch(roles, a) } else { roll(roles, a) }, 0.35 * limp);
        curl(sk, pose, &roles.tail, |a| pitch(roles, a), 0.3 * limp);
        for leg in &roles.legs {
            let tuck = if upright { 0.0 } else { if leg.front { 0.25 } else { -0.25 } };
            pose.rotations[leg.chain[0]] = sk.turned(leg.chain[0], pitch(roles, tuck * limp));
        }
        // Turning about the root joint leaves the body in the air or in the ground; settle it
        // so the lowest joint rests half a limb's thickness above the floor.
        let lowest = sk.pose_world(pose).iter().map(|m| m.w_axis.y).fold(f32::MAX, f32::min);
        let rest_on = roles.floor + 0.07 * roles.height * fall;
        pose.root_t = sk.root_moved(Vec3::Y * (rest_on - lowest));
    })
}

/// Head down to graze, nibbling, and up again.
fn eat(sk: &Skeleton, roles: &Roles) -> Clip {
    Clip::made("eat", true, 4.0, sk, |t, pose| {
        let down = pulse(t, 0.0, 0.2, 0.8, 1.0);
        let nibble = 0.05 * (t * TAU * 6.0).sin() * down;
        curl(sk, pose, &roles.head, |a| pitch(roles, a), 0.7 * down + nibble);
        bow(sk, roles, pose, 0.55 * down);
        for (i, &j) in roles.tail.iter().enumerate() {
            pose.rotations[j] = sk.turned(j, yaw(0.15 * (t * TAU * 2.0 - i as f32 * 0.7).sin()));
        }
    })
}
