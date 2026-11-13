//! A person moved by their own skeleton and feet, no clips: the two-legged walker, begun the
//! way the four-legged one was (the developer, 2026-11-12: "building it from ground up in
//! the same way we approach the wolf"): the arithmetic and its tests here, then a figure of
//! boxes in the game (`--limbs-person`), and a real model only when the boxes move rightly.
//!
//! What a person's gait is made of, each a line or two below: one clock for the stride, a
//! foot down for the first part of its turn and swung for the rest (most of it down at a
//! walk, so both are down between steps; less than half at a run, so neither is, between);
//! a foot that is down does not move, and is set down where it will be under its hip half
//! way through its time on the ground; it lands toe up and leaves heel up; the hips are
//! carried by the legs, as high as they reach, so they fall as the legs spread and rise over
//! the standing one; they sway over the standing foot and turn toward the leg that is
//! ahead; the shoulders turn the other way and each arm swings against its own leg, bent
//! more the faster it goes; the whole leans into its speed. Metres and seconds.

use crate::{Block, two_bone};
use glam::{Mat3, Quat, Vec2, Vec3};
use std::f32::consts::TAU;

/// A person's measures, metres.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// Thigh and shin, and how high the ankle is over the sole; the foot's length.
    pub thigh: f32,
    pub shin: f32,
    pub ankle: f32,
    pub foot: f32,
    /// Between the two hip joints; from them up to the shoulders; between the shoulders.
    pub hips: f32,
    pub torso: f32,
    pub shoulders: f32,
    pub upper: f32,
    pub fore: f32,
    pub hand: f32,
    pub neck: f32,
    pub head: f32,
}

impl Frame {
    /// A grown person so tall.
    pub fn person(tall: f32) -> Self {
        Self { thigh: 0.245 * tall, shin: 0.245 * tall, ankle: 0.045 * tall, foot: 0.15 * tall, hips: 0.1 * tall, torso: 0.29 * tall, shoulders: 0.22 * tall, upper: 0.165 * tall, fore: 0.15 * tall, hand: 0.06 * tall, neck: 0.05 * tall, head: 0.13 * tall }
    }
    /// How high the hip joints are as it stands: its legs a little short of straight.
    pub fn stands(&self) -> f32 {
        self.ankle + (self.thigh + self.shin) * 0.985
    }
}

#[derive(Clone, Copy, Debug)]
struct Foot {
    /// The place on the ground under its ankle when it is flat (in the air: under where it is,
    /// raised), which way it points, and how it is tipped (toe up over nothing, heel up under).
    at: Vec3,
    yaw: f32,
    pitch: f32,
    /// On the ground; and where it left it.
    down: bool,
    from: Vec3,
}

/// Where everything is at one moment, in the world: enough to draw it or to turn a real
/// skeleton's bones to.
#[derive(Clone, Copy, Debug)]
pub struct Stance {
    /// The middle of the hips, the middle of the shoulders, the middle of the head.
    pub pelvis: (Vec3, Quat),
    pub chest: (Vec3, Quat),
    pub head: (Vec3, Quat),
    /// Each leg (left, right): hip, knee, ankle, and how the foot is turned.
    pub legs: [[Vec3; 3]; 2],
    pub feet: [Quat; 2],
    /// Each arm: shoulder, elbow, wrist, the hand's end.
    pub arms: [[Vec3; 4]; 2],
}

