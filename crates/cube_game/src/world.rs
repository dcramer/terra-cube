//! The world's terrain (`cube_sim::terrain`), meshed a chunk at a time, with sun, sky, fog and
//! a sea that runs on to the horizon.

use bevy::{asset::RenderAssetUsages, light::CascadeShadowConfigBuilder, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::terrain::{self, Land, SEA};
use cube_voxel::{CELL, World as Voxels, mesh::mesh_chunk};

pub struct WorldPlugin;

/// The voxel world, and the lie of the land it was made from (heights, biomes, trees).
#[derive(Resource)]
pub struct Terrain(pub Voxels, pub Land);

impl Terrain {
    /// The ground's height at a point, metres: the land itself, not a tree over it.
    pub fn ground(&self, x: f32, z: f32) -> f32 {
        self.1.height((x / CELL).floor() as i32, (z / CELL).floor() as i32) as f32 * CELL
    }
}

/// Far things fade into the sky: the island's far shore is hazy, the sea has no edge.
pub fn fog() -> DistanceFog {
    DistanceFog { color: SKY, falloff: FogFalloff::Linear { start: 80.0, end: 330.0 }, ..default() }
}

pub const SKY: Color = Color::srgb(0.53, 0.72, 0.90);

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(SKY)).add_systems(Startup, setup);
    }
}

pub fn spawn_lights(commands: &mut Commands, shadows: bool) {
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.85, 1.0), brightness: 480.0, ..default() });
    commands.spawn((
        DirectionalLight { illuminance: 3_800.0, shadow_maps_enabled: shadows, color: Color::srgb(1.0, 0.96, 0.88), ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, -0.9, 0.0)),
        CascadeShadowConfigBuilder { first_cascade_far_bound: 12.0, maximum_distance: 160.0, ..default() }.build(),
    ));
}

fn setup(mut commands: Commands, args: Res<crate::Args>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let (voxels, land) = terrain::generate(args.seed, terrain::RADIUS);
    let material = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, reflectance: 0.2, ..default() });
    // Water is its own mesh and material: it is blended over what is under it.
    let water = materials.add(StandardMaterial { base_color: Color::WHITE, alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.25, reflectance: 0.5, ..default() });
    for &chunk in voxels.chunks.keys() {
        for (liquid, material) in [(false, &material), (true, &water)] {
            let data = mesh_chunk(&voxels, chunk, liquid);
            if data.indices.is_empty() {
                continue;
            }
            let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, data.colors)
                .with_inserted_indices(Indices::U32(data.indices));
            commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone())));
        }
    }
    // The sea beyond the world's edge: four sheets round it at the water's level, out past the fog.
    let (edge, far, level) = (land.span as f32 * CELL, 1500.0, SEA as f32 * CELL - 0.01);
    let [r, g, b] = cube_voxel::Block::Water.colors().0;
    let sea = materials.add(StandardMaterial { base_color: Color::linear_rgb(r, g, b), perceptual_roughness: 0.25, reflectance: 0.5, ..default() });
    for (x, z, w, d) in [(0.0, -(edge + far) / 2.0, far, (far - edge) / 2.0), (0.0, (edge + far) / 2.0, far, (far - edge) / 2.0), (-(edge + far) / 2.0, 0.0, (far - edge) / 2.0, edge), ((edge + far) / 2.0, 0.0, (far - edge) / 2.0, edge)] {
        commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(w * 2.0, d * 2.0))), MeshMaterial3d(sea.clone()), Transform::from_xyz(x, level, z)));
    }
    commands.insert_resource(Terrain(voxels, land));
    spawn_lights(&mut commands, true);
}
