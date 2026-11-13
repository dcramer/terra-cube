//! What has been cut loose is seen to fall (`cube_sim::loose`): each piece is drawn as the
//! cells it is, moved as the simulation moves it, until it lands and is the world's again.
//! The simulation owns all of it; this only draws, and sounds it: a cut is heard and throws
//! chips, a tree cracks as it begins to go, and what lands is heard by its size and throws
//! up what it is made of.

use crate::motion::Live;
use bevy::{asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::loose::Part;
use cube_voxel::{Block, SPLIT};

pub struct LoosePlugin;

impl Plugin for LoosePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (fall, told, chips).after(crate::motion::tick));
    }
}

/// A piece being drawn: which; and, once it has landed, the seconds it is still drawn (the
/// chunks it went into are meshed on other threads, and for the frames that takes it was
/// nowhere: a felled tree blinked as it came to rest).
#[derive(Component)]
struct Falling(u32, Option<f32>);

fn fall(mut commands: Commands, live: Option<Res<Live>>, look: Option<Res<crate::world::Look>>, mut drawn: Query<(Entity, &mut Falling, &mut Transform)>, time: Res<Time>, mut meshes: ResMut<Assets<Mesh>>, mut making: Local<std::collections::HashMap<u32, bevy::tasks::Task<cube_voxel::mesh::MeshData>>>) {
    let (Some(live), Some(look)) = (live, look) else { return };
    let pieces = &live.0.loose.pieces;
    for (entity, mut falling, mut at) in &mut drawn {
        match pieces.iter().find(|p| p.id == falling.0) {
            // (A point of it that was at `p` is at pivot + shift + turn (p - pivot).)
            // (About what it turns about now: that changes as a tree's top strikes and it
            // settles about its top. Drawn about where it first stood, it was gone for the
            // second it took to settle.)
            Some(piece) => *at = Transform { translation: piece.pivot + piece.shift - piece.turn * piece.pivot, rotation: piece.turn, scale: Vec3::ONE },
            None => {
                let left = falling.1.unwrap_or(0.25) - time.delta_secs();
                falling.1 = Some(left);
                if left <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
    // Each new piece's mesh is made on another thread (a tree is ten thousand cells, and made
    // here it was a frame of 40 ms as it came loose) and drawn when it is ready, a frame or
    // two on: the chunks it was cut from are still drawn as they were for as long.
    making.retain(|id, _| pieces.iter().any(|p| p.id == *id));
    for piece in pieces.iter().filter(|p| !drawn.iter().any(|(_, f, _)| f.0 == p.id)) {
        match making.get_mut(&piece.id).map(bevy::tasks::futures::check_ready) {
            Some(Some(data)) => {
                making.remove(&piece.id);
                let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, data.colors)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, data.tangents)
                    .with_inserted_indices(Indices::U32(data.indices));
                commands.spawn((Falling(piece.id, None), Mesh3d(meshes.add(mesh)), MeshMaterial3d(look.solid.clone()), Transform { translation: piece.pivot + piece.shift - piece.turn * piece.pivot, rotation: piece.turn, scale: Vec3::ONE }));
            }
            Some(None) => {}
            None => {
                let cells = piece.cells.clone();
                making.insert(piece.id, bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { shape(&cells) }));
            }
        }
    }
}

