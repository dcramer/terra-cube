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
    // (A leg's end is where it meets the ground: the point of the floor under its last
    // joint as it stands, carried by that joint. `sole` is that point in the joint's frame.)
    let sole = |j: usize| stood[j].inverse().transform_point3(at(j).with_y(origin.y));
    let mut legs: Vec<(usize, [usize; 3], [Vec3; 3])> = roles.legs.iter().enumerate().map(|(i, l)| {
        let (hip, knee) = clips::folds_at(sk, l);
        (i, [hip, knee, l.foot()], [into(at(hip)), into(at(knee)), into(at(l.foot()).with_y(origin.y))])
    }).collect();
    legs.sort_by(|a, b| b.2[0].z.total_cmp(&a.2[0].z));
    legs[..2].sort_by(|a, b| a.2[0].x.total_cmp(&b.2[0].x));
    legs[2..].sort_by(|a, b| a.2[0].x.total_cmp(&b.2[0].x));
    let low: [Vec3; 4] = std::array::from_fn(|i| legs[i].2[0]);
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
            let foot = walker.feet()[n] - Vec3::Z * gone + Vec3::new(0.0, 0.0, mid);
            clips::reach_point(sk, roles, &mut pose, &roles.legs[*i], (*hip, *knee), (*end, sole(*end)), origin + out_of(foot));
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

/// What a lying pose must be (`assets/creatures/poses.toml`, written from photographs).
#[derive(serde::Deserialize)]
struct Lying {
    belly: [f32; 2],
    elbow: f32,
    hock: f32,
    forearm: f32,
    lower_hind: f32,
    back: f32,
    tail: f32,
}

fn lying() -> Lying {
    #[derive(serde::Deserialize)]
    struct File {
        lie: Lying,
    }
    toml::from_str::<File>(include_str!("../../../assets/creatures/poses.toml")).expect("assets/creatures/poses.toml").lie
}

/// A leg as lying needs it: where it folds, the joint under its knee (the hock, or the
/// leg's last joint where the rig ends there), its last joint.
struct Folded {
    n: usize,
    hip: usize,
    knee: usize,
    heel: usize,
    end: usize,
}

fn folded(sk: &Skeleton, roles: &Roles) -> Vec<Folded> {
    roles.legs.iter().enumerate().map(|(n, l)| {
        let (hip, knee) = clips::folds_at(sk, l);
        let heel = l.chain.iter().position(|&j| j == knee).map_or(l.foot(), |i| l.chain[(i + 1).min(l.chain.len() - 1)]);
        Folded { n, hip, knee, heel, end: l.foot() }
    }).collect()
}

/// Every joint that is a leg's or the tail's: what is left is the trunk, and its belly.
fn limbs(roles: &Roles) -> Vec<usize> {
    roles.legs.iter().flat_map(|l| l.chain.iter().copied()).chain(roles.tail.iter().copied()).collect()
}

