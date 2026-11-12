//! Feet are put on the ground that is there. A clip is made for flat ground and the ground
//! is blocks and slopes: a foot the clip sets down on the level is in the air on the downhill
//! side and in the hill on the other. After the clip has posed the body and before the pose
//! is used, each leg is bent (two bones, the knee the way it already bends) so its foot is as
//! far above the ground under it as the clip had it above the level, and the body is let
//! down as far as its lowest foot has to reach.
//!
//! Which bones are a leg is forge's to say, for every creature alike (`leg0`, `leg1`… in its
//! `meta.toml`, from the roles it reads off the skeleton): nothing here knows a biped from a
//! deer.

use crate::creatures::{Creature, Registry};
use crate::motion::{Live, Mover};
use crate::world::Terrain;
use bevy::math::Affine3A;
use bevy::prelude::*;

pub struct FeetPlugin;

impl Plugin for FeetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, find).add_systems(PostUpdate, plant.after(bevy::app::AnimationSystems).before(bevy::transform::TransformSystems::Propagate));
    }
}

/// A creature's legs: each its hip, its knee and the joint that stands on the ground; the bone
/// its whole body hangs from; and how much of the planting is on, 0 to 1 (eased, so leaving
/// the ground and coming back to it are not a snap).
#[derive(Component)]
pub struct Legs {
    pub legs: Vec<[Entity; 3]>,
    root: Option<Entity>,
    on: f32,
}

/// How far a foot may be moved up or down from where the clip has it, in the creature's heights.
const REACH: f32 = 0.17;
/// Feet are planted for what is this near the eye, metres: further off nobody can tell.
const NEAR: f32 = 30.0;

/// Finds each new creature's leg bones by the names forge wrote.
fn find(mut commands: Commands, registry: Res<Registry>, creatures: Query<(Entity, &Creature), (With<Mover>, Without<Legs>)>, children: Query<&Children>, names: Query<&Name>) {
    for (entity, creature) in &creatures {
        let bones = &registry.0[creature.def].meta.bones;
        if children.iter_descendants(entity).next().is_none() {
            continue;
        }
        // (A name that is on two bones is no use: forge leaves such a leg out.)
        let named = |name: &str| {
            let mut found = children.iter_descendants(entity).filter(|e| names.get(*e).is_ok_and(|n| n.as_str() == name));
            found.next().filter(|_| found.next().is_none())
        };
        let legs: Vec<[Entity; 3]> = (0..8)
            .filter_map(|i| bones.get(&format!("leg{i}")))
            .filter_map(|chain| {
                let mut parts = chain.split('>').map(named);
                Some([parts.next()??, parts.next()??, parts.next()??])
            })
            .collect();
        commands.entity(entity).insert(Legs { legs, root: bones.get("root").and_then(|n| named(n)), on: 0.0 });
    }
}

/// Where a bone is in the world this frame: its own place under each of its parents' in turn,
/// up to the creature, whose place is already this frame's (the bones' own are last frame's
/// until the engine works them out, after this).
fn world_of(bone: Entity, top: Entity, top_at: &Transform, parents: &Query<&ChildOf>, poses: &Query<&mut Transform, Without<Mover>>) -> Option<Affine3A> {
    let mut chain = Vec::new();
    let mut at = bone;
    while at != top {
        chain.push(poses.get(at).ok()?.compute_affine());
        at = parents.get(at).ok()?.parent();
        if chain.len() > 64 {
            return None;
        }
    }
    Some(chain.iter().rev().fold(top_at.compute_affine(), |world, local| world * *local))
}

fn turn_of(world: &Affine3A) -> Quat {
    Quat::from_mat3a(&bevy::math::Mat3A::from_cols(world.matrix3.x_axis.normalize(), world.matrix3.y_axis.normalize(), world.matrix3.z_axis.normalize()))
}

/// Turns a bone by `turn`, a turn in the world, from where its pose has it.
fn turn_bone(bone: Entity, turn: Quat, top: Entity, top_at: &Transform, parents: &Query<&ChildOf>, poses: &mut Query<&mut Transform, Without<Mover>>) -> Option<()> {
    let parent = parents.get(bone).ok()?.parent();
    let frame = if parent == top { top_at.rotation } else { turn_of(&world_of(parent, top, top_at, parents, poses)?) };
    let mut pose = poses.get_mut(bone).ok()?;
    pose.rotation = (frame.inverse() * turn * frame * pose.rotation).normalize();
    Some(())
}

