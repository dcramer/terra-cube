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

/// How fast the ground passes under the feet of an in-place walk, in file units a second:
/// the mean backward speed of each foot while it is low.
fn ground_speed(sk: &Skeleton, roles: &Roles, clip: &Clip) -> Option<f32> {
    let worlds: Vec<_> = clip.frames.iter().map(|p| sk.pose_world(p)).collect();
    let mut speeds = Vec::new();
    for leg in &roles.legs {
        let at: Vec<Vec3> = worlds.iter().map(|w| w[leg.foot()].w_axis.truncate()).collect();
        let (lo, hi) = at.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)));
        let mut sum = 0.0;
        let mut n = 0;
        for w in at.windows(2) {
            let back = -(w[1] - w[0]).dot(roles.forward) * clips::FPS;
            if w[0].y <= lo + 0.3 * (hi - lo) && back > 0.0 {
                sum += back;
                n += 1;
            }
        }
        if n > 0 {
            speeds.push(sum / n as f32);
        }
    }
    (!speeds.is_empty()).then(|| speeds.iter().sum::<f32>() / speeds.len() as f32)
}

pub fn finish(source: &Path, creature: &Creature, plan: &Plan) -> Result<Finished> {
    let mut glb = Glb::read(source)?;
    let skeleton = Skeleton::read(&glb)?;
    let roles = Roles::read(&skeleton, Facing::parse(&creature.facing)?);

    // Clips: Tripo's under our names, then the made ones the plan asks for.
    let theirs = clips::read(&glb, &skeleton)?;
    let mut out: Vec<Clip> = Vec::new();
    for name in &plan.clips {
        let preset = plan.presets.get(name);
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
                Clip { name: name.clone(), looping, frames, source: "tripo" }
            }
            None => {
                let walk = out.iter().find(|c| c.name == "walk");
                let stand = out.iter().find(|c| c.name == "idle" && c.source == "tripo").map(|c| &c.frames[0]);
                clips::make(name, &skeleton, &roles, walk, stand, plan.upright).with_context(|| format!("no preset and no recipe for the clip `{name}` (clips.rs `make`)"))?
            }
        };
        let mut clip = clip;
        if matches!(name.as_str(), "walk" | "run") && !roles.legs.is_empty() {
            clips::grounded(&skeleton, &roles, &mut clip.frames);
        }
        out.push(clip);
    }

    // Stand it at the origin, facing +Z, at its height.
    let (lo, hi) = bounds(&glb)?;
    let scale = creature.height / (hi.y - lo.y).max(1e-5);
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
    // A walk nobody can measure moves about a body length a cycle.
    let speed = |c: Option<&Clip>| c.map(|c| ground_speed(&skeleton, &roles, c).unwrap_or(roles.length / c.seconds()) * scale).unwrap_or(0.0);
    let name = |j: Option<&usize>| j.map(|&j| skeleton.joints[j].name.clone());
    let mut bones = std::collections::BTreeMap::new();
    for (role, joint) in [("root", Some(&skeleton.root)), ("head", roles.head.last()), ("neck", roles.head.first()), ("tail", roles.tail.first())] {
        if let Some(n) = name(joint) {
            bones.insert(role.to_string(), n);
        }
    }
    let meta = Meta {
        height: creature.height,
        length: (hi - lo).dot(roles.forward).abs() * scale,
        walk_speed: speed(walk),
        run_speed: speed(run),
        triangles: glb.triangles(),
        joints: skeleton.joints.len(),
        clips: out.iter().map(|c| (c.name.clone(), ClipMeta { seconds: c.seconds(), looping: c.looping, source: c.source.into() })).collect(),
        bones,
    };
    Ok(Finished { glb, skeleton, roles, clips: out, meta })
}

/// Runs the finish for a creature and writes `model.glb` and `meta.toml`.
pub fn run(id: &str, style: &spec::Style) -> Result<Finished> {
    let creature: Creature = spec::load(&spec::dir(id).join("creature.toml"))?;
    let plan = style.plans.get(&creature.plan).with_context(|| format!("{id}: no plan `{}` in style.toml", creature.plan))?;
    let source = spec::work(id).join("rigged.glb");
    if !source.exists() {
        bail!("{id}: {} is missing; `cargo forge build {id}` fetches it", source.display());
    }
    let mut done = finish(&source, &creature, plan)?;
    done.glb.write(&spec::dir(id).join("model.glb"))?;
    spec::save(&spec::dir(id).join("meta.toml"), &done.meta, "# Measured by `cargo forge finish`; do not edit.\n")?;
    Ok(done)
}
