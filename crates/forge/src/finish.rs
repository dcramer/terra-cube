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

/// How fast a recorded gait goes, two ways, over each pair of legs (a biped's one, an animal's
/// fore and hind). `stance`: of a pair, the foot that is lower is the one carrying the body,
/// and the body goes forward as fast as that foot goes back; the middle of those speeds.
/// `steps`: how far apart the pair's feet get, fore and aft, added up over the clip: each
/// swing from one foot ahead to the other ahead is a step's travel. The first is right for a
/// run too (a step's travel leaves out the flight); the second is the check on it.
fn recorded_speed(sk: &Skeleton, roles: &Roles, clip: &Clip) -> Option<(f32, f32)> {
    let pair = |front: bool| -> Option<[usize; 2]> {
        let of: Vec<usize> = roles.legs.iter().filter(|l| l.front == front).map(|l| l.foot()).collect();
        (of.len() == 2).then(|| [of[0], of[1]])
    };
    let pairs: Vec<[usize; 2]> = if roles.legs.len() == 2 { vec![[roles.legs[0].foot(), roles.legs[1].foot()]] } else { [pair(true), pair(false)].into_iter().flatten().collect() };
    if pairs.is_empty() {
        return None;
    }
    let worlds: Vec<_> = clip.frames.iter().map(|p| sk.pose_world(p)).collect();
    let (mut stance, mut swung) = (Vec::new(), 0.0);
    for w in worlds.windows(2) {
        let at = |m: &Vec<glam::Mat4>, j: usize| m[j].w_axis.truncate();
        for &[a, b] in &pairs {
            let (low, high) = if at(&w[0], a).y < at(&w[0], b).y { (a, b) } else { (b, a) };
            if at(&w[0], high).y - at(&w[0], low).y > 0.01 * roles.height {
                stance.push(-(at(&w[1], low) - at(&w[0], low)).dot(roles.forward) * clips::FPS);
            }
            let apart = |m: &Vec<glam::Mat4>| (at(m, a) - at(m, b)).dot(roles.forward);
            swung += (apart(&w[1]) - apart(&w[0])).abs();
        }
    }
    stance.retain(|v| *v > 0.0);
    stance.sort_by(f32::total_cmp);
    let steps = swung / 2.0 / clip.seconds() / pairs.len() as f32;
    Some((*stance.get(stance.len() / 2)?, steps))
}

