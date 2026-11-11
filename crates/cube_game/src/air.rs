//! What is in the air, so the world is not a still picture: pollen drifting on the wind,
//! leaves coming down in the woods, butterflies over the meadows, birds wheeling high up.
//!
//! These are dressing, not creatures. Nothing in the simulation knows of them and nothing
//! reacts to them; they are kept round the camera and exist only while it looks. So they
//! live here, in the crate that draws, and take their chances from their own small generator
//! and not the simulation's seeded one. A bird that can be hunted is a creature and belongs
//! to `cube_sim`; these are the ones only ever seen.

use crate::world::Terrain;
use bevy::{light::NotShadowCaster, prelude::*};
use cube_sim::terrain::Biome;
use cube_voxel::CELL;
use std::f32::consts::TAU;

pub struct AirPlugin;

impl Plugin for AirPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, (motes, leaves, butterflies, birds, glow));
    }
}

/// How many of each there are round the camera.
const MOTES: usize = 260;
const LEAVES: usize = 36;
const BUTTERFLIES: usize = 14;
const FLOCKS: usize = 3;

/// The wind, as in `assets/shaders/wind.wesl`: which way, and metres a second for what
/// drifts on it.
const WIND: Vec3 = Vec3::new(0.8, 0.0, 0.6);
const DRIFT: f32 = 0.45;

/// 0 to 1, a different one each call.
fn roll(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state & 0xFFFF) as f32 / 65535.0
}

/// The pollen's material, kept so its own light can follow the camera's exposure.
#[derive(Resource)]
struct Pale(Handle<StandardMaterial>);

/// A speck of pollen or down. `0` is its own number, so no two move alike.
#[derive(Component)]
struct Mote(f32);

/// A leaf on its way down: how it tumbles, and its own number.
#[derive(Component)]
struct Leaf(Vec3, f32);

/// A butterfly, and the spot it keeps to.
#[derive(Component)]
struct Butterfly {
    home: Vec3,
    own: f32,
}

/// A wing, of a butterfly or a bird: which side, and how fast and far it beats.
#[derive(Component)]
struct Wing {
    side: f32,
    beat: f32,
    sweep: f32,
}

/// A bird in a flock that wheels round a point: how far out, how high, how fast, where in
/// the circle.
#[derive(Component)]
struct Bird {
    middle: Vec3,
    out: f32,
    speed: f32,
    at: f32,
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut state = 0x9E37_79B9u32;
    let far = Vec3::new(0.0, -500.0, 0.0);
    // Pollen: lit like anything else, so it flares crossing a shaft of sun and dims in shade,
    // with a little light of its own so it is not lost there.
    let speck = meshes.add(Sphere::new(0.011).mesh().ico(1).unwrap());
    let pale = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.96, 0.78), emissive: LinearRgba::rgb(260.0, 235.0, 150.0), perceptual_roughness: 1.0, ..default() });
    commands.insert_resource(Pale(pale.clone()));
    for _ in 0..MOTES {
        let size = 0.6 + roll(&mut state) * 1.2;
        commands.spawn((Mote(roll(&mut state) * 100.0), Mesh3d(speck.clone()), MeshMaterial3d(pale.clone()), Transform::from_translation(far).with_scale(Vec3::splat(size)), NotShadowCaster));
    }
    // Leaves: a flat chip of one of four autumn-turning greens and browns.
    let chip = meshes.add(Cuboid::new(0.07, 0.008, 0.05));
    let tones = [Color::srgb(0.30, 0.42, 0.12), Color::srgb(0.52, 0.44, 0.12), Color::srgb(0.50, 0.26, 0.08), Color::srgb(0.22, 0.34, 0.10)].map(|c| materials.add(StandardMaterial { base_color: c, perceptual_roughness: 0.9, double_sided: true, ..default() }));
    for i in 0..LEAVES {
        let spin = Vec3::new(roll(&mut state) - 0.5, roll(&mut state) - 0.5, roll(&mut state) - 0.5) * 6.0;
        commands.spawn((Leaf(spin, roll(&mut state) * 100.0), Mesh3d(chip.clone()), MeshMaterial3d(tones[i % 4].clone()), Transform::from_translation(far), NotShadowCaster));
    }
    // Butterflies: a body with two wings hinged along it, white, yellow or blue.
    let wing = meshes.add(Cuboid::new(0.07, 0.003, 0.08));
    let coats = [Color::srgb(0.95, 0.93, 0.85), Color::srgb(0.95, 0.78, 0.2), Color::srgb(0.35, 0.5, 0.9)].map(|c| materials.add(StandardMaterial { base_color: c, perceptual_roughness: 0.8, double_sided: true, cull_mode: None, ..default() }));
    for i in 0..BUTTERFLIES {
        let own = roll(&mut state) * 100.0;
        commands.spawn((Butterfly { home: far, own }, Transform::from_translation(far), Visibility::default())).with_children(|body| {
            for side in [-1.0f32, 1.0] {
                // Hinged at the body: the wing's mesh sits out from the hinge.
                body.spawn((Wing { side, beat: 26.0 + own * 0.05, sweep: 1.1 }, Transform::default(), Visibility::default())).with_child((Mesh3d(wing.clone()), MeshMaterial3d(coats[i % 3].clone()), Transform::from_xyz(side * 0.037, 0.0, 0.0), NotShadowCaster));
            }
        });
    }
    // Birds: dark, a body and two long wings, seen from below against the sky.
    let (trunk, span) = (meshes.add(Cuboid::new(0.12, 0.08, 0.42)), meshes.add(Cuboid::new(0.55, 0.02, 0.2)));
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.07, 0.07, 0.08), perceptual_roughness: 0.9, double_sided: true, cull_mode: None, ..default() });
    for _ in 0..FLOCKS {
        let turn = roll(&mut state) * TAU;
        let middle = Vec3::new(turn.cos(), 0.0, turn.sin()) * (30.0 + roll(&mut state) * 90.0) + Vec3::Y * (58.0 + roll(&mut state) * 40.0);
        let (out, speed) = (22.0 + roll(&mut state) * 30.0, 7.0 + roll(&mut state) * 4.0);
        for b in 0..4 + (roll(&mut state) * 5.0) as usize {
            let bird = Bird { middle: middle + Vec3::Y * (roll(&mut state) - 0.5) * 6.0, out: out + (roll(&mut state) - 0.5) * 9.0, speed, at: b as f32 * 0.22 + roll(&mut state) * 0.2 };
            commands.spawn((bird, Mesh3d(trunk.clone()), MeshMaterial3d(dark.clone()), Transform::from_translation(far), NotShadowCaster)).with_children(|body| {
                for side in [-1.0f32, 1.0] {
                    body.spawn((Wing { side, beat: 5.5, sweep: 0.55 }, Transform::default(), Visibility::default())).with_child((Mesh3d(span.clone()), MeshMaterial3d(dark.clone()), Transform::from_xyz(side * 0.3, 0.0, 0.0), NotShadowCaster));
                }
            });
        }
    }
}

