//! Motion a person animated by hand, carried onto a Tripo humanoid: Quaternius's Universal
//! Animation Library (CC0; `tools/library.sh` fetches it to `.work/library/`). A plan names
//! the clips it takes from it (`[plans.biped.library]` in style.toml).
//!
//! The two skeletons differ in everything but what they are a skeleton of: bone names, the
//! pose they rest in (theirs arms down, Tripo's a T), which way each bone's own axes lie,
//! which way the figure faces. So nothing is carried in a bone's own frame. For each bone,
//! in the world: how far it has turned from the library's standing pose is put on ours,
//! after ours is first turned to point the way theirs points standing (arms down). The hips
//! go where theirs go, by the two figures' heights. (Carrying turns-from-rest bone by bone
//! in Blender, 2026-11-11, twisted the trunk: the rests were not the same pose.)

use crate::clips::FPS;
use crate::glb::Glb;
use crate::rig::{Pose, Skeleton};
use anyhow::{Context, Result, bail};
use glam::{Mat4, Quat, Vec3};
use std::path::PathBuf;

pub struct Library {
    glb: Glb,
    sk: Skeleton,
}

/// The library's file, where `tools/library.sh` puts it.
pub fn file() -> PathBuf {
    PathBuf::from(".work").join("library").join("UAL1_Standard.glb")
}

/// The clip in which the library's figure stands as it does for carrying: arms a little down.
const STOOD: &str = "A_TPose";

/// Ours (Mixamo's names, as Tripo rigs a person) and theirs, and for each the bone of ours
/// it points at (none: it turns as the one before it does).
fn bones() -> Vec<(String, String, Option<String>)> {
    let mut out: Vec<(String, String, Option<String>)> = Vec::new();
    let mut add = |ours: &str, theirs: &str, aim: &str| out.push((ours.into(), theirs.into(), (!aim.is_empty()).then(|| aim.into())));
    add("Hips", "pelvis", "");
    add("Spine", "spine_01", "Spine1");
    add("Spine1", "spine_02", "Spine2");
    add("Spine2", "spine_03", "Neck");
    add("Neck", "neck_01", "Head");
    add("Head", "Head", "");
    for (side, s) in [("Left", "l"), ("Right", "r")] {
        add(&format!("{side}Shoulder"), &format!("clavicle_{s}"), &format!("{side}Arm"));
        add(&format!("{side}Arm"), &format!("upperarm_{s}"), &format!("{side}ForeArm"));
        add(&format!("{side}ForeArm"), &format!("lowerarm_{s}"), &format!("{side}Hand"));
        add(&format!("{side}Hand"), &format!("hand_{s}"), &format!("{side}HandMiddle1"));
        add(&format!("{side}UpLeg"), &format!("thigh_{s}"), &format!("{side}Leg"));
        add(&format!("{side}Leg"), &format!("calf_{s}"), &format!("{side}Foot"));
        add(&format!("{side}Foot"), &format!("foot_{s}"), &format!("{side}ToeBase"));
        add(&format!("{side}ToeBase"), &format!("ball_{s}"), "");
        for (finger, f) in [("Index", "index"), ("Middle", "middle"), ("Ring", "ring"), ("Pinky", "pinky"), ("Thumb", "thumb")] {
            for n in 1..=3 {
                add(&format!("{side}Hand{finger}{n}"), &format!("{f}_0{n}_{s}"), &if n < 3 { format!("{side}Hand{finger}{}", n + 1) } else { String::new() });
            }
        }
    }
    out
}

fn pos(m: &Mat4) -> Vec3 {
    m.w_axis.truncate()
}
fn rot(m: &Mat4) -> Quat {
    m.to_scale_rotation_translation().1
}
/// Which way a figure faces, from where its hips are: its left crossed with up.
fn facing(left: Vec3, right: Vec3) -> Vec3 {
    (left - right).with_y(0.0).normalize_or(Vec3::X).cross(Vec3::Y)
}

impl Library {
    pub fn read() -> Result<Self> {
        let path = file();
        if !path.exists() {
            bail!("{} is missing: `tools/library.sh` fetches it (Quaternius's Universal Animation Library, CC0)", path.display());
        }
        let glb = Glb::read(&path)?;
        let sk = Skeleton::read(&glb)?;
        Ok(Self { glb, sk })
    }

