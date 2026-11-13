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
        app.add_systems(Update, walk);
    }
}

#[derive(Component)]
struct Part(usize);

/// The trial's body, how far round its ring it is, and the seconds it has been going.
#[derive(Resource)]
struct Trial(Walker, f32, f32);

/// How wide the ring is, metres; and what it does when: how fast it goes (m/s, backwards if
/// under nothing) and how far down it lies.
const RING: f32 = 2.4;
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

fn walk(mut commands: Commands, args: Res<crate::Args>, time: Res<Time>, loading: Res<crate::loading::Loading>, terrain: Option<Res<crate::world::Terrain>>, trial: Option<ResMut<Trial>>, mut parts: Query<(&Part, &mut Transform)>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut speed: Local<f32>) {
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
    let boxes = trial.0.boxes();
    for (part, mut seen) in &mut parts {
        if let Some(b) = boxes.get(part.0) {
            *seen = Transform { translation: b.at, rotation: b.turn, scale: b.size };
        }
    }
}
