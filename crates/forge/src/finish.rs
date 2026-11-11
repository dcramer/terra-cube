//! The local, free, repeatable half of a build: Tripo's rigged export in, the game's model out.
//! Stands the model at the origin facing +Z at its height in metres, names and makes its clips,
//! and measures what the game needs to know (`meta.toml`). Run again after any change to
//! clips.rs or style.toml: it never talks to Tripo.

use crate::clips::{self, Clip};
use crate::glb::Glb;
use crate::rig::{Facing, Roles, Skeleton};
use crate::spec::{self, ClipMeta, Creature, Meta, Plan};
use anyhow::{Context, Result, bail};
use glam::{Quat, Vec3};
use serde_json::{Value, json};
use std::path::Path;

pub struct Finished {
    pub glb: Glb,
    pub skeleton: Skeleton,
    pub roles: Roles,
    pub clips: Vec<Clip>,
    pub meta: Meta,
    /// Metres to a unit of the file the skeleton and clips are in.
    pub scale: f32,
}

/// Mesh bounds from the POSITION accessors' own min and max.
fn bounds(glb: &Glb) -> Result<(Vec3, Vec3)> {
    let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
    for mesh in glb.array("meshes") {
        for p in mesh["primitives"].as_array().into_iter().flatten() {
            let a = &glb.array("accessors")[p["attributes"]["POSITION"].as_u64().context("primitive without POSITION")? as usize];
            let v = |key: &str| -> Vec3 { Vec3::from_slice(&a[key].as_array().map_or(vec![0.0; 3], |m| m.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect())) };
            lo = lo.min(v("min"));
            hi = hi.max(v("max"));
        }
    }
    if lo.x > hi.x {
        bail!("the model has no mesh");
    }
    Ok((lo, hi))
}

/// How fast the ground passes under the feet of an in-place gait, in file units a second:
/// the middle backward speed of feet while they are down. The middle, not the mean, because
/// that is the speed at which the fewest planted frames slide (`check` measures the same).
fn ground_speed(sk: &Skeleton, roles: &Roles, clip: &Clip) -> Option<f32> {
    let floor = roles.legs.iter().map(|l| sk.position(l.foot()).y).fold(f32::MAX, f32::min);
    let worlds: Vec<_> = clip.frames.iter().map(|p| sk.pose_world(p)).collect();
    // A foot is down within a hundredth of the body's height of the floor; widen that for a
    // clip whose feet never quite get there.
    for near in [0.01, 0.03, 0.08] {
        let mut speeds = Vec::new();
        for w in worlds.windows(2) {
            for leg in &roles.legs {
                let foot = leg.foot();
                if w.iter().all(|m| m[foot].w_axis.y - floor <= near * roles.height) {
                    speeds.push(-(w[1][foot].w_axis.truncate() - w[0][foot].w_axis.truncate()).dot(roles.forward) * clips::FPS);
                }
            }
        }
        if speeds.len() >= 6 {
            speeds.sort_by(f32::total_cmp);
            let middle = speeds[speeds.len() / 2];
            return (middle > 0.0).then_some(middle);
        }
    }
    None
}

