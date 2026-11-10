//! Voxel world data and meshing. No engine types: the game crate turns [`MeshData`] into a
//! render mesh, and the simulation will read and write [`World`] without a renderer.
//!
//! Map: `Block` (what a cell is) → `Chunk` (32³ cells) → `World` (chunks by position) →
//! `terrain::generate` (fills a world) → `mesh::mesh_chunk` (visible faces with baked AO).

pub mod mesh;
pub mod terrain;

use glam::{IVec3, Vec3};
use std::collections::HashMap;

/// Cells along a chunk's edge.
pub const CHUNK: i32 = 32;
/// A cell's edge in metres. Half a metre: finer than Minecraft's 1 m, as Eco's blocks read.
pub const CELL: f32 = 0.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
#[repr(u8)]
pub enum Block {
    #[default]
    Air = 0,
    Grass,
    Dirt,
    Stone,
    Sand,
    Snow,
    Water,
}

impl Block {
    pub fn solid(self) -> bool {
        !matches!(self, Block::Air | Block::Water)
    }
    /// Hides the faces of neighbours behind it.
    pub fn opaque(self) -> bool {
        self.solid()
    }
    /// Linear RGB of the block's top and of its sides.
    pub fn colors(self) -> ([f32; 3], [f32; 3]) {
        match self {
            Block::Air => ([0.0; 3], [0.0; 3]),
            Block::Grass => ([0.21, 0.42, 0.10], [0.27, 0.17, 0.09]),
            Block::Dirt => ([0.27, 0.17, 0.09], [0.25, 0.16, 0.085]),
            Block::Stone => ([0.30, 0.30, 0.31], [0.26, 0.26, 0.275]),
            Block::Sand => ([0.66, 0.56, 0.36], [0.62, 0.52, 0.33]),
            Block::Snow => ([0.85, 0.87, 0.92], [0.78, 0.80, 0.88]),
            Block::Water => ([0.08, 0.26, 0.42], [0.08, 0.26, 0.42]),
        }
    }
}

#[derive(Clone)]
pub struct Chunk {
    cells: Box<[Block]>,
}

impl Default for Chunk {
    fn default() -> Self {
        Self { cells: vec![Block::Air; (CHUNK * CHUNK * CHUNK) as usize].into_boxed_slice() }
    }
}

impl Chunk {
    fn index(p: IVec3) -> usize {
        (p.x + p.z * CHUNK + p.y * CHUNK * CHUNK) as usize
    }
    pub fn get(&self, p: IVec3) -> Block {
        self.cells[Self::index(p)]
    }
    pub fn set(&mut self, p: IVec3, b: Block) {
        self.cells[Self::index(p)] = b;
    }
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(|&b| b == Block::Air)
    }
}

/// Chunks by chunk position (cell position / CHUNK, floored).
#[derive(Default)]
pub struct World {
    pub chunks: HashMap<IVec3, Chunk>,
}

impl World {
    pub fn split(cell: IVec3) -> (IVec3, IVec3) {
        (cell.div_euclid(IVec3::splat(CHUNK)), cell.rem_euclid(IVec3::splat(CHUNK)))
    }
    pub fn get(&self, cell: IVec3) -> Block {
        let (c, l) = Self::split(cell);
        self.chunks.get(&c).map_or(Block::Air, |ch| ch.get(l))
    }
    pub fn set(&mut self, cell: IVec3, b: Block) {
        let (c, l) = Self::split(cell);
        self.chunks.entry(c).or_default().set(l, b);
    }
    /// How far along a ray the first solid cell is, in metres, if within `reach`. Walks the
    /// grid a cell face at a time, so nothing thin is skipped.
    pub fn raycast(&self, from: Vec3, dir: Vec3, reach: f32) -> Option<f32> {
        let dir = dir.try_normalize()?;
        let mut cell = (from / CELL).floor().as_ivec3();
        let step = dir.signum().as_ivec3();
        // Distance along the ray to the next cell face on each axis, and between faces.
        let between = (Vec3::splat(CELL) / dir.abs()).min(Vec3::splat(f32::MAX));
        let next_face = (cell + step.max(IVec3::ZERO)).as_vec3() * CELL;
        let mut next = ((next_face - from) / dir).abs().min(Vec3::splat(f32::MAX));
        for axis in 0..3 {
            if dir[axis] == 0.0 {
                next[axis] = f32::MAX;
            }
        }
        let mut at = 0.0;
        while at <= reach {
            if self.get(cell).solid() {
                return Some(at);
            }
            let axis = if next.x <= next.y && next.x <= next.z { 0 } else if next.y <= next.z { 1 } else { 2 };
            at = next[axis];
            next[axis] += between[axis];
            cell[axis] += step[axis];
        }
        None
    }

    /// The y of the top face of the highest solid cell in a column, in metres.
    pub fn ground_height(&self, x: f32, z: f32) -> f32 {
        let (cx, cz) = ((x / CELL).floor() as i32, (z / CELL).floor() as i32);
        let top = self.chunks.keys().map(|c| c.y).max().unwrap_or(0) * CHUNK + CHUNK - 1;
        let bottom = self.chunks.keys().map(|c| c.y).min().unwrap_or(0) * CHUNK;
        (bottom..=top)
            .rev()
            .find(|&y| self.get(IVec3::new(cx, y, cz)).solid())
            .map_or(0.0, |y| (y + 1) as f32 * CELL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_across_chunks() {
        let mut w = World::default();
        w.set(IVec3::new(-1, 40, 33), Block::Stone);
        assert_eq!(w.get(IVec3::new(-1, 40, 33)), Block::Stone);
        assert_eq!(w.get(IVec3::new(0, 40, 33)), Block::Air);
        assert_eq!(w.ground_height(-0.25, 16.7), 41.0 * CELL);
    }

    #[test]
    fn a_ray_stops_at_the_first_solid_face() {
        let mut w = World::default();
        w.set(IVec3::new(6, 2, 0), Block::Stone); // x from 3.0 to 3.5
        let from = Vec3::new(0.25, 1.25, 0.25);
        assert_eq!(w.raycast(from, Vec3::X, 10.0), Some(2.75));
        assert_eq!(w.raycast(from, Vec3::X, 2.0), None);
        assert_eq!(w.raycast(from, Vec3::NEG_X, 10.0), None);
        let slant = w.raycast(Vec3::new(0.25, 4.0, 0.25), Vec3::new(3.0, -2.75, 0.0), 10.0).unwrap();
        // It meets the face at x = 3.0, eleven twelfths of the way along (3, -2.75).
        assert!((slant - 3.0f32.hypot(2.75) * 11.0 / 12.0).abs() < 1e-3, "{slant}");
    }

    #[test]
    fn a_lone_block_meshes_to_six_faces() {
        let mut w = World::default();
        w.set(IVec3::new(3, 3, 3), Block::Stone);
        let m = mesh::mesh_chunk(&w, IVec3::ZERO, false);
        assert_eq!(m.positions.len(), 24);
        assert_eq!(m.indices.len(), 36);
    }

    #[test]
    fn terrain_is_deterministic_and_grounded() {
        let a = terrain::generate(7, 2);
        let b = terrain::generate(7, 2);
        assert_eq!(a.ground_height(3.0, 3.0), b.ground_height(3.0, 3.0));
        assert!(a.ground_height(3.0, 3.0) > 0.0);
    }
}
