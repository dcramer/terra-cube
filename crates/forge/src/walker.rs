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
    ground: f32,
    inside: f32,
    touch: f32,
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

/// A model's shape, for fitting a pose to the ground: its skin, how thick it is round each
/// joint (a leg is not a line: laid down, its elbow lies its own radius over the floor), and
/// how thick its trunk is round the line from its hips to its shoulders.
struct Shape {
    body: crate::check::Skinned,
    radius: Vec<f32>,
    trunk: f32,
    floor: f32,
    /// The fore hips' height as it stands: what lengths are told in parts of.
    leg: f32,
    legs: Vec<Folded>,
}

fn to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let along = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
    p.distance(a + (b - a) * along)
}

impl Shape {
    fn of(glb: &crate::glb::Glb, sk: &Skeleton, roles: &Roles) -> Option<Self> {
        if roles.legs.len() != 4 || roles.legs.iter().any(|l| l.chain.len() < 3) {
            return None;
        }
        let body = crate::check::Skinned::of(glb, sk)?;
        let rest = body.at(sk, &sk.rest());
        let legs = folded(sk, roles);
        let floor = roles.legs.iter().map(|l| sk.position(l.foot()).y).fold(f32::MAX, f32::min);
        let leg = legs.iter().filter(|l| roles.legs[l.n].front).map(|l| sk.position(l.hip).y - floor).fold(0.0f32, f32::max).max(1e-4);
        // Round each joint: how far the skin that follows it stands off its bone, at the
        // ninth corner of ten (at the seventh a forearm laid by it was still 3% in the floor).
        let part = |mut far: Vec<f32>, of: f32| {
            far.sort_by(f32::total_cmp);
            far.get((far.len() as f32 * of) as usize).copied()
        };
        let radius = (0..sk.joints.len()).map(|j| {
            let to = sk.joints[j].children.first().map_or(sk.position(j), |c| sk.position(*c));
            part((0..rest.len()).filter(|i| body.most[*i] == j).map(|i| to_segment(rest[i], sk.position(j), to)).collect(), 0.9).unwrap_or(0.05 * leg)
        }).collect();
        let (fore, hind) = Self::ends(&legs, roles, &|j| sk.position(j));
        let skip = limbs(roles);
        let trunk = part((0..rest.len()).filter(|i| !skip.contains(&body.most[*i])).map(|i| to_segment(rest[i], hind, fore)).collect(), 0.5).unwrap_or(0.2 * leg);
        Some(Self { body, radius, trunk, floor, leg, legs })
    }

    /// How thick a leg is at its thickest from a joint of it to its end: how high that part
    /// of it lies when it is laid on the ground.
    fn thick(&self, roles: &Roles, l: &Folded, from: usize) -> f32 {
        let chain = &roles.legs[l.n].chain;
        let start = chain.iter().position(|&j| j == from).unwrap_or(0);
        chain[start..].iter().map(|&j| self.radius[j]).fold(0.0, f32::max)
    }

    /// The middle of the fore hips and of the hind.
    fn ends(legs: &[Folded], roles: &Roles, at: &dyn Fn(usize) -> Vec3) -> (Vec3, Vec3) {
        let mean = |front: bool| legs.iter().filter(|l| roles.legs[l.n].front == front).map(|l| at(l.hip)).sum::<Vec3>() / 2.0;
        (mean(true), mean(false))
    }
}

/// A lying pose in numbers, each as `poses.toml` tells it (lengths in parts of a leg).
struct Measures {
    chest: f32,
    loins: f32,
    /// How far each elbow and hock is over the ground, less its own thickness.
    elbow: f32,
    hock: f32,
    forearm: f32,
    lower: f32,
    paws: bool,
    knees: bool,
    back: f32,
    tail: f32,
    /// The lowest of all its skin, and of all but its tail's.
    ground: f32,
    body: f32,
    /// How far a leg's joint is inside its trunk.
    inside: f32,
}