/// A lying pose held against what lying must be: each measure in a few words, and whether
/// all of them hold. Lengths are parts of the leg (the fore hip's height as it stands).
pub fn measured(glb: &crate::glb::Glb, sk: &Skeleton, roles: &Roles, pose: &Pose) -> Option<(String, bool)> {
    if roles.legs.len() != 4 || roles.legs.iter().any(|l| l.chain.len() < 3) {
        return None;
    }
    let (must, legs, world) = (lying(), folded(sk, roles), sk.pose_world(pose));
    let floor = roles.legs.iter().map(|l| sk.position(l.foot()).y).fold(f32::MAX, f32::min);
    let at = |j: usize| world[j].w_axis.truncate();
    let leg = legs.iter().filter(|l| roles.legs[l.n].front).map(|l| sk.position(l.hip).y - floor).fold(0.0f32, f32::max).max(1e-4);
    let slope = |from: Vec3, to: Vec3| ((to.y - from.y).abs() / (to - from).with_y(0.0).length().max(1e-4)).atan().to_degrees();
    let sole = |l: &Folded, carried_by: usize| world[carried_by].transform_point3(sk.pose_world(&sk.rest())[carried_by].inverse().transform_point3(sk.position(l.end).with_y(floor)));
    let worst = |front: bool, f: &dyn Fn(&Folded) -> f32| legs.iter().filter(|l| roles.legs[l.n].front == front).map(f).fold(f32::MIN, f32::max);
    // (The chest and the loins apart: by the lowest of the whole trunk a wolf's chest stood a
    // hand over the ground with its loins on it, and the measure passed.)
    let mid = legs.iter().map(|l| at(l.hip).dot(roles.forward)).sum::<f32>() / 4.0;
    let under = |fore: bool| crate::check::lowest(glb, sk, pose, &limbs(roles), Some((roles.forward, mid, fore))).map(|y| (y - floor) / leg);
    let (chest, loins) = (under(true)?, under(false)?);
    let belly = if chest.abs() > loins.abs() { chest } else { loins };
    let elbow = worst(true, &|l| (at(l.knee).y - floor) / leg);
    let hock = worst(false, &|l| (at(l.heel).y - floor) / leg);
    let forearm = worst(true, &|l| slope(at(l.knee), sole(l, l.end)));
    let lower = worst(false, &|l| slope(at(l.heel), sole(l, l.heel)));
    let paws = legs.iter().all(|l| { let (from, to) = if roles.legs[l.n].front { (at(l.knee), sole(l, l.end)) } else { (at(l.heel), sole(l, l.heel)) }; (to - from).dot(roles.forward) > 0.0 });
    let knees = legs.iter().filter(|l| !roles.legs[l.n].front).all(|l| (at(l.knee) - at(l.hip)).dot(roles.forward) > 0.0);
    let back = roles.spine.first().zip(roles.spine.last()).map_or(0.0, |(a, b)| slope(at(*a), at(*b)));
    let others: Vec<usize> = (0..sk.joints.len()).filter(|j| !roles.tail.contains(j)).collect();
    let tail = if roles.tail.is_empty() { 0.0 } else { crate::check::lowest(glb, sk, pose, &others, None).map_or(0.0, |y| (y - floor) / leg) };
    let good = belly >= must.belly[0] && belly <= must.belly[1] && elbow <= must.elbow && hock <= must.hock && forearm <= must.forearm && lower <= must.lower_hind && paws && knees && back <= must.back && tail >= must.tail;
    Some((format!("chest {:+.0}% and loins {:+.0}% of a leg over the ground · elbows {:.0}% up · hocks {:.0}% up · forearms {:.0}° from flat · lower hind legs {:.0}° · paws ahead {} · knees ahead of hips {} · back {:.0}° · tail {:+.0}%", chest * 100.0, loins * 100.0, elbow * 100.0, hock * 100.0, forearm, lower, if paws { "yes" } else { "NO" }, if knees { "yes" } else { "NO" }, back, tail * 100.0), good))
    .map(|(text, good): (String, bool)| (text.replacen("{chest}", "", 1), good))
}