    /// The names of its clips.
    pub fn clips(&self) -> Vec<String> {
        self.glb.array("animations").iter().map(|a| a["name"].as_str().unwrap_or("").to_owned()).collect()
    }

    /// Every joint's place in the world at each moment of a clip, at [`FPS`].
    fn worlds(&self, name: &str) -> Result<Vec<Vec<Mat4>>> {
        let anim = self.glb.array("animations").iter().find(|a| a["name"].as_str() == Some(name)).with_context(|| format!("the library has no clip `{name}` (it has: {})", self.clips().join(" ")))?;
        let samplers = anim["samplers"].as_array().cloned().unwrap_or_default();
        type Track = Option<(Vec<f32>, Vec<f32>)>;
        let n = self.sk.joints.len();
        let (mut turns, mut moves): (Vec<Track>, Vec<Track>) = (vec![None; n], vec![None; n]);
        let mut seconds = 0.0f32;
        for ch in anim["channels"].as_array().into_iter().flatten() {
            let Some(joint) = ch["target"]["node"].as_u64().and_then(|node| self.sk.joints.iter().position(|j| j.node == node as usize)) else { continue };
            let s = &samplers[ch["sampler"].as_u64().unwrap_or(0) as usize];
            let path = ch["target"]["path"].as_str().unwrap_or("");
            if path != "rotation" && path != "translation" {
                continue;
            }
            let times = self.glb.floats(s["input"].as_u64().context("sampler without input")? as usize)?.0;
            let mut values = self.glb.floats(s["output"].as_u64().context("sampler without output")? as usize)?.0;
            let wide = if path == "rotation" { 4 } else { 3 };
            if s["interpolation"].as_str() == Some("CUBICSPLINE") {
                values = values.chunks(wide * 3).flat_map(|c| c[wide..wide * 2].to_vec()).collect();
            }
            seconds = seconds.max(times.last().copied().unwrap_or(0.0));
            if path == "rotation" { turns[joint] = Some((times, values)) } else { moves[joint] = Some((times, values)) }
        }
        let frames = ((seconds * FPS).round() as usize).max(1);
        let at = |times: &[f32], t: f32| -> (usize, usize, f32) {
            let hi = times.partition_point(|&k| k < t).min(times.len() - 1);
            let lo = hi.saturating_sub(1);
            let span = times[hi] - times[lo];
            (lo, hi, if span > 0.0 { ((t - times[lo]) / span).clamp(0.0, 1.0) } else { 0.0 })
        };
        let order = parents_first(&self.sk);
        Ok((0..=frames)
            .map(|i| {
                let t = seconds * i as f32 / frames as f32;
                let mut world = vec![Mat4::IDENTITY; n];
                for &j in &order {
                    let joint = &self.sk.joints[j];
                    let turn = turns[j].as_ref().map_or(joint.rest_r, |(times, v)| {
                        let (lo, hi, f) = at(times, t);
                        let q = |k: usize| Quat::from_slice(&v[k * 4..k * 4 + 4]).normalize();
                        q(lo).slerp(q(hi), f)
                    });
                    let place = moves[j].as_ref().map_or(joint.rest_t, |(times, v)| {
                        let (lo, hi, f) = at(times, t);
                        let p = |k: usize| Vec3::from_slice(&v[k * 3..k * 3 + 3]);
                        p(lo).lerp(p(hi), f)
                    });
                    world[j] = joint.parent.map_or(self.sk.base, |p| world[p]) * Mat4::from_scale_rotation_translation(joint.rest_s, turn, place);
                }
                world
            })
            .collect())
    }

