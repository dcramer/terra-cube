//! The numbers a finished model is held to before anyone looks at it: one line a check,
//! `ok|FAIL name value (bar)`. The bars are in style.toml `[checks]`. The skin is posed on the
//! CPU through every clip, so a bad rig shows as stretch here rather than in the game.

use crate::clips::Clip;
use crate::finish::Finished;
use crate::glb::Glb;
use crate::rig::{Pose, Skeleton};
use crate::spec::Bars;
use anyhow::{Context, Result};
use glam::{Mat4, Vec3};

pub struct Line {
    pub ok: bool,
    pub text: String,
}

fn line(ok: bool, name: &str, value: String, bar: String) -> Line {
    Line { ok, text: format!("{} {name:<12} {value} ({bar})", if ok { "ok  " } else { "FAIL" }) }
}

struct Skin {
    positions: Vec<Vec3>,
    joints: Vec<[usize; 4]>,
    weights: Vec<[f32; 4]>,
    edges: Vec<(usize, usize)>,
}

fn skin(glb: &Glb) -> Result<Skin> {
    let mut s = Skin { positions: Vec::new(), joints: Vec::new(), weights: Vec::new(), edges: Vec::new() };
    for mesh in glb.array("meshes") {
        for p in mesh["primitives"].as_array().into_iter().flatten() {
            let attr = |name: &str| p["attributes"][name].as_u64().map(|a| a as usize);
            let (Some(pos), Some(j), Some(w)) = (attr("POSITION"), attr("JOINTS_0"), attr("WEIGHTS_0")) else { continue };
            let base = s.positions.len();
            s.positions.extend(glb.floats(pos)?.0.chunks(3).map(Vec3::from_slice));
            s.joints.extend(glb.floats(j)?.0.chunks(4).map(|c| [c[0] as usize, c[1] as usize, c[2] as usize, c[3] as usize]));
            s.weights.extend(glb.floats(w)?.0.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]));
            let idx: Vec<usize> = match p["indices"].as_u64() {
                Some(a) => glb.floats(a as usize)?.0.iter().map(|&i| i as usize + base).collect(),
                None => (base..s.positions.len()).collect(),
            };
            // At most about 30,000 edges: enough to find the worst places.
            let step = (idx.len() / 3 / 10_000).max(1);
            for t in idx.chunks(3).step_by(step) {
                if t.len() == 3 {
                    s.edges.extend([(t[0], t[1]), (t[1], t[2]), (t[2], t[0])]);
                }
            }
        }
    }
    Ok(s)
}

/// Where the muzzle is, as the model stands: the corner of its skin furthest forward and
/// low, and the bone it follows most. No bone ends at a muzzle, so nothing in the skeleton
/// says where one is; and not of the head's skin alone, since a goat's one head bone turned
/// out to carry only its horns.
pub fn nose(glb: &Glb, sk: &Skeleton, roles: &crate::rig::Roles) -> Option<(usize, Vec3)> {
    let s = skin(glb).ok()?;
    let at = posed(&s, sk, &sk.rest());
    let most = |i: usize| (0..4).max_by(|a, b| s.weights[i][*a].total_cmp(&s.weights[i][*b])).map(|k| s.joints[i][k]).unwrap_or(0);
    let score = |i: &usize| at[*i].dot(roles.forward) - 0.3 * at[*i].y;
    let pick = (0..at.len()).max_by(|a, b| score(a).total_cmp(&score(b)))?;
    Some((most(pick), at[pick]))
}

/// How far under the floor it stands on at rest the skin goes at each pose, file units (0
/// where it does not): what a carried clip is raised by, since a thick body laid down where a
/// thin one lay goes into the ground by the difference.
pub fn sunk(glb: &Glb, sk: &Skeleton, frames: &[Pose]) -> Vec<f32> {
    let Ok(s) = skin(glb) else { return vec![0.0; frames.len()] };
    let lowest = |pose: &Pose| posed(&s, sk, pose).iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let floor = lowest(&sk.rest());
    frames.iter().map(|pose| (floor - lowest(pose)).max(0.0)).collect()
}

fn posed(s: &Skin, sk: &Skeleton, pose: &Pose) -> Vec<Vec3> {
    let mats: Vec<Mat4> = sk.pose_world(pose).iter().zip(&sk.inverse_bind).map(|(w, ib)| *w * *ib).collect();
    s.positions
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let (mut out, mut total) = (Vec3::ZERO, 0.0);
            for k in 0..4 {
                let w = s.weights[i][k];
                if w > 0.0 {
                    out += mats[s.joints[i][k]].transform_point3(p) * w;
                    total += w;
                }
            }
            if total > 0.0 { out / total } else { p }
        })
        .collect()
}

