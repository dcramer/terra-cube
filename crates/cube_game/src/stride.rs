//! A person moved by the two-legged walker as the game runs (`cube_anim::biped`): after the
//! clip has posed the body and before the pose is used, the skeleton is stood as forge says
//! it rests and every bone the walker knows is turned to where the walker has it. So the
//! feet are planted on the ground that is there at whatever speed the body is going, a turn
//! or a stop or a leap is the walker's own, and nothing is mixed from one clip to another.
//!
//! What the walker does not do is still a clip's: a blow and a flinch from the chest up, over
//! the walker's legs (so a blow can be struck at a run); dying and swimming, the whole body.
//!
//! Only for what forge marks (`[stride]` in its `meta.toml`: `walked = [..]` in its
//! `creature.toml`). Proven on a figure of boxes first (`make person`).

use crate::creatures::{Creature, Registry};
use crate::feet::{turn_bone, world_of};
use crate::motion::{Live, Mover};
use crate::world::Terrain;
use bevy::prelude::*;
use cube_anim::biped::{Biped, Frame};
use cube_sim::life::Act;

pub struct StridePlugin;

impl Plugin for StridePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, find).add_systems(PostUpdate, pose.after(crate::feet::plant).before(bevy::transform::TransformSystems::Propagate));
    }
}

/// A body the walker moves: every bone forge gave a rest for, with that rest and whether it
/// is of the upper body; the bones the walker turns; and, once it has been measured standing,
/// the walker and what was measured.
#[derive(Component)]
pub struct Strode {
    rests: Vec<(Entity, Quat, bool)>,
    seat: Vec3,
    hips: Entity,
    spine: Entity,
    neck: Entity,
    /// The walker's first leg and arm are this figure's right.
    legs: [[Entity; 3]; 2],
    arms: [[Entity; 3]; 2],
    knuckles: [[Entity; 3]; 2],
    fitted: Option<Fitted>,
    /// How much of the clip is let through: from the chest up, and over the whole body.
    upper: f32,
    whole: f32,
    /// Seconds it has been off the ground.
    off: f32,
}

struct Fitted {
    walker: Biped,
    /// How the hips, the chest's bone, the neck and each foot are turned as it stands, in the
    /// model's own frame; the hips' bone from the middle of the hip joints; each palm's
    /// facing in its hand's frame.
    stood: [Quat; 5],
    offset: Vec3,
    palms: [Vec3; 2],
}

fn find(mut commands: Commands, args: Res<crate::Args>, registry: Res<Registry>, creatures: Query<(Entity, &Creature), (With<Mover>, Without<Strode>)>, children: Query<&Children>, names: Query<&Name>) {
    if args.clips_only {
        return;
    }
    for (entity, creature) in &creatures {
        let Some(stride) = registry.0.get(creature.def).and_then(|d| d.meta.stride.as_ref()) else { continue };
        let all: Vec<(Entity, &str)> = children.iter_descendants(entity).filter_map(|e| Some((e, names.get(e).ok()?.as_str()))).collect();
        let exact = |name: &str| all.iter().find(|(_, n)| *n == name).map(|(e, _)| *e);
        let bone = |name: &str| all.iter().find(|(_, n)| n.rsplit(':').next() == Some(name)).map(|(e, _)| *e);
        let Some(rests) = stride.bones.iter().zip(&stride.rests).map(|(n, q)| Some((exact(n)?, Quat::from_array(*q), false))).collect::<Option<Vec<_>>>() else { continue };
        let side = |s: &str| Some(([bone(&format!("{s}UpLeg"))?, bone(&format!("{s}Leg"))?, bone(&format!("{s}Foot"))?], [bone(&format!("{s}Arm"))?, bone(&format!("{s}ForeArm"))?, bone(&format!("{s}Hand"))?], [bone(&format!("{s}HandIndex1"))?, bone(&format!("{s}HandMiddle1"))?, bone(&format!("{s}HandPinky1"))?]));
        let (Some((rl, ra, rk)), Some((ll, la, lk)), Some(hips), Some(spine), Some(neck)) = (side("Right"), side("Left"), bone("Hips"), bone("Spine1").or_else(|| bone("Spine")), bone("Neck")) else { continue };
        // (The upper body: the spine's first bone and all that hangs from it.)
        let chest = bone("Spine").unwrap_or(spine);
        let above: Vec<Entity> = std::iter::once(chest).chain(children.iter_descendants(chest)).collect();
        let rests = rests.into_iter().map(|(e, q, _)| (e, q, above.contains(&e))).collect();
        commands.entity(entity).insert(Strode { rests, seat: Vec3::from_array(stride.seat), hips, spine, neck, legs: [rl, ll], arms: [ra, la], knuckles: [rk, lk], fitted: None, upper: 0.0, whole: 0.0, off: 0.0 });
    }
}

