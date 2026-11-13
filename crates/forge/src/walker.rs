//! A four-legged creature's gaits, made by walking it: the body moved by its feet
//! (`cube_anim`), fitted to this model's own legs, walked at the speed the gait is for with no
//! window, and one stride of it written down as a clip like any other. So what moves the
//! boxes of `make limbs` is what every animal's walk, run and sprint are, and the checks, the
//! sheet and the game need know nothing new.
//!
//! The routine, end to end: `cargo forge finish ID` fits and bakes, `cargo forge check ID`
//! judges the result by number (feet on the floor, no sliding, skin not torn, the loop
//! closed), `cargo forge sheet ID` is the one picture to look at.

use crate::clips::{self, FPS};
use crate::rig::{Pose, Roles, Skeleton};
use cube_anim::Walker;
use glam::{Mat3, Quat, Vec3};

/// One stride at `speed` (the skeleton's own units a second), from where one fore foot leaves
/// the ground to where it next does, starting each moment from how the creature stands.
/// `metres` is how many metres one of the skeleton's units is: the walker works in metres
/// (how long a stride is and when a trot becomes a gallop go by real size), so the creature
/// is walked at its true size and the poses brought back. Nothing where it has not four legs
/// that fold.
pub fn stride(sk: &Skeleton, roles: &Roles, stand: &Pose, speed: f32, metres: f32) -> Option<Vec<Pose>> {
    let speed = speed * metres;
    if roles.legs.len() != 4 || roles.legs.iter().any(|l| l.chain.len() < 3) || speed <= 0.0 {
        return None;
    }
    // The walker's frame: x its right, y up, z ahead, from the point on the floor under the
    // middle of its feet.
    let stood = sk.pose_world(stand);
    let at = |j: usize| stood[j].w_axis.truncate();
    let (ahead, right) = (roles.forward, Vec3::Y.cross(roles.forward).normalize());
    let feet: Vec<Vec3> = roles.legs.iter().map(|l| at(l.foot())).collect();
    let origin = (feet.iter().sum::<Vec3>() / 4.0).with_y(feet.iter().map(|f| f.y).fold(f32::MAX, f32::min));
    let into = |p: Vec3| Vec3::new((p - origin).dot(right), p.y - origin.y, (p - origin).dot(ahead)) * metres;
    let out_of = |v: Vec3| (right * v.x + Vec3::Y * v.y + ahead * v.z) / metres;
    // Its legs, in the walker's order: fore before hind, of each pair the one on -x first.
    let mut legs: Vec<(usize, [usize; 3], [Vec3; 3])> = roles.legs.iter().enumerate().map(|(i, l)| {
        let (hip, knee) = clips::folds_at(sk, l);
        (i, [hip, knee, l.foot()], [into(at(hip)), into(at(knee)), into(at(l.foot()))])
    }).collect();
    legs.sort_by(|a, b| b.2[0].z.total_cmp(&a.2[0].z));
    legs[..2].sort_by(|a, b| a.2[0].x.total_cmp(&b.2[0].x));
    legs[2..].sort_by(|a, b| a.2[0].x.total_cmp(&b.2[0].x));
    let ankle: [f32; 4] = std::array::from_fn(|i| legs[i].2[2].y.max(0.0));
    let low: [Vec3; 4] = std::array::from_fn(|i| legs[i].2[0] - Vec3::Y * ankle[i]);
    let bones: [(f32, f32); 4] = std::array::from_fn(|i| (legs[i].2[0].distance(legs[i].2[1]), legs[i].2[1].distance(legs[i].2[2])));
    let (tall, mid) = (low.iter().map(|h| h.y).sum::<f32>() / 4.0, (low[0].z + low[2].z) / 2.0);
    let mut walker = Walker::fitted(low, bones, std::array::from_fn(|i| legs[i].2[1]), Vec3::ZERO, 0.0);
    // How it stands at each moment: the trunk where the walker's is, each leg bent to its foot.
    let basis = Quat::from_mat3(&Mat3::from_cols(right, Vec3::Y, ahead));
    let (root, middle) = (at(sk.root), origin + out_of(Vec3::new(0.0, tall, mid)));
    let pose_of = |walker: &Walker, gone: f32| {
        let mut pose = stand.clone();
        let (trunk, turn) = walker.trunk();
        let turn = (basis * turn * basis.inverse()).normalize();
        clips::turn_now(sk, &mut pose, sk.root, turn);
        // (Turned about its own joint, the body's middle has moved: it is put where the trunk is.)
        let moved = root + turn * (middle - root);
        let want = middle + out_of(trunk - Vec3::new(0.0, tall, gone));
        pose.root_t += sk.base.inverse().transform_vector3(want - moved);
        for (n, (i, [hip, knee, end], _)) in legs.iter().enumerate() {
            let foot = walker.feet()[n] - Vec3::Z * gone + Vec3::new(0.0, ankle[n], mid);
            clips::reach_by(sk, roles, &mut pose, &roles.legs[*i], (*hip, *knee), *end, origin + out_of(foot));
        }
        pose
    };
    // Walked until it is in its stride, then one stride of it kept.
    let (dt, every) = (1.0 / (FPS * 4.0), 4);
    let (mut gone, mut frames, mut began) = (0.0f32, Vec::new(), None);
    for tick in 0..(12.0 / dt) as usize {
        gone += speed * dt;
        let before = walker.steps[0];
        walker.step(dt, Vec3::Z * gone, 0.0, 0.0, |_, _| 0.0);
        let stepped = walker.steps[0] != before;
        match began {
            None if stepped && tick as f32 * dt > 3.0 => began = Some(tick),
            Some(from) if stepped => {
                let _ = from;
                break;
            }
            _ => {}
        }
        if let Some(from) = began && (tick - from) % every == 0 {
            frames.push(pose_of(&walker, gone));
        }
    }
    if frames.len() < 4 || frames.iter().any(|p| p.rotations.iter().any(|r| !r.is_finite())) {
        return None;
    }
    frames.push(frames[0].clone());
    clips::close_loop(&mut frames);
    Some(frames)
}