impl Measures {
    fn of(shape: &Shape, sk: &Skeleton, roles: &Roles, pose: &Pose) -> Self {
        let (world, skin) = (sk.pose_world(pose), shape.body.at(sk, pose));
        let (floor, leg, legs) = (shape.floor, shape.leg, &shape.legs);
        let at = |j: usize| world[j].w_axis.truncate();
        let skip = limbs(roles);
        let mid = legs.iter().map(|l| at(l.hip).dot(roles.forward)).sum::<f32>() / 4.0;
        let low = |keep: &dyn Fn(usize) -> bool| (0..skin.len()).filter(|i| keep(*i)).map(|i| skin[i].y).fold(f32::MAX, f32::min);
        let over = |y: f32| if y == f32::MAX { 0.0 } else { (y - floor) / leg };
        let trunk = |fore: bool| over(low(&|i| !skip.contains(&shape.body.most[i]) && (skin[i].dot(roles.forward) > mid) == fore));
        let slope = |from: Vec3, to: Vec3| ((to.y - from.y).abs() / (to - from).with_y(0.0).length().max(1e-4)).atan().to_degrees();
        let rest = sk.pose_world(&sk.rest());
        let sole = |l: &Folded, by: usize| world[by].transform_point3(rest[by].inverse().transform_point3(sk.position(l.end).with_y(floor)));
        let worst = |front: bool, f: &dyn Fn(&Folded) -> f32| legs.iter().filter(|l| roles.legs[l.n].front == front).map(f).fold(f32::MIN, f32::max);
        let (fore, hind) = Shape::ends(legs, roles, &at);
        Self {
            chest: trunk(true),
            loins: trunk(false),
            elbow: worst(true, &|l| (at(l.knee).y - floor - shape.thick(roles, l, l.knee)) / leg),
            hock: worst(false, &|l| (at(l.heel).y - floor - shape.thick(roles, l, l.heel)) / leg),
            forearm: worst(true, &|l| slope(at(l.knee), at(l.end))),
            lower: worst(false, &|l| if l.end != l.heel { slope(at(l.heel), at(l.end)) } else { slope(at(l.heel), sole(l, l.heel)) }),
            paws: legs.iter().all(|l| {
                let (from, to) = if roles.legs[l.n].front { (at(l.knee), at(l.end)) } else if l.end != l.heel { (at(l.heel), at(l.end)) } else { (at(l.heel), sole(l, l.heel)) };
                (to - from).dot(roles.forward) > 0.0
            }),
            knees: legs.iter().filter(|l| !roles.legs[l.n].front).all(|l| (at(l.knee) - at(l.hip)).dot(roles.forward) > 0.0),
            back: slope(hind, fore),
            tail: if roles.tail.is_empty() { 0.0 } else { over(low(&|i| roles.tail.contains(&shape.body.most[i]))) },
            ground: over(low(&|_| true)),
            body: over(low(&|i| !roles.tail.contains(&shape.body.most[i]))),
            inside: legs.iter().flat_map(|l| [l.knee, l.heel, l.end]).map(|j| (0.8 * shape.trunk - to_segment(at(j), hind, fore)).max(0.0) / leg).fold(0.0, f32::max),
        }
    }

    fn good(&self, must: &Lying) -> bool {
        self.ground >= must.ground && self.body <= must.touch && self.chest.min(self.loins) >= must.belly[0] && self.chest.max(self.loins) <= must.belly[1] && self.elbow <= must.elbow && self.hock <= must.hock && self.forearm <= must.forearm && self.lower <= must.lower_hind && self.paws && self.knees && self.back <= must.back && self.tail >= must.tail && self.inside <= must.inside
    }

    /// How far from lying it is, as one number: nothing where every measure is met in
    /// full, more the further any is from it. What the search makes small. (The tail is
    /// laid down afterwards and is not counted here.)
    fn short(&self) -> f32 {
        let over = |v: f32| v.max(0.0);
        20.0 * over(-self.body) + 3.0 * over(self.body) + self.chest.abs() + self.loins.abs() + self.elbow.abs() + self.hock.abs() + self.forearm / 90.0 + self.lower / 90.0 + self.back / 90.0 + 4.0 * self.inside + if self.paws { 0.0 } else { 3.0 } + if self.knees { 0.0 } else { 3.0 }
    }

    fn said(&self) -> String {
        let yes = |b: bool| if b { "yes" } else { "NO" };
        format!("chest {:+.0}% and loins {:+.0}% of a leg over the ground · elbows {:+.0}% and hocks {:+.0}% over their own thickness · forearms {:.0}° from flat · lower hind legs {:.0}° · paws ahead {} · knees ahead of hips {} · back {:.0}° · tail {:+.0}% · lowest skin {:+.0}% · legs in the body {:.0}%", self.chest * 100.0, self.loins * 100.0, self.elbow * 100.0, self.hock * 100.0, self.forearm, self.lower, yes(self.paws), yes(self.knees), self.back, self.tail * 100.0, self.ground * 100.0, self.inside * 100.0)
    }
}

/// A lying pose held against what lying must be: each measure in a few words, and whether
/// all of them hold.
pub fn measured(glb: &crate::glb::Glb, sk: &Skeleton, roles: &Roles, pose: &Pose) -> Option<(String, bool)> {
    let shape = Shape::of(glb, sk, roles)?;
    let m = Measures::of(&shape, sk, roles, pose);
    Some((m.said(), m.good(&lying())))
}