/// A piece drawn as the world draws its cells, where it stood: a world of its own holding
/// only the piece, meshed by the world's own mesher, so that what falls is what stood and
/// what will lie. (Drawn first as plain cubes of one colour, a birch went grey as it fell.)
fn shape(cells: &std::collections::HashMap<IVec3, Part>) -> cube_voxel::mesh::MeshData {
    let mut alone = cube_voxel::World::default();
    for (cell, part) in cells {
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
    all
}

/// A chip thrown up: how it is going, and the seconds it has left.
#[derive(Component)]
struct Chip(Vec3, f32);

/// What the simulation noted since last frame is heard and seen: each cut, each thing come
/// loose, each landing.
#[allow(clippy::too_many_arguments)]
fn told(mut commands: Commands, live: Option<ResMut<Live>>, mut bank: ResMut<crate::audio::Bank>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut cube: Local<Option<Handle<Mesh>>>, mut paints: Local<std::collections::HashMap<[u8; 3], Handle<StandardMaterial>>>, mut n: Local<u32>, time: Res<Time>, mut clattered: Local<f32>) {
    let Some(mut live) = live else { return };
    let loose = &mut live.0.loose;
    let cube = cube.get_or_insert_with(|| meshes.add(Cuboid::new(1.0, 1.0, 1.0))).clone();
    // (Nothing here matters to the world: where a chip goes is only seen.)
    let mut odd = || {
        *n = n.wrapping_mul(1664525).wrapping_add(1013904223);
        (*n >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    };
    let mut throw = |commands: &mut Commands, at: Vec3, block: Block, many: usize, reach: f32, up: f32| {
        let colour = block.colors().1;
        let key = colour.map(|c| (c * 255.0) as u8);
        let paint = paints.entry(key).or_insert_with(|| materials.add(StandardMaterial { base_color: Color::linear_rgb(colour[0], colour[1], colour[2]), perceptual_roughness: 0.95, ..default() })).clone();
        for _ in 0..many {
            let (out, size) = (Vec3::new(odd(), 0.0, odd()), 0.03 + 0.04 * odd().abs());
            commands.spawn((Chip(out * reach * 2.5 + Vec3::Y * up * (0.5 + odd().abs()), 0.7 + 0.5 * odd().abs()), Mesh3d(cube.clone()), MeshMaterial3d(paint.clone()), Transform { translation: at + out * reach * 0.3 + Vec3::Y * 0.1, scale: Vec3::splat(size), ..default() }, bevy::light::NotShadowCaster));
        }
    };
    for (at, taken) in loose.cuts.drain(..) {
        let (name, block) = if taken[1] >= taken[0] && taken[1] >= taken[2] { ("chop", Block::Wood) } else if taken[2] > taken[0] { ("pick", Block::Stone) } else { ("dig", Block::Dirt) };
        bank.play(&mut commands, name, Some(at), 0.8);
        throw(&mut commands, at, block, 7, 0.4, 2.2);
    }
    for (at, cells, over) in loose.begun.drain(..) {
        if over && cells > 200 {
            bank.play(&mut commands, "crack", Some(at), 1.0);
        }
    }
    for landed in loose.heard.drain(..) {
        let size = (landed.cells as f32).cbrt() * cube_voxel::CELL;
        // A great thing is a crash, a lump a clatter; louder the faster it came.
        let loud = (landed.speed / 12.0).clamp(0.25, 1.0);
        // (A crown that breaks is ninety limbs landing in half a second: a clatter is heard a
        // few times in that, not ninety.)
        let now = time.elapsed_secs();
        if landed.cells > 400 {
            bank.play(&mut commands, "crash", Some(landed.at), loud);
        } else if now - *clattered > 0.12 {
            *clattered = now;
            bank.pitched(&mut commands, "rubble", Some(landed.at), loud * 0.6, (1.6 - size * 0.5).clamp(0.8, 1.5));
        }
        throw(&mut commands, landed.at.with_y(landed.at.y - size * 0.3), Block::Dirt, (6.0 + size * 10.0).min(40.0) as usize, size.max(0.5), 2.5);
    }
}

/// Chips fly, fall and are gone.
fn chips(mut commands: Commands, time: Res<Time>, mut flying: Query<(Entity, &mut Chip, &mut Transform)>) {
    let dt = time.delta_secs();
    for (entity, mut chip, mut at) in &mut flying {
        chip.0.y -= 14.0 * dt;
        at.translation += chip.0 * dt;
        at.rotate_local_x(6.0 * dt);
        chip.1 -= dt;
        if chip.1 < 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