/// The height of the ground as it is drawn under a point, looked for a little above and below
/// the height given.
pub fn ground_at(terrain: &Terrain, x: f32, z: f32, near: f32) -> Option<f32> {
    let level = (near / cube_voxel::CELL).round() as i32;
    [0, 1, -1, 2, -2].into_iter().find_map(|d| {
        let cell = IVec3::new((x / cube_voxel::CELL).floor() as i32, level + d, (z / cube_voxel::CELL).floor() as i32);
        (terrain.0.get(cell - IVec3::Y).ground() && !terrain.0.get(cell).ground()).then(|| cube_voxel::mesh::eased_under(&terrain.0, Vec3::new(x, (level + d) as f32 * cube_voxel::CELL, z))).flatten()
    })
}

#[allow(clippy::too_many_arguments)]
fn plant(
    time: Res<Time>,
    args: Res<crate::Args>,
    live: Option<Res<Live>>,
    terrain: Option<Res<Terrain>>,
    registry: Res<Registry>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut creatures: Query<(Entity, &Mover, &Creature, &Transform, &mut Legs)>,
    parents: Query<&ChildOf>,
    mut poses: Query<&mut Transform, Without<Mover>>,
) {
    let (Some(live), Some(terrain)) = (live, terrain) else { return };
    if args.loose_feet {
        return;
    }
    let eye = cameras.iter().next().map(|c| c.translation());
    for (entity, mover, creature, at, mut legs) in &mut creatures {
        let Some(agent) = live.agent(mover) else { continue };
        if legs.legs.is_empty() || eye.is_some_and(|e| e.distance(at.translation) > NEAR) {
            continue;
        }
        // Only what stands on the ground has its feet put on it; the change is eased.
        let standing = agent.body.grounded && !agent.body.swimming && agent.dead.is_none();
        legs.on = (legs.on + (if standing { 1.0 } else { -1.0 }) * time.delta_secs() * 6.0).clamp(0.0, 1.0);
        if legs.on <= 0.0 {
            continue;
        }
        let reach = registry.0[creature.def].meta.height * REACH * at.scale.y;
        // Where the clip has each foot, and how far the ground there is from the level the
        // clip was made for (the height the creature itself is drawn at).
        let mut wants = Vec::with_capacity(legs.legs.len());
        for [_, _, tip] in &legs.legs {
            let Some(foot) = world_of(*tip, entity, at, &parents, &poses).map(|w| Vec3::from(w.translation)) else { continue };
            let off = ground_at(&terrain, foot.x, foot.z, at.translation.y).map_or(0.0, |g| (g - at.translation.y).clamp(-reach, reach)) * legs.on;
            wants.push(off);
        }
        if wants.len() != legs.legs.len() {
            continue;
        }
        // The body comes down as far as its lowest foot must go: a leg cannot be made longer.
        let drop = wants.iter().copied().fold(0.0f32, f32::min);
        if let Some(root) = legs.root.filter(|_| drop < -1e-4)
            && let Some(parent) = parents.get(root).ok().map(|p| p.parent())
            && let Some(frame) = if parent == entity { Some(at.compute_affine()) } else { world_of(parent, entity, at, &parents, &poses) }
            && let Ok(mut pose) = poses.get_mut(root)
        {
            pose.translation += Vec3::from(frame.matrix3.inverse() * bevy::math::Vec3A::from(Vec3::Y * drop));
        }
        for ([hip, knee, tip], off) in legs.legs.clone().into_iter().zip(wants) {
            let rise = off - drop;
            if rise.abs() < 1e-4 {
                continue;
            }
            let (Some(a), Some(b), Some(p)) = (
                world_of(hip, entity, at, &parents, &poses).map(|w| Vec3::from(w.translation)),
                world_of(knee, entity, at, &parents, &poses).map(|w| Vec3::from(w.translation)),
                world_of(tip, entity, at, &parents, &poses).map(|w| Vec3::from(w.translation)),
            ) else {
                continue;
            };
            let target = p + Vec3::Y * rise;
            let (upper, lower) = (a.distance(b), b.distance(p));
            if upper < 1e-5 || lower < 1e-5 {
                continue;
            }
            // The knee bends, the way it is bent already, until the foot is as far from the
            // hip as it has to be; then the whole leg swings to put it there.
            let far = a.distance(target).clamp((upper - lower).abs() + 1e-4, (upper + lower) * 0.999);
            let now = (a - b).angle_between(p - b);
            let want = ((upper * upper + lower * lower - far * far) / (2.0 * upper * lower)).clamp(-1.0, 1.0).acos();
            let axis = (a - b).cross(p - b).try_normalize().unwrap_or(at.rotation * Vec3::X);
            let bend = Quat::from_axis_angle(axis, want - now);
            if turn_bone(knee, bend, entity, at, &parents, &mut poses).is_none() {
                continue;
            }
            let bent = b + bend * (p - b);
            let (Some(from), Some(to)) = ((bent - a).try_normalize(), (target - a).try_normalize()) else { continue };
            turn_bone(hip, Quat::from_rotation_arc(from, to), entity, at, &parents, &mut poses);
        }
    }
}