/// Lying down as a dog or a wolf lies, fitted to this model as a whole. The pose is seven
/// numbers (how far the body comes down and how it is tipped; how far ahead and out each
/// fore paw is put; how far back and out each hock; how high its limbs lie) and one number says how far a pose
/// is from lying ([`Measures::short`]: every measure of `poses.toml` at once, on the
/// skin as it would be drawn). The six are searched together until that is as small as it
/// gets. (Until 2026-11-12 each part was placed in turn by rule and the measures only
/// refused the result: clear the ground and the legs went into the body, mend that and
/// the chest rose. The developer asked how it would be solved as a whole.)
///
/// What is built for each six: a fore leg reaches its paw to the place given, flat on the
/// ground, and its elbow falls where it falls; a hind leg puts its hock on the ground at
/// the place given, its own thickness up, with the lower leg flat ahead of it and the knee
/// folded up by the flank; a paw stays flat as it stood. Nothing where the creature has not
/// four legs that fold.
pub fn lie(glb: &crate::glb::Glb, sk: &Skeleton, roles: &Roles, stand: &Pose) -> Option<Vec<Pose>> {
    let shape = Shape::of(glb, sk, roles)?;
    let (floor, leg, up, ahead) = (shape.floor, shape.leg, Vec3::Y, roles.forward);
    let stood = sk.pose_world(stand);
    let at = |j: usize| stood[j].w_axis.truncate();
    let turn_of = |m: &glam::Mat4| m.to_scale_rotation_translation().1;
    let (fore, hind) = Shape::ends(&shape.legs, roles, &at);
    let (root, middle) = (at(sk.root), (fore + hind) / 2.0);
    let paw = |end: usize| at(end).y - floor < 0.15 * leg;
    // How high the part of a leg that lies on the ground lies: as thick as it is at its
    // thickest from the joint given to its end (a forearm laid on the floor by its bone was
    // a tenth of a leg into it), and no lower than its paw's joint stood.
    // (`much`: how much of its thickness, which the search settles with the rest: read off
    // the skin a limb's radius is a little full, and laid at all of it the whole animal
    // lay a hand over the ground, touching nothing.)
    let lies_at = |l: &Folded, from: usize, much: f32| (shape.thick(roles, l, from) * much).max(at(l.end).y - floor);
    let built = |p: &[f32; 7]| {
        let mut pose = stand.clone();
        let tipped = Quat::from_axis_angle(ahead.cross(up).normalize(), -p[1]);
        clips::turn_now(sk, &mut pose, sk.root, tipped);
        pose.root_t += sk.base.inverse().transform_vector3(middle + tipped * (root - middle) - root - up * p[0] * leg);
        for l in &shape.legs {
            let of = &roles.legs[l.n];
            let out = roles.side * if of.left { 1.0 } else { -1.0 };
            let hip = sk.pose_world(&pose)[l.hip].w_axis.truncate().with_y(floor);
            if of.front {
                let to = hip + ahead * p[2] * leg + out * p[3] * leg;
                if paw(l.end) {
                    clips::reach_by(sk, roles, &mut pose, of, (l.hip, l.knee), l.end, to + up * lies_at(l, l.knee, p[6]));
                } else {
                    let sole = stood[l.end].inverse().transform_point3(at(l.end).with_y(floor));
                    clips::reach_point(sk, roles, &mut pose, of, (l.hip, l.knee), (l.end, sole), to + up * lies_at(l, l.knee, p[6]));
                }
            } else {
                let high = lies_at(l, l.heel, p[6]);
                let hock = hip - ahead * p[4] * leg + out * p[5] * leg + up * high;
                clips::reach_by(sk, roles, &mut pose, of, (l.hip, l.knee), l.heel, hock);
                let now = sk.pose_world(&pose);
                let heel = now[l.heel].w_axis.truncate();
                let (from, to) = if l.end != l.heel {
                    (now[l.end].w_axis.truncate() - heel, (heel + ahead * at(l.heel).distance(at(l.end))).with_y(floor + high) - heel)
                } else {
                    let sole = stood[l.heel].inverse().transform_point3(at(l.end).with_y(floor));
                    (now[l.heel].transform_point3(sole) - heel, ahead)
                };
                if let (Some(from), Some(to)) = (from.try_normalize(), to.try_normalize()) {
                    clips::turn_now(sk, &mut pose, l.heel, Quat::from_rotation_arc(from, to));
                }
            }
            // (A paw stays flat on the ground as it stood, whatever its leg has done.)
            if paw(l.end) && (of.front || l.end != l.heel) {
                let now = turn_of(&sk.pose_world(&pose)[l.end]);
                clips::turn_now(sk, &mut pose, l.end, turn_of(&stood[l.end]) * now.inverse());
            }
        }
        pose
    };
    let short = |p: &[f32; 7]| {
        let pose = built(p);
        if pose.rotations.iter().any(|r| !r.is_finite()) {
            return f32::MAX;
        }
        Measures::of(&shape, sk, roles, &pose).short() + 2.0 * (shape.body.tears(sk, &pose) - 0.9).max(0.0)
    };
    // From a first guess (the body down by as far as its belly stands over the ground), each
    // of the six tried a step either way, whatever makes it less short of lying kept; the
    // steps halved when none does.
    let standing = Measures::of(&shape, sk, roles, stand);
    // (From several first guesses, legs close in and legs wide, and the best of them kept:
    // from one, the search stopped with the legs under the body holding it a tenth of a leg
    // off the ground, where laid out wide of it the body comes down.)
    let (mut p, mut best) = ([0.0f32; 7], f32::MAX);
    for (fore_out, hind_out, fore_ahead) in [(0.07, 0.15, 0.45), (0.3, 0.4, 0.45), (0.3, 0.15, 0.2), (0.07, 0.4, 0.7), (0.45, 0.55, 0.3)] {
        let mut q0 = [(standing.chest + standing.loins) / 2.0, 0.0, fore_ahead, fore_out, 0.12, hind_out, 0.8];
        let mut step = [0.08, 0.06, 0.12, 0.05, 0.12, 0.05, 0.15];
        let mut now = short(&q0);
        for _ in 0..6 {
            let mut better = true;
            while better {
                better = false;
                for i in 0..7 {
                    for way in [1.0, -1.0] {
                        let mut q = q0;
                        q[i] += way * step[i];
                        q[6] = q[6].clamp(0.3, 1.3);
                        let s = short(&q);
                        if s < now - 1e-4 {
                            (q0, now, better) = (q, s, true);
                        }
                    }
                }
            }
            step = step.map(|s| s / 2.0);
        }
        if now < best {
            (p, best) = (q0, now);
        }
    }
    let mut pose = built(&p);
    if best == f32::MAX {
        return None;
    }
    // Its tail lies along the ground behind it, not through it: raised from its root, a
    // little at a time, until none of its skin is under the floor.
    if let (Some(&first), Some(&last)) = (roles.tail.first(), roles.tail.last()) {
        for _ in 0..12 {
            let skin = shape.body.at(sk, &pose);
            if (0..skin.len()).filter(|i| roles.tail.contains(&shape.body.most[*i])).all(|i| skin[i].y >= floor) {
                break;
            }
            let now = sk.pose_world(&pose);
            let Some(axis) = (now[last].w_axis.truncate() - now[first].w_axis.truncate()).cross(up).try_normalize() else { break };
            clips::turn_now(sk, &mut pose, first, Quat::from_axis_angle(axis, 0.1));
        }
    }
    if std::env::var_os("FORGE_TRACE").is_some() {
        // (And what of it is lowest, by the joint it follows: where to look when it is in the ground.)
        let skin = shape.body.at(sk, &pose);
        let mut low: std::collections::BTreeMap<usize, f32> = Default::default();
        for (at, most) in skin.iter().zip(&shape.body.most) {
            let e = low.entry(*most).or_insert(f32::MAX);
            *e = e.min((at.y - floor) / leg);
        }
        let mut low: Vec<(usize, f32)> = low.into_iter().filter(|l| l.1 < -0.01).collect();
        low.sort_by(|a, b| a.1.total_cmp(&b.1));
        eprintln!("lie under the ground: {}", low.iter().take(6).map(|(j, y)| format!("{} {:+.0}%", sk.joints[*j].name, y * 100.0)).collect::<Vec<_>>().join(" · "));
        eprintln!("lie fitted: down {:.2} tip {:.2} · fore paws {:.2} ahead {:.2} out · hocks {:.2} back {:.2} out · limbs at {:.2} of their thickness · short of lying by {best:.3}", p[0], p[1], p[2], p[3], p[4], p[5], p[6]);
    }
    // Five seconds of it, breathing: the body rising and sinking a very little.
    let n = (FPS * 5.0) as usize;
    Some((0..=n).map(|i| {
        let mut breath = pose.clone();
        breath.root_t += sk.base.inverse().transform_vector3(up * 0.006 * leg * (0.5 - 0.5 * (i as f32 / n as f32 * std::f32::consts::TAU).cos()));
        breath
    }).collect())
}