/// The skin's bounds in a pose, in the file's space: the body as it stands, where the rest
/// pose of a T-posed rig would count its arms as width.
pub fn bounds(glb: &Glb, sk: &Skeleton, pose: &Pose) -> Result<(Vec3, Vec3)> {
    let s = skin(glb)?;
    Ok(posed(&s, sk, pose).iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p))))
}

/// How far the skin reaches along a direction in a pose: its extent, in the file's units.
/// True whatever way the direction points, where a bounding box is only for the axes.
pub fn extent(glb: &Glb, sk: &Skeleton, pose: &Pose, along: Vec3) -> Result<f32> {
    let s = skin(glb)?;
    let (lo, hi) = posed(&s, sk, pose).iter().map(|p| p.dot(along)).fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    Ok(hi - lo)
}

/// A model's skin read once, to be posed many times: for whoever searches for a pose and
/// must look at where the skin goes at each try.
pub struct Skinned {
    s: Skin,
    rest_len: Vec<f32>,
    /// The joint each corner follows most.
    pub most: Vec<usize>,
}

impl Skinned {
    pub fn of(glb: &Glb, sk: &Skeleton) -> Option<Self> {
        let s = skin(glb).ok()?;
        let rest = posed(&s, sk, &sk.rest());
        let rest_len = s.edges.iter().map(|&(a, b)| rest[a].distance(rest[b])).collect();
        let most = (0..s.positions.len()).map(|i| (0..4).max_by(|a, b| s.weights[i][*a].total_cmp(&s.weights[i][*b])).map_or(0, |k| s.joints[i][k])).collect();
        Some(Self { s, rest_len, most })
    }
    /// Where every corner of the skin is in a pose.
    pub fn at(&self, sk: &Skeleton, pose: &Pose) -> Vec<Vec3> {
        posed(&self.s, sk, pose)
    }
    /// How far a pose pulls the skin, as a clip's check measures it.
    pub fn tears(&self, sk: &Skeleton, pose: &Pose) -> f32 {
        stretch(&self.s, sk, &self.rest_len, &Clip { name: String::new(), looping: true, frames: vec![pose.clone()], source: "walker", speed: None })
    }
}

/// How far a run of poses pulls the skin, as the check of a clip will measure it: for
/// whoever makes a clip to know before it is kept.
pub fn tears(glb: &Glb, sk: &Skeleton, frames: &[Pose]) -> f32 {
    let Ok(s) = skin(glb) else { return 0.0 };
    let rest = posed(&s, sk, &sk.rest());
    let rest_len: Vec<f32> = s.edges.iter().map(|&(a, b)| rest[a].distance(rest[b])).collect();
    stretch(&s, sk, &rest_len, &Clip { name: String::new(), looping: true, frames: frames.to_vec(), source: "walker", speed: None })
}

/// The 99th-percentile edge stretch of a clip's worst sampled frame, against the rest pose.
fn stretch(s: &Skin, sk: &Skeleton, rest_len: &[f32], clip: &Clip) -> f32 {
    let step = (clip.frames.len() / 8).max(1);
    let mut worst = 0.0f32;
    for pose in clip.frames.iter().step_by(step) {
        let v = posed(s, sk, pose);
        let mut ratios: Vec<f32> = s.edges.iter().zip(rest_len).filter(|(_, r)| **r > 1e-6).map(|(&(a, b), &r)| (v[a].distance(v[b]) / r - 1.0).abs()).collect();
        if ratios.is_empty() {
            continue;
        }
        let k = ((ratios.len() as f32 * 0.99) as usize).min(ratios.len() - 1);
        let (_, p99, _) = ratios.select_nth_unstable_by(k, |a, b| a.total_cmp(b));
        worst = worst.max(*p99);
    }
    worst
}

