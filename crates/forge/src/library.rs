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

/// The library's file, where `tools/library.sh` puts it; and the animal's, a wolf a person
/// animated (Quaternius's Ultimate Animated Animals, CC0), whose motion goes on anything
/// with four legs.
pub fn file() -> PathBuf {
    PathBuf::from(".work").join("library").join("UAL1_Standard.glb")
}
pub fn beast() -> PathBuf {
    PathBuf::from(".work").join("library").join("Wolf.glb")
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
        Self::open(file())
    }

    pub fn open(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            bail!("{} is missing: `tools/library.sh` fetches it (Quaternius's hand-animated motion, CC0)", path.display());
        }
        let glb = Glb::read(&path)?;
        let sk = Skeleton::read_loose(&glb)?;
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

impl Library {
    /// A clip of the hand-animated animal as poses of any four-legged skeleton, whose bones
    /// are known only by their roles. The trunk, neck and tail turn as the animal's do, each
    /// bone as the one the same way along its chain; then each foot is put where the animal's
    /// foot is, by the two legs' lengths, and the leg bent to reach it. So the feet strike and
    /// lift as a person drew them, on a body of any build.
    pub fn carry_beast(&self, name: &str, to: &Skeleton, roles: &crate::rig::Roles) -> Result<Vec<Pose>> {
        let frames = self.worlds(name)?;
        let stood: Vec<Mat4> = self.sk.joints.iter().map(|j| j.world).collect();
        let theirs = |n: &str| self.sk.joints.iter().position(|j| j.name == n).with_context(|| format!("the animal has no bone `{n}`"));
        let chain = |names: &[&str]| names.iter().map(|n| theirs(n)).collect::<Result<Vec<usize>>>();
        let (spine, neck) = (chain(&["Back", "Torso", "Torso2", "Torso3"])?, chain(&["Neck1", "Neck2", "Neck3", "Head"])?);
        let tail = chain(&["Tail1", "Tail2", "Tail3", "Tail4", "Tail5", "Tail6", "Tail7", "Tail8"])?;
        // A leg of theirs: where it leaves the body, and its last bone, whose end is the foot
        // (a node past the last bone, not a bone itself).
        let nodes = self.glb.array("nodes");
        let end = |j: usize| -> Vec3 {
            let ends = nodes[self.sk.joints[j].node]["children"].as_array().into_iter().flatten().filter_map(|c| nodes.get(c.as_u64()? as usize)).find(|n| n["name"].as_str().is_some_and(|n| n.ends_with("_end")));
            ends.and_then(|n| n["translation"].as_array()).map_or(Vec3::ZERO, |t| Vec3::new(t[0].as_f64().unwrap_or(0.0) as f32, t[1].as_f64().unwrap_or(0.0) as f32, t[2].as_f64().unwrap_or(0.0) as f32))
        };
        let leg = |front: bool, left: bool| -> Result<(usize, usize, Vec3)> {
            let side = if left { "L" } else { "R" };
            let (hip, last) = if front { (theirs(&format!("FrontUpperLeg.{side}"))?, theirs(&format!("FrontLowerLeg.{side}"))?) } else { (theirs(&format!("BackLeg.{side}"))?, theirs(&format!("BackLowerLeg.{side}"))?) };
            Ok((hip, last, end(last)))
        };
        let foot = |all: &[Mat4], l: &(usize, usize, Vec3)| all[l.1].transform_point3(l.2);
        // The two turned to face the same way: theirs from tail to head.
        let from = (pos(&stood[neck[3]]) - pos(&stood[spine[0]])).with_y(0.0).normalize_or(Vec3::Z);
        let turn = if from.dot(roles.forward) < -0.999 { Quat::from_rotation_y(std::f32::consts::PI) } else { Quat::from_rotation_arc(from, roles.forward) };
        let rest: Vec<Mat4> = to.joints.iter().map(|j| j.world).collect();
        // Which of their bones each of ours turns with: the one as far along its chain.
        let mut with: Vec<Option<usize>> = vec![None; to.joints.len()];
        let mut along = |ours: &[usize], their: &[usize]| {
            for (i, &j) in ours.iter().enumerate() {
                with[j] = Some(their[((i as f32 + 0.5) / ours.len() as f32 * their.len() as f32) as usize]);
            }
        };
        along(&roles.spine, &spine);
        along(&roles.head, &neck);
        along(&roles.tail, &tail);
        with[to.root] = Some(spine[0]);
        // Sizes: how high each stands at the shoulder, for how far the body moves; each leg by its own length.
        let floor = |all: &[Mat4]| all.iter().map(|m| pos(m).y).fold(f32::MAX, f32::min);
        let fore = leg(true, true)?;
        let shoulder_theirs = pos(&stood[fore.0]).y - foot(&stood, &fore).y;
        let shoulder_ours = roles.legs.iter().filter(|l| l.front).map(|l| pos(&rest[l.chain[0]]).y).fold(f32::MIN, f32::max) - floor(&rest);
        let big = (shoulder_ours / shoulder_theirs.max(1e-5)).max(1e-5);
        let order = parents_first(to);
        let base_turn = rot(&to.base);
        let mut out = Vec::with_capacity(frames.len());
        for frame in &frames {
            let mut pose = to.rest();
            let mut world = vec![Quat::IDENTITY; to.joints.len()];
            for &j in &order {
                let above = to.joints[j].parent.map_or(base_turn, |p| world[p]);
                world[j] = match with[j] {
                    Some(s) => {
                        let now = (turn * (rot(&frame[s]) * rot(&stood[s]).inverse()) * turn.inverse() * rot(&rest[j])).normalize();
                        pose.rotations[j] = (above.inverse() * now).normalize();
                        now
                    }
                    None => above * to.joints[j].rest_r,
                };
            }
            let moved = turn * ((pos(&frame[spine[0]]) - pos(&stood[spine[0]])) * big);
            pose.root_t = to.base.inverse().transform_point3(pos(&rest[to.root]) + moved);
            for ours in &roles.legs {
                // The leg, from where it leaves the body to the foot, lies as theirs lies: their
                // leg's line, by the two legs' lengths, from our own hip where the body now is.
                // (Feet put where theirs stood on the ground, whatever the body did, left a
                // wolf dead on its back with its legs in the air: its body had gone further
                // than its legs could follow.)
                let their = leg(ours.front, ours.left)?;
                let (line_ours, line_theirs) = (pos(&rest[ours.foot()]) - pos(&rest[ours.chain[0]]), turn * (foot(&stood, &their) - pos(&stood[their.0])));
                let fit = Quat::from_rotation_arc(line_theirs.normalize_or(Vec3::NEG_Y), line_ours.normalize_or(Vec3::NEG_Y));
                let hip = pos(&to.pose_world(&pose)[ours.chain[0]]);
                let target = hip + fit * (turn * (foot(frame, &their) - pos(&frame[their.0]))) * (line_ours.length() / line_theirs.length().max(1e-5));
                let was = pose.rotations.clone();
                crate::clips::reach(to, roles, &mut pose, ours, ours.foot(), target);
                if pose.rotations.iter().any(|r| !r.is_finite()) {
                    pose.rotations = was;
                }
            }
            out.push(pose);
        }
        Ok(out)
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
