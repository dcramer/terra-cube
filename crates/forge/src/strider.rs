//! A person's model moved by the two-legged walker of `cube_anim::biped`: the figure of
//! boxes' own motion, written down on a real skeleton bone for bone. A humanoid's rig is
//! kept as Tripo made it (its bones are Mixamo's and its skin is sound: `library.rs` shows
//! clips reach it pose for pose); only what moves it is ours. The walker is given this
//! skeleton's own measures, so every joint it places is where the bone can be.
//!
//! A trial (2026-11-12, `[plans.strode]`, the player as `player-own`).

use crate::clips::{self, Clip, FPS};
use crate::rig::{Pose, Skeleton};
use anyhow::{Context, Result, bail};
use cube_anim::biped::{Biped, Frame};
use glam::{Mat3, Quat, Vec3};

pub struct Strider {
    frame: Frame,
    /// Metres to a unit of the file; the point on the ground under its hips as it stands,
    /// and the walker's x (this figure's left) and the way it faces, in the file.
    metres: f32,
    under: Vec3,
    right: Vec3,
    ahead: Vec3,
    /// The bones: hips, the spine's bone that turns the chest, the neck; each leg's thigh,
    /// shin and foot; each arm's upper, fore and hand (left, then right).
    hips: usize,
    spine: usize,
    neck: usize,
    legs: [[usize; 3]; 2],
    arms: [[usize; 3]; 2],
    /// The hips' bone from the middle of the two hip joints, as it stands.
    offset: Vec3,
    /// Each hand as the model was made: which way its palm faces and its fingers point, and
    /// the bones of its four fingers.
    palms: [(Vec3, Vec3); 2],
    fingers: [Vec<usize>; 2],
}

fn rot(m: &glam::Mat4) -> Quat {
    m.to_scale_rotation_translation().1
}

impl Strider {
    /// `height`: how tall the person is, metres.
    pub fn new(sk: &Skeleton, height: f32) -> Result<Self> {
        let bone = |n: &str| sk.joints.iter().position(|j| j.name.rsplit(':').next() == Some(n)).with_context(|| format!("no bone `{n}`: is this a humanoid's skeleton (`rig = \"humanoid\"`)?"));
        let at = |j: usize| sk.joints[j].world.w_axis.truncate();
        let side = |s: &str| -> Result<([usize; 3], [usize; 3])> { Ok(([bone(&format!("{s}UpLeg"))?, bone(&format!("{s}Leg"))?, bone(&format!("{s}Foot"))?], [bone(&format!("{s}Arm"))?, bone(&format!("{s}ForeArm"))?, bone(&format!("{s}Hand"))?])) };
        let ((ll, la), (rl, ra)) = (side("Left")?, side("Right")?);
        let hips = bone("Hips")?;
        if hips != sk.root {
            bail!("the hips are not the skeleton's root");
        }
        let (floor, top) = sk.joints.iter().fold((f32::MAX, f32::MIN), |(lo, hi), j| (lo.min(j.world.w_axis.y), hi.max(j.world.w_axis.y)));
        // (The highest joint is the head's, short of the crown.)
        let metres = height / ((top - floor) * 1.06);
        let m = |a: Vec3, b: Vec3| a.distance(b) * metres;
        let person = Frame::person(height);
        let (seat, shoulders) = ((at(ll[0]) + at(rl[0])) / 2.0, (at(la[0]) + at(ra[0])) / 2.0);
        let frame = Frame { thigh: m(at(ll[0]), at(ll[1])), shin: m(at(ll[1]), at(ll[2])), ankle: (at(ll[2]).y - floor) * metres, hips: m(at(ll[0]), at(rl[0])), torso: (shoulders.y - seat.y) * metres, shoulders: m(at(la[0]), at(ra[0])), upper: m(at(la[0]), at(la[1])), fore: m(at(la[1]), at(la[2])), ..person };
        // The walker's own frame has its first leg at -x and looks along +z: turned into the
        // file (and not mirrored into it), its x is this figure's left and its first leg this
        // figure's right. A stride is the same either way round. (With x as the figure's right
        // it faced backwards: it leaned back and swung its arms behind it.)
        let right = (at(rl[0]) - at(ll[0])).with_y(0.0).normalize_or(Vec3::X);
        // A hand's palm, from where its fingers leave it: across them and along them. (The
        // model stands with its arms out; which way its palms face there is its maker's choice.)
        let hand = |s: &str, mirror: f32| -> Result<((Vec3, Vec3), Vec<usize>)> {
            let (wrist, index, pinky, middle) = (at(bone(&format!("{s}Hand"))?), at(bone(&format!("{s}HandIndex1"))?), at(bone(&format!("{s}HandPinky1"))?), at(bone(&format!("{s}HandMiddle1"))?));
            let along = (middle - wrist).normalize_or(Vec3::X);
            let fingers = ["Index", "Middle", "Ring", "Pinky"].iter().flat_map(|f| (1..=3).map(move |n| format!("{s}Hand{f}{n}"))).filter_map(|n| bone(&n).ok()).collect();
            Ok(((along.cross(index - pinky).normalize_or(Vec3::NEG_Y) * mirror, along), fingers))
        };
        let ((lp, lf), (rp, rf)) = (hand("Left", 1.0)?, hand("Right", -1.0)?);
        let (ll, la, rl, ra, palms, fingers) = (rl, ra, ll, la, [rp, lp], [rf, lf]);
        Ok(Self { frame, metres, under: seat.with_y(floor), right: -right, ahead: Vec3::Y.cross(right), hips, spine: bone("Spine1").or_else(|_| bone("Spine"))?, neck: bone("Neck")?, legs: [ll, rl], arms: [la, ra], offset: at(hips) - seat, palms, fingers })
    }

