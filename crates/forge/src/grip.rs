//! How each hand of a person's model holds a handle, read off its skeleton as it was made
//! and written to `meta.toml` for the game (`cube_game::held`): where in the palm the handle
//! lies and how it is turned, in the hand bone's own frame, and each finger bone's turn when
//! the hand is closed round it. (Until 2026-11-12 a thing was pinned to the hand's bone and
//! the hand left open: "the handle isn't in any kind of hand grip form".)

use crate::rig::Skeleton;
use crate::spec::Grip;
use glam::{Mat3, Quat, Vec3};
use std::collections::BTreeMap;

/// The skeleton as it rests, for the game's two-legged walker: every bone's own turn, the
/// fingers a little curled as a hand at ease has them.
pub fn rests(sk: &Skeleton) -> crate::spec::Stride {
    let eased = bent(sk, 1.0, [0.4, 0.5, 0.35], [0.1, 0.2, 0.2]);
    let curled: BTreeMap<&str, [f32; 4]> = eased.values().flat_map(|g| g.bones.iter().map(String::as_str).zip(g.turns.iter().copied())).collect();
    crate::spec::Stride { bones: sk.joints.iter().map(|j| j.name.clone()).collect(), rests: sk.joints.iter().map(|j| curled.get(j.name.as_str()).copied().unwrap_or(j.rest_r.to_array())).collect(), seat: sk.joints[sk.root].rest_t.to_array() }
}

/// `unit`: metres to a unit of the file.
pub fn grips(sk: &Skeleton, unit: f32) -> BTreeMap<String, Grip> {
    bent(sk, unit, [1.15, 1.45, 0.8], [0.35, 0.55, 0.5])
}

/// Each hand with its fingers bent so much at each joint, and its thumb.
fn bent(sk: &Skeleton, unit: f32, fingers: [f32; 3], thumb_bends: [f32; 3]) -> BTreeMap<String, Grip> {
    let bone = |n: &str| sk.joints.iter().position(|j| j.name.rsplit(':').next() == Some(n));
    let at = |j: usize| sk.joints[j].world.w_axis.truncate();
    let turned = |j: usize| sk.joints[j].world.to_scale_rotation_translation().1;
    let mut out = BTreeMap::new();
    for (side, mirror) in [("Left", 1.0f32), ("Right", -1.0)] {
        let name = |part: &str| format!("{side}Hand{part}");
        let (Some(hand), Some(index), Some(middle), Some(pinky)) = (bone(&name("")), bone(&name("Index1")), bone(&name("Middle1")), bone(&name("Pinky1"))) else { continue };
        // The hand's own directions: along its fingers, across its knuckles toward the thumb,
        // and out of its palm (which side that is follows from which hand it is).
        let along = (at(middle) - at(hand)).normalize_or(Vec3::X);
        let across = at(index) - at(pinky);
        let across = (across - along * across.dot(along)).normalize_or(Vec3::Z);
        let palm = along.cross(across) * mirror;
        // The handle lies across the palm under the knuckles, out past the thumb; what it
        // carries faces the way the knuckles do.
        let knuckles = (at(index) + at(middle) + at(pinky)) / 3.0;
        let middle_of = knuckles * 0.72 + at(hand) * 0.28 + palm * 0.024 / unit;
        let frame = Quat::from_mat3(&Mat3::from_cols(along, across, along.cross(across)));
        let (mut bones, mut turns) = (Vec::new(), Vec::new());
        // Closed: each finger's three joints bent toward the palm, the thumb brought over.
        let mut close = |part: &str, axis: Vec3, bends: [f32; 3]| {
            for (n, bend) in bends.iter().enumerate() {
                let Some(j) = bone(&name(&format!("{part}{}", n + 1))) else { continue };
                let local = turned(j).inverse() * Quat::from_axis_angle(axis, *bend) * turned(j);
                bones.push(sk.joints[j].name.clone());
                turns.push((sk.joints[j].rest_r * local).normalize().to_array());
            }
        };
        for finger in ["Index", "Middle", "Ring", "Pinky"] {
            close(finger, along.cross(palm).normalize_or(Vec3::Z), fingers);
        }
        if let (Some(a), Some(b)) = (bone(&name("Thumb1")), bone(&name("Thumb2"))) {
            let thumb = (at(b) - at(a)).normalize_or(across);
            close("Thumb", thumb.cross(palm - across * 0.6).normalize_or(along), thumb_bends);
        }
        out.insert(side.to_owned(), Grip { at: sk.joints[hand].world.inverse().transform_point3(middle_of).to_array(), turn: (turned(hand).inverse() * frame).normalize().to_array(), bones, turns });
    }
    out
}
