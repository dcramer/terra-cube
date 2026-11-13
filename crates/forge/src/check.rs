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
    off_floor(glb, sk, frames).into_iter().map(|under| under.max(0.0)).collect()
}

/// How far the lowest of its skin is from the floor it stands on at rest, at each pose: under
/// it (over nothing) or held up off it (under nothing). What has fallen is on the ground:
/// a death carried onto a body whose middle is higher than the one it was drawn on lay on
/// its side in the air.
pub fn off_floor(glb: &Glb, sk: &Skeleton, frames: &[Pose]) -> Vec<f32> {
    let Ok(s) = skin(glb) else { return vec![0.0; frames.len()] };
    let lowest = |pose: &Pose| posed(&s, sk, pose).iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let floor = lowest(&sk.rest());
    frames.iter().map(|pose| floor - lowest(pose)).collect()
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

/// The box each part of it fills in a pose: its head (from the neck out), its tail, each leg
/// and arm, and its trunk, which is all the rest. A corner of the skin is of the part whose
/// bone it follows most, or the nearest such bone up from that one (an ear is the head's, a
/// finger the arm's). In metres (`scale` to a unit of the file) from the ground under the
/// middle of the whole: x to its left, y up, z the way it faces. A part with too little
/// skin to be struck (under eight corners) is left to the trunk.
pub fn parts(glb: &Glb, sk: &Skeleton, pose: &Pose, roles: &crate::rig::Roles, scale: f32) -> Result<Vec<crate::spec::HitBox>> {
    let s = skin(glb)?;
    let at = posed(&s, sk, pose);
    let mut of: Vec<Option<usize>> = vec![None; sk.joints.len()];
    let mut names: Vec<&str> = vec!["trunk"];
    let mut claim = |bones: &[usize], name: &'static str, names: &mut Vec<&str>| {
        names.push(name);
        for b in bones {
            of[*b] = Some(names.len() - 1);
        }
    };
    claim(&roles.head, "head", &mut names);
    claim(&roles.tail, "tail", &mut names);
    for leg in &roles.legs {
        claim(&leg.chain, "leg", &mut names);
    }
    for arm in &roles.arms {
        claim(&arm.chain, "arm", &mut names);
    }
    let part = |mut j: usize| loop {
        if let Some(p) = of[j] {
            return p;
        }
        match sk.joints[j].parent {
            Some(up) => j = up,
            None => return 0,
        }
    };
    let up = roles.side.cross(roles.forward).normalize_or(Vec3::Y);
    let up = if up.y < 0.0 { -up } else { up };
    let frame = |p: Vec3| Vec3::new(p.dot(roles.side), p.dot(up), p.dot(roles.forward));
    let all = at.iter().map(|p| frame(*p)).fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
    let origin = Vec3::new((all.0.x + all.1.x) / 2.0, all.0.y, (all.0.z + all.1.z) / 2.0);
    let mut boxes: Vec<(Vec3, Vec3, usize)> = vec![(Vec3::MAX, Vec3::MIN, 0); names.len()];
    for (i, p) in at.iter().enumerate() {
        let most = (0..4).max_by(|a, b| s.weights[i][*a].total_cmp(&s.weights[i][*b])).map_or(0, |k| s.joints[i][k]);
        let (b, p) = (&mut boxes[part(most)], (frame(*p) - origin) * scale);
        *b = (b.0.min(p), b.1.max(p), b.2 + 1);
    }
    // (What is too slight is the trunk's.)
    for n in 1..boxes.len() {
        if boxes[n].2 > 0 && boxes[n].2 < 8 {
            let small = boxes[n];
            boxes[0] = (boxes[0].0.min(small.0), boxes[0].1.max(small.1), boxes[0].2 + small.2);
            boxes[n].2 = 0;
        }
    }
    Ok(boxes.iter().zip(&names).filter(|(b, _)| b.2 > 0).map(|(b, name)| crate::spec::HitBox { part: (*name).into(), lo: b.0.to_array().map(|v| (v * 1000.0).round() / 1000.0), hi: b.1.to_array().map(|v| (v * 1000.0).round() / 1000.0) }).collect())
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
        // `FORGE_TRACE=1`: of the hundredth of its edges stretched most, which bones hold them.
        if std::env::var_os("FORGE_TRACE").is_some() {
            let mut all: Vec<(f32, usize)> = s.edges.iter().zip(rest_len).filter(|(_, r)| **r > 1e-6).map(|(&(a, b), &r)| ((v[a].distance(v[b]) / r - 1.0).abs(), s.joints[a][0])).collect();
            all.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut by = std::collections::BTreeMap::new();
            for (_, bone) in all.iter().take(all.len() / 100) {
                *by.entry(sk.joints[*bone].name.clone()).or_insert(0) += 1;
            }
            println!("stretch {}: worst {:.0}% · held by {by:?}", clip.name, all.first().map_or(0.0, |a| a.0 * 100.0));
            // And where on it they are, as it rests: which pair of bones each lies between,
            // and the box they fill (file units). The bones alone did not say: two guesses
            // at where a toad tore were wrong before this was asked, 2026-11-13.
            let rest = posed(s, sk, &sk.rest());
            let mut torn: Vec<(f32, usize, usize)> = s.edges.iter().zip(rest_len).filter(|(_, r)| **r > 1e-6).map(|(&(a, b), &r)| ((v[a].distance(v[b]) / r - 1.0).abs(), a, b)).collect();
            torn.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut pairs: std::collections::BTreeMap<String, (usize, Vec3, Vec3)> = Default::default();
            for (_, a, b) in torn.iter().take(torn.len() / 100) {
                let (x, y) = (&sk.joints[s.joints[*a][0]].name, &sk.joints[s.joints[*b][0]].name);
                let e = pairs.entry(if x <= y { format!("{x}+{y}") } else { format!("{y}+{x}") }).or_insert((0, Vec3::MAX, Vec3::MIN));
                *e = (e.0 + 1, e.1.min(rest[*a]).min(rest[*b]), e.2.max(rest[*a]).max(rest[*b]));
            }
            let mut pairs: Vec<_> = pairs.into_iter().collect();
            pairs.sort_by_key(|p| std::cmp::Reverse(p.1.0));
            println!("stretch {} where: {}", clip.name, pairs.iter().take(5).map(|(k, (n, lo, hi))| format!("{k} ×{n} from {lo:.2} to {hi:.2}")).collect::<Vec<_>>().join(" · "));
        }
        let k = ((ratios.len() as f32 * 0.99) as usize).min(ratios.len() - 1);
        let (_, p99, _) = ratios.select_nth_unstable_by(k, |a, b| a.total_cmp(b));
        worst = worst.max(*p99);
    }
    worst
}

