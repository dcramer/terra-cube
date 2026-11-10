//! Voxel world data and meshing. No engine types: the game crate turns [`MeshData`] into a
//! render mesh, and the simulation will read and write [`World`] without a renderer.
//!
//! Map: `Block` (what a cell is) → `Chunk` (32³ cells) → `World` (chunks by position) →
//! `terrain::generate` (fills a world) → `mesh::mesh_chunk` (visible faces with baked AO).

pub mod mesh;
pub mod terrain;

use glam::IVec3;
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
    fn a_lone_block_meshes_to_six_faces() {
        let mut w = World::default();
        w.set(IVec3::new(3, 3, 3), Block::Stone);
        let m = mesh::mesh_chunk(&w, IVec3::ZERO);
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