    /// The skeleton as the walker has it at one moment, less how far it has gone.
    fn pose(&self, sk: &Skeleton, walker: &Biped, gone: f32) -> Pose {
        let s = walker.stance();
        let file = |w: Vec3| self.under + (self.right * w.x + Vec3::Y * w.y + self.ahead * (w.z - gone)) / self.metres;
        let basis = Quat::from_mat3(&Mat3::from_cols(self.right, Vec3::Y, self.ahead));
        let q = |w: Quat| (basis * w * basis.inverse()).normalize();
        let mut pose = sk.rest();
        let (pelvis, chest, head) = (q(s.pelvis.1), q(s.chest.1), q(s.head.1));
        clips::turn_now(sk, &mut pose, self.hips, pelvis);
        clips::turn_now(sk, &mut pose, self.spine, chest * pelvis.inverse());
        clips::turn_now(sk, &mut pose, self.neck, head * chest.inverse());
        pose.root_t = sk.base.inverse().transform_point3(file(s.pelvis.0) + pelvis * self.offset);
        // Each bone pointed from its joint to the next of the walker's.
        let aim = |pose: &mut Pose, from: usize, to: usize, way: Vec3| {
            let world = sk.pose_world(pose);
            let now = world[to].w_axis.truncate() - world[from].w_axis.truncate();
            if let (Some(now), Some(way)) = (now.try_normalize(), way.try_normalize()) {
                clips::turn_now(sk, pose, from, Quat::from_rotation_arc(now, way));
            }
        };
        for i in 0..2 {
            let ([thigh, shin, foot], [hip, knee, ankle]) = (self.legs[i], s.legs[i].map(file));
            aim(&mut pose, thigh, shin, knee - hip);
            aim(&mut pose, shin, foot, ankle - knee);
            // The foot turned as the walker's is, from flat as it stood.
            let turned = rot(&sk.pose_world(&pose)[foot]) * rot(&sk.joints[foot].world).inverse();
            clips::turn_now(sk, &mut pose, foot, q(s.feet[i]) * turned.inverse());
            let ([upper, fore, hand], [shoulder, elbow, wrist, _]) = (self.arms[i], s.arms[i].map(file));
            aim(&mut pose, upper, fore, elbow - shoulder);
            aim(&mut pose, fore, hand, wrist - elbow);
            // The forearm turned about itself until the palm faces the body, as a hand hangs
            // and swings; and the fingers curled a little, as a hand at ease has them. (Left
            // as the model was made, arms out and fingers spread, they were carried like plates.)
            let turned = rot(&sk.pose_world(&pose)[hand]) * rot(&sk.joints[hand].world).inverse();
            let (palm, along) = (turned * self.palms[i].0, (wrist - elbow).normalize_or(Vec3::NEG_Y));
            let inward = basis * (s.chest.1 * Vec3::X * if i == 0 { 1.0 } else { -1.0 });
            let flat = |v: Vec3| (v - along * v.dot(along)).normalize_or(Vec3::X);
            let (from, to) = (flat(palm), flat(inward));
            let twist = Quat::from_axis_angle(along, from.cross(to).dot(along).atan2(from.dot(to)));
            clips::turn_now(sk, &mut pose, fore, twist);
            let curl = (twist * turned * self.palms[i].1.cross(self.palms[i].0)).normalize_or(Vec3::Z);
            for &finger in &self.fingers[i] {
                clips::turn_now(sk, &mut pose, finger, Quat::from_axis_angle(curl, 0.4));
            }
        }
        pose
    }

    /// One of its clips, made by the walker: standing, or a stride at a walk, run or sprint.
    pub fn clip(&self, name: &str, sk: &Skeleton) -> Result<Clip> {
        let size = ((self.frame.thigh + self.frame.shin) / 0.86).sqrt();
        let pace = size * match name {
            // (A walk as people walk. The run and sprint are what the game has had a person
            // go at since 2026-11-12, kept so that nothing else about the game changes.)
            "walk" => 1.4,
            "run" => 5.2,
            "sprint" => 6.5,
            "idle" => 0.0,
            _ => bail!("the two-legged walker has no `{name}` yet (it stands, walks, runs and sprints)"),
        };
        let mut walker = Biped::new(self.frame, Vec3::ZERO, 0.0);
        let dt = 1.0 / (FPS * 4.0);
        if pace == 0.0 {
            for _ in 0..(2.0 / dt) as usize {
                walker.step(dt, Vec3::ZERO, 0.0, false, |_, _| 0.0);
            }
            return Ok(Clip { name: name.into(), looping: true, frames: vec![self.pose(sk, &walker, 0.0); (FPS * 4.0) as usize + 1], source: "walker", speed: None });
        }
        // Walked until it is in its stride, then one stride kept: from one left footfall to the next.
        let (mut gone, mut speed, mut began, mut frames) = (0.0f32, 0.0f32, None, Vec::new());
        for tick in 0..(16.0 / dt) as usize {
            speed += (pace - speed).min(4.0 * dt);
            gone += speed * dt;
            let before = walker.phase;
            walker.step(dt, Vec3::Z * gone, 0.0, false, |_, _| 0.0);
            let wrapped = walker.phase < before;
            if began.is_none() && wrapped && tick as f32 * dt > 5.0 {
                began = Some(tick);
            } else if began.is_some() && wrapped {
                break;
            }
            if let Some(from) = began && (tick - from).is_multiple_of(4) {
                frames.push(self.pose(sk, &walker, gone));
            }
        }
        if frames.len() < 4 {
            bail!("the walker took no stride at {pace:.1} m/s");
        }
        frames.push(frames[0].clone());
        clips::close_loop(&mut frames);
        Ok(Clip { name: name.into(), looping: true, frames, source: "walker", speed: Some(pace / self.metres) })
    }
}
