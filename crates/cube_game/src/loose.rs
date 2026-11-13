//! What has been cut loose is seen to fall (`cube_sim::loose`): each piece is drawn as the
//! cells it is, moved as the simulation moves it, until it lands and is the world's again.
//! The simulation owns all of it; this only draws.

use crate::motion::Live;
use bevy::{asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::loose::{Part, Piece};
use cube_voxel::{BIT, Block, CELL, SPLIT};

pub struct LoosePlugin;

impl Plugin for LoosePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, fall.after(crate::motion::tick));
    }
}

/// A piece being drawn: which, and what it turns about.
#[derive(Component)]
struct Falling(u32, Vec3);

fn fall(mut commands: Commands, live: Option<Res<Live>>, mut drawn: Query<(Entity, &Falling, &mut Transform)>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut paint: Local<Option<Handle<StandardMaterial>>>) {
    let Some(live) = live else { return };
    let pieces = &live.0.loose.pieces;
    for (entity, falling, mut at) in &mut drawn {
        match pieces.iter().find(|p| p.id == falling.0) {
            // (A point of it that was at `p` is at pivot + shift + turn (p - pivot).)
            Some(piece) => *at = Transform { translation: falling.1 + piece.shift - piece.turn * falling.1, rotation: piece.turn, scale: Vec3::ONE },
            None => commands.entity(entity).despawn(),
        }
    }
    for piece in pieces.iter().filter(|p| !drawn.iter().any(|(_, f, _)| f.0 == p.id)) {
        let paint = paint.get_or_insert_with(|| materials.add(StandardMaterial { perceptual_roughness: 0.95, ..default() })).clone();
        commands.spawn((Falling(piece.id, piece.pivot), Mesh3d(meshes.add(shape(piece))), MeshMaterial3d(paint), Transform::default()));
    }
}

/// A piece as cubes, where it stood in the world: a cell a cube, a broken cell its bits;
/// only the faces that nothing of the piece covers.
fn shape(piece: &Piece) -> Mesh {
    let (mut positions, mut normals, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let bit_at = |cell: IVec3, l: IVec3| -> Block {
        // (A bit beside this cell's own: in the cell next to it.)
        let (cell, l) = (cell + l.div_euclid(IVec3::splat(SPLIT)), l.rem_euclid(IVec3::splat(SPLIT)));
        match piece.cells.get(&cell) {
            Some(Part::Whole(b)) => *b,
            Some(Part::Bits(bits)) => bits[(l.x + l.z * SPLIT + l.y * SPLIT * SPLIT) as usize],
            None => Block::Air,
        }
    };
    let mut cube = |lo: Vec3, size: f32, block: Block, open: [bool; 6]| {
        let (top, side) = block.colors();
        for (n, (normal, corners)) in FACES.iter().enumerate().filter(|(n, _)| open[*n]) {
            let from = positions.len() as u32;
            let tint = if n == 2 { top } else { side };
            for corner in corners {
                positions.push((lo + *corner * size).to_array());
                normals.push(normal.to_array());
                colors.push([tint[0], tint[1], tint[2], 1.0]);
            }
            indices.extend([from, from + 1, from + 2, from, from + 2, from + 3]);
        }
    };
    for (cell, part) in &piece.cells {
        match part {
            Part::Whole(block) => {
                let open = SIDES.map(|d| !matches!(piece.cells.get(&(*cell + d)), Some(Part::Whole(b)) if b.solid()));
                cube(cell.as_vec3() * CELL, CELL, *block, open);
            }
            Part::Bits(bits) => {
                for (i, block) in bits.iter().enumerate().filter(|(_, b)| b.solid()) {
                    let l = IVec3::new(i as i32 % SPLIT, i as i32 / (SPLIT * SPLIT), i as i32 / SPLIT % SPLIT);
                    let open = SIDES.map(|d| !bit_at(*cell, l + d).solid());
                    if open.iter().any(|o| *o) {
                        cube(cell.as_vec3() * CELL + l.as_vec3() * BIT, BIT, *block, open);
                    }
                }
            }
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices))
}

const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];
/// Each face of a unit cube, in the order of [`SIDES`]: its normal, and its corners turning
/// left about it as seen from outside.
const FACES: [(Vec3, [Vec3; 4]); 6] = [
    (Vec3::X, [Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 1.0), Vec3::new(1.0, 0.0, 1.0)]),
    (Vec3::NEG_X, [Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 1.0, 1.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 0.0)]),
    (Vec3::Y, [Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 1.0), Vec3::new(1.0, 1.0, 1.0), Vec3::new(1.0, 1.0, 0.0)]),
    (Vec3::NEG_Y, [Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 1.0)]),
    (Vec3::Z, [Vec3::new(1.0, 0.0, 1.0), Vec3::new(1.0, 1.0, 1.0), Vec3::new(0.0, 1.0, 1.0), Vec3::new(0.0, 0.0, 1.0)]),
    (Vec3::NEG_Z, [Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0)]),
];