pub fn finish(source: &Path, creature: &Creature, plan: &Plan, max_slide: f32) -> Result<Finished> {
    let mut glb = Glb::read(source)?;
    let skeleton = Skeleton::read(&glb)?;
    let roles = Roles::read(&skeleton, Facing::parse(&creature.facing)?);
    // A four-legged creature with any other number of legs has a skeleton that is short of
    // one (Tripo's rig sometimes is) or one the reader gets wrong. Say so; do not make clips.
    if creature.plan == "quadruped" && roles.legs.len() != 4 {
        bail!("{} legs found on a four-legged creature: the rig is short of a leg, or misread. `cargo forge inspect` shows it; a rig short of a leg wants rigging again", roles.legs.len());
    }

    let (lo, hi) = bounds(&glb)?;
    let scale = creature.height / (hi.y - lo.y).max(1e-5);

    // Clips: Tripo's under our names, then the made ones the plan asks for.
    let theirs = clips::read(&glb, &skeleton)?;
    let mut out: Vec<Clip> = Vec::new();
    for name in &plan.clips {
        let preset = plan.presets.get(name).filter(|_| !creature.made.contains(name));
        let found = preset.and_then(|p| theirs.iter().position(|c| c.name.rsplit(':').next() == Some(p.as_str())));
        let clip = match found {
            Some(i) => {
                let looping = !matches!(name.as_str(), "attack" | "hurt" | "die" | "jump");
                let mut frames = match plan.trim.get(name) {
                    Some(&[from, to, speed]) => clips::trimmed(&theirs[i], from, to, speed),
                    None => theirs[i].frames.clone(),
                };
                if matches!(name.as_str(), "walk" | "run") && creature.gait_gain != 1.0 {
                    clips::gained(&skeleton, &mut frames, creature.gait_gain);
                }
                if looping {
                    clips::close_loop(&mut frames);
                }
                Clip { name: name.clone(), looping, frames, source: "tripo", speed: None }
            }
            None => {
                let stand = out.iter().find(|c| c.name == "idle" && c.source == "tripo").map(|c| &c.frames[0]);
                clips::make(name, &skeleton, &roles, &out, stand, plan.upright, scale, (1.0 + creature.gait_gain) / 2.0).with_context(|| format!("no preset and no recipe for the clip `{name}` (clips.rs `make`)"))?
            }
        };
        let mut clip = clip;
        // A body that needed its gait reined in needs the same of a leap, by half as much.
        if matches!(name.as_str(), "jump" | "fall") && clip.source == "made" && creature.gait_gain != 1.0 {
            clips::gained(&skeleton, &mut clip.frames, (1.0 + creature.gait_gain) / 2.0);
        }
        if matches!(name.as_str(), "walk" | "run" | "sprint") && !roles.legs.is_empty() && clip.speed.is_none() {
            clips::grounded(&skeleton, &roles, &mut clip.frames);
        }
        // A recorded gait: find its speed and hold its feet to it. If they still will not hold
        // (the preset is fitted to another body's proportions), a made gait takes its place.
        if matches!(name.as_str(), "walk" | "run" | "sprint") && clip.speed.is_none() && roles.legs.len() >= 2 {
            let longer = plan.stride.get(name).copied().unwrap_or(1.0);
            if let Some(pace) = ground_speed(&skeleton, &roles, &clip).map(|p| p * longer) {
                clips::planted(&skeleton, &roles, &mut clip.frames, pace);
                clip.speed = Some(pace);
            }
            let slid = clip.speed.map_or(1.0, |pace| clips::slide(&skeleton, &roles, &clip.frames, pace));
            if slid > max_slide {
                let stand = out.iter().find(|c| c.name == "idle" && c.source == "tripo").map(|c| &c.frames[0]);
                let made: Vec<Clip> = out.iter().filter(|c| c.source == "made").map(|c| Clip { name: c.name.clone(), looping: c.looping, frames: Vec::new(), source: "made", speed: c.speed }).collect();
                if let Some(better) = clips::make(name, &skeleton, &roles, &made, stand, plan.upright, scale, (1.0 + creature.gait_gain) / 2.0).filter(|c| c.speed.is_some()) {
                    clip = better;
                }
            }
        }
        out.push(clip);
    }

    // Stand it at the origin, facing +Z, at its height.
    let turn = Quat::from_rotation_arc(roles.forward, Vec3::Z);
    let feet = roles.legs.iter().map(|l| skeleton.position(l.foot())).sum::<Vec3>() / roles.legs.len().max(1) as f32;
    let centre = if roles.legs.is_empty() { (lo + hi) / 2.0 } else { feet };
    let pivot = Vec3::new(centre.x, lo.y, centre.z);
    let offset = -(turn * (pivot * scale));
    let scene = glb.json["scene"].as_u64().unwrap_or(0) as usize;
    let roots = glb.json["scenes"][scene]["nodes"].clone();
    let nodes = glb.json["nodes"].as_array_mut().context("no nodes")?;
    nodes.push(json!({ "name": "creature", "children": roots, "translation": offset.to_array(), "rotation": turn.to_array(), "scale": [scale, scale, scale] }));
    let wrapper = nodes.len() - 1;
    glb.json["scenes"][scene]["nodes"] = json!([wrapper]);
    glb.json["asset"]["generator"] = Value::String("terra-cube forge".into());

    clips::write(&mut glb, &skeleton, &out);

    let walk = out.iter().find(|c| c.name == "walk");
    let run = out.iter().find(|c| c.name == "run");
    let sprint = out.iter().find(|c| c.name == "sprint");
    // A walk nobody can measure moves about a body length a cycle.
    let speed = |c: Option<&Clip>| c.map(|c| c.speed.or_else(|| ground_speed(&skeleton, &roles, c)).unwrap_or(roles.length / c.seconds()) * scale).unwrap_or(0.0);
    let name = |j: Option<&usize>| j.map(|&j| skeleton.joints[j].name.clone());
    let mut bones = std::collections::BTreeMap::new();
    for (role, joint) in [("root", Some(&skeleton.root)), ("head", roles.head.last()), ("neck", roles.head.first()), ("tail", roles.tail.first())] {
        if let Some(n) = name(joint) {
            bones.insert(role.to_string(), n);
        }
    }
    for (i, leg) in roles.legs.iter().enumerate() {
        bones.insert(format!("foot{i}"), skeleton.joints[leg.foot()].name.clone());
    }
    // Its size as it stands (the first frame of idle), not as it rests: a T-pose is all arms.
    let stand = out.iter().find(|c| c.name == "idle").map_or_else(|| skeleton.rest(), |c| c.frames[0].clone());
    let (slo, shi) = crate::check::bounds(&glb, &skeleton, &stand).unwrap_or((lo, hi));
    let meta = Meta {
        height: creature.height,
        length: crate::check::extent(&glb, &skeleton, &stand, roles.forward).unwrap_or((shi - slo).dot(roles.forward).abs()) * scale,
        width: crate::check::extent(&glb, &skeleton, &stand, roles.side).unwrap_or((shi - slo).dot(roles.side).abs()) * scale,
        walk_speed: speed(walk),
        run_speed: speed(run),
        sprint_speed: speed(sprint),
        triangles: glb.triangles(),
        joints: skeleton.joints.len(),
        clips: out.iter().map(|c| (c.name.clone(), ClipMeta { seconds: c.seconds(), looping: c.looping, source: c.source.into() })).collect(),
        bones,
    };
    Ok(Finished { glb, skeleton, roles, clips: out, meta, scale })
}

/// Runs the finish for a creature and writes `model.glb` and `meta.toml`.
pub fn run(id: &str, style: &spec::Style) -> Result<Finished> {
    let creature: Creature = spec::load(&spec::dir(id).join("creature.toml"))?;
    let plan = style.plans.get(&creature.plan).with_context(|| format!("{id}: no plan `{}` in style.toml", creature.plan))?;
    let source = spec::work(id).join("rigged.glb");
    if !source.exists() {
        bail!("{id}: {} is missing; `cargo forge build {id}` fetches it", source.display());
    }
    let mut done = finish(&source, &creature, plan, style.checks.max_slide)?;
    done.glb.write(&spec::dir(id).join("model.glb"))?;
    spec::save(&spec::dir(id).join("meta.toml"), &done.meta, "# Measured by `cargo forge finish`; do not edit.\n")?;
    Ok(done)
}
