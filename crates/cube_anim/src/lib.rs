//! A body moved by its skeleton and its feet, with no clips: a skeleton whose bones we name
//! and size ourselves, legs bent by inverse kinematics to feet that are planted on the ground
//! and step when the body has left them behind, and a body carried by where its feet are.
//! No engine types: it is plain arithmetic, tested here, and whoever draws it asks for the
//! boxes ([`Walker::boxes`]).
//!
//! Begun 2026-11-12, at the developer's asking, after a day of carrying other people's clips
//! onto skeletons read by their shape: get skeletal animation and inverse kinematics to work
//! on cubes, on a skeleton that is known, and only then on real models. What clips cannot do
//! comes free here: starting, stopping, turning on the spot, any speed between two gaits,
//! standing on a slope, lying down with the feet where they stood.

use glam::{Quat, Vec2, Vec3};

/// How a four-legged body is built, metres. The whole skeleton: everything else follows.
#[derive(Clone, Copy, Debug)]
pub struct Build {
    /// From shoulders to hips, and across them.
    pub long: f32,
    pub wide: f32,
    /// How high the shoulders and hips stand.
    pub tall: f32,
    /// How deep the trunk is, and how thick a leg.
    pub deep: f32,
    pub leg: f32,
    pub neck: f32,
    pub head: f32,
    pub tail: f32,
}

impl Build {
    /// A wolf-sized animal: the one the tests and the first pictures use.
    pub fn dog() -> Self {
        Self { long: 0.75, wide: 0.26, tall: 0.62, deep: 0.28, leg: 0.08, neck: 0.28, head: 0.26, tail: 0.45 }
    }
    /// The upper and the lower bone of a leg: together a little more than the height it
    /// stands at, so a standing leg is a little bent and has something to give.
    fn bones(&self) -> (f32, f32) {
        // (Of the height its legs leave it at, not of its whole height: at 0.54 of that each,
        // it stood on legs a quarter too long for it, bent like a crouching thing's.)
        let high = self.tall - self.deep * 0.35;
        (high * 0.53, high * 0.53)
    }
    /// Where each leg leaves the trunk, in the body's own frame (x its right, z ahead):
    /// fore left, fore right, hind left, hind right.
    fn hips(&self) -> [Vec3; 4] {
        let (x, z) = (self.wide / 2.0, self.long / 2.0);
        [Vec3::new(-x, 0.0, z), Vec3::new(x, 0.0, z), Vec3::new(-x, 0.0, -z), Vec3::new(x, 0.0, -z)]
    }
}

/// Where the middle joint of a two-boned limb goes, for its far end to be at `to` (or as near
/// as its length allows): the two bones `a` and `b` long, bent toward `pole`. Gives the joint
/// and where the end comes to.
pub fn two_bone(from: Vec3, to: Vec3, a: f32, b: f32, pole: Vec3) -> (Vec3, Vec3) {
    let reach = (to - from).length().clamp((a - b).abs() + 1e-4, a + b - 1e-4);
    let along = (to - from).try_normalize().unwrap_or(Vec3::NEG_Y);
    // (The triangle of the two bones and the line between its ends.)
    let near = (a * a - b * b + reach * reach) / (2.0 * reach);
    let out = (a * a - near * near).max(0.0).sqrt();
    let side = (pole - along * pole.dot(along)).try_normalize().unwrap_or_else(|| along.any_orthonormal_vector());
    (from + along * near + side * out, from + along * reach)
}

/// One foot: where it is planted, and the step it is taking, if it is.
#[derive(Clone, Copy, Debug)]
struct Foot {
    at: Vec3,
    step: Option<(Vec3, Vec3, f32, f32)>,
}

/// A box to be drawn: its middle, how it is turned, its size.
#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub at: Vec3,
    pub turn: Quat,
    pub size: Vec3,
    /// Which part it is: 0 trunk, 1 leg, 2 neck and head, 3 tail.
    pub part: u8,
}

