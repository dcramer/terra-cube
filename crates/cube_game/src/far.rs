//! The land beyond the cells: the whole island as a light mesh made from the lie of the land
//! (`Land`: heights, biomes, where the trees stand), shown wherever the real cells have not
//! been made. A region's worth to a piece, four metres between its points, with a plain
//! shape standing in for each tree. It is what makes a mountain two valleys off something
//! that is there, for a few thousand faces; walk toward it and the real ground takes its place.

use crate::world::{Look, Regions, Terrain};
use bevy::{asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::{
    plants::Tree,
    terrain::{self, Biome, SEA},
};
use cube_voxel::{Block, CELL, texture::{Grain, MEAN, SPAN}};

pub struct FarPlugin;

impl Plugin for FarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (build, show).chain());
    }
}

/// A piece of the far land: the region it stands in for.
#[derive(Component)]
struct Far((i32, i32));

/// Metres between the far land's points.
const STEP: f32 = 4.0;
/// How far under the true ground it lies, metres, so that where both are drawn the true
/// ground is the one seen.
const SUNK: f32 = 0.7;
/// A region's far piece is hidden once its cells are made and the camera is this near its
/// middle (by then its real meshes have had time to be made).
const NEAR: f32 = 150.0;

#[derive(Default)]
struct Shape {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
    tangents: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Shape {
    fn point(&mut self, at: Vec3, normal: Vec3, colour: [f32; 3]) -> u32 {
        self.positions.push(at.to_array());
        self.normals.push(normal.to_array());
        self.colors.push([colour[0] / MEAN, colour[1] / MEAN, colour[2] / MEAN, 1.0]);
        self.uvs.push(Grain::Plain.uv(SPAN as f32 * 0.5, SPAN as f32 * 0.5));
        self.tangents.push([1.0, 0.0, 0.0, 1.0]);
        self.positions.len() as u32 - 1
    }

    /// A box from `lo` to `hi`, with no underside.
    fn block(&mut self, lo: Vec3, hi: Vec3, colour: [f32; 3]) {
        let corner = |x: bool, y: bool, z: bool| Vec3::new(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z });
        for (normal, quad) in [
            (Vec3::Y, [(false, true, false), (false, true, true), (true, true, true), (true, true, false)]),
            (Vec3::X, [(true, false, false), (true, true, false), (true, true, true), (true, false, true)]),
            (Vec3::NEG_X, [(false, false, true), (false, true, true), (false, true, false), (false, false, false)]),
            (Vec3::Z, [(true, false, true), (true, true, true), (false, true, true), (false, false, true)]),
            (Vec3::NEG_Z, [(false, false, false), (false, true, false), (true, true, false), (true, false, false)]),
        ] {
            // The sides in a little shade, as the true trees' are.
            let shade = if normal == Vec3::Y { 1.0 } else { 0.72 };
            let at: Vec<u32> = quad.iter().map(|c| self.point(corner(c.0, c.1, c.2), normal, colour.map(|v| v * shade))).collect();
            self.indices.extend([at[0], at[1], at[2], at[0], at[2], at[3]]);
        }
    }
}

impl Shape {
    /// A rounded crown: a ball of a few dozen faces, squashed to its reach.
    fn crown(&mut self, middle: Vec3, radii: Vec3, colour: [f32; 3], ball: &[[Vec3; 3]]) {
        for corners in ball {
            let start = self.positions.len() as u32;
            for d in corners {
                // Lit from above more than below, as a crown is: its underside is in its own shade.
                let shade = 0.62 + 0.45 * (d.y * 0.5 + 0.5);
                self.point(middle + *d * radii, (*d / radii).normalize_or(Vec3::Y), colour.map(|c| c * shade));
            }
            self.indices.extend([start, start + 1, start + 2]);
        }
    }

    /// A cone standing on `foot`: a conifer's crown.
    fn cone(&mut self, foot: Vec3, wide: f32, tall: f32, colour: [f32; 3]) {
        const SIDES: usize = 7;
        let tip = self.point(foot + Vec3::Y * tall, Vec3::Y, colour);
        let ring: Vec<u32> = (0..SIDES)
            .map(|k| {
                let turn = k as f32 / SIDES as f32 * std::f32::consts::TAU;
                let out = Vec3::new(turn.cos(), 0.0, turn.sin());
                self.point(foot + out * wide, (out + Vec3::Y * 0.5).normalize(), colour.map(|c| c * 0.7))
            })
            .collect();
        for k in 0..SIDES {
            self.indices.extend([tip, ring[(k + 1) % SIDES], ring[k]]);
        }
    }
}

