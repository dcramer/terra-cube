//! A four-legged body of boxes moved by its skeleton and its feet, with no clips
//! (`cube_anim`): `--limbs` puts one in a scene and sends it round a ring about the place
//! looked at, standing, walking, running, slowing, turning about, lying down and getting up,
//! so that all of it is seen in one moving picture. A trial of the way of moving things,
//! on cubes, before it is asked of any real model.

use bevy::prelude::*;
use cube_anim::{Build, Walker};

pub struct LimbsPlugin;

impl Plugin for LimbsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (walk, adopt)).add_systems(PostUpdate, (pose, wild).after(bevy::app::AnimationSystems).before(bevy::transform::TransformSystems::Propagate));
    }
}

#[derive(Component)]
struct Part(usize);

/// A real creature moved the same way: its model stood still by its idle clip, then its whole
/// body put where the walker's trunk is and each leg bent to the walker's foot. Its walker is
/// fitted to it once its bones are there: where its legs leave it, how long their bones are.
#[derive(Component, Default)]
pub struct Real {
    walker: Option<Walker>,
    /// Each leg's hip, knee and end, in the walker's order; how high the end stands over the
    /// ground at rest; and where the trunk's middle is in the model as it stands.
    legs: Vec<[Entity; 3]>,
    ankle: Vec<f32>,
    middle: Vec3,
    /// How big it was when fitted (the young are small), and whether its legs could not be found.
    size: f32,
    lost: bool,
}

/// After its clip has stood it and before the pose is used: the model is moved by its feet.
#[allow(clippy::too_many_arguments)]
fn pose(
    args: Res<crate::Args>,
    time: Res<Time>,
    terrain: Option<Res<crate::world::Terrain>>,
    trial: Option<Res<Trial>>,
    registry: Res<crate::creatures::Registry>,
    mut reals: Query<(Entity, &crate::creatures::Creature, &mut Real)>,
    children: Query<&Children>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    mut poses: Query<&mut Transform, Without<crate::motion::Mover>>,
    cameras: Query<Entity, With<Camera3d>>,
    mut eye: Local<Option<Vec3>>,
) {
    let (Some(terrain), Some(trial)) = (terrain, trial) else { return };
    let ground = |x: f32, z: f32| cube_voxel::mesh::eased_under(&terrain.0, Vec3::new(x, terrain.ground(x, z), z)).unwrap_or_else(|| terrain.ground(x, z));
    // (It goes round the same ring as the boxes, half a turn behind them.)
    let turn = trial.1 + std::f32::consts::PI;
    let (x, z) = (args.at.0 + turn.sin() * RING, args.at.1 + turn.cos() * RING);
    let (at, yaw) = (Vec3::new(x, ground(x, z), z), turn + std::f32::consts::FRAC_PI_2);
    for (entity, creature, mut real) in &mut reals {
        let Ok(stood) = poses.get(entity).copied() else { continue };
        if real.walker.is_none() {
            // Its legs, by the names forge wrote for where each folds; as it stands now.
            let bones = &registry.0[creature.def].meta.bones;
            let named = |name: &str| children.iter_descendants(entity).find(|e| names.get(*e).is_ok_and(|n| n.as_str() == name));
            let found: Vec<[Entity; 3]> = (0..4).filter_map(|i| bones.get(&format!("fold{i}"))).filter_map(|chain| {
                let mut parts = chain.split('>').map(&named);
                Some([parts.next()??, parts.next()??, parts.next()??])
            }).collect();
            if found.len() != 4 {
                if std::env::var_os("LIMBS_TRACE").is_some() {
                    println!("limbs: {} of 4 legs found by name ({} bones under it, {} fold names)", found.len(), children.iter_descendants(entity).count(), (0..4).filter(|i| bones.contains_key(&format!("fold{i}"))).count());
                }
                continue;
            }
            let upright = Transform::from_translation(stood.translation);
            let place = |bone: Entity| crate::feet::world_of(bone, entity, &upright, &parents, &poses).map(|w| Vec3::from(w.translation) - stood.translation);
            let Some(mut legs) = found.iter().map(|[hip, knee, end]| Some(([*hip, *knee, *end], place(*hip)?, place(*knee)?, place(*end)?))).collect::<Option<Vec<_>>>() else { continue };
            // (Fore before hind, and of each pair the one on -x first: the walker's order.)
            legs.sort_by(|a, b| b.1.z.total_cmp(&a.1.z));
            legs[..2].sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
            legs[2..].sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
            let hips: [Vec3; 4] = std::array::from_fn(|i| legs[i].1);
            let lengths: [(f32, f32); 4] = std::array::from_fn(|i| (legs[i].1.distance(legs[i].2), legs[i].2.distance(legs[i].3)));
            // (The walker's foot is on the ground and the leg's end is the joint over it: the
            // walker's hip is taken that much lower.)
            let ankle: Vec<f32> = legs.iter().map(|l| l.3.y.max(0.0)).collect();
            let low: [Vec3; 4] = std::array::from_fn(|i| hips[i] - Vec3::Y * ankle[i]);
            let knees: [Vec3; 4] = std::array::from_fn(|i| legs[i].2);
            let walker = Walker::fitted(low, lengths, knees, at, yaw);
            real.middle = Vec3::new(0.0, low.iter().map(|h| h.y).sum::<f32>() / 4.0, (low[0].z + low[2].z) / 2.0);
            if std::env::var_os("LIMBS_TRACE").is_some() {
                println!("limbs: fitted · hips {hips:.2?} · bones {lengths:.2?} · ankles {ankle:.2?}");
            }
            (real.legs, real.ankle, real.walker) = (legs.iter().map(|l| l.0).collect(), ankle, Some(walker));
        }
        let Real { walker: Some(walker), legs, ankle, middle, .. } = &mut *real else { continue };
        if time.delta_secs() > 0.0 {
            walker.step(time.delta_secs(), at, yaw, asked(trial.2).1, ground);
        }
        // The whole model goes where the trunk is, turned as it is turned.
        let (trunk, turned) = walker.trunk();
        let placed = Transform { translation: trunk - turned * *middle, rotation: turned, scale: stood.scale };
        if let Ok(mut seen) = poses.get_mut(entity) {
            *seen = placed;
        }
        // A big moving picture follows it: from ahead and to one side, a little over its
        // back, coming after it smoothly.
        if args.big && args.gif && let Some(mut camera) = cameras.iter().next().and_then(|c| poses.get_mut(c).ok()) {
            let want = trunk + Quat::from_rotation_y(yaw + 2.2) * Vec3::Z * 2.3 + Vec3::Y * 0.55;
            let now = eye.map_or(want, |e| e.lerp(want, 1.0 - (-2.5 * time.delta_secs()).exp()));
            *eye = Some(now);
            *camera = Transform::from_translation(now).looking_at(trunk - Vec3::Y * 0.1, Vec3::Y);
        }
        bend(entity, &placed, legs, walker.feet(), walker.legs().map(|l| l[1]), ankle, &parents, &mut poses);
    }
}

