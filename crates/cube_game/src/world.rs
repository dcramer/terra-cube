//! The test terrain, meshed a chunk at a time, with sun, sky and fog.

use bevy::{asset::RenderAssetUsages, light::CascadeShadowConfigBuilder, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_voxel::{World as Voxels, mesh::mesh_chunk, terrain};

pub struct WorldPlugin;

/// The voxel world, for ground height under creatures.
#[derive(Resource)]
pub struct Terrain(pub Voxels);

pub const SKY: Color = Color::srgb(0.53, 0.72, 0.90);

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(SKY)).add_systems(Startup, setup);
    }
}

pub fn spawn_lights(commands: &mut Commands, shadows: bool) {
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.85, 1.0), brightness: 110.0, ..default() });
    commands.spawn((
        DirectionalLight { illuminance: 3_800.0, shadow_maps_enabled: shadows, color: Color::srgb(1.0, 0.96, 0.88), ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, -0.9, 0.0)),
        CascadeShadowConfigBuilder { first_cascade_far_bound: 12.0, maximum_distance: 120.0, ..default() }.build(),
    ));
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let voxels = terrain::generate(7, 3);
    let material = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, reflectance: 0.2, ..default() });
    for &chunk in voxels.chunks.keys() {
        let data = mesh_chunk(&voxels, chunk);
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
    commands.insert_resource(Terrain(voxels));
    spawn_lights(&mut commands, true);
}