#[derive(Clone, Debug)]
pub struct Walker {
    pub build: Build,
    /// Where it is over the ground (the point under its middle), which way it faces
    /// (radians, 0 looking along +z), how fast it is going.
    pub at: Vec3,
    pub yaw: f32,
    vel: Vec3,
    feet: [Foot; 4],
    /// How far down it is: 0 standing, 1 lying on its belly. Eased toward what is asked.
    pub down: f32,
    /// The trunk's middle and how it is turned, as its feet carry it.
    trunk: (Vec3, Quat),
    /// The tail's end, which comes after the body.
    tail: Vec3,
    /// Steps taken, by foot: for whoever counts them.
    pub steps: [u32; 4],
    /// Where each leg leaves the trunk, from the trunk's middle in its own frame, and the
    /// lengths of its two bones: the build's, or a real model's own ([`Walker::fitted`]).
    hip: [Vec3; 4],
    bone: [(f32, f32); 4],
    /// Which way each leg's middle joint bends, in the trunk's frame: a fore leg's back and a
    /// hind leg's ahead, or the way a real model's own legs are bent as it stands.
    pole: [Vec3; 4],
    /// Lying, it goes right down and tucks its legs in beside it. Not a real model's, yet:
    /// its legs are swung from shoulders and hips that are skin, and folded flat beside it
    /// the wolf's hindquarters were wrenched up and a leg thrown out behind (looked at
    /// large, 2026-11-12). It sinks half way with its feet where they stood, which nothing
    /// about it breaks: a crouch, until its skeleton can be folded properly.
    tucks: bool,
}

impl Walker {
    pub fn new(build: Build, at: Vec3, yaw: f32) -> Self {
        let turn = Quat::from_rotation_y(yaw);
        let feet = build.hips().map(|h| Foot { at: at + turn * h, step: None });
        let trunk = (at + Vec3::Y * build.tall, turn);
        Self { build, at, yaw, vel: Vec3::ZERO, feet, down: 0.0, trunk, tail: at - turn * Vec3::Z * (build.long / 2.0 + build.tail) + Vec3::Y * build.tall, steps: [0; 4], hip: build.hips().map(|h| h - Vec3::Y * build.deep * 0.35), bone: [build.bones(); 4], pole: [Vec3::NEG_Z, Vec3::NEG_Z, Vec3::Z, Vec3::Z], tucks: true }
    }

    /// A walker for a body that is already made: where each of its legs leaves it as it
    /// stands (in its own frame, from the point on the ground under its middle; fore left,
    /// fore right, hind left, hind right, left being -x) and the lengths of each leg's two
    /// bones, and where each leg's middle joint is (the same frame): its legs go on bending
    /// the way they are bent. Its build is read off those.
    pub fn fitted(hips: [Vec3; 4], bone: [(f32, f32); 4], knees: [Vec3; 4], at: Vec3, yaw: f32) -> Self {
        let tall = hips.iter().map(|h| h.y).sum::<f32>() / 4.0;
        let (fore, hind) = ((hips[0].z + hips[1].z) / 2.0, (hips[2].z + hips[3].z) / 2.0);
        let wide = ((hips[1].x - hips[0].x).abs() + (hips[3].x - hips[2].x).abs()) / 2.0;
        let build = Build { long: fore - hind, wide, tall, deep: tall * 0.45, leg: tall * 0.13, neck: tall * 0.45, head: tall * 0.4, tail: tall * 0.7 };
        let middle = Vec3::new(0.0, tall, (fore + hind) / 2.0);
        let mut w = Self::new(build, at, yaw);
        (w.hip, w.bone, w.tucks) = (hips.map(|h| h - middle), bone, false);
        for i in 0..4 {
            // (Over the ground: which way the knee stands off the line from hip to foot.)
            let off = Vec3::new(knees[i].x - hips[i].x, 0.0, knees[i].z - hips[i].z);
            if off.length() > 0.04 * (bone[i].0 + bone[i].1) {
                w.pole[i] = off.normalize();
            }
        }
        let turn = Quat::from_rotation_y(yaw);
        for (foot, h) in w.feet.iter_mut().zip(hips) {
            foot.at = at + turn * Vec3::new(h.x, 0.0, h.z - middle.z);
        }
        w
    }

