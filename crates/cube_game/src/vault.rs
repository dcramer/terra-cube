//! What the sky is given that the engine's air does not give it. The air (`world::look`)
//! scatters the sun and the moon and is right about most hours; these are the places it is
//! not, each found by looking (`make sky`) and each a plain shape round the camera with a
//! colour of its own, laid on the sky behind everything else:
//!
//! - *A floor under the night.* With no moon the air is lit by nothing and the sky was
//!   black, darker than the clouds and the land under it, which is a night inside out. The
//!   sky at night is never black: deep blue, a little lighter toward its edge.
//! - *Blue overhead while the sun is low.* Exposed for the land at sunrise and sunset, the
//!   air overhead comes out a dead grey. It is blended toward the blue it has to the eye,
//!   most overhead and not at all at the horizon, where the colour of the hour is.
//! - *A glow round the sun*, the light the air throws forward round it, which the air's
//!   own picture of the sky is too coarse to hold: without it the sun was a white dot.
//! - *A ring of light round the moon.*
//!
//! They are dressing, like the stars (`air`): the simulation knows of none of it.

use crate::air::DOME;
use bevy::{asset::RenderAssetUsages, light::NotShadowCaster, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::sky;
use std::f32::consts::{FRAC_PI_2, TAU};

pub struct VaultPlugin;

impl Plugin for VaultPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, turn);
    }
}

/// The night's floor and the low sun's blue: both stay round the camera.
#[derive(Component)]
struct Vault;
/// The glow round the sun, and the ring round the moon.
#[derive(Component)]
struct Aureole;
#[derive(Component)]
struct Halo;

/// Their materials, in that order, and what each was last set to.
#[derive(Resource)]
struct Lights([Handle<StandardMaterial>; 4]);

/// The night sky with no moon, overhead: what is added to the air's own. (As the picture has
/// it, not as light: these are not lit and the camera's exposure does not touch them.)
const NIGHT: Vec3 = Vec3::new(0.006, 0.012, 0.034);
/// The blue the sky overhead is turned toward while the sun is low, and how far at most.
const BLUE: Vec3 = Vec3::new(0.10, 0.17, 0.44);
const BLUED: f32 = 0.62;
/// How far out from the sun its glow reaches and from the moon its ring, radians.
const AUREOLE: f32 = 0.3;
const HALO: f32 = 0.12;

/// The sky as a bowl round the camera, from a little under the land's edge to overhead; each
/// point coloured by how high it is (the sine of it).
fn bowl(colour: impl Fn(f32) -> [f32; 4]) -> Mesh {
    const RINGS: u32 = 24;
    const ROUND: u32 = 48;
    let (mut positions, mut normals, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for ring in 0..=RINGS {
        let high = -0.14 + (FRAC_PI_2 + 0.14) * ring as f32 / RINGS as f32;
        for step in 0..=ROUND {
            let turn = TAU * step as f32 / ROUND as f32;
            let to = Vec3::new(turn.cos() * high.cos(), high.sin(), turn.sin() * high.cos());
            // Beyond the stars, which are on `DOME`.
            positions.push((to * DOME * 1.04).to_array());
            normals.push((-to).to_array());
            colors.push(colour(high.sin()));
        }
    }
    for ring in 0..RINGS {
        for step in 0..ROUND {
            let (a, b) = (ring * (ROUND + 1) + step, (ring + 1) * (ROUND + 1) + step);
            indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices))
}

/// A round glow one across, brightest at its middle and gone at its rim, falling off fast
/// at first as light round a lamp does.
fn glow() -> Mesh {
    const ROUND: u32 = 40;
    const RINGS: [(f32, f32); 6] = [(0.0, 1.0), (0.06, 0.62), (0.16, 0.3), (0.34, 0.12), (0.62, 0.035), (1.0, 0.0)];
    let (mut positions, mut normals, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (out, bright) in RINGS {
        for step in 0..=ROUND {
            let turn = TAU * step as f32 / ROUND as f32;
            positions.push([turn.cos() * out, turn.sin() * out, 0.0]);
            normals.push([0.0, 0.0, 1.0]);
            colors.push([1.0, 1.0, 1.0, bright]);
        }
    }
    for ring in 0..RINGS.len() as u32 - 1 {
        for step in 0..ROUND {
            let (a, b) = (ring * (ROUND + 1) + step, (ring + 1) * (ROUND + 1) + step);
            indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices))
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // Each has its own light and no other, is not hazed, and is laid over what is behind it:
    // added to it, or (the blue) blended with it by how much of it there is at each point.
    let mut laid = |mode: AlphaMode| materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, alpha_mode: mode, fog_enabled: false, cull_mode: None, ..default() });
    let lights = [laid(AlphaMode::Add), laid(AlphaMode::Blend), laid(AlphaMode::Add), laid(AlphaMode::Add)];
    let far = Transform::from_xyz(0.0, -50_000.0, 0.0);
    // The night's floor: half as light again at the land's edge as overhead.
    let floor = bowl(|up| {
        let edge = 1.0 + 0.5 * (1.0 - up.max(0.0)).powi(3);
        [edge, edge, edge, 1.0]
    });
    // The blue: none of it at the land's edge, all of it from half way up.
    let blue = bowl(|up| {
        let t = ((up - 0.1) / 0.6).clamp(0.0, 1.0);
        [1.0, 1.0, 1.0, t * t * (3.0 - 2.0 * t)]
    });
    commands.spawn((Vault, Mesh3d(meshes.add(floor)), MeshMaterial3d(lights[0].clone()), far, NotShadowCaster));
    commands.spawn((Vault, Mesh3d(meshes.add(blue)), MeshMaterial3d(lights[1].clone()), far, NotShadowCaster));
    let round = meshes.add(glow());
    commands.spawn((Aureole, Mesh3d(round.clone()), MeshMaterial3d(lights[2].clone()), far, NotShadowCaster));
    commands.spawn((Halo, Mesh3d(round), MeshMaterial3d(lights[3].clone()), far, NotShadowCaster));
    commands.insert_resource(Lights(lights));
}