fn ball() -> Vec<[Vec3; 3]> {
    let mesh = Sphere::new(1.0).mesh().ico(1).unwrap();
    let Some(bevy::mesh::VertexAttributeValues::Float32x3(at)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { return Vec::new() };
    let Some(Indices::U32(index)) = mesh.indices() else { return Vec::new() };
    index.chunks(3).map(|t| [Vec3::from(at[t[0] as usize]).normalize(), Vec3::from(at[t[1] as usize]).normalize(), Vec3::from(at[t[2] as usize]).normalize()]).collect()
}

/// Makes every piece once the land is there.
fn build(mut commands: Commands, terrain: Option<Res<Terrain>>, look: Option<Res<Look>>, mut meshes: ResMut<Assets<Mesh>>, mut done: Local<bool>) {
    let (Some(terrain), Some(look)) = (terrain, look) else { return };
    if std::mem::replace(&mut *done, true) {
        return;
    }
    let land = &terrain.1;
    let (size, edge) = (terrain::REGION as f32 * CELL, land.span.div_euclid(terrain::REGION));
    let ground = |x: f32, z: f32| land.height((x / CELL).floor() as i32, (z / CELL).floor() as i32) as f32 * CELL;
    // Which trees stand in which region.
    let mut trees: std::collections::HashMap<(i32, i32), Vec<usize>> = std::collections::HashMap::new();
    for (i, foot) in land.trees.iter().enumerate() {
        trees.entry((foot.x.div_euclid(terrain::REGION), foot.z.div_euclid(terrain::REGION))).or_default().push(i);
    }
    let n = (size / STEP) as i32;
    let crowns = ball();
    for rz in -edge..edge {
        for rx in -edge..edge {
            let (ox, oz) = (rx as f32 * size, rz as f32 * size);
            let mut shape = Shape::default();
            let (mut dry, mut wet) = (false, false);
            for j in 0..=n {
                for i in 0..=n {
                    let (x, z) = (ox + i as f32 * STEP, oz + j as f32 * STEP);
                    let h = ground(x, z);
                    dry |= h > SEA as f32 * CELL;
                    wet |= h < SEA as f32 * CELL;
                    let slope = Vec3::new(ground(x - STEP, z) - ground(x + STEP, z), STEP * 2.0, ground(x, z - STEP) - ground(x, z + STEP)).normalize();
                    let block = match land.biome((x / CELL).floor() as i32, (z / CELL).floor() as i32) {
                        Biome::Meadow => Block::Grass,
                        Biome::Forest => Block::Turf,
                        Biome::Rock => Block::Stone,
                        Biome::Snow => Block::Snow,
                        Biome::Shore | Biome::Water => Block::Sand,
                    };
                    // A steep face shows its side, as a cliff does.
                    let colour = if slope.y < 0.75 { block.colors().1 } else { block.colors().0 };
                    shape.point(Vec3::new(x - ox, h - SUNK, z - oz), slope, colour);
                }
            }
            // Where any of it is under water, the water's surface over it.
            if wet {
                let level = SEA as f32 * CELL - 0.03;
                let mesh = Mesh::from(Plane3d::default().mesh().size(size, size));
                commands.spawn((Far((rx, rz)), Mesh3d(meshes.add(mesh)), MeshMaterial3d(look.sea.clone()), Transform::from_xyz(ox + size * 0.5, level, oz + size * 0.5), bevy::light::NotShadowCaster));
            }
            if !dry {
                continue;
            }
            for j in 0..n {
                for i in 0..n {
                    let at = |i: i32, j: i32| (i + j * (n + 1)) as u32;
                    shape.indices.extend([at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
                }
            }
            for &t in trees.get(&(rx, rz)).into_iter().flatten() {
                let (foot, tall, sort) = (land.trees[t], land.grown[t], land.sorts[t]);
                let at = Vec3::new(foot.x as f32 * CELL - ox, foot.y as f32 * CELL - SUNK, foot.z as f32 * CELL - oz);
                let bark = if sort == Tree::Birch { Block::Birch } else { Block::Wood }.colors().1;
                let trunk = tall * 0.028;
                let block = |shape: &mut Shape, from: f32, to: f32, wide: f32, colour: [f32; 3]| shape.block(at + Vec3::new(-wide, from * tall, -wide), at + Vec3::new(wide, to * tall, wide), colour);
                let up = |part: f32| at + Vec3::Y * tall * part;
                match sort {
                    // A point of two tiers of cone.
                    Tree::Pine => {
                        block(&mut shape, 0.0, 0.3, trunk, bark);
                        shape.cone(up(0.26), tall * 0.17, tall * 0.5, Block::Needles.colors().0);
                        shape.cone(up(0.55), tall * 0.11, tall * 0.45, Block::Needles.colors().0);
                    }
                    Tree::Snag => block(&mut shape, 0.0, 1.0, trunk * 1.2, bark),
                    // A trunk and a rounded crown as wide as it is tall, or for a birch a narrow one.
                    _ => {
                        let (leaf, wide, from) = match sort {
                            Tree::Birch => (Block::LeavesPale, 0.15, 0.45),
                            Tree::Willow => (Block::LeavesPale, 0.38, 0.2),
                            _ => (Block::Leaves, 0.33, 0.36),
                        };
                        block(&mut shape, 0.0, from + 0.1, trunk, bark);
                        shape.crown(up((from + 1.0) / 2.0), Vec3::new(tall * wide, tall * (1.0 - from) / 2.0, tall * wide), leaf.colors().0, &crowns);
                    }
                }
            }
            let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, shape.positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, shape.normals)
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, shape.colors)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, shape.uvs)
                .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, shape.tangents)
                .with_inserted_indices(Indices::U32(shape.indices));
            commands.spawn((Far((rx, rz)), Mesh3d(meshes.add(mesh)), MeshMaterial3d(look.solid.clone()), Transform::from_xyz(ox, 0.0, oz), bevy::light::NotShadowCaster));
        }
    }
}

/// A piece is shown unless the real ground of its region is there and near.
fn show(regions: Res<Regions>, cameras: Query<&GlobalTransform, With<Camera3d>>, mut pieces: Query<(&Far, &Transform, &mut Visibility)>) {
    let Some(eye) = cameras.iter().next().map(|c| c.translation()) else { return };
    let half = terrain::REGION as f32 * CELL * 0.5;
    for (far, at, mut seen) in &mut pieces {
        let size = half * 2.0;
        let middle = Vec2::new((far.0.0 as f32 + 0.5) * size, (far.0.1 as f32 + 0.5) * size);
        let _ = at;
        let away = (middle - eye.xz()).length();
        let want = if regions.is_made(far.0) && away < NEAR { Visibility::Hidden } else { Visibility::Inherited };
        if *seen != want {
            *seen = want;
        }
    }
}