    /// A clip of the library as poses of another skeleton, a Tripo humanoid's.
    pub fn carry(&self, name: &str, to: &Skeleton) -> Result<Vec<Pose>> {
        let frames = self.worlds(name)?;
        let stood = self.worlds(STOOD)?.into_iter().next().context("the library's standing clip is empty")?;
        let theirs = |n: &str| self.sk.joints.iter().position(|j| j.name == n);
        let ours = |n: &str| to.joints.iter().position(|j| j.name.rsplit(':').next() == Some(n));
        let rest: Vec<Mat4> = to.joints.iter().map(|j| j.world).collect();
        // Which bone of ours follows which of theirs, and the turn that points ours, at rest,
        // the way theirs points standing.
        let table = bones();
        let (mut pairs, mut fix): (Vec<Option<usize>>, Vec<Quat>) = (vec![None; to.joints.len()], vec![Quat::IDENTITY; to.joints.len()]);
        for (mine, yours, _) in &table {
            if let (Some(j), Some(s)) = (ours(mine), theirs(yours)) {
                pairs[j] = Some(s);
            }
        }
        let (Some(hips), Some(pelvis)) = (ours("Hips").filter(|h| *h == to.root), theirs("pelvis")) else { bail!("no hips to carry motion onto: is this a humanoid's skeleton (`rig = \"humanoid\"`)?") };
        let (Some(ll), Some(rl), Some(tl), Some(tr)) = (ours("LeftUpLeg"), ours("RightUpLeg"), theirs("thigh_l"), theirs("thigh_r")) else { bail!("no thighs to tell which way the figures face") };
        // The two figures turned to face the same way.
        let (from, toward) = (facing(pos(&stood[tl]), pos(&stood[tr])), facing(pos(&rest[ll]), pos(&rest[rl])));
        let turn = if from.dot(toward) < -0.999 { Quat::from_rotation_y(std::f32::consts::PI) } else { Quat::from_rotation_arc(from, toward) };
        let order = parents_first(to);
        for &j in &order {
            let Some(s) = pairs[j] else { continue };
            let (_, _, aim) = table.iter().find(|t| ours(&t.0) == Some(j)).context("a paired bone is in the table")?;
            fix[j] = match aim.as_ref().and_then(|a| Some((ours(a)?, theirs(&table.iter().find(|t| &t.0 == a)?.1)?))) {
                Some((child, their_child)) => {
                    let (way, their_way) = ((pos(&rest[child]) - pos(&rest[j])).normalize_or(Vec3::Y), turn * (pos(&stood[their_child]) - pos(&stood[s])).normalize_or(Vec3::Y));
                    Quat::from_rotation_arc(way, their_way)
                }
                // (The last of a chain turns as the one before it: a fingertip, a toe, the head.)
                None => to.joints[j].parent.filter(|p| pairs[*p].is_some() && j != hips).map_or(Quat::IDENTITY, |p| fix[p]),
            };
        }
        // Heights: each figure's hips over its own lowest joint.
        let floor = |all: &[Mat4]| all.iter().map(|m| pos(m).y).fold(f32::MAX, f32::min);
        let (floor_ours, floor_theirs) = (floor(&rest), floor(&stood));
        let tall = (pos(&rest[hips]).y - floor_ours) / (pos(&stood[pelvis]).y - floor_theirs).max(1e-5);
        let base_turn = rot(&to.base);
        Ok(frames
            .iter()
            .map(|frame| {
                let mut pose = to.rest();
                let mut world = vec![Quat::IDENTITY; to.joints.len()];
                for &j in &order {
                    let above = to.joints[j].parent.map_or(base_turn, |p| world[p]);
                    world[j] = match pairs[j] {
                        Some(s) => {
                            let turned = turn * (rot(&frame[s]) * rot(&stood[s]).inverse()) * turn.inverse();
                            let now = (turned * fix[j] * rot(&rest[j])).normalize();
                            pose.rotations[j] = (above.inverse() * now).normalize();
                            now
                        }
                        None => above * to.joints[j].rest_r,
                    };
                }
                let moved = turn * ((pos(&frame[pelvis]) - pos(&stood[pelvis])).with_y(0.0) * tall);
                let at = Vec3::new(pos(&rest[hips]).x + moved.x, floor_ours + (pos(&frame[pelvis]).y - floor_theirs) * tall, pos(&rest[hips]).z + moved.z);
                pose.root_t = to.base.inverse().transform_point3(at);
                pose
            })
            .collect())
    }
}

/// A skeleton's joints, each after the one it hangs from.
fn parents_first(sk: &Skeleton) -> Vec<usize> {
    let mut order = vec![sk.root];
    let mut i = 0;
    while i < order.len() {
        order.extend(sk.joints[order[i]].children.iter().copied());
        i += 1;
    }
    order
}