/// The colour the air leaves the sun at a height (the sine of it), near enough: so much air
/// in the way, once overhead and thirty-eight times at the land's edge, taking blue first.
pub fn through(up: f32) -> Vec3 {
    let air = 1.0 / (up.max(-0.01) + 0.026);
    Vec3::new((-0.06 * air).exp(), (-0.11 * air).exp(), (-0.22 * air).exp())
}

/// They stay round the camera and follow the hour.
#[allow(clippy::type_complexity)]
fn turn(
    live: Option<Res<crate::motion::Live>>,
    lights: Res<Lights>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut vaults: Query<&mut Transform, (With<Vault>, Without<Aureole>, Without<Halo>)>,
    mut aureole: Query<&mut Transform, (With<Aureole>, Without<Halo>)>,
    mut halo: Query<&mut Transform, With<Halo>>,
    mut was: Local<[Vec4; 4]>,
) {
    let Some(eye) = cameras.iter().next().map(|c| c.translation()) else { return };
    let when = live.map_or(sky::START, |l| l.0.when);
    let (to_sun, to_moon, day) = (sky::sun(when), sky::moon(when), sky::daylight(when));
    for mut at in &mut vaults {
        at.translation = eye;
    }
    // A glow faces the camera from where its light is, just inside the stars.
    let place = |to: Vec3, across: f32| Transform::from_translation(eye + to * DOME * 0.97).looking_at(eye, Vec3::Y).with_scale(Vec3::splat(DOME * across.tan()));
    if let Ok(mut at) = aureole.single_mut() {
        *at = place(to_sun, AUREOLE);
    }
    if let Ok(mut at) = halo.single_mut() {
        *at = place(to_moon, HALO);
    }
    // The floor comes up as twilight ends (the stars' own hour), less under a moon, whose
    // light the air already has in it.
    let dark = ((-0.105 - to_sun.y) / 0.12).clamp(0.0, 1.0);
    let floor = NIGHT * dark * (1.0 - 0.5 * sky::moonlight(when));
    // The blue is for a sun within a hand of the land's edge, over or under.
    let low = (1.0 - ((to_sun.y - 0.02) / 0.26).abs()).clamp(0.0, 1.0);
    let blued = BLUED * low.sqrt() * (1.0 - dark);
    // The sun's glow is its own colour at that height, and goes out as it goes under.
    let tint = through(to_sun.y);
    let sun = tint / tint.max_element() * (0.42 * ((to_sun.y + 0.04) / 0.08).clamp(0.0, 1.0));
    // The moon's ring by how full it is, in the night only.
    let moon = Vec3::new(0.55, 0.68, 1.0) * (0.2 * sky::moon_lit(when) * (1.0 - day) * ((to_moon.y + 0.02) / 0.06).clamp(0.0, 1.0));
    let now = [floor.extend(1.0), BLUE.extend(blued), sun.extend(1.0), moon.extend(1.0)];
    for (i, colour) in now.into_iter().enumerate() {
        if (colour - was[i]).abs().max_element() < 0.0004 {
            continue;
        }
        was[i] = colour;
        if let Some(mut material) = materials.get_mut(&lights.0[i]) {
            material.base_color = Color::linear_rgba(colour.x, colour.y, colour.z, colour.w);
        }
    }
}