    /// The trunk's middle and how it is turned; and each foot's place on the ground.
    pub fn trunk(&self) -> (Vec3, Quat) {
        self.trunk
    }
    pub fn feet(&self) -> [Vec3; 4] {
        self.feet.map(|f| f.at)
    }

    /// One moment: the body is now at `at` facing `yaw`; `rest` is how far down it should be
    /// (0 to 1); `ground` gives the height of the ground at a place.
    pub fn step(&mut self, dt: f32, at: Vec3, yaw: f32, rest: f32, ground: impl Fn(f32, f32) -> f32) {
        let b = self.build;
        self.vel = if dt > 0.0 { (at - self.at) / dt } else { Vec3::ZERO };
        let turned = (yaw - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        (self.at, self.yaw) = (at, yaw);
        self.down += (rest.clamp(0.0, 1.0) - self.down) * (1.0 - (-3.0 * dt).exp());
        let turn = Quat::from_rotation_y(yaw);
        let flat = Vec2::new(self.vel.x, self.vel.z);
        let speed = flat.length();
        // A stride is longer the faster it goes, and is taken quicker. Fast enough for its
        // size it gallops: the fore feet together and then the hind, in long bounds, where at
        // a trot they go in diagonal pairs. (It trotted at every speed at first, and at a run
        // its legs went like a sewing machine under a level back: the developer, of the
        // wolf, 2026-11-12: "the run looks dumb".)
        // (By its legs' length, as a pendulum's swing goes: a badger gallops at a speed a
        // wolf walks at. All of this is in metres and seconds.)
        let legs = self.bone.iter().map(|b| b.0 + b.1).sum::<f32>() / 4.0;
        let gallops = speed > 1.15 * (9.81 * legs).sqrt();
        // (No longer than its legs allow: a stride is taken from as far behind a hip as ahead of it.)
        let longest = self.bone.iter().map(|b| b.0 + b.1).fold(f32::MAX, f32::min) * 1.15;
        let stride = if gallops { (0.3 + speed * 0.3).min(longest) } else { (0.18 + speed * 0.3).min(longest) };
        let takes = if gallops { (stride / speed * 0.5).clamp(0.13, 0.24) } else { (stride / speed.max(0.6) * 0.55).clamp(0.11, 0.3) };
        // Where each foot belongs: under where its leg leaves the trunk, and ahead by half a
        // stride the way it is going, so it lands where the body will be.
        // Lying, its legs are tucked in beside it, as an animal's are: each foot a little out
        // from under its hip and a little ahead, the fore leg folded back on itself with its
        // elbow behind, the hind folded forward with its knee ahead, both flat on the ground
        // along the body. (Left where they stood the legs folded through the body; stretched
        // out ahead, which was tried next, is not how anything lies. The developer: "don't
        // they bend their legs in?")
        let lies = rest > 0.5 && self.tucks;
        let bone = self.bone;
        let mut n = 0;
        let homes = self.hip.map(|h| {
            let reach = bone[n].0 + bone[n].1;
            n += 1;
            // (Nearly at full stretch, so the knee lies low: drawn up to half its length a hind
            // leg's knee stood over the wolf's back. And the hind well out, beside the belly.)
            let out = if lies { Vec3::new(h.x.signum() * if h.z > 0.0 { 0.16 } else { 0.24 } * reach, 0.0, if h.z > 0.0 { 0.22 * reach } else { 0.1 * reach }) } else { Vec3::ZERO };
            // (Ahead by the way it is going, and never by more than half the leg: at a sprint a
            // foot was set down further ahead of its hip than the leg is long.)
            let p = at + turn * (Vec3::new(h.x, 0.0, h.z) + out) + (Vec3::new(flat.x, 0.0, flat.y) * takes * 0.9).clamp_length_max(reach * 0.5);
            Vec3::new(p.x, ground(p.x, p.z), p.z)
        });
        // The feet go in diagonal pairs, as a trot: fore left with hind right. A foot steps
        // when it has been left further from where it belongs than the stride allows (or the
        // body has turned under it), and only while the other pair has its feet down.
        let far = |f: &Foot, home: Vec3| f.at.distance(home);
        let pairs = if gallops { [[0, 1], [2, 3]] } else { [[0, 3], [1, 2]] };
        for (n, pair) in pairs.into_iter().enumerate() {
            let other = pairs[1 - n];
            let wants = pair.iter().any(|&i| self.feet[i].step.is_none() && far(&self.feet[i], homes[i]) > stride * 0.5 + if speed < 0.05 && turned.abs() < 1e-3 && !lies { 0.06 } else { 0.0 });
            let free = other.iter().all(|&i| self.feet[i].step.is_none());
            // (At a gallop it need not wait: there is a moment in each bound with no foot down.
            // Made to wait, a foot was left behind further than its leg is long.)
            if wants && (free || gallops) {
                for &i in &pair {
                    if self.feet[i].step.is_none() && far(&self.feet[i], homes[i]) > 0.03 {
                        self.feet[i].step = Some((self.feet[i].at, homes[i], 0.0, takes));
                        self.steps[i] += 1;
                    }
                }
            }
        }
        for (i, foot) in self.feet.iter_mut().enumerate() {
            if let Some((from, _, done, takes)) = &mut foot.step {
                *done += dt / *takes;
                // (It lands where it now belongs, not where that was when it left.)
                let (s, to) = (done.min(1.0), homes[i]);
                let ease = s * s * (3.0 - 2.0 * s);
                foot.at = from.lerp(to, ease) + Vec3::Y * (4.0 * s * (1.0 - s)) * b.tall * if gallops { 0.24 } else { 0.12 + 0.1 * (speed / 4.0).min(1.0) };
                if *done >= 1.0 {
                    foot.at = to;
                    foot.step = None;
                }
            } else {
                // Planted: it does not move. (Only down onto ground that has changed under it.)
                foot.at.y = ground(foot.at.x, foot.at.z);
            }
        }
        // The trunk is carried by its feet: as high over their middle as it stands (less as
        // it lies down, to its belly on the ground), tipped to the slope from hind feet to
        // fore, rolled to the one from left to right, with a little rise and fall as it goes.
        let mid = |a: usize, c: usize| (self.feet[a].at + self.feet[c].at) / 2.0;
        let (fore, hind, left, right) = (mid(0, 1), mid(2, 3), mid(0, 2), mid(1, 3));
        let floor = (fore.y + hind.y) / 2.0;
        let stands = b.tall * (1.0 - self.down) + if self.tucks { b.deep / 2.0 + 0.02 } else { b.tall * 0.5 } * self.down;
        // (In a bound the whole body rises with the feet that are off the ground.)
        let aloft = self.feet.iter().filter(|f| f.step.is_some()).count() as f32 / 4.0;
        let bob = if gallops { aloft * 0.14 * b.tall } else if speed > 0.1 { (self.steps.iter().sum::<u32>() as f32 * 1.7).sin() * 0.008 * b.tall } else { 0.0 };
        let pitch = -((fore.y - hind.y) / b.long).clamp(-0.6, 0.6).atan() * (1.0 - self.down * 0.5);
        let roll = ((right.y - left.y) / b.wide).clamp(-0.4, 0.4).atan() * 0.5;
        let want = (Vec3::new(at.x, floor + stands + bob, at.z), turn * Quat::from_rotation_x(pitch) * Quat::from_rotation_z(roll));
        // And no higher than its legs reach: at a run a foot is left far behind and another
        // set down far ahead, and the body comes down between them as a running animal's
        // does, by as much as keeps every leg its own length.
        let ease = 1.0 - (-14.0 * dt).exp();
        self.trunk = (self.trunk.0.lerp(want.0, ease).with_x(at.x).with_z(at.z), self.trunk.1.slerp(want.1, ease));
        let over = self.hip.iter().zip(&self.feet).zip(&self.bone).map(|((h, f), (upper, lower))| {
            let reach = (upper + lower) * 0.985;
            let hip = self.trunk.0 + self.trunk.1 * *h;
            let across = Vec2::new(hip.x - f.at.x, hip.z - f.at.z).length();
            hip.y - (f.at.y + (reach * reach - across * across).max(0.0).sqrt())
        }).fold(0.0f32, f32::max);
        self.trunk.0.y -= over.min(b.tall * 0.45);
        // The tail's end comes after the body, and hangs.
        let tail_root = self.trunk.0 - self.trunk.1 * Vec3::Z * b.long / 2.0;
        let hangs = tail_root - self.trunk.1 * Vec3::Z * b.tail * 0.8 - Vec3::Y * b.tail * (0.45 + 0.4 * self.down);
        self.tail = self.tail.lerp(hangs, 1.0 - (-6.0 * dt).exp());
        self.tail = tail_root + (self.tail - tail_root).clamp_length(b.tail * 0.6, b.tail);
    }

    /// The joints of each leg in the world: where it leaves the trunk, its knee, its foot.
    pub fn legs(&self) -> [[Vec3; 3]; 4] {
        let hips = self.hip;
        let ahead = self.trunk.1 * Vec3::Z;
        std::array::from_fn(|i| {
            let (a, b) = self.bone[i];
            let hip = self.trunk.0 + self.trunk.1 * hips[i];
            // A fore leg's elbow points back and a hind leg's knee ahead; lying, both go out
            // to the side a little so the leg folds beside the body and not through it.
            let out = self.trunk.1 * Vec3::X * hips[i].x.signum();
            let _ = ahead;
            // (Standing, the joint is a little up and out of the line from hip to foot; lying,
            // it lies out flat along the ground beside the body.)
            let down = if self.tucks { self.down } else { 0.0 };
            let pole = self.trunk.1 * self.pole[i] + out * (0.15 + 0.5 * down) + Vec3::Y * (0.3 * (1.0 - down) + 0.12 * down);
            let (knee, foot) = two_bone(hip, self.feet[i].at, a, b, pole);
            [hip, knee, foot]
        })
    }

    /// Everything to draw, as boxes.
    pub fn boxes(&self) -> Vec<Block> {
        let b = self.build;
        let (mid, turn) = self.trunk;
        let between = |p: Vec3, q: Vec3, thick: f32, part: u8| Block { at: (p + q) / 2.0, turn: Quat::from_rotation_arc(Vec3::Y, (q - p).try_normalize().unwrap_or(Vec3::Y)), size: Vec3::new(thick, p.distance(q).max(0.01), thick), part };
        let mut out = vec![Block { at: mid, turn, size: Vec3::new(b.wide, b.deep, b.long + b.leg), part: 0 }];
        for [hip, knee, foot] in self.legs() {
            out.push(between(hip, knee, b.leg * 1.15, 1));
            out.push(between(knee, foot, b.leg * 0.9, 1));
            // A paw, flat on the ground under the leg's end.
            out.push(Block { at: foot + Vec3::Y * b.leg * 0.25 + turn * Vec3::Z * b.leg * 0.3, turn: Quat::from_rotation_y(self.yaw), size: Vec3::new(b.leg * 1.1, b.leg * 0.5, b.leg * 1.7), part: 1 });
        }
        // The neck rises ahead of the shoulders and the head looks the way it is going; lying,
        // both come down along the ground.
        let ahead = turn * Vec3::Z;
        let shoulder = mid + ahead * b.long / 2.0 + Vec3::Y * b.deep * 0.2;
        let lift = 0.75 - 0.6 * self.down - 0.25 * (self.vel.length() / 5.0).min(1.0);
        let nape = shoulder + (ahead * (1.0 - lift * 0.6) + Vec3::Y * lift).normalize() * b.neck;
        out.push(between(shoulder, nape, b.deep * 0.5, 2));
        let look = (Quat::from_rotation_y(self.yaw) * Vec3::Z - Vec3::Y * (0.1 + 0.25 * self.down)).normalize();
        out.push(Block { at: nape + look * b.head * 0.4, turn: Quat::from_rotation_arc(Vec3::Z, look), size: Vec3::new(b.deep * 0.62, b.deep * 0.6, b.head), part: 2 });
        let tail_root = mid - ahead * b.long / 2.0 + Vec3::Y * b.deep * 0.2;
        out.push(between(tail_root, self.tail, b.leg * 0.8, 3));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_bones_reach_what_is_in_reach_and_bend_toward_the_pole() {
        let (knee, end) = two_bone(Vec3::Y, Vec3::new(0.3, 0.0, 0.0), 0.6, 0.6, Vec3::Z);
        assert!(end.distance(Vec3::new(0.3, 0.0, 0.0)) < 1e-3 && (knee.distance(Vec3::Y) - 0.6).abs() < 1e-3 && (knee.distance(end) - 0.6).abs() < 1e-3 && knee.z > 0.1, "knee {knee} end {end}");
        // Out of reach, the limb is straight toward it and neither bone is any longer.
        let (knee, end) = two_bone(Vec3::ZERO, Vec3::X * 5.0, 0.6, 0.6, Vec3::Y);
        assert!((end.x - 1.2).abs() < 1e-3 && (knee.distance(end) - 0.6).abs() < 1e-3);
    }

    /// Walks a body in a straight line and then round a bend, and gives what its feet did.
    fn walk(speed: f32, seconds: f32) -> (Walker, f32, f32) {
        let mut w = Walker::new(Build::dog(), Vec3::ZERO, 0.0);
        let (dt, mut slid, mut gap) = (1.0 / 60.0, 0.0f32, 0.0f32);
        let (mut at, mut yaw) = (Vec3::ZERO, 0.0f32);
        for tick in 0..(seconds * 60.0) as u32 {
            if tick > 180 {
                yaw += 0.6 * dt;
            }
            at += Quat::from_rotation_y(yaw) * Vec3::Z * speed * dt;
            let before: Vec<(Vec3, bool)> = w.feet.iter().map(|f| (f.at, f.step.is_none())).collect();
            w.step(dt, at, yaw, 0.0, |_, _| 0.0);
            // A foot that was down and is still down has not moved at all.
            for (f, (was, down)) in w.feet.iter().zip(before) {
                if down && f.step.is_none() {
                    slid = slid.max(f.at.distance(was));
                }
            }
            // And every leg's end is on its foot: none is asked to be longer than it is.
            for (leg, f) in w.legs().iter().zip(&w.feet) {
                gap = gap.max(leg[2].distance(f.at));
            }
        }
        (w, slid, gap)
    }

    #[test]
    fn planted_feet_do_not_slide_and_every_leg_reaches_its_foot_at_a_walk_and_a_run() {
        for speed in [0.4, 1.2, 3.0, 5.0] {
            let (w, slid, gap) = walk(speed, 8.0);
            assert!(slid < 1e-4, "at {speed} m/s a planted foot moved {slid} m");
            assert!(gap < 0.03, "at {speed} m/s a leg was {gap} m short of its foot");
            assert!(w.steps.iter().all(|s| *s as f32 > speed * 2.0), "at {speed} m/s it took {:?} steps in eight seconds", w.steps);
            let even = *w.steps.iter().max().unwrap() - *w.steps.iter().min().unwrap();
            assert!(even <= 3, "its feet did not take turns: {:?}", w.steps);
        }
    }

    #[test]
    fn it_stands_still_without_stepping_and_turns_on_the_spot_by_stepping() {
        let mut w = Walker::new(Build::dog(), Vec3::ZERO, 0.0);
        for _ in 0..240 {
            w.step(1.0 / 60.0, Vec3::ZERO, 0.0, 0.0, |_, _| 0.0);
        }
        assert_eq!(w.steps, [0; 4], "it shuffled its feet standing still");
        for tick in 0..240 {
            w.step(1.0 / 60.0, Vec3::ZERO, tick as f32 / 240.0 * 3.0, 0.0, |_, _| 0.0);
        }
        assert!(w.steps.iter().all(|s| *s >= 2), "turned half round on the spot it stepped {:?}", w.steps);
    }

    #[test]
    fn it_lies_down_with_its_legs_tucked_in_and_gets_up_again() {
        let mut w = Walker::new(Build::dog(), Vec3::ZERO, 0.0);
        for _ in 0..240 {
            w.step(1.0 / 60.0, Vec3::ZERO, 0.0, 1.0, |_, _| 0.0);
        }
        let b = w.build;
        assert!(w.trunk.0.y < b.deep && w.trunk.0.y - b.deep / 2.0 > 0.0, "lying, its trunk is at {}", w.trunk.0.y);
        // Each leg is folded up beside it: its foot near its hip and wide of it, its fore
        // elbows behind and its hind knees ahead, every joint on or over the ground and no
        // higher than its back, and every bone still its own length.
        let (upper, lower) = b.bones();
        for (i, [hip, knee, foot]) in w.legs().into_iter().enumerate() {
            assert!(hip.distance(foot) < 0.6 * (upper + lower), "leg {i} is not folded: {} of {}", hip.distance(foot), upper + lower);
            assert!(foot.x.abs() > hip.x.abs(), "leg {i}: its foot is under the body: foot {foot} hip {hip}");
            assert!(if i < 2 { knee.z < hip.z } else { knee.z > hip.z }, "leg {i} is folded the wrong way: knee {knee} hip {hip}");
            assert!(foot.y.abs() < 1e-3 && knee.y > -1e-3 && knee.y < w.trunk.0.y + b.deep / 2.0, "leg {i}: foot {foot} knee {knee}");
            assert!((hip.distance(knee) - upper).abs() < 1e-3 && (knee.distance(foot) - lower).abs() < 1e-3);
        }
        for _ in 0..240 {
            w.step(1.0 / 60.0, Vec3::ZERO, 0.0, 0.0, |_, _| 0.0);
        }
        assert!((w.trunk.0.y - b.tall).abs() < 0.03, "it did not get up: {}", w.trunk.0.y);
        assert!(w.legs().iter().all(|[hip, _, foot]| (foot.z - hip.z).abs() < 0.12), "up again, its feet are not back under it");
    }

    #[test]
    fn on_a_slope_its_feet_are_on_the_ground_and_its_back_follows_the_hill() {
        let hill = |_: f32, z: f32| z * 0.3;
        let mut w = Walker::new(Build::dog(), Vec3::ZERO, 0.0);
        let mut at = Vec3::ZERO;
        for _ in 0..300 {
            at.z += 1.2 / 60.0;
            at.y = hill(0.0, at.z);
            w.step(1.0 / 60.0, at, 0.0, 0.0, hill);
        }
        assert!(w.feet.iter().filter(|f| f.step.is_none()).all(|f| (f.at.y - hill(0.0, f.at.z)).abs() < 1e-3));
        let nose_up = (w.trunk.1 * Vec3::Z).y;
        assert!(nose_up > 0.15, "going uphill its back was level: {nose_up}");
    }
}