/// The least a going creature's foot must swing fore and aft, of its leg's length.
const STIRS: f32 = 0.3;

pub fn check(done: &Finished, required: &[String], bars: &Bars, max_stretch: f32, max_slide: f32) -> Result<Vec<Line>> {
    let (sk, roles, meta) = (&done.skeleton, &done.roles, &done.meta);
    let mut out = Vec::new();
    out.push(line(meta.triangles <= bars.max_triangles, "triangles", meta.triangles.to_string(), format!("≤ {}", bars.max_triangles)));
    out.push(line(meta.joints <= bars.max_joints, "joints", meta.joints.to_string(), format!("≤ {}", bars.max_joints)));
    if crate::crawl::made(&done.glb) {
        out.push(line(roles.legs.len() >= 2, "roles", format!("a body, a head, a tail and {} legs: built here", roles.legs.len()), "at least two legs".into()));
    } else if crate::own::made(&done.glb) {
        out.push(line(sk.joints.len() == crate::own::BONES.len(), "roles", "a trunk, neck, head, tail and four legs: the walker's own bones".into(), "rigged here".into()));
    } else if crate::shaped::made(&done.glb) {
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
        // Every number of it is a number: a turn or a place that is not (a leg solved past
        // its reach) draws nothing at all, and passes every bar below, since nothing compares
        // with it. (The hedgehog walked unseen, and was passed, until it was looked at.)
        let broken = clip.frames.iter().filter(|f| !f.root_t.is_finite() || f.rotations.iter().any(|r| !r.is_finite())).count();
        if broken > 0 {
            ok = false;
            notes.push(format!("{broken} of {} frames are not numbers", clip.frames.len()));
        }
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
            // Its legs go: how far the foot that goes furthest swings fore and aft under the
            // body, of how far up its leg leaves it. A body carried along on legs that
            // hardly stir passes every other bar (a hedgehog, a frog and a toad did, and were
            // found only by looking).
            let swing = roles.legs.iter().map(|l| {
                let along: Vec<f32> = clip.frames.iter().map(|f| { let w = sk.pose_world(f); (w[l.foot()].w_axis.truncate() - w[sk.root].w_axis.truncate()).dot(roles.forward) }).collect();
                let long = (sk.position(l.chain[0]).y - roles.floor).max(roles.height * 0.05);
                (along.iter().copied().fold(f32::MIN, f32::max) - along.iter().copied().fold(f32::MAX, f32::min)) / long
            }).fold(0.0f32, f32::max);
            ok &= swing >= STIRS;
            if std::env::var_os("FORGE_TRACE").is_some() {
                println!("swing {} {:.0}%", clip.name, swing * 100.0);
            }
            if swing < STIRS {
                notes.push(format!("legs swing {:.0}% of their length", swing * 100.0));
            }
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