pub fn check(done: &Finished, required: &[String], bars: &Bars, max_stretch: f32, max_slide: f32) -> Result<Vec<Line>> {
    let (sk, roles, meta) = (&done.skeleton, &done.roles, &done.meta);
    let mut out = Vec::new();
    out.push(line(meta.triangles <= bars.max_triangles, "triangles", meta.triangles.to_string(), format!("≤ {}", bars.max_triangles)));
    out.push(line(meta.joints <= bars.max_joints, "joints", meta.joints.to_string(), format!("≤ {}", bars.max_joints)));
    if crate::shaped::made(&done.glb) {
        // Rigged here from its shape: the bones are ours and all there, or it would not have finished.
        out.push(line(sk.joints.len() == crate::shaped::BONES.len(), "roles", "a body, its two ends and two wings of two bones".into(), "rigged here".into()));
    } else {
        out.push(line(
            roles.legs.len() >= 2 && !roles.head.is_empty(),
            "roles",
            format!("{} legs, head {}, tail {}, {} other", roles.legs.len(), roles.head.len(), roles.tail.len(), roles.others.len()),
            "≥ 2 legs and a head".into(),
        ));
    }
    let missing: Vec<&str> = required.iter().filter(|c| !meta.clips.contains_key(*c)).map(String::as_str).collect();
    out.push(line(missing.is_empty(), "clips", format!("{} of {}{}", required.len() - missing.len(), required.len(), if missing.is_empty() { String::new() } else { format!(", missing {}", missing.join(" ")) }), "all".into()));

    let s = skin(&done.glb).context("reading the skin")?;
    let bare = s.weights.iter().filter(|w| w.iter().sum::<f32>() < 0.5).count();
    out.push(line(bare == 0, "weights", format!("{bare} vertices unweighted"), "0".into()));

    let rest = posed(&s, sk, &sk.rest());
    let rest_len: Vec<f32> = s.edges.iter().map(|&(a, b)| rest[a].distance(rest[b])).collect();
    let floor = roles.legs.iter().map(|l| sk.position(l.foot()).y).fold(f32::MAX, f32::min);
    for clip in &done.clips {
        let mut notes = Vec::new();
        let mut ok = true;
        let st = stretch(&s, sk, &rest_len, clip);
        ok &= st <= max_stretch;
        notes.push(format!("stretch {:.0}%", st * 100.0));
        if clip.looping {
            let (a, b) = (&clip.frames[0], clip.frames.last().unwrap());
            let gap = a.rotations.iter().zip(&b.rotations).map(|(x, y)| x.angle_between(*y)).fold(0.0, f32::max);
            ok &= gap <= bars.max_loop_gap;
            notes.push(format!("loop gap {:.1}°", gap.to_degrees()));
        }
        if !roles.legs.is_empty() && matches!(clip.name.as_str(), "idle" | "walk" | "run" | "sprint" | "eat") {
            // The lowest foot should stay on the floor it stands on at rest. A run may leave
            // the ground, so there only sinking counts.
            let run = matches!(clip.name.as_str(), "run" | "sprint");
            let err = clip
                .frames
                .iter()
                .map(|p| {
                    let w = sk.pose_world(p);
                    let off = roles.legs.iter().map(|l| w[l.foot()].w_axis.y).fold(f32::MAX, f32::min) - floor;
                    if run { (-off).max(0.0) } else { off.abs() }
                })
                .fold(0.0, f32::max)
                / roles.height;
            ok &= err <= bars.max_foot_error;
            notes.push(format!("feet off floor {:.1}%", err * 100.0));
        }
        let pace = match clip.name.as_str() {
            "walk" => meta.walk_speed,
            "run" => meta.run_speed,
            "sprint" => meta.sprint_speed,
            _ => 0.0,
        };
        if pace > 0.0 && roles.legs.len() >= 2 {
            // Moved at its measured speed, a foot that is down should stay where it is put.
            let slide = crate::clips::slide(sk, roles, &clip.frames, pace / done.scale);
            // Only a made gait is held to it: its feet are put down by rule. A recorded one is a
            // person's, whose ankle does not go at one speed while the foot is down (heel,
            // roll, toe), and failing it for that had every recorded walk replaced by a shuffle.
            if clip.source == "made" {
                ok &= slide <= max_slide;
                notes.push(format!("slide {:.0}%", slide * 100.0));
            }
        }
        // However it was made, nothing of a creature at rest is in the ground: the lowest
        // of all its skin against the floor it stands on (the developer saw a wolf's paws in
        // the floor at a glance, and no check knew).
        if clip.name == "lie" {
            let under = sunk(&done.glb, sk, &clip.frames[..1]).first().copied().unwrap_or(0.0) / roles.height.max(1e-4);
            ok &= under <= 0.03;
            notes.push(format!("in the ground {:.0}%", under * 100.0));
        }
        if clip.name == "lie" && clip.source == "walker" && let Some((text, good)) = crate::walker::measured(&done.glb, sk, roles, &clip.frames[0]) {
            out.push(line(good, "pose lie", text, "assets/creatures/poses.toml".into()));
        }
        out.push(line(ok, &format!("clip {}", clip.name), format!("{:.2}s {} · {}", clip.seconds(), clip.source, notes.join(" · ")), format!("stretch ≤ {:.0}%, gap ≤ {:.0}°, feet ≤ {:.0}%, slide ≤ {:.0}%", max_stretch * 100.0, bars.max_loop_gap.to_degrees(), bars.max_foot_error * 100.0, max_slide * 100.0)));
    }
    Ok(out)
}
