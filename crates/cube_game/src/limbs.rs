//! A four-legged body of boxes moved by its skeleton and its feet, with no clips
//! (`cube_anim`), and a person the same way: `--limbs` puts one in a scene and sends it round a ring about the place
//! looked at, standing, walking, running, slowing, turning about, lying down and getting up,
//! so that all of it is seen in one moving picture. A trial of the way of moving things,
//! on cubes, before it is asked of any real model.

use bevy::prelude::*;
use cube_anim::{Build, Walker};

pub struct LimbsPlugin;

impl Plugin for LimbsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (walk, stride));
    }
}

#[derive(Component)]
struct Part(usize);

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

fn walk(mut commands: Commands, args: Res<crate::Args>, time: Res<Time>, loading: Res<crate::loading::Loading>, terrain: Option<Res<crate::world::Terrain>>, trial: Option<ResMut<Trial>>, mut parts: Query<(&Part, &mut Transform)>, mut cameras: Query<&mut Transform, (With<Camera3d>, Without<Part>)>, mut eye: Local<Option<Vec3>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut speed: Local<f32>) {
    let (true, false, Some(terrain)) = (args.limbs, args.person, terrain) else { return };
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
    if args.big && args.gif && let Some(mut camera) = cameras.iter_mut().next() {
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

/// The trial's person, how far round the ring they are, the seconds they have been going,
/// and how fast.
#[derive(Resource)]
struct Person(cube_anim::biped::Biped, f32, f32, f32);

/// What the person does when, m/s: stands, walks, runs, sprints, walks, stops.
fn paced(t: f32) -> f32 {
    match t % 16.0 {
        t if t < 1.0 => 0.0,
        t if t < 5.0 => 1.4,
        t if t < 9.0 => 3.8,
        t if t < 11.0 => 6.0,
        t if t < 13.5 => 1.4,
        _ => 0.0,
    }
}

/// `--limbs-person`: a person of boxes sent round the ring, moved by `cube_anim::biped`.
#[allow(clippy::too_many_arguments)]
fn stride(mut commands: Commands, args: Res<crate::Args>, time: Res<Time>, loading: Res<crate::loading::Loading>, terrain: Option<Res<crate::world::Terrain>>, trial: Option<ResMut<Person>>, mut parts: Query<(&Part, &mut Transform)>, mut cameras: Query<&mut Transform, (With<Camera3d>, Without<Part>)>, mut eye: Local<Option<Vec3>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let (true, true, Some(terrain)) = (args.limbs, args.person, terrain) else { return };
    let ground = |x: f32, z: f32| cube_voxel::mesh::eased_under(&terrain.0, Vec3::new(x, terrain.ground(x, z), z)).unwrap_or_else(|| terrain.ground(x, z));
    // (A wider ring than the animal's: a sprint round a small one is all turning.)
    let ring = RING * 2.5;
    let place = |turn: f32| {
        let (x, z) = (args.at.0 + turn.sin() * ring, args.at.1 + turn.cos() * ring);
        Vec3::new(x, ground(x, z), z)
    };
    let Some(mut trial) = trial else {
        let person = cube_anim::biped::Biped::new(cube_anim::biped::Frame::person(1.75), place(0.0), std::f32::consts::FRAC_PI_2);
        let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
        let colours = [Color::srgb(0.45, 0.4, 0.3), Color::srgb(0.25, 0.24, 0.22), Color::srgb(0.75, 0.58, 0.45), Color::srgb(0.5, 0.44, 0.34)];
        let looks: Vec<_> = colours.iter().map(|c| materials.add(StandardMaterial { base_color: *c, perceptual_roughness: 0.9, ..default() })).collect();
        for (i, b) in person.boxes().iter().enumerate() {
            commands.spawn((Part(i), Mesh3d(cube.clone()), MeshMaterial3d(looks[b.part as usize].clone()), Transform::default()));
        }
        commands.insert_resource(Person(person, 0.0, 0.0, 0.0));
        return;
    };
    let dt = time.delta_secs();
    if !loading.up && dt > 0.0 {
        trial.2 += dt;
        let want = paced(trial.2);
        trial.3 += (want - trial.3).clamp(-6.0 * dt, 4.0 * dt);
        trial.1 += trial.3 / ring * dt;
        // (And leaps: once standing, once at a walk, once at a run. Up at 5 m/s, as the game's own.)
        let hop = [0.4f32, 3.0, 7.0].iter().map(|from| (trial.2 % 16.0) - from).filter(|t| *t > 0.0).map(|t| 5.0 * t - 7.0 * t * t).fold(0.0, f32::max);
        let (at, turn) = (place(trial.1) + Vec3::Y * hop, trial.1 + std::f32::consts::FRAC_PI_2);
        trial.0.step(dt, at, turn, hop > 0.0, ground);
    }
    if args.big && args.gif && let Some(mut camera) = cameras.iter_mut().next() {
        let middle = trial.0.stance().pelvis.0;
        let want = middle + Quat::from_rotation_y(trial.0.yaw + 1.9) * Vec3::Z * 2.9 + Vec3::Y * 0.3;
        let now = eye.map_or(want, |e| e.lerp(want, 1.0 - (-4.0 * dt).exp()));
        *eye = Some(now);
        *camera = Transform::from_translation(now).looking_at(middle, Vec3::Y);
    }
    let boxes = trial.0.boxes();
    for (part, mut seen) in &mut parts {
        if let Some(b) = boxes.get(part.0) {
            *seen = Transform { translation: b.at, rotation: b.turn, scale: b.size };
        }
    }
}