pub fn finish(source: &Path, creature: &Creature, plan: &Plan, _max_slide: f32) -> Result<Finished> {
    // A model built here of boxes has no file to read; any other is Tripo's export.
    let mut glb = match &creature.built {
        Some(built) => crate::built::model(built)?,
        None => Glb::read(source)?,
    };
    // What flies or swims is given its bones here (`shaped`), whoever made its mesh.
    if plan.rig == "forge" {
        crate::shaped::rig(&mut glb, creature.built.is_some())?;
    }
    let skeleton = Skeleton::read(&glb)?;
    // (A model built here faces +z: it was built so.)
    let mut roles = Roles::read(&skeleton, if creature.built.is_some() { Facing::Told(Vec3::Z) } else { Facing::parse(&creature.facing)? });
    roles.nose = crate::check::nose(&glb, &skeleton, &roles);
    // A four-legged creature with any other number of legs has a skeleton that is short of
    // one (Tripo's rig sometimes is) or one the reader gets wrong. Say so; do not make clips.
    if creature.plan == "quadruped" && roles.legs.len() != 4 {
        bail!("{} legs found on a four-legged creature: the rig is short of a leg, or misread. `cargo forge inspect` shows it; a rig short of a leg wants rigging again", roles.legs.len());
    }

    let (lo, hi) = bounds(&glb)?;
    let scale = match creature.span {
        // (Built here, it is already its true size.)
        _ if creature.built.is_some() => 1.0,
        Some(span) => span / (hi.x - lo.x).max(hi.z - lo.z).max(1e-5),
        None => creature.height / (hi.y - lo.y).max(1e-5),
    };

    // Clips: Tripo's under our names, then the made ones the plan asks for.
    let theirs = clips::read(&glb, &skeleton)?;
    let mut out: Vec<Clip> = Vec::new();
    let mut library: Option<crate::library::Library> = None;
    for name in plan.clips.iter().filter(|c| !creature.without.contains(c)) {
        let preset = plan.presets.get(name).filter(|_| !creature.made.contains(name));
        // (A described motion's clip is named by its description, cut short: the start is enough.)
        let found = preset.and_then(|p| theirs.iter().position(|c| c.name.rsplit(':').next() == Some(p.as_str()) || (plan.motions.contains_key(p) && c.name.starts_with(p.as_str()))));
        // A clip of the library, carried onto this skeleton, comes before Tripo's own.
        let carried = match plan.library.get(name).filter(|_| !creature.made.contains(name)) {
            Some(theirs) => {
                if library.is_none() {
                    library = Some(crate::library::Library::read()?);
                }
                Some(Clip { name: name.clone(), looping: true, frames: library.as_ref().unwrap().carry(theirs, &skeleton)?, source: "library", speed: None })
            }
            None => None,
        };
        let recorded = carried.as_ref().or(found.map(|i| &theirs[i]));
        let clip = match recorded {
            Some(taken) => {
                let source = taken.source;
                let looping = !matches!(name.as_str(), "attack" | "hurt" | "die" | "jump" | "land");
                let mut frames = match plan.trim.get(name) {
                    Some(&[from, to, speed]) => clips::trimmed(taken, from, to, speed),
                    None => taken.frames.clone(),
                };
                if matches!(name.as_str(), "walk" | "run") && creature.gait_gain != 1.0 {
                    clips::gained(&skeleton, &mut frames, creature.gait_gain);
                }
                if looping {
                    clips::close_loop(&mut frames);
                    clips::centred(&skeleton, &roles, &mut frames);
                }
                // A leap is recorded leaving the ground, and the game is what lifts the body:
                // each moment of it is stood on its own lowest foot.
                if matches!(name.as_str(), "jump" | "fall" | "land") {
                    clips::footed(&skeleton, &roles, &mut frames);
                }
                // The library's swimmer lies at the height of its own feet; the game holds a
                // swimmer by the middle. It is raised until its hips, on the whole, are where
                // they are standing.
                if name == "swim" && source == "library" {
                    let high = |pose: &crate::rig::Pose| skeleton.pose_world(pose)[skeleton.root].w_axis.y;
                    let lift = high(&skeleton.rest()) - frames.iter().map(high).sum::<f32>() / frames.len().max(1) as f32;
                    for pose in &mut frames {
                        pose.root_t += skeleton.base.inverse().transform_vector3(Vec3::Y * lift);
                    }
                }
                // And no part of it goes under the ground: a troll laid down where the library's
                // slight figure lay was a hand's depth into the floor, and a golem's toes in a lunge.
                if source == "library" && name != "swim" {
                    let under = crate::check::sunk(&glb, &skeleton, &frames);
                    for (pose, under) in frames.iter_mut().zip(under) {
                        pose.root_t += skeleton.base.inverse().transform_vector3(Vec3::Y * under);
                    }
                }
                Clip { name: name.clone(), looping, frames, source, speed: None }
            }
            None => {
                let stand = out.iter().find(|c| c.name == "idle" && c.source != "made").map(|c| &c.frames[0]);
                clips::make(name, &skeleton, &roles, &out, stand, plan.upright, scale, (1.0 + creature.gait_gain) / 2.0, creature.ease, creature.hops).with_context(|| format!("no preset and no recipe for the clip `{name}` (clips.rs `make`)"))?
            }
        };
        let mut clip = clip;
        // A body that needed its gait reined in needs the same of a leap, by half as much.
        if matches!(name.as_str(), "jump" | "fall") && clip.source == "made" && creature.gait_gain != 1.0 {
            clips::gained(&skeleton, &mut clip.frames, (1.0 + creature.gait_gain) / 2.0);
        }
        if matches!(name.as_str(), "walk" | "run" | "sprint") && !roles.legs.is_empty() && clip.speed.is_none() && clip.source == "made" {
            clips::grounded(&skeleton, &roles, &mut clip.frames);
        }
        // A recorded gait is kept as it was recorded, and the game moves the body at the speed
        // its feet say. Until 2026-11-11 its feet were then locked to a straight line at that
        // speed, its stride lengthened, and the whole gait replaced by a made one where the
        // feet still slid by our measure. Set side by side (three sheets of the player), the
        // recorded walk was a person walking and what replaced it was a shuffle: an ankle
        // does not travel at one speed while its foot is down (the heel lands, the foot
        // rolls, the toe pushes off), so the measure failed a good clip.
        if matches!(name.as_str(), "walk" | "run" | "sprint") && clip.source != "made" && clip.speed.is_none()
            && let Some((stance, steps)) = recorded_speed(&skeleton, &roles, &clip)
        {
            if std::env::var_os("FORGE_TRACE").is_some() {
                eprintln!("{name} recorded: {:.2} m/s by the foot that is down, {:.2} by its steps", stance * scale, steps * scale);
            }
            // Where the two disagree by more than a quarter the body is not one the first
            // can read (the troll's run: its lower foot is not always the one down).
            clip.speed = Some(if (stance / steps - 1.0).abs() < 0.25 { stance } else { steps });
            // The library's sprint is a sprinter's, near twice the run (its steps say less: a
            // sprinter is in the air between them). The game's sprint is half as fast again
            // as the run, so the clip is played that much slower and its feet still hold.
            if let (true, Some(run)) = (name == "sprint" && clip.source == "library", out.iter().find(|c| c.name == "run").and_then(|c| c.speed)) {
                let slower = (run * 1.5 / stance).clamp(0.6, 1.0);
                clip.frames = clips::trimmed(&clip, 0.0, clip.seconds(), slower);
                clip.speed = Some(stance * slower);
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
        // For the game to put the foot on the ground that is there (`cube_game::feet`): the
        // hip, the knee, and what stands on the ground (an upright figure's ankle, an
        // animal's toe), as the made gaits take them. A leg whose bones' names are not its
        // own (Tripo has given two bones one name) is left out.
        let tip = if plan.upright && leg.chain.len() >= 4 { leg.chain[2] } else { leg.foot() };
        let chain = [leg.chain[0], leg.chain[1.min(leg.chain.len() - 1)], tip].map(|j| skeleton.joints[j].name.clone());
        let own = |n: &String| skeleton.joints.iter().filter(|j| &j.name == n).count() == 1;
        if leg.chain.len() >= 3 && chain.iter().all(own) && chain[0] != chain[1] && chain[1] != chain[2] {
            bones.insert(format!("leg{i}"), chain.join(">"));
        }
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
    if !source.exists() && creature.built.is_none() {
        bail!("{id}: {} is missing; `cargo forge build {id}` fetches it", source.display());
    }
    let mut done = finish(&source, &creature, plan, style.checks.max_slide)?;
    done.glb.write(&spec::dir(id).join("model.glb"))?;
    spec::save(&spec::dir(id).join("meta.toml"), &done.meta, "# Measured by `cargo forge finish`; do not edit.\n")?;
    Ok(done)
}
