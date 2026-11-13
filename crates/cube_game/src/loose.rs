//! What has been cut loose is seen to fall (`cube_sim::loose`): each piece is drawn as the
//! cells it is, moved as the simulation moves it, until it lands and is the world's again.
//! The simulation owns all of it; this only draws.

use crate::motion::Live;
use bevy::{asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::loose::{Part, Piece};
use cube_voxel::{Block, SPLIT};

pub struct LoosePlugin;

impl Plugin for LoosePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, fall.after(crate::motion::tick));
    }
}

/// A piece being drawn: which, and what it turns about.
#[derive(Component)]
struct Falling(u32, Vec3);

fn fall(mut commands: Commands, live: Option<Res<Live>>, look: Option<Res<crate::world::Look>>, mut drawn: Query<(Entity, &Falling, &mut Transform)>, mut meshes: ResMut<Assets<Mesh>>) {
    let (Some(live), Some(look)) = (live, look) else { return };
    let pieces = &live.0.loose.pieces;
    for (entity, falling, mut at) in &mut drawn {
        match pieces.iter().find(|p| p.id == falling.0) {
            // (A point of it that was at `p` is at pivot + shift + turn (p - pivot).)
            Some(piece) => *at = Transform { translation: falling.1 + piece.shift - piece.turn * falling.1, rotation: piece.turn, scale: Vec3::ONE },
            None => commands.entity(entity).despawn(),
        }
    }
    for piece in pieces.iter().filter(|p| !drawn.iter().any(|(_, f, _)| f.0 == p.id)) {
        commands.spawn((Falling(piece.id, piece.pivot), Mesh3d(meshes.add(shape(piece))), MeshMaterial3d(look.solid.clone()), Transform::default()));
    }
}

/// A piece drawn as the world draws its cells, where it stood: a world of its own holding
/// only the piece, meshed by the world's own mesher, so that what falls is what stood and
/// what will lie. (Drawn first as plain cubes of one colour, a birch went grey as it fell.)
fn shape(piece: &Piece) -> Mesh {
    let mut alone = cube_voxel::World::default();
    for (cell, part) in &piece.cells {
        match part {
            Part::Whole(block) => alone.set(*cell, *block),
            Part::Bits(bits) => {
                for (i, block) in bits.iter().enumerate().filter(|(_, b)| **b != Block::Air) {
                    let i = i as i32;
                    alone.set_bit(*cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT), *block);
                }
            }
        }
    }
    let mut all = cube_voxel::mesh::MeshData::default();
    let chunks: Vec<IVec3> = alone.chunks.keys().copied().collect();
    for chunk in chunks {
        let data = cube_voxel::mesh::mesh_chunk(&alone, chunk, false);
        let from = all.positions.len() as u32;
        all.positions.extend(data.positions);
        all.normals.extend(data.normals);
        all.colors.extend(data.colors);
        all.uvs.extend(data.uvs);
        all.tangents.extend(data.tangents);
        all.indices.extend(data.indices.into_iter().map(|i| i + from));
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, all.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, all.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, all.colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, all.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, all.tangents)
        .with_inserted_indices(Indices::U32(all.indices))
}