/// Where the camera is, if there is one that looks at the world.
fn eye(cameras: &Query<&GlobalTransform, With<Camera3d>>) -> Option<Vec3> {
    cameras.iter().next().map(|c| c.translation())
}

/// Pollen drifts downwind and wanders, kept in a box round the camera: one that leaves by
/// one side comes in by the other, so there is always some wherever the camera goes.
fn motes(time: Res<Time>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut motes: Query<(&Mote, &mut Transform)>) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    let reach = Vec3::new(13.0, 5.0, 13.0);
    for (mote, mut at) in &mut motes {
        let own = mote.0;
        let wander = Vec3::new((t * 0.7 + own).sin(), (t * 0.9 + own * 1.7).sin() * 0.6, (t * 0.6 + own * 2.3).cos()) * 0.18;
        at.translation += (WIND * DRIFT + wander) * dt;
        let mut off = at.translation - eye;
        // Far off (at the start, or after the camera jumps): anywhere in the box, by its number.
        if off.abs().max_element() > reach.x * 3.0 {
            off = Vec3::new((own * 12.989).fract() * 2.0 - 1.0, (own * 78.233).fract() * 2.0 - 1.0, (own * 37.719).fract() * 2.0 - 1.0) * reach;
        }
        // Out of the box (or never yet in it): in again on the far side.
        let wrapped = Vec3::new((off.x + reach.x).rem_euclid(reach.x * 2.0) - reach.x, (off.y + reach.y).rem_euclid(reach.y * 2.0) - reach.y, (off.z + reach.z).rem_euclid(reach.z * 2.0) - reach.z);
        at.translation = eye + wrapped;
        let floor = terrain.ground(at.translation.x, at.translation.z) + 0.15;
        if at.translation.y < floor {
            at.translation.y = floor + (own * 7.3).rem_euclid(3.0);
        }
    }
}