/// Each leg is bent to its foot: the knee until the leg's end is as far from the hip as the
/// foot is, then the whole leg swung to it.
fn bend(entity: Entity, placed: &Transform, legs: &[[Entity; 3]], feet: [Vec3; 4], knees: [Vec3; 4], ankle: &[f32], parents: &Query<&ChildOf>, poses: &mut Query<&mut Transform, Without<crate::motion::Mover>>) {
    for ((([hip, knee, end], foot), ankle), toward) in legs.iter().zip(feet).zip(ankle.iter()).zip(knees) {
        let of = |bone: Entity, poses: &Query<&mut Transform, Without<crate::motion::Mover>>| crate::feet::world_of(bone, entity, placed, parents, poses).map(|w| Vec3::from(w.translation));
        let (Some(a), Some(b), Some(p)) = (of(*hip, poses), of(*knee, poses), of(*end, poses)) else { continue };
        let target = foot + Vec3::Y * *ankle;
        let (upper, lower) = (a.distance(b), b.distance(p));
        if upper < 1e-5 || lower < 1e-5 {
            continue;
        }
        let far = a.distance(target).clamp((upper - lower).abs() + 1e-4, (upper + lower) * 0.999);
        let now = (a - b).angle_between(p - b);
        let want = ((upper * upper + lower * lower - far * far) / (2.0 * upper * lower)).clamp(-1.0, 1.0).acos();
        let axis = (a - b).cross(p - b).try_normalize().unwrap_or(placed.rotation * Vec3::X);
        let bend = Quat::from_axis_angle(axis, want - now);
        if crate::feet::turn_bone(*knee, bend, entity, placed, parents, poses).is_none() {
            continue;
        }
        let bent = b + bend * (p - b);
        let (Some(from), Some(to)) = ((bent - a).try_normalize(), (target - a).try_normalize()) else { continue };
        let swing = Quat::from_rotation_arc(from, to);
        // And the whole leg is turned about the line from hip to foot until its knee is on
        // the side the walker's is: bent whichever way it happened to be, a leg nearly
        // straight went over to bend backwards from one frame to the next.
        let knee_now = a + swing * (b - a);
        let off = |p: Vec3| (p - a) - to * (p - a).dot(to);
        let twist = match (off(knee_now).try_normalize(), off(toward + Vec3::Y * *ankle).try_normalize()) {
            (Some(now), Some(want)) => Quat::from_axis_angle(to, now.cross(want).dot(to).atan2(now.dot(want))),
            _ => Quat::IDENTITY,
        };
        crate::feet::turn_bone(*hip, twist * swing, entity, placed, parents, poses);
    }
}