fn turned(world: &bevy::math::Affine3A) -> Quat {
    Quat::from_mat3a(&bevy::math::Mat3A::from_cols(world.matrix3.x_axis.normalize(), world.matrix3.y_axis.normalize(), world.matrix3.z_axis.normalize()))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn pose(time: Res<Time>, live: Option<Res<Live>>, terrain: Option<Res<Terrain>>, mut creatures: Query<(Entity, &Mover, &Transform, &mut Strode)>, parents: Query<&ChildOf>, mut poses: Query<&mut Transform, Without<Mover>>) {
    let (Some(live), Some(terrain)) = (live, terrain) else { return };
    let dt = time.delta_secs();
    for (entity, mover, top, mut strode) in &mut creatures {
        let Some(agent) = live.agent(mover) else { continue };
        let strode = &mut *strode;
        // What the clip has, kept for what is the clip's; then the skeleton as it rests.
        let Some(clip) = strode.rests.iter().map(|(e, _, _)| poses.get(*e).ok().map(|p| p.rotation)).collect::<Option<Vec<Quat>>>() else { continue };
        let Ok(clip_seat) = poses.get(strode.hips).map(|p| p.translation) else { continue };
        for (bone, rest, _) in &strode.rests {
            if let Ok(mut pose) = poses.get_mut(*bone) {
                pose.rotation = *rest;
            }
        }
        if let Ok(mut pose) = poses.get_mut(strode.hips) {
            pose.translation = strode.seat;
        }
        let place = |bone: Entity, poses: &Query<&mut Transform, Without<Mover>>| world_of(bone, entity, top, &parents, poses).map(|w| (Vec3::from(w.translation), turned(&w)));
        let facing = top.rotation * Vec3::Z;
        let yaw = facing.x.atan2(facing.z);
        if strode.fitted.is_none() {
            // Measured as it stands: the walker's person is this one.
            let at = |bone: Entity| place(bone, &poses);
            let (Some(hips), Some(spine), Some(neck)) = (at(strode.hips), at(strode.spine), at(strode.neck)) else { continue };
            let Some(joints) = [0, 1].map(|i| Some((strode.legs[i].map(&at), strode.arms[i].map(&at), strode.knuckles[i].map(&at)))).into_iter().collect::<Option<Vec<_>>>() else { continue };
            let Some(joints) = joints.into_iter().map(|(l, a, k)| Some(([l[0]?, l[1]?, l[2]?], [a[0]?, a[1]?, a[2]?], [k[0]?.0, k[1]?.0, k[2]?.0]))).collect::<Option<Vec<_>>>() else { continue };
            let ((rl, ra, rk), (ll, la, lk)) = (joints[0], joints[1]);
            let tall = (neck.0.y - top.translation.y) / 0.87;
            let (seat, shoulders) = ((rl[0].0 + ll[0].0) / 2.0, (ra[0].0 + la[0].0) / 2.0);
            let frame = Frame { thigh: rl[0].0.distance(rl[1].0), shin: rl[1].0.distance(rl[2].0), ankle: rl[2].0.y - top.translation.y, hips: rl[0].0.distance(ll[0].0), torso: shoulders.y - seat.y, shoulders: ra[0].0.distance(la[0].0), upper: ra[0].0.distance(ra[1].0), fore: ra[1].0.distance(ra[2].0), ..Frame::person(tall) };
            let own = top.rotation.inverse();
            // (A palm faces across its fingers and its knuckles: which side, by which hand.)
            let palm = |hand: (Vec3, Quat), [index, middle, pinky]: [Vec3; 3], mirror: f32| {
                let along = (middle - hand.0).normalize_or(Vec3::X);
                hand.1.inverse() * (along.cross(index - pinky).normalize_or(Vec3::NEG_Y) * mirror)
            };
            strode.fitted = Some(Fitted { walker: Biped::new(frame, top.translation, yaw), stood: [own * hips.1, own * spine.1, own * neck.1, own * rl[2].1, own * ll[2].1], offset: own * (hips.0 - seat), palms: [palm(ra[2], rk, -1.0), palm(la[2], lk, 1.0)] });
        }
        let Strode { fitted: Some(fitted), .. } = strode else { continue };
        // In the air only once it has been off the ground a moment: a body crossing a step
        // is off it for a tick or two.
        strode.off = if agent.body.grounded || agent.act == Act::Swim { 0.0 } else { strode.off + dt };
        if dt > 0.0 {
            // (Put somewhere else at a stroke, it is stood there and not walked there.)
            if fitted.walker.at.distance(top.translation) > 2.5 {
                fitted.walker = Biped::new(fitted.walker.frame, top.translation, yaw);
            }
            let near = top.translation.y;
            fitted.walker.step(dt, top.translation, yaw, strode.off > 0.07, |x, z| crate::feet::ground_at(&terrain, x, z, near).filter(|g| (g - near).abs() < 0.6).unwrap_or(near));
        }
        let s = fitted.walker.stance();
        // `STRIDE_TRACE=1`: each moment a knee or an ankle of the walker's is under the ground
        // drawn there, which and by how much, and what the walker was doing.
        if std::env::var_os("STRIDE_TRACE").is_some() {
            for (i, leg) in s.legs.iter().enumerate() {
                for (k, joint) in leg.iter().enumerate().skip(1) {
                    if let Some(ground) = crate::feet::ground_at(&terrain, joint.x, joint.z, top.translation.y).filter(|g| *g > joint.y + 0.03) {
                        println!("stride: leg {i} {} {:.0} cm under · down {:?} · phase {:.2} · body {:.2} foot {:.2} ground-at-foot {:.2}", ["hip", "knee", "ankle"][k], (ground - joint.y) * 100.0, fitted.walker.down(), fitted.walker.phase, top.translation.y, fitted.walker.feet()[i].y, crate::feet::ground_at(&terrain, fitted.walker.feet()[i].x, fitted.walker.feet()[i].z, top.translation.y).unwrap_or(f32::NAN));
                    }
                }
            }
        }
        // The hips, the chest and the head, each turned as the walker has it.
        let set = |bone: Entity, to: Quat, poses: &mut Query<&mut Transform, Without<Mover>>| {
            if let Some((_, now)) = world_of(bone, entity, top, &parents, poses).map(|w| (0, turned(&w))) {
                turn_bone(bone, to * now.inverse(), entity, top, &parents, poses);
            }
        };
        set(strode.hips, s.pelvis.1 * fitted.stood[0], &mut poses);
        let under = parents.get(strode.hips).ok().map(|p| p.parent()).and_then(|p| if p == entity { Some(top.compute_affine()) } else { world_of(p, entity, top, &parents, &poses) });
        if let (Some(under), Ok(mut pose)) = (under, poses.get_mut(strode.hips)) {
            pose.translation = under.inverse().transform_point3(s.pelvis.0 + s.pelvis.1 * fitted.offset);
        }
        set(strode.spine, s.chest.1 * fitted.stood[1], &mut poses);
        set(strode.neck, s.head.1 * fitted.stood[2], &mut poses);
        // Each bone of a limb pointed from its joint to the walker's next.
        let aim = |from: Entity, to: Entity, way: Vec3, poses: &mut Query<&mut Transform, Without<Mover>>| {
            let (Some(a), Some(b)) = (world_of(from, entity, top, &parents, poses), world_of(to, entity, top, &parents, poses)) else { return };
            if let (Some(now), Some(way)) = ((Vec3::from(b.translation) - Vec3::from(a.translation)).try_normalize(), way.try_normalize()) {
                turn_bone(from, Quat::from_rotation_arc(now, way), entity, top, &parents, poses);
            }
        };
        for i in 0..2 {
            let ([thigh, shin, foot], [hip, knee, ankle]) = (strode.legs[i], s.legs[i]);
            aim(thigh, shin, knee - hip, &mut poses);
            aim(shin, foot, ankle - knee, &mut poses);
            set(foot, s.feet[i] * fitted.stood[3 + i], &mut poses);
            let ([upper, fore, hand], [shoulder, elbow, wrist, _]) = (strode.arms[i], s.arms[i]);
            aim(upper, fore, elbow - shoulder, &mut poses);
            aim(fore, hand, wrist - elbow, &mut poses);
            // The forearm turned about itself until the palm faces the body.
            if let (Some(held), Some(along)) = (world_of(hand, entity, top, &parents, &poses), (wrist - elbow).try_normalize()) {
                let level = |v: Vec3| (v - along * v.dot(along)).try_normalize();
                if let (Some(from), Some(to)) = (level(turned(&held) * fitted.palms[i]), level(s.chest.1 * Vec3::X * if i == 0 { 1.0 } else { -1.0 })) {
                    turn_bone(fore, Quat::from_axis_angle(along, from.cross(to).dot(along).atan2(from.dot(to))), entity, top, &parents, &mut poses);
                }
            }
        }
        // What is the clip's: a blow or a flinch from the chest up; dying or swimming, all of it.
        let blow = agent.swing.is_some() || agent.doing.is_some_and(|d| d.0 == Act::Attack) || agent.act == Act::Hurt;
        let all = agent.dead.is_some() || matches!(agent.act, Act::Swim | Act::Die | Act::Lie | Act::Eat);
        let ease = 1.0 - (-12.0 * dt).exp();
        strode.upper += (blow as u8 as f32 - strode.upper) * ease;
        strode.whole += (all as u8 as f32 - strode.whole) * ease;
        for ((bone, _, above), theirs) in strode.rests.iter().zip(&clip) {
            let much = if *above { strode.upper.max(strode.whole) } else { strode.whole };
            if much > 0.001 && let Ok(mut pose) = poses.get_mut(*bone) {
                pose.rotation = pose.rotation.slerp(*theirs, much);
            }
        }
        if strode.whole > 0.001 && let Ok(mut pose) = poses.get_mut(strode.hips) {
            pose.translation = pose.translation.lerp(clip_seat, strode.whole);
        }
    }
}