/// A leaf tumbles down in a wood, swinging as it falls; on the ground it is gone, and
/// another starts from the crowns somewhere near the camera.
fn leaves(time: Res<Time>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut leaves: Query<(&Leaf, &mut Transform)>, mut state: Local<u32>) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    if *state == 0 {
        *state = 0x1234_5679;
    }
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    for (leaf, mut at) in &mut leaves {
        let own = leaf.1;
        let swing = Vec3::new((t * 1.9 + own).sin(), 0.0, (t * 1.6 + own * 1.3).cos()) * 0.5;
        at.translation += (WIND * DRIFT * 1.4 + swing + Vec3::NEG_Y * (0.55 + 0.25 * (t * 2.3 + own).sin())) * dt;
        at.rotation *= Quat::from_scaled_axis(leaf.0 * dt);
        let ground = terrain.ground(at.translation.x, at.translation.z);
        if at.translation.y < ground + 0.03 || at.translation.distance(eye) > 45.0 {
            // A few tries for a spot under trees; with none, it waits out of sight.
            at.translation = Vec3::new(0.0, -500.0, 0.0);
            for _ in 0..6 {
                let (turn, out) = (roll(&mut state) * TAU, 3.0 + roll(&mut state) * 22.0);
                let (x, z) = (eye.x + turn.cos() * out, eye.z + turn.sin() * out);
                if terrain.1.biome((x / CELL).floor() as i32, (z / CELL).floor() as i32) == Biome::Forest {
                    at.translation = Vec3::new(x, terrain.ground(x, z) + 4.0 + roll(&mut state) * 9.0, z);
                    break;
                }
            }
        }
    }
}

/// A butterfly keeps to a few metres of meadow near the camera, flying a loose looping path
/// low over the grass, its wings beating too fast to count.
fn butterflies(time: Res<Time>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut flies: Query<(&mut Butterfly, &mut Transform), Without<Wing>>, mut wings: Query<(&Wing, &mut Transform)>, mut state: Local<u32>) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    if *state == 0 {
        *state = 0x0BAD_5EED;
    }
    let t = time.elapsed_secs();
    for (mut fly, mut at) in &mut flies {
        if fly.home.distance(eye) > 38.0 {
            fly.home = Vec3::new(0.0, -500.0, 0.0);
            for _ in 0..6 {
                let (turn, out) = (roll(&mut state) * TAU, 4.0 + roll(&mut state) * 24.0);
                let (x, z) = (eye.x + turn.cos() * out, eye.z + turn.sin() * out);
                if terrain.1.biome((x / CELL).floor() as i32, (z / CELL).floor() as i32) == Biome::Meadow {
                    fly.home = Vec3::new(x, 0.0, z);
                    break;
                }
            }
        }
        if fly.home.y < -100.0 {
            at.translation = fly.home;
            continue;
        }
        let own = fly.own;
        let path = |t: f32| Vec3::new((t * 0.6 + own).sin() * 2.2 + (t * 1.7 + own * 2.0).sin() * 0.6, 0.0, (t * 0.47 + own * 1.4).cos() * 2.2 + (t * 1.3 + own).cos() * 0.6);
        let (now, soon) = (fly.home + path(t), fly.home + path(t + 0.1));
        let high = terrain.ground(now.x, now.z) + 0.55 + 0.3 * (t * 2.1 + own).sin() + 0.12 * (t * 9.0 + own).sin();
        at.translation = Vec3::new(now.x, high, now.z);
        at.look_to((soon - now).normalize_or(Vec3::X), Vec3::Y);
    }
    for (wing, mut hinge) in &mut wings {
        // Up from flat to nearly closed over the back, and down again.
        hinge.rotation = Quat::from_rotation_z(wing.side * (0.15 + wing.sweep * (t * wing.beat).sin().abs()));
    }
}

/// Birds wheel in loose flocks high over the land, banked into the turn, a few slow
/// wingbeats and then a glide.
fn birds(time: Res<Time>, mut birds: Query<(&Bird, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (bird, mut at) in &mut birds {
        let round = bird.at + t * bird.speed / bird.out;
        let rise = (t * 0.21 + bird.at * 3.0).sin() * 3.0;
        at.translation = bird.middle + Vec3::new(round.cos(), 0.0, round.sin()) * bird.out + Vec3::Y * rise;
        let ahead = Vec3::new(-round.sin(), 0.0, round.cos());
        *at = at.looking_to(ahead, Vec3::Y);
        at.rotate_local_z(-0.35);
    }
}

/// Pollen's own light is set for day. As the camera opens up for night it is turned down by
/// as much, or every speck would blaze like a lamp.
fn glow(pale: Option<Res<Pale>>, cameras: Query<&bevy::camera::Exposure, With<Camera3d>>, mut materials: ResMut<Assets<StandardMaterial>>, mut was: Local<f32>) {
    let (Some(pale), Some(exposure)) = (pale, cameras.iter().next()) else { return };
    if (exposure.ev100 - *was).abs() < 0.05 {
        return;
    }
    *was = exposure.ev100;
    if let Some(mut material) = materials.get_mut(&pale.0) {
        // Dimmer still by night: pollen is not fireflies.
        let dim = 2.0f32.powf(exposure.ev100 - 13.4) * (0.25 + 0.75 * ((exposure.ev100 - 6.0) / 7.0).clamp(0.0, 1.0));
        material.emissive = LinearRgba::rgb(260.0 * dim, 235.0 * dim, 150.0 * dim);
    }
}