/// Every creature of the world that forge measured for it is moved by its feet.
fn adopt(mut commands: Commands, args: Res<crate::Args>, registry: Res<crate::creatures::Registry>, new: Query<(Entity, &crate::creatures::Creature), (With<crate::motion::Mover>, Without<Real>)>) {
    if !args.by_feet {
        return;
    }
    for (entity, creature) in &new {
        if registry.0[creature.def].meta.walk.is_some() {
            commands.entity(entity).insert(Real::default());
        }
    }
}

/// How near the eye a creature is moved by its feet, metres: further off it plays its clips,
/// and nobody can tell.
const NEAR: f32 = 45.0;

/// The creatures of the world, moved by their feet: standing, going and lying. Where the
/// simulation has the body is where the walker is taken; its trunk and feet come of that.
/// (Until 2026-11-12 these were clips, made here or carried from a wolf somebody drew: see
/// `cube_anim`.) Anything else it does (eating, a blow, a fall, swimming) is still its clip,
/// and it is fitted afresh when it comes back to its feet.
#[allow(clippy::too_many_arguments)]
fn wild(
    time: Res<Time>,
    live: Option<Res<crate::motion::Live>>,
    terrain: Option<Res<crate::world::Terrain>>,
    registry: Res<crate::creatures::Registry>,
    eyes: Query<&GlobalTransform, With<Camera3d>>,
    mut tops: Query<(Entity, &crate::motion::Mover, &mut crate::creatures::Creature, &mut Transform, &mut Real), With<crate::motion::Mover>>,
    children: Query<&Children>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    mut poses: Query<&mut Transform, Without<crate::motion::Mover>>,
) {
    use cube_sim::life::Act;
    let (Some(live), Some(terrain)) = (live, terrain) else { return };
    let eye = eyes.iter().next().map(|e| e.translation());
    for (entity, mover, mut creature, mut seen, mut real) in &mut tops {
        let Some(agent) = live.agent(mover) else { continue };
        let on = !real.lost && agent.dead.is_none() && agent.body.grounded && !agent.body.swimming && matches!(agent.act, Act::Idle | Act::Walk | Act::Run | Act::Sprint | Act::Lie) && eye.is_none_or(|e| e.distance(seen.translation) < NEAR);
        if creature.walked != on {
            creature.walked = on;
        }
        // (The frame it comes to its feet its clip is not yet the still one: it is moved from the next.)
        if !on || creature.hold.is_none() {
            real.walker = None;
            continue;
        }
        let Some(walk) = &registry.0[creature.def].meta.walk else { continue };
        if real.legs.is_empty() {
            let named = |name: &str| children.iter_descendants(entity).find(|e| names.get(*e).is_ok_and(|n| n.as_str() == name));
            let found: Vec<[Entity; 3]> = walk.legs.iter().filter_map(|chain| {
                let mut parts = chain.split('>').map(&named);
                Some([parts.next()??, parts.next()??, parts.next()??])
            }).collect();
            if found.len() != 4 {
                // (Its model is not there yet; or, if it is, it has no such bones and never will.)
                real.lost = children.iter_descendants(entity).any(|e| names.contains(e));
                continue;
            }
            real.legs = found;
        }
        let (at, yaw, size) = (seen.translation, seen.rotation.to_euler(EulerRot::YXZ).0, seen.scale.y);
        if real.walker.is_none() || (real.size - size).abs() > 0.02 {
            let low: [Vec3; 4] = std::array::from_fn(|i| (Vec3::from_array(walk.hips[i]) - Vec3::Y * walk.ankle[i]) * size);
            real.middle = Vec3::new(0.0, low.iter().map(|h| h.y).sum::<f32>() / 4.0, (low[0].z + low[2].z) / 2.0);
            real.ankle = walk.ankle.iter().map(|a| a * size).collect();
            let knees: [Vec3; 4] = std::array::from_fn(|i| walk.knees.get(i).map_or(Vec3::from_array(walk.hips[i]), |k| Vec3::from_array(*k)) * size);
            (real.size, real.walker) = (size, Some(Walker::fitted(low, std::array::from_fn(|i| (walk.bones[i][0] * size, walk.bones[i][1] * size)), knees, at, yaw)));
        }
        let Real { walker: Some(walker), legs, ankle, middle, .. } = &mut *real else { continue };
        let ground = |x: f32, z: f32| crate::feet::ground_at(&terrain, x, z, at.y).unwrap_or(at.y);
        if time.delta_secs() > 0.0 {
            walker.step(time.delta_secs(), at, yaw, if agent.act == Act::Lie { 1.0 } else { 0.0 }, ground);
        }
        let (trunk, turned) = walker.trunk();
        let placed = Transform { translation: trunk - turned * *middle, rotation: turned, scale: seen.scale };
        *seen = placed;
        bend(entity, &placed, legs, walker.feet(), walker.legs().map(|l| l[1]), ankle, &parents, &mut poses);
    }
}

