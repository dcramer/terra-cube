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
        app.add_systems(Startup, setup).add_systems(Update, (motes, leaves, butterflies, birds, glow, heavens, songbirds, dragonflies));
    }
}

/// How many of each there are round the camera.
const MOTES: usize = 170;
const SONGBIRDS: usize = 18;
const DRAGONFLIES: usize = 12;
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

/// The pollen's material and the fireflies', kept so their own light can follow the hour.
#[derive(Resource)]
struct Pale(Handle<StandardMaterial>, Handle<StandardMaterial>);

/// A mote that is a firefly, and how big it is by day.
#[derive(Component)]
struct Firefly(f32);

/// A star: which way it lies from anywhere.
#[derive(Component)]
struct Star(Vec3);
#[derive(Component)]
struct MoonDisc;

/// The stars' material and the moon's.
#[derive(Resource)]
struct Heavens(Handle<StandardMaterial>, Handle<StandardMaterial>);

const STARS: usize = 420;
/// How far off the stars and moon are drawn, metres: inside the camera's far plane.
const DOME: f32 = 700.0;

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

/// A small bird of the meadows and woods. It is either down, hopping and pecking where it
/// landed (`to`), or in the air between `from` and `to` since `since` for `flight` seconds.
#[derive(Component)]
struct Songbird {
    from: Vec3,
    to: Vec3,
    since: f32,
    flight: f32,
    own: f32,
    facing: f32,
}

/// A wing that folds against the body when its bird is down.
#[derive(Component)]
struct Folds;

