//! A rig made here, from a model's shape, for bodies Tripo's rigger cannot be trusted with:
//! what flies. Its bird came back with a leg called a spine and a wing called a tail, folded
//! against the body where no bone could open it. So a flyer is modelled with its wings spread
//! flat (`[plans.flyer]`), not rigged at Tripo at all, and given its bones here: a body, a
//! fore end and an aft end along it, and two bones out along each wing. Each point of the
//! mesh is held by where it is: out along the span it is wing, toward either end of the body
//! it is that end. The bones are ours, so unlike Tripo's their names can be gone by.

use crate::glb::Glb;
use anyhow::{Context, Result, bail};
use glam::{Mat4, Vec3};
use serde_json::json;

/// The bones, in the order the skin lists them. A wing's second bone is its outer half.
pub const BONES: [&str; 7] = ["body", "fore", "aft", "wing_l1", "wing_l2", "wing_r1", "wing_r2"];

fn smooth(lo: f32, hi: f32, v: f32) -> f32 {
    let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Whether a model was rigged here: it has our bones.
pub fn made(glb: &Glb) -> bool {
    glb.array("nodes").iter().any(|n| n["name"] == "wing_l1")
}

/// Gives an unrigged, spread-winged model its skeleton and skin.
pub fn rig(glb: &mut Glb) -> Result<()> {
    if !glb.array("skins").is_empty() {
        bail!("the model already has a skin: a flyer is rigged here, not at Tripo");
    }
    // The node that carries the mesh, and where that puts it in the file's space.
    let nodes = glb.array("nodes").to_vec();
    let holder = nodes.iter().position(|n| n.get("mesh").is_some()).context("no node carries a mesh")?;
    let local = |n: &serde_json::Value| -> Mat4 {
        if let Some(m) = n.get("matrix").and_then(|m| m.as_array()) {
            return Mat4::from_cols_slice(&m.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect::<Vec<_>>());
        }
        let f = |key: &str, default: &[f32]| -> Vec<f32> { n.get(key).and_then(|a| a.as_array()).map_or(default.to_vec(), |a| a.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect()) };
        Mat4::from_scale_rotation_translation(Vec3::from_slice(&f("scale", &[1.0; 3])), glam::Quat::from_slice(&f("rotation", &[0.0, 0.0, 0.0, 1.0])).normalize(), Vec3::from_slice(&f("translation", &[0.0; 3])))
    };
    let mut placed = local(&nodes[holder]);
    let mut at = holder;
    while let Some(parent) = nodes.iter().position(|n| n["children"].as_array().is_some_and(|c| c.iter().any(|c| c.as_u64() == Some(at as u64)))) {
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
    let all = || points.iter().flatten().copied();
    let (lo, hi) = all().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
    let (middle, size) = ((lo + hi) / 2.0, hi - lo);
    // The wings lie along whichever way it is wider; the body along the other.
    let (side, along) = if size.x >= size.z { (Vec3::X, Vec3::Z) } else { (Vec3::Z, Vec3::X) };
    let (span, long) = (size.dot(side), size.dot(along));
    // The body's own middle, by its thickest part: the points near the centre line.
    let core: Vec<Vec3> = all().filter(|p| (*p - middle).dot(side).abs() < span * 0.06).collect();
    let centre = if core.is_empty() { middle } else { core.iter().copied().sum::<Vec3>() / core.len() as f32 };
    // Where the body ends and the wing begins, each side; and where the wing's outer half does.
    let (root, elbow) = (span * 0.09, span * 0.27);
    let places = [
        centre,
        centre + along * long * 0.3,
        centre - along * long * 0.3,
        centre + side * root,
        centre + side * elbow,
        centre - side * root,
        centre - side * elbow,
    ];
    let parents = [None, Some(0), Some(0), Some(0), Some(3), Some(0), Some(5)];

    // Each point's four bones and how much each holds it.
    for (p, pts) in primitives.iter().enumerate() {
        let (mut joints, mut weights) = (Vec::with_capacity(pts_len(&points[p]) * 4), Vec::with_capacity(pts_len(&points[p]) * 4));
        let _ = pts;
        for at in &points[p] {
            let d = *at - centre;
            let (out, fore) = (d.dot(side), d.dot(along));
            let outer = smooth(elbow * 0.8, elbow * 1.3, out.abs());
            let inner = (smooth(root * 0.6, root * 1.7, out.abs()) - outer).max(0.0);
            let body = (1.0 - outer - inner).max(0.0);
            // Of what the body holds, its ends take the part toward them.
            let end = smooth(long * 0.12, long * 0.42, fore.abs()) * 0.85 * body;
            let (wing1, wing2) = if out >= 0.0 { (3u8, 4u8) } else { (5, 6) };
            let which = if fore >= 0.0 { 1u8 } else { 2 };
            joints.extend([0u8, which, wing1, wing2]);
            let w = [body - end, end, inner, outer];
            let sum: f32 = w.iter().sum::<f32>().max(1e-6);
            weights.extend(w.map(|v| v / sum));
        }
        let count = points[p].len();
        let j = glb.add_raw(&joints, 5121, "VEC4", count);
        let w = glb.add_floats(&weights, "VEC4", false);
        glb.json["meshes"][mesh]["primitives"][p]["attributes"]["JOINTS_0"] = json!(j);
        glb.json["meshes"][mesh]["primitives"][p]["attributes"]["WEIGHTS_0"] = json!(w);
    }

    // The bones as nodes, in the file's space under the scene; the mesh is drawn by them.
    let first = glb.array("nodes").len();
    let list = glb.json["nodes"].as_array_mut().context("no nodes")?;
    for (i, name) in BONES.iter().enumerate() {
        let from = parents[i].map_or(Vec3::ZERO, |p: usize| places[p]);
        let children: Vec<usize> = (0..BONES.len()).filter(|c| parents[*c] == Some(i)).map(|c| first + c).collect();
        let mut node = json!({ "name": name, "translation": (places[i] - from).to_array() });
        if !children.is_empty() {
            node["children"] = json!(children);
        }
        list.push(node);
    }
    list[holder]["skin"] = json!(0);
    let scene = glb.json["scene"].as_u64().unwrap_or(0) as usize;
    glb.json["scenes"][scene]["nodes"].as_array_mut().context("no scene")?.push(json!(first));
    // A skinned mesh is put where its bones put it and nowhere else, so where its own node
    // had put it goes into each bone's binding.
    let binds: Vec<f32> = places.iter().flat_map(|at| (Mat4::from_translation(-*at) * placed).to_cols_array()).collect();
    let inverse = glb.add_floats(&binds, "MAT4", false);
    let skins = json!([{ "joints": (first..first + BONES.len()).collect::<Vec<_>>(), "inverseBindMatrices": inverse, "skeleton": first }]);
    glb.json["skins"] = skins;
    Ok(())
}

fn pts_len(p: &[Vec3]) -> usize {
    p.len()
}