/// Lying down as a dog or a wolf lies: the body let down onto its elbows, each forearm flat
/// on the ground and pointing ahead; behind, the hock on the ground and the whole lower leg
/// from hock to paw flat and pointing ahead beside the belly, the knee folded up by the
/// flank. (Three tries before this stood it on its paws in a squat, folded its legs through
/// itself, or stretched them out. The developer, 2026-11-12: "the whole lower back leg lays
/// flat on the ground and the paw comes forward ... you can look at a picture".)
///
/// A fore leg is two parts here, shoulder to elbow and elbow to paw; a hind leg three, with
/// the joint below the knee (the hock, or the leg's last joint where the rig ends there)
/// put on the ground and what it carries turned to lie flat. Nothing where the creature has
/// not four legs that fold.
pub fn lie(glb: &crate::glb::Glb, sk: &Skeleton, roles: &Roles, stand: &Pose) -> Option<Vec<Pose>> {
    if roles.legs.len() != 4 || roles.legs.iter().any(|l| l.chain.len() < 3) {
        return None;
    }
    let stood = sk.pose_world(stand);
    let at = |j: usize| stood[j].w_axis.truncate();
    let (ahead, up) = (roles.forward, Vec3::Y);
    let floor = roles.legs.iter().map(|l| at(l.foot()).y).fold(f32::MAX, f32::min);
    type Leg = Folded;
    let legs = folded(sk, roles);
    let ground = |j: usize| at(j).with_y(floor);
    let fore: Vec<&Leg> = legs.iter().filter(|l| roles.legs[l.n].front).collect();
    let hind: Vec<&Leg> = legs.iter().filter(|l| !roles.legs[l.n].front).collect();
    if fore.len() != 2 || hind.len() != 2 {
        return None;
    }
    let mean = |of: &[&Leg], f: &dyn Fn(&Leg) -> f32| of.iter().map(|l| f(l)).sum::<f32>() / of.len() as f32;
    let fore_high = mean(&fore, &|l| at(l.hip).y - floor);
    let hind_high = mean(&hind, &|l| at(l.hip).y - floor);
    let thick = 0.07 * fore_high;
    // The body comes down until its belly is on the ground: by the skin of its trunk, not
    // its legs' or its tail's (`poses.toml`, learned).
    // (At both ends: its chest by as far as its chest stands over the ground and its loins
    // by as far as they do, the body tipping between the two.)
    let along = |j: usize| at(j).dot(ahead);
    let (fore_at, hind_at) = (mean(&fore, &|l| along(l.hip)), mean(&hind, &|l| along(l.hip)));
    let mid = (fore_at + hind_at) / 2.0;
    let under = |front: bool| crate::check::lowest(glb, sk, stand, &limbs(roles), Some((ahead, mid, front))).map(|y| y - floor);
    let (chest, loins) = (under(true)?.min(fore_high - thick).max(0.0), under(false)?.min(hind_high - thick).max(0.0));
    let tip = ((chest - loins) / (fore_at - hind_at).abs().max(1e-4) * 0.5).clamp(-0.2, 0.2).atan();
    let lowest = (chest + loins) / 2.0;
    let lain = |down: f32| {
    let mut pose = stand.clone();
    // (Tipped about its own middle, nose down where its chest has further to come, and let down.)
    let (root, middle) = (at(sk.root), (fore.iter().chain(&hind).map(|l| at(l.hip)).sum::<Vec3>()) / 4.0);
    let tipped = Quat::from_axis_angle(ahead.cross(up).normalize(), -tip);
    clips::turn_now(sk, &mut pose, sk.root, tipped);
    pose.root_t += sk.base.inverse().transform_vector3(middle + tipped * (root - middle) - root - up * down);
    // Its tail lies along the ground behind it, not through it: raised from its root, a
    // little at a time, until none of its skin is under the floor.
    if let (Some(&first), Some(&last)) = (roles.tail.first(), roles.tail.last()) {
        let others: Vec<usize> = (0..sk.joints.len()).filter(|j| !roles.tail.contains(j)).collect();
        for _ in 0..10 {
            if crate::check::lowest(glb, sk, &pose, &others, None).is_none_or(|y| y >= floor) {
                break;
            }
            let now = sk.pose_world(&pose);
            let along = now[last].w_axis.truncate() - now[first].w_axis.truncate();
            let Some(axis) = along.cross(up).try_normalize() else { break };
            clips::turn_now(sk, &mut pose, first, Quat::from_axis_angle(axis, 0.1));
        }
    }
    for l in &legs {
        let leg = &roles.legs[l.n];
        let out = roles.side * if leg.left { 1.0 } else { -1.0 };
        let now = sk.pose_world(&pose);
        let hip = now[l.hip].w_axis.truncate();
        if leg.front {
            // The elbow on the ground behind the shoulder, the forearm flat ahead of it:
            // the paw is put as far ahead of the elbow as the forearm is long.
            let sole = stood[l.end].inverse().transform_point3(ground(l.end));
            let (a, b) = (at(l.hip).distance(at(l.knee)), at(l.knee).distance(ground(l.end)));
            let high = (hip.y - floor - thick).clamp(0.0, a * 0.98);
            let back = (a * a - high * high).max(0.0).sqrt();
            let paw = hip.with_y(floor) + ahead * (b - back) + out * thick;
            clips::reach_point(sk, roles, &mut pose, leg, (l.hip, l.knee), (l.end, sole), paw);
        } else {
            // Folded in a Z: the hock on the ground a little behind the hip and wide of the
            // belly, so the thigh goes ahead to the knee and the shank back to the hock; and
            // the lower leg flat ahead of the hock, the paw as far ahead as it is long.
            let sole = stood[l.heel].inverse().transform_point3(ground(l.end));
            let lower = at(l.heel).distance(ground(l.end));
            let reach = at(l.hip).distance(at(l.knee)) + at(l.knee).distance(at(l.heel));
            let hock = hip.with_y(floor + thick * 0.4) - ahead * 0.12 * reach + out * 2.0 * thick;
            let paw = hock.with_y(floor) + ahead * lower;
            clips::reach_by(sk, roles, &mut pose, leg, (l.hip, l.knee), l.heel, hock);
            let now = sk.pose_world(&pose);
            let (heel, toe) = (now[l.heel].w_axis.truncate(), now[l.heel].transform_point3(sole));
            if let (Some(from), Some(to)) = ((toe - heel).try_normalize(), (paw - heel).try_normalize()) {
                clips::turn_now(sk, &mut pose, l.heel, Quat::from_rotation_arc(from, to));
            }
        }
    }
    pose
    };
    let down = lowest;
    let pose = lain(down);
    // (And not at all where a leg is left under the ground, or it has hardly come down:
    // on such a body this is not lying, and it keeps what it had.)
    let world = sk.pose_world(&pose);
    let buried = legs.iter().any(|l| [l.knee, l.heel, l.end].iter().any(|&j| world[j].w_axis.y < floor - 2.0 * thick));
    if pose.rotations.iter().any(|r| !r.is_finite()) || buried || down < 0.25 * fore_high {
        return None;
    }
    // Five seconds of it, breathing: the body rising and sinking a very little.
    let n = (FPS * 5.0) as usize;
    Some((0..=n).map(|i| {
        let mut breath = pose.clone();
        breath.root_t += sk.base.inverse().transform_vector3(up * 0.006 * fore_high * (0.5 - 0.5 * (i as f32 / n as f32 * std::f32::consts::TAU).cos()));
        breath
    }).collect())
}