/// A dragonfly: the water it keeps over, and the dart it is making or has made.
#[derive(Component)]
struct Dragonfly {
    over: Vec3,
    from: Vec3,
    to: Vec3,
    since: f32,
    own: f32,
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
    let pale = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.96, 0.78), emissive: LinearRgba::rgb(120.0, 108.0, 70.0), perceptual_roughness: 1.0, ..default() });
    // One in six is a firefly: by day a speck like the rest, by night a slow green lamp.
    let lamp = materials.add(StandardMaterial { base_color: Color::srgb(0.9, 1.0, 0.6), perceptual_roughness: 1.0, ..default() });
    commands.insert_resource(Pale(pale.clone(), lamp.clone()));
    for i in 0..MOTES {
        let size = 0.6 + roll(&mut state) * 1.2;
        let mut mote = commands.spawn((Mote(roll(&mut state) * 100.0), Mesh3d(speck.clone()), Transform::from_translation(far).with_scale(Vec3::splat(size)), NotShadowCaster));
        if i % 6 == 0 {
            mote.insert((Firefly(size), MeshMaterial3d(lamp.clone())));
        } else {
            mote.insert(MeshMaterial3d(pale.clone()));
        }
    }
    // The night sky: stars on a dome round the camera, and a moon. Their own light only, not
    // dimmed by haze; `heavens` turns them up as the day goes.
    let star = meshes.add(Sphere::new(1.0).mesh().ico(0).unwrap());
    let starlight = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: false, fog_enabled: false, perceptual_roughness: 1.0, ..default() });
    for _ in 0..STARS {
        // Evenly over the upper sky, a few of them large.
        let (turn, up) = (roll(&mut state) * TAU, 0.05 + roll(&mut state) * 0.95);
        let at = Vec3::new(turn.cos() * (1.0 - up * up).sqrt(), up, turn.sin() * (1.0 - up * up).sqrt());
        let size = 0.35 + roll(&mut state).powi(4) * 1.1;
        commands.spawn((Star(at), Mesh3d(star.clone()), MeshMaterial3d(starlight.clone()), Transform::from_translation(far).with_scale(Vec3::splat(size)), NotShadowCaster));
    }
    let moonlight = materials.add(StandardMaterial { base_color: Color::BLACK, fog_enabled: false, perceptual_roughness: 1.0, ..default() });
    commands.spawn((MoonDisc, Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap())), MeshMaterial3d(moonlight.clone()), Transform::from_translation(far).with_scale(Vec3::splat(16.0)), NotShadowCaster));
    commands.insert_resource(Heavens(starlight, moonlight));
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
    // Small birds: a round body, a head, and two short wings that fold when it is down.
    let (breast, crown, pinion) = (meshes.add(Cuboid::new(0.07, 0.07, 0.11)), meshes.add(Cuboid::new(0.05, 0.05, 0.05)), meshes.add(Cuboid::new(0.1, 0.012, 0.07)));
    let plumage = [Color::srgb(0.45, 0.33, 0.22), Color::srgb(0.62, 0.3, 0.16), Color::srgb(0.3, 0.34, 0.4)].map(|c| materials.add(StandardMaterial { base_color: c, perceptual_roughness: 0.9, double_sided: true, cull_mode: None, ..default() }));
    for i in 0..SONGBIRDS {
        let coat = plumage[i % 3].clone();
        commands.spawn((Songbird { from: far, to: far, since: -100.0, flight: 0.0, own: roll(&mut state) * 100.0, facing: 0.0 }, Mesh3d(breast.clone()), MeshMaterial3d(coat.clone()), Transform::from_translation(far))).with_children(|body| {
            body.spawn((Mesh3d(crown.clone()), MeshMaterial3d(coat.clone()), Transform::from_xyz(0.0, 0.05, -0.065)));
            for side in [-1.0f32, 1.0] {
                body.spawn((Wing { side, beat: 34.0, sweep: 1.0 }, Folds, Transform::from_xyz(side * 0.03, 0.02, 0.0), Visibility::default())).with_child((Mesh3d(pinion.clone()), MeshMaterial3d(coat.clone()), Transform::from_xyz(side * 0.05, 0.0, 0.0), NotShadowCaster));
            }
        });
    }
    // Dragonflies: a thin bright body and four clear wings held flat, too fast to see beat.
    let (needle, pane) = (meshes.add(Cuboid::new(0.012, 0.012, 0.09)), meshes.add(Cuboid::new(0.11, 0.001, 0.022)));
    let enamel = [Color::srgb(0.1, 0.5, 0.75), Color::srgb(0.2, 0.6, 0.25), Color::srgb(0.75, 0.2, 0.15)].map(|c| materials.add(StandardMaterial { base_color: c, perceptual_roughness: 0.3, ..default() }));
    let clear = materials.add(StandardMaterial { base_color: Color::srgba(0.9, 0.95, 1.0, 0.35), alpha_mode: AlphaMode::Blend, double_sided: true, cull_mode: None, ..default() });
    for i in 0..DRAGONFLIES {
        commands.spawn((Dragonfly { over: far, from: far, to: far, since: 0.0, own: roll(&mut state) * 100.0 }, Mesh3d(needle.clone()), MeshMaterial3d(enamel[i % 3].clone()), Transform::from_translation(far), NotShadowCaster)).with_children(|body| {
            for back in [-0.012f32, 0.012] {
                body.spawn((Mesh3d(pane.clone()), MeshMaterial3d(clear.clone()), Transform::from_xyz(0.0, 0.008, back - 0.02), NotShadowCaster));
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
fn butterflies(time: Res<Time>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut flies: Query<(&mut Butterfly, &mut Transform), Without<Wing>>, mut wings: Query<(&Wing, &mut Transform), Without<Folds>>, mut state: Local<u32>) {
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
        // (A folding wing is its bird's to move: `songbirds`.)
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
/// as much, or every speck would blaze like a lamp; the fireflies among it are turned up
/// instead, and each one swells and fades in its own time.
fn glow(time: Res<Time>, pale: Option<Res<Pale>>, cameras: Query<&bevy::camera::Exposure, With<Camera3d>>, mut materials: ResMut<Assets<StandardMaterial>>, mut flies: Query<(&Firefly, &Mote, &mut Transform)>, mut was: Local<f32>) {
    let (Some(pale), Some(exposure)) = (pale, cameras.iter().next()) else { return };
    let night = 1.0 - ((exposure.ev100 - 1.0) / 7.0).clamp(0.0, 1.0);
    let t = time.elapsed_secs();
    for (fly, mote, mut at) in &mut flies {
        let pulse = (0.5 + 0.5 * (t * 1.1 + mote.0 * 3.0).sin()).powi(3);
        at.scale = Vec3::splat(fly.0 * (1.0 + night * (0.2 + 2.2 * pulse)) * (1.0 - night * (1.0 - pulse).powi(6)));
    }
    if (exposure.ev100 - *was).abs() < 0.05 {
        return;
    }
    *was = exposure.ev100;
    // Dimmer still by night: pollen is not fireflies.
    let dim = 2.0f32.powf(exposure.ev100 - 13.4) * (0.25 + 0.75 * ((exposure.ev100 - 6.0) / 7.0).clamp(0.0, 1.0));
    if let Some(mut material) = materials.get_mut(&pale.0) {
        material.emissive = LinearRgba::rgb(120.0 * dim, 108.0 * dim, 70.0 * dim);
    }
    if let Some(mut material) = materials.get_mut(&pale.1) {
        let day = 2.0f32.powf(exposure.ev100 - 13.4);
        material.emissive = LinearRgba::rgb(120.0 * day + 9.0 * night, 108.0 * day + 16.0 * night, 70.0 * day + 2.0 * night);
    }
}

/// Stars and the moon keep their places in the sky wherever the camera goes, and come out as
/// the light goes: nothing by day, faint at dusk, sharp in full night.
fn heavens(live: Option<Res<crate::motion::Live>>, heavens: Option<Res<Heavens>>, cameras: Query<&GlobalTransform, With<Camera3d>>, mut materials: ResMut<Assets<StandardMaterial>>, mut stars: Query<(&Star, &mut Transform), Without<MoonDisc>>, mut moon: Query<&mut Transform, With<MoonDisc>>, mut was: Local<f32>) {
    let (Some(eye), Some(heavens)) = (eye(&cameras), heavens) else { return };
    let hour = live.map_or(cube_sim::sky::START, |l| l.0.hour);
    let (to_sun, night) = (cube_sim::sky::sun(hour), 1.0 - cube_sim::sky::daylight(hour));
    for (star, mut at) in &mut stars {
        at.translation = if night > 0.0 { eye + star.0 * DOME } else { Vec3::new(0.0, -500.0, 0.0) };
    }
    // Where `world::sky` puts the moon's light.
    let to_moon = (-to_sun + Vec3::new(0.0, 0.25, 0.3)).normalize();
    if let Ok(mut at) = moon.single_mut() {
        at.translation = if night > 0.0 && to_moon.y > -0.05 { eye + to_moon * DOME } else { Vec3::new(0.0, -500.0, 0.0) };
    }
    if (night - *was).abs() < 0.01 {
        return;
    }
    *was = night;
    if let Some(mut material) = materials.get_mut(&heavens.0) {
        material.emissive = LinearRgba::rgb(0.9, 0.95, 1.1) * (2.4 * night * night);
    }
    if let Some(mut material) = materials.get_mut(&heavens.1) {
        material.emissive = LinearRgba::rgb(1.0, 0.97, 0.88) * (1.6 * night);
    }
}

/// Small birds live on the ground near the camera: a few hops, a peck, a look round, and
/// every so often a low flight to somewhere else. Anything that comes within a few metres
/// (the player, here the camera's own ground) puts them up at once, and they go further.
#[allow(clippy::type_complexity)]
fn songbirds(
    time: Res<Time>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    terrain: Option<Res<Terrain>>,
    live: Option<Res<crate::motion::Live>>,
    mut birds: Query<(&mut Songbird, &mut Transform, &Children), Without<Wing>>,
    mut wings: Query<(&Wing, &mut Transform), With<Folds>>,
    mut state: Local<u32>,
) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    if *state == 0 {
        *state = 0x5EED_B12D;
    }
    let t = time.elapsed_secs();
    // What scares them: every creature that walks, the player among them.
    let walkers: Vec<Vec3> = live.map(|l| l.0.agents.values().map(|a| a.body.pos).collect()).unwrap_or_default();
    for (mut bird, mut at, children) in &mut birds {
        let flying = t - bird.since < bird.flight;
        if !flying {
            let scared = walkers.iter().any(|w| w.distance(bird.to) < 4.5);
            let restless = (t * 0.13 + bird.own).fract() < 0.004;
            let lost = bird.to.distance(eye) > 40.0;
            if scared || restless || lost {
                // Somewhere else on dry ground: near if it is only restless, far if scared.
                for _ in 0..6 {
                    let (turn, out) = (roll(&mut state) * TAU, if lost { 6.0 + roll(&mut state) * 22.0 } else if scared { 12.0 + roll(&mut state) * 14.0 } else { 3.0 + roll(&mut state) * 8.0 });
                    let middle = if lost { eye } else { bird.to };
                    let (x, z) = (middle.x + turn.cos() * out, middle.z + turn.sin() * out);
                    if matches!(terrain.1.biome((x / CELL).floor() as i32, (z / CELL).floor() as i32), Biome::Meadow | Biome::Forest) {
                        let from = if lost { Vec3::new(x, terrain.ground(x, z) + 6.0, z) } else { at.translation };
                        (bird.from, bird.to, bird.since) = (from, Vec3::new(x, terrain.ground(x, z) + 0.04, z), t);
                        bird.flight = (bird.from.distance(bird.to) / 7.5).max(0.4);
                        break;
                    }
                }
            }
        }
        let flying = t - bird.since < bird.flight;
        if flying {
            // A low arc, quick to rise and gliding in.
            let part = (t - bird.since) / bird.flight;
            let eased = part * part * (3.0 - 2.0 * part);
            let arc = bird.from.distance(bird.to) * 0.22 * (4.0 * part * (1.0 - part));
            let now = bird.from.lerp(bird.to, eased) + Vec3::Y * arc;
            let ahead = (bird.to - bird.from).with_y(0.0).normalize_or(Vec3::X);
            bird.facing = ahead.x.atan2(ahead.z) + std::f32::consts::PI;
            *at = Transform::from_translation(now).with_rotation(Quat::from_rotation_y(bird.facing) * Quat::from_rotation_x(0.5 - part));
        } else {
            // Down: hops in little bursts, turning between them, with a peck now and then.
            let beat = t * 2.4 + bird.own;
            let hopping = beat.fract() < 0.35 && (beat.floor() * 0.37 + bird.own).fract() < 0.6;
            if beat.fract() < 0.02 {
                bird.facing += (roll(&mut state) - 0.5) * 1.6;
            }
            let hop = if hopping { (beat.fract() / 0.35 * std::f32::consts::PI).sin() } else { 0.0 };
            let forward = Quat::from_rotation_y(bird.facing) * Vec3::NEG_Z;
            if hopping {
                let step = forward * 0.25 * time.delta_secs();
                let next = bird.to + step;
                bird.to = Vec3::new(next.x, terrain.ground(next.x, next.z) + 0.04, next.z);
            }
            let peck = if !hopping && (beat.floor() * 0.61 + bird.own).fract() < 0.4 { (beat.fract() * TAU * 2.0).sin().max(0.0) * 0.7 } else { 0.0 };
            *at = Transform::from_translation(bird.to + Vec3::Y * (0.035 + hop * 0.05)).with_rotation(Quat::from_rotation_y(bird.facing) * Quat::from_rotation_x(-peck));
        }
        for &child in children {
            if let Ok((wing, mut hinge)) = wings.get_mut(child) {
                // Beating in the air, folded down along the body on the ground.
                hinge.rotation = if flying { Quat::from_rotation_z(wing.side * (wing.sweep * (t * wing.beat).sin())) } else { Quat::from_rotation_z(wing.side * -1.35) };
            }
        }
    }
}

/// Dragonflies keep to water near the camera: hanging still in the air for a moment, then
/// gone a couple of metres in a blink and hanging again.
fn dragonflies(time: Res<Time>, cameras: Query<&GlobalTransform, With<Camera3d>>, terrain: Option<Res<Terrain>>, mut flies: Query<(&mut Dragonfly, &mut Transform)>, mut state: Local<u32>) {
    let (Some(eye), Some(terrain)) = (eye(&cameras), terrain) else { return };
    if *state == 0 {
        *state = 0xD2A6_0F17;
    }
    let t = time.elapsed_secs();
    let water = cube_sim::terrain::SEA_LEVEL;
    let wet = |x: f32, z: f32| terrain.1.biome((x / CELL).floor() as i32, (z / CELL).floor() as i32) == Biome::Water;
    for (mut fly, mut at) in &mut flies {
        if fly.over.distance(eye) > 45.0 {
            fly.over = Vec3::new(0.0, -500.0, 0.0);
            for _ in 0..8 {
                let (turn, out) = (roll(&mut state) * TAU, 4.0 + roll(&mut state) * 34.0);
                let (x, z) = (eye.x + turn.cos() * out, eye.z + turn.sin() * out);
                if wet(x, z) {
                    fly.over = Vec3::new(x, water, z);
                    (fly.from, fly.to, fly.since) = (fly.over + Vec3::Y * 0.6, fly.over + Vec3::Y * 0.6, t);
                    break;
                }
            }
        }
        if fly.over.y < -100.0 {
            at.translation = fly.over;
            continue;
        }
        // Hang, then dart: a new place over the same water every second or so.
        let hang = 0.7 + (fly.own * 3.7).fract() * 1.1;
        if t - fly.since > hang {
            let (turn, out) = (roll(&mut state) * TAU, 0.8 + roll(&mut state) * 2.6);
            let mut to = fly.over + Vec3::new(turn.cos() * out, 0.25 + roll(&mut state) * 0.9, turn.sin() * out);
            if !wet(to.x, to.z) {
                to = fly.over + Vec3::Y * 0.5;
            }
            (fly.from, fly.to, fly.since) = (fly.to, to, t);
        }
        let dart = ((t - fly.since) / 0.16).clamp(0.0, 1.0);
        let now = fly.from.lerp(fly.to, dart * dart * (3.0 - 2.0 * dart)) + Vec3::Y * 0.015 * (t * 31.0 + fly.own).sin();
        let ahead = (fly.to - fly.from).with_y(0.0).normalize_or(Vec3::X);
        *at = Transform::from_translation(now).looking_to(ahead, Vec3::Y);
    }
}