#[derive(Clone, Debug)]
pub struct Biped {
    pub frame: Frame,
    pub at: Vec3,
    pub yaw: f32,
    vel: Vec3,
    /// Where in its stride it is, 0 to 1: the left foot comes down at 0, the right at a half.
    pub phase: f32,
    feet: [Foot; 2],
    /// How much it is running, 0 a walk to 1; how much it is going at all; how far it leans.
    runs: f32,
    goes: f32,
    lean: f32,
    /// How high the hips are carried.
    high: f32,
    /// A foot being brought back under it as it stands, and how far through that it is.
    shuffle: Option<(usize, f32)>,
    /// Steps taken, by foot.
    pub steps: [u32; 2],
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Biped {
    pub fn new(frame: Frame, at: Vec3, yaw: f32) -> Self {
        let turn = Quat::from_rotation_y(yaw);
        let feet = [-1.0, 1.0].map(|side| {
            let p = at + turn * Vec3::X * side * frame.hips / 2.0;
            Foot { at: p, yaw, pitch: 0.0, down: true, from: p }
        });
        Self { frame, at, yaw, vel: Vec3::ZERO, phase: 0.0, feet, runs: 0.0, goes: 0.0, lean: 0.0, high: frame.stands(), shuffle: None, steps: [0; 2] }
    }

    /// Whether each foot is on the ground.
    pub fn down(&self) -> [bool; 2] {
        self.feet.map(|f| f.down)
    }
    /// Where each foot is (the place under its ankle).
    pub fn feet(&self) -> [Vec3; 2] {
        self.feet.map(|f| f.at)
    }

    /// One moment: the body is now at `at` facing `yaw`; `ground` gives the ground's height.
    pub fn step(&mut self, dt: f32, at: Vec3, yaw: f32, ground: impl Fn(f32, f32) -> f32) {
        let f = self.frame;
        let leg = f.thigh + f.shin;
        // (A smaller person's stride is quicker and shorter, as a pendulum's swing goes.)
        let size = (leg / 0.86).sqrt();
        let was = self.vel;
        self.vel = if dt > 0.0 { (at - self.at) / dt } else { Vec3::ZERO };
        (self.at, self.yaw) = (at, yaw);
        let flat = Vec2::new(self.vel.x, self.vel.z);
        let speed = flat.length();
        let ease = |rate: f32| 1.0 - (-rate * dt).exp();
        self.runs += (smooth((speed / size - 2.3) / 0.9) - self.runs) * ease(8.0);
        self.goes += (smooth(speed / (0.9 * size)) - self.goes) * ease(8.0);
        let r = self.runs;
        // Strides a second (two steps to a stride): about one at a walk, one and a half at a
        // run, two at a sprint. And how much of its turn a foot is down.
        let rate = (0.6 + 0.24 * speed / size) / size;
        let duty = 0.62 - 0.27 * r;
        let turn = Quat::from_rotation_y(yaw);
        let home = |i: usize| {
            let p = at + turn * Vec3::X * (i as f32 * 2.0 - 1.0) * f.hips / 2.0;
            Vec3::new(p.x, ground(p.x, p.z), p.z)
        };
        let going = speed > 0.15;
        // Stopped, it finishes the step it was in, and stands.
        let both = |phase: f32| phase.fract() < duty && (phase + 0.5).fract() < duty;
        if going || !both(self.phase) || self.feet.iter().any(|f| !f.down) && self.shuffle.is_none() {
            self.phase = (self.phase + rate.max(0.9) * dt).fract();
            self.shuffle = None;
        }
        let stepping = going || !both(self.phase);
        for i in 0..2 {
            let p = (self.phase + i as f32 * 0.5).fract();
            // It comes down ahead of its hip by rather less than half of how far the body goes
            // while it is down: a leg reaches further behind, where the heel comes up and the
            // foot's own length is added to it. (At half, the hips fell 11 cm at every step.)
            let ahead = Vec3::new(flat.x, 0.0, flat.y) * (duty / rate) * 0.38;
            let land = {
                let h = home(i) + ahead.clamp_length_max(leg * 0.75);
                Vec3::new(h.x, ground(h.x, h.z), h.z)
            };
            let foot = &mut self.feet[i];
            if self.shuffle.is_some_and(|s| s.0 == i) {
                continue;
            }
            if p < duty || !stepping {
                if !foot.down {
                    (foot.at, foot.down, foot.yaw) = (land, true, yaw);
                    self.steps[i] += 1;
                }
                foot.at.y = ground(foot.at.x, foot.at.z);
                // Toe up as it lands (at a walk, on its heel), flat under the body, heel up as it leaves.
                let s = p / duty;
                let want = if !stepping { 0.0 } else if s < 0.14 { 0.2 * (1.0 - s / 0.14) * (1.0 - r) } else if s > 0.5 { -((s - 0.5) / 0.5).powi(2) * (0.95 + 0.15 * r) } else { 0.0 };
                foot.pitch = want * self.goes;
            } else {
                if foot.down {
                    (foot.down, foot.from) = (false, foot.at);
                }
                // Swung through: lifted, heel highest early (at a run it comes up behind),
                // toe coming up ahead of it to land.
                let s = (p - duty) / (1.0 - duty);
                let lift = (std::f32::consts::PI * s).sin() * leg * (0.07 + 0.2 * r);
                foot.at = foot.from.lerp(land, smooth(s)) + Vec3::Y * lift;
                foot.yaw = yaw;
                let (leaves, peak, lands) = (-(0.95 + 0.15 * r), -(1.0 + 0.6 * r), 0.2 * (1.0 - r));
                foot.pitch = self.goes * if s < 0.3 { leaves + (peak - leaves) * smooth(s / 0.3) } else { peak + (lands - peak) * smooth((s - 0.3) / 0.7) };
            }
        }
        // Standing, a foot left away from under it (or pointing where it was going) is
        // brought back, one at a time.
        if !stepping {
            let off = |yaw_now: f32, foot: &Foot, home: Vec3| foot.at.distance(home) > 0.07 * size || ((foot.yaw - yaw_now + TAU / 2.0).rem_euclid(TAU) - TAU / 2.0).abs() > 0.35;
            if self.shuffle.is_none()
                && let Some(i) = (0..2).find(|&i| off(yaw, &self.feet[i], home(i)))
            {
                self.feet[i].from = self.feet[i].at;
                self.shuffle = Some((i, 0.0));
                self.steps[i] += 1;
            }
            if let Some((i, done)) = &mut self.shuffle {
                *done += dt / 0.28;
                let (s, to) = (done.min(1.0), home(*i));
                let foot = &mut self.feet[*i];
                foot.at = foot.from.lerp(to, smooth(s)) + Vec3::Y * (std::f32::consts::PI * s).sin() * leg * 0.05;
                (foot.yaw, foot.pitch, foot.down) = (yaw, 0.0, s >= 1.0);
                if s >= 1.0 {
                    foot.at = to;
                    self.shuffle = None;
                }
            }
        }
        // It leans into its speed, and into gathering it.
        let gathers = (self.vel - was).dot(turn * Vec3::Z) / dt.max(1e-4);
        self.lean += (0.03 * self.goes + 0.13 * r + (gathers * 0.012).clamp(-0.08, 0.1) - self.lean) * ease(5.0);
        // The hips are carried as high as the legs reach and no higher (so they fall as the
        // legs spread, and rise over the one that stands); at a run they are lowest over the
        // foot that is down and highest with neither down.
        let floor = self.feet.iter().filter(|f| f.down).map(|f| f.at.y).fold(f32::MAX, f32::min);
        let floor = if floor == f32::MAX { ground(at.x, at.z) } else { floor.min(ground(at.x, at.z)) };
        let bound = -(2.0 * TAU * (self.phase - duty / 2.0)).cos() * 0.035 * leg * r;
        let want = floor + f.stands() - 0.05 * leg * r + bound;
        self.high += (want - self.high) * ease(18.0);
        let stance = self.stance();
        let over = (0..2)
            .map(|i| {
                let (hip, foot) = (stance.legs[i][0], &self.feet[i]);
                // (A foot in the air is not carrying it: the leg will bend to wherever it is.)
                let ankle = self.ankle(foot);
                let across = Vec2::new(hip.x - ankle.x, hip.z - ankle.z).length();
                if foot.down { hip.y - (ankle.y + ((leg * 0.985).powi(2) - across * across).max(0.0).sqrt()) } else { 0.0 }
            })
            .fold(0.0f32, f32::max);
        self.high -= over.min(leg * 0.4);
    }

    /// Where a foot's ankle is: over its place when flat; tipped, it turns on the ball of the
    /// foot (heel up) or on the heel (toe up).
    fn ankle(&self, foot: &Foot) -> Vec3 {
        let f = self.frame;
        let turn = Quat::from_rotation_y(foot.yaw);
        let pivot = if foot.pitch < 0.0 { 0.5 } else { -0.25 } * f.foot;
        foot.at + turn * (Vec3::Z * pivot + Quat::from_rotation_x(-foot.pitch) * Vec3::new(0.0, f.ankle, -pivot))
    }

    /// Where everything is now.
    pub fn stance(&self) -> Stance {
        let f = self.frame;
        let (r, goes) = (self.runs, self.goes);
        let turn = Quat::from_rotation_y(self.yaw);
        let duty = 0.62 - 0.27 * r;
        let wave = (TAU * self.phase).cos();
        let over = (TAU * (self.phase - duty / 2.0)).cos();
        // The hips: over the standing foot, turned toward the leg that is ahead, the side
        // whose leg is swinging a little dropped.
        let twist = (0.08 + 0.1 * r) * goes * wave;
        let hips = turn * Quat::from_rotation_y(twist) * Quat::from_rotation_z(-0.035 * goes * (1.0 - r) * over) * Quat::from_rotation_x(self.lean * 0.4);
        let middle = Vec3::new(self.at.x, self.high, self.at.z) - turn * Vec3::X * (0.022 * (1.0 - r) + 0.008) * goes * over;
        // The shoulders turn against the hips, on a back that leans; the head stays level.
        let chest = turn * Quat::from_rotation_y(-twist * 0.9) * Quat::from_rotation_x(self.lean);
        let shoulders = middle + turn * Quat::from_rotation_x(self.lean * 0.7) * Vec3::Y * f.torso;
        let head = (shoulders + turn * Quat::from_rotation_x(self.lean * 0.4) * Vec3::Y * (f.neck + f.head / 2.0), turn * Quat::from_rotation_x(self.lean * 0.25));
        let legs = [0, 1].map(|i| {
            let side = i as f32 * 2.0 - 1.0;
            let hip = middle + hips * Vec3::X * side * f.hips / 2.0;
            let foot = &self.feet[i];
            // (The knee bends ahead, and a little out.)
            let (knee, ankle) = two_bone(hip, self.ankle(foot), f.thigh, f.shin, turn * Vec3::new(side * 0.08, 0.0, 1.0));
            [hip, knee, ankle]
        });
        let feet = [0, 1].map(|i| Quat::from_rotation_y(self.feet[i].yaw) * Quat::from_rotation_x(-self.feet[i].pitch));
        // Each arm swings against its own leg: back as that leg comes down ahead. Bent a
        // little at a walk and square at a run, and more as it comes forward.
        let arms = [0, 1].map(|i| {
            let side = i as f32 * 2.0 - 1.0;
            let swing = side * wave * (0.38 + 0.4 * r) * goes;
            let bend = 0.22 + 1.25 * r + swing.max(0.0) * (0.5 - 0.2 * r);
            let out = Quat::from_rotation_z(side * (0.09 + 0.1 * r));
            let shoulder = shoulders + chest * Vec3::X * side * f.shoulders / 2.0;
            let elbow = shoulder + chest * out * Quat::from_rotation_x(-swing + 0.25 * r) * Vec3::NEG_Y * f.upper;
            let along = chest * out * Quat::from_rotation_x(-swing + 0.25 * r - bend) * Vec3::NEG_Y;
            [shoulder, elbow, elbow + along * f.fore, elbow + along * (f.fore + f.hand)]
        });
        Stance { pelvis: (middle, hips), chest: (shoulders, chest), head, legs, feet, arms }
    }

    /// The figure as boxes: 0 trunk, 1 legs, 2 head and hands, 3 arms.
    pub fn boxes(&self) -> Vec<Block> {
        let f = self.frame;
        let s = self.stance();
        let ahead = Quat::from_rotation_y(self.yaw) * Vec3::Z;
        let between = |a: Vec3, b: Vec3, wide: f32, deep: f32, part: u8| {
            let y = (b - a).try_normalize().unwrap_or(Vec3::Y);
            let x = y.cross(ahead).try_normalize().unwrap_or(Vec3::X);
            Block { at: (a + b) / 2.0, turn: Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y))), size: Vec3::new(wide, a.distance(b), deep), part }
        };
        let tall = f.thigh + f.shin;
        let mut out = vec![
            Block { at: s.pelvis.0 + s.pelvis.1 * Vec3::Y * f.torso * 0.12, turn: s.pelvis.1, size: Vec3::new(f.hips * 1.9, f.torso * 0.36, tall * 0.2), part: 0 },
            Block { at: s.chest.0 - s.chest.1 * Vec3::Y * f.torso * 0.3, turn: s.chest.1, size: Vec3::new(f.shoulders * 0.86, f.torso * 0.62, tall * 0.22), part: 0 },
            Block { at: s.head.0, turn: s.head.1, size: Vec3::new(f.head * 0.8, f.head, f.head * 0.9), part: 2 },
        ];
        for i in 0..2 {
            let [hip, knee, ankle] = s.legs[i];
            out.push(between(hip, knee, tall * 0.15, tall * 0.16, 1));
            out.push(between(knee, ankle, tall * 0.11, tall * 0.12, 1));
            out.push(Block { at: ankle + s.feet[i] * Vec3::new(0.0, -f.ankle / 2.0, f.foot * 0.25), turn: s.feet[i], size: Vec3::new(tall * 0.11, f.ankle, f.foot), part: 3 });
            let [shoulder, elbow, wrist, tip] = s.arms[i];
            out.push(between(shoulder, elbow, tall * 0.1, tall * 0.1, 3));
            out.push(between(elbow, wrist, tall * 0.085, tall * 0.085, 3));
            out.push(between(wrist, tip, tall * 0.09, tall * 0.05, 2));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a stretch of going at one speed was like.
    struct Went {
        /// Steps a second; the share of the time with both feet down, and with neither.
        cadence: f32,
        both: f32,
        neither: f32,
        /// The furthest any leg was asked to reach, over its length; the most a foot moved
        /// while it was down; how far the hips rose and fell.
        reach: f32,
        slid: f32,
        bob: f32,
        /// How the left hand's place ahead of the body went with the right foot's (1: as one).
        arms: f32,
    }

    fn go(speed: f32, seconds: f32) -> (Biped, Went) {
        let f = Frame::person(1.75);
        let mut b = Biped::new(f, Vec3::ZERO, 0.0);
        let dt = 1.0 / 120.0;
        let (mut z, mut v, mut t) = (0.0f32, 0.0f32, 0.0f32);
        let (mut both, mut neither, mut n, mut reach, mut slid) = (0, 0, 0, 0.0f32, 0.0f32);
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        let (mut steps, mut pairs) = (0, Vec::new());
        while t < seconds {
            v += (speed - v).clamp(-5.0 * dt, 4.0 * dt);
            z += v * dt;
            t += dt;
            let (was, down) = (b.feet(), b.down());
            b.step(dt, Vec3::Z * z, 0.0, |_, _| 0.0);
            // (Judged once it is up to speed.)
            if (v - speed).abs() > 0.01 || t < 3.0 {
                steps = b.steps.iter().sum::<u32>();
                continue;
            }
            n += 1;
            let s = b.stance();
            for i in 0..2 {
                if down[i] && b.down()[i] {
                    slid = slid.max(was[i].distance(b.feet()[i]));
                }
                reach = reach.max(s.legs[i][0].distance(s.legs[i][2]) / (f.thigh + f.shin));
            }
            both += b.down().iter().all(|d| *d) as u32;
            neither += b.down().iter().all(|d| !*d) as u32;
            (low, high) = (low.min(s.pelvis.0.y), high.max(s.pelvis.0.y));
            pairs.push((s.arms[0][2].z - z, b.feet()[1].z - z));
        }
        let (ma, mb) = (pairs.iter().map(|p| p.0).sum::<f32>() / pairs.len() as f32, pairs.iter().map(|p| p.1).sum::<f32>() / pairs.len() as f32);
        let (ab, aa, bb) = pairs.iter().fold((0.0, 0.0, 0.0), |(ab, aa, bb), p| (ab + (p.0 - ma) * (p.1 - mb), aa + (p.0 - ma).powi(2), bb + (p.1 - mb).powi(2)));
        let seen = n as f32 * dt;
        let went = Went { cadence: (b.steps.iter().sum::<u32>() - steps) as f32 / seen, both: both as f32 / n as f32, neither: neither as f32 / n as f32, reach, slid, bob: high - low, arms: ab / (aa * bb).sqrt().max(1e-9) };
        (b, went)
    }

    #[test]
    fn it_walks_as_a_person_walks() {
        let (_, w) = go(1.4, 12.0);
        // About two steps a second, always a foot down and both between steps; no leg asked
        // past its length; a foot that is down stays put; the hips rise and fall a few
        // centimetres; and each arm goes with the other side's leg.
        assert!((1.6..2.3).contains(&w.cadence), "cadence {:.2} steps/s", w.cadence);
        assert!(w.neither == 0.0 && (0.1..0.4).contains(&w.both), "both down {:.2}, neither {:.2}", w.both, w.neither);
        assert!(w.reach <= 1.0, "a leg reached {:.3} of its length", w.reach);
        assert!(w.slid < 1e-4, "a planted foot moved {:.4} m", w.slid);
        assert!((0.015..0.09).contains(&w.bob), "hips rise and fall {:.3} m", w.bob);
        assert!(w.arms > 0.6, "left hand with right foot {:.2}", w.arms);
    }

    #[test]
    fn it_runs_with_both_feet_off_the_ground_between_steps() {
        let (_, w) = go(4.0, 10.0);
        assert!((2.5..3.6).contains(&w.cadence), "cadence {:.2} steps/s", w.cadence);
        assert!(w.both == 0.0 && w.neither > 0.15, "both down {:.2}, neither {:.2}", w.both, w.neither);
        assert!(w.reach <= 1.0 && w.slid < 1e-4, "reach {:.3}, slid {:.4}", w.reach, w.slid);
        assert!(w.arms > 0.6, "left hand with right foot {:.2}", w.arms);
    }

    #[test]
    fn it_stops_and_stands_with_its_feet_under_it() {
        let f = Frame::person(1.75);
        let mut b = Biped::new(f, Vec3::ZERO, 0.0);
        let dt = 1.0 / 120.0;
        let mut z = 0.0;
        for i in 0..(8.0 / dt) as usize {
            let v = if i as f32 * dt < 4.0 { 1.4 } else { 0.0 };
            z += v * dt;
            b.step(dt, Vec3::Z * z, 0.0, |_, _| 0.0);
        }
        let s = b.stance();
        assert!(b.down() == [true, true], "a foot is still in the air");
        for (i, foot) in b.feet().iter().enumerate() {
            assert!(Vec2::new(foot.x - s.legs[i][0].x, foot.z - z).length() < 0.08, "foot {i} is {:?} with the body at z {z:.2}", foot);
        }
        assert!((s.pelvis.0.y - f.stands()).abs() < 0.02, "hips at {:.3}, stands {:.3}", s.pelvis.0.y, f.stands());
    }
}