/// The trial's body, how far round its ring it is, and the seconds it has been going.
#[derive(Resource)]
struct Trial(Walker, f32, f32);

/// How wide the ring is, metres; and what it does when: how fast it goes (m/s, backwards if
/// under nothing) and how far down it lies.
const RING: f32 = 3.6;
fn asked(t: f32) -> (f32, f32) {
    match t % 16.0 {
        t if t < 1.0 => (0.0, 0.0),
        t if t < 4.0 => (1.1, 0.0),
        t if t < 7.0 => (3.6, 0.0),
        t if t < 8.5 => (1.1, 0.0),
        t if t < 9.5 => (0.0, 0.0),
        t if t < 13.0 => (0.0, 1.0),
        t if t < 14.5 => (0.0, 0.0),
        _ => (1.1, 0.0),
    }
}

fn walk(mut commands: Commands, registry: Res<crate::creatures::Registry>, args: Res<crate::Args>, time: Res<Time>, loading: Res<crate::loading::Loading>, terrain: Option<Res<crate::world::Terrain>>, trial: Option<ResMut<Trial>>, mut parts: Query<(&Part, &mut Transform)>, mut cameras: Query<&mut Transform, (With<Camera3d>, Without<Part>)>, mut eye: Local<Option<Vec3>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut speed: Local<f32>) {
    let (true, Some(terrain)) = (args.limbs, terrain) else { return };
    let ground = |x: f32, z: f32| cube_voxel::mesh::eased_under(&terrain.0, Vec3::new(x, terrain.ground(x, z), z)).unwrap_or_else(|| terrain.ground(x, z));
    let place = |turn: f32| {
        let (x, z) = (args.at.0 + turn.sin() * RING, args.at.1 + turn.cos() * RING);
        Vec3::new(x, ground(x, z), z)
    };
    let Some(mut trial) = trial else {
        let walker = Walker::new(Build::dog(), place(0.0), std::f32::consts::FRAC_PI_2);
        let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
        let colours = [Color::srgb(0.55, 0.42, 0.3), Color::srgb(0.42, 0.31, 0.22), Color::srgb(0.6, 0.47, 0.34), Color::srgb(0.36, 0.27, 0.2)];
        let looks: Vec<_> = colours.iter().map(|c| materials.add(StandardMaterial { base_color: *c, perceptual_roughness: 0.9, ..default() })).collect();
        for (i, b) in walker.boxes().iter().enumerate() {
            commands.spawn((Part(i), Mesh3d(cube.clone()), MeshMaterial3d(looks[b.part as usize].clone()), Transform::default()));
        }
        commands.insert_resource(Trial(walker, 0.0, 0.0));
        // And the real one beside it, stood still by its idle clip held at its start.
        if let Some(def) = args.limbs_as.as_deref().and_then(|id| registry.index(id)) {
            let mut creature = crate::creatures::Creature::new(def);
            (creature.clip_name, creature.hold) = (Some("idle".into()), Some(0.0));
            commands.spawn((creature, Real::default(), crate::creatures::Picture, Transform::from_translation(place(std::f32::consts::PI)), Visibility::default()));
        }
        return;
    };
    let dt = time.delta_secs();
    if !loading.up && dt > 0.0 {
        trial.2 += dt;
        let (want, rest) = asked(trial.2);
        // (It gets up to speed and comes down from it as a body does, not at once.)
        *speed += (want - *speed).clamp(-5.0 * dt, 4.0 * dt);
        trial.1 += *speed / RING * dt;
        let (at, turn) = (place(trial.1), trial.1 + std::f32::consts::FRAC_PI_2);
        trial.0.step(dt, at, turn, rest, ground);
    }
    // A big moving picture of the boxes alone follows them, from the side and a little ahead.
    if args.big && args.gif && args.limbs_as.is_none() && let Some(mut camera) = cameras.iter_mut().next() {
        let trunk = trial.0.trunk().0;
        let want = trunk + Quat::from_rotation_y(trial.0.yaw + 1.9) * Vec3::Z * 2.6 + Vec3::Y * 0.5;
        let now = eye.map_or(want, |e| e.lerp(want, 1.0 - (-2.5 * dt).exp()));
        *eye = Some(now);
        *camera = Transform::from_translation(now).looking_at(trunk - Vec3::Y * 0.1, Vec3::Y);
    }
    let boxes = trial.0.boxes();
    for (part, mut seen) in &mut parts {
        if let Some(b) = boxes.get(part.0) {
            *seen = Transform { translation: b.at, rotation: b.turn, scale: b.size };
        }
    }
}
