//! Voxel world data and meshing. No engine types: the game crate turns [`MeshData`] into a
//! render mesh, and the simulation will read and write [`World`] without a renderer.
//!
//! Map: `Block` (what a cell is) → `Chunk` (32³ cells) → `World` (chunks by position, and
//! the cells that have been broken into bits) → `mesh::mesh_chunk` (visible faces with baked
//! AO). Worlds are made in `cube_sim::terrain`.
//!
//! Two sizes. A cell is [`CELL`] across and almost all of the world is whole cells. Any cell
//! can be broken into [`SPLIT`]³ bits, each [`BIT`] across, when something smaller than a
//! cell happens to it: a hole dug in it, a thing built on it. Only broken cells cost more.

pub mod mesh;
pub mod texture;

use glam::{IVec3, Vec3};
use std::collections::HashMap;

/// Cells along a chunk's edge.
pub const CHUNK: i32 = 32;
/// A cell's edge in metres. A quarter of a metre: a quarter of Minecraft's, so slopes read as
/// slopes and a tree's crown as round. Everything that means a distance is written in metres
/// and turned to cells with this (see [`cells`]), so it can change again.
pub const CELL: f32 = 0.25;

/// How many bits a cell breaks into along each edge, and a bit's edge in metres: 5 cm, the
/// smallest thing in the world.
pub const SPLIT: i32 = 5;
pub const BIT: f32 = CELL / SPLIT as f32;
/// Where the `n`th bit face is, metres. Exact wherever it is also a cell's face, which
/// `n as f32 * BIT` is not: 0.05 has no exact binary form.
pub fn bit_face(n: i32) -> f32 {
    n as f32 * CELL / SPLIT as f32
}
/// The bits of one broken cell, x then z then y.
pub type Bits = [Block; (SPLIT * SPLIT * SPLIT) as usize];

/// A length in metres as a whole number of cells, to the nearest.
pub fn cells(metres: f32) -> i32 {
    (metres / CELL).round() as i32
}

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
    /// The darker ground under trees.
    Turf,
    Gravel,
    Wood,
    Leaves,
    /// A conifer's foliage.
    Needles,
    /// A cell broken into bits: what is in it is in [`World::bits`].
    Broken,
    /// A birch's pale bark.
    Birch,
    /// Ground cover, drawn as blades and stems and walked through: long grass, a flower, reeds.
    Tuft,
    Flower,
    Reed,
    /// Foliage of other trees: pale and yellow-green (birch, willow), and deep (an old oak).
    LeavesPale,
    LeavesDeep,
    /// Stone in other beds: a dark band and a pale one, so a cliff shows how it was laid down.
    Slate,
    Chalk,
    /// Moss over stone, and the brown litter of fallen leaves under trees.
    Moss,
    Litter,
    /// More ground cover: a fern's wide fronds, a mushroom.
    Fern,
    Mushroom,
}

impl Block {
    pub fn solid(self) -> bool {
        !matches!(self, Block::Air | Block::Water) && !self.sprig()
    }
    /// Whether it is ground cover: drawn as a few blades, not as a cube.
    pub fn sprig(self) -> bool {
        matches!(self, Block::Tuft | Block::Flower | Block::Reed | Block::Fern | Block::Mushroom)
    }
    /// Leaves or needles: what a crown is made of.
    pub fn foliage(self) -> bool {
        matches!(self, Block::Leaves | Block::LeavesPale | Block::LeavesDeep | Block::Needles)
    }
    /// Hides the faces of neighbours behind it. A broken cell may have gaps, so it does not.
    pub fn opaque(self) -> bool {
        self.solid() && self != Block::Broken
    }
    /// Linear RGB of the block's top and of its sides.
    /// The land itself, as against what grows or is built on it: soil, sand, snow and stone.
    /// Its surface is drawn eased into slopes (`mesh::Eased`); everything else is drawn as
    /// the cubes it is.
    pub fn ground(self) -> bool {
        matches!(self, Block::Grass | Block::Dirt | Block::Stone | Block::Sand | Block::Snow | Block::Turf | Block::Gravel | Block::Slate | Block::Chalk | Block::Moss | Block::Litter)
    }

    pub fn colors(self) -> ([f32; 3], [f32; 3]) {
        match self {
            Block::Air | Block::Broken => ([0.0; 3], [0.0; 3]),
            Block::Grass => ([0.21, 0.42, 0.10], [0.15, 0.30, 0.08]),
            Block::Dirt => ([0.27, 0.17, 0.09], [0.25, 0.16, 0.085]),
            Block::Stone => ([0.30, 0.30, 0.31], [0.26, 0.26, 0.275]),
            Block::Sand => ([0.66, 0.56, 0.36], [0.62, 0.52, 0.33]),
            Block::Snow => ([0.85, 0.87, 0.92], [0.78, 0.80, 0.88]),
            Block::Water => ([0.05, 0.15, 0.24], [0.05, 0.15, 0.24]),
            Block::Turf => ([0.12, 0.30, 0.09], [0.09, 0.22, 0.07]),
            Block::Gravel => ([0.40, 0.38, 0.35], [0.36, 0.34, 0.32]),
            Block::Wood => ([0.30, 0.20, 0.11], [0.22, 0.14, 0.08]),
            Block::Leaves => ([0.16, 0.38, 0.10], [0.12, 0.31, 0.08]),
            Block::Needles => ([0.07, 0.24, 0.13], [0.05, 0.19, 0.11]),
            Block::Birch => ([0.62, 0.60, 0.54], [0.66, 0.64, 0.58]),
            Block::Tuft => ([0.30, 0.50, 0.12], [0.22, 0.42, 0.10]),
            Block::Flower => ([0.85, 0.70, 0.12], [0.22, 0.42, 0.10]),
            Block::Reed => ([0.55, 0.50, 0.24], [0.42, 0.44, 0.18]),
            Block::LeavesPale => ([0.36, 0.50, 0.12], [0.28, 0.42, 0.10]),
            Block::LeavesDeep => ([0.09, 0.27, 0.08], [0.07, 0.22, 0.07]),
            Block::Slate => ([0.19, 0.20, 0.23], [0.16, 0.17, 0.20]),
            Block::Chalk => ([0.46, 0.44, 0.40], [0.41, 0.39, 0.36]),
            Block::Moss => ([0.20, 0.33, 0.09], [0.25, 0.26, 0.24]),
            Block::Litter => ([0.26, 0.17, 0.07], [0.20, 0.13, 0.07]),
            Block::Fern => ([0.10, 0.30, 0.10], [0.09, 0.27, 0.09]),
            Block::Mushroom => ([0.55, 0.14, 0.08], [0.70, 0.66, 0.56]),
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
    #[inline]
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
    /// The cells that are [`Block::Broken`], by cell position, and what each bit is.
    pub bits: HashMap<IVec3, Box<Bits>>,
}

impl World {
    pub fn split(cell: IVec3) -> (IVec3, IVec3) {
        (cell.div_euclid(IVec3::splat(CHUNK)), cell.rem_euclid(IVec3::splat(CHUNK)))
    }
    pub fn get(&self, cell: IVec3) -> Block {
        let (c, l) = Self::split(cell);
        self.chunks.get(&c).map_or(Block::Air, |ch| ch.get(l))
    }
    /// Makes a cell one whole block (never [`Block::Broken`]: use [`World::set_bit`]).
    pub fn set(&mut self, cell: IVec3, b: Block) {
        let (c, l) = Self::split(cell);
        let chunk = self.chunks.entry(c).or_default();
        if chunk.get(l) == Block::Broken {
            self.bits.remove(&cell);
        }
        chunk.set(l, b);
    }

    fn bit_index(bit: IVec3) -> (IVec3, usize) {
        let l = bit.rem_euclid(IVec3::splat(SPLIT));
        (bit.div_euclid(IVec3::splat(SPLIT)), (l.x + l.z * SPLIT + l.y * SPLIT * SPLIT) as usize)
    }

    /// What is at a bit position (cell position × [`SPLIT`], plus the bit within the cell).
    pub fn bit(&self, bit: IVec3) -> Block {
        let (cell, i) = Self::bit_index(bit);
        match self.get(cell) {
            Block::Broken => self.bits.get(&cell).map_or(Block::Air, |b| b[i]),
            whole => whole,
        }
    }

    /// Sets one bit. The cell it is in breaks if it was whole, and is made whole again if
    /// that leaves every bit of it the same.
    pub fn set_bit(&mut self, bit: IVec3, b: Block) {
        let (cell, i) = Self::bit_index(bit);
        let was = self.get(cell);
        if was == b {
            return;
        }
        if was != Block::Broken {
            self.set(cell, Block::Broken);
            self.bits.insert(cell, Box::new([was; (SPLIT * SPLIT * SPLIT) as usize]));
        }
        let bits = self.bits.get_mut(&cell).expect("a broken cell has its bits");
        bits[i] = b;
        if bits.iter().all(|&other| other == b) {
            self.set(cell, b);
        }
    }

    /// Makes everything within `radius` metres of a point `b`, to the bit: digging with air,
    /// building with anything else.
    pub fn ball(&mut self, middle: Vec3, radius: f32, b: Block) {
        self.ellipsoid(middle, Vec3::splat(radius), b, false);
    }

    /// Makes everything inside an ellipsoid (metres) `b`, to the bit. With `only_air` it fills
    /// air and leaves what is there: a stone set on the ground, not through it.
    pub fn ellipsoid(&mut self, middle: Vec3, radii: Vec3, b: Block, only_air: bool) {
        self.shape(middle - radii, middle + radii, |p| ((p - middle) / radii).length_squared() < 1.0, b, only_air);
    }

    /// Makes everything inside a convex shape `b`, to the bit: `inside` says whether a point
    /// (metres) is in it, and it lies between `lo` and `hi`. Cells wholly inside are set
    /// whole; only the ones its surface cuts through break.
    pub fn shape(&mut self, lo: Vec3, hi: Vec3, inside: impl Fn(Vec3) -> bool, b: Block, only_air: bool) {
        let (lo, hi) = ((lo / CELL).floor().as_ivec3(), (hi / CELL).floor().as_ivec3());
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    let cell = IVec3::new(x, y, z);
                    let was = self.get(cell);
                    if only_air && was != Block::Broken && was.solid() {
                        continue;
                    }
                    // The cell's eight corners: all in, it is set whole (the shape is convex).
                    let corners = (0..8).filter(|i| inside((cell + IVec3::new(i & 1, i >> 1 & 1, i >> 2 & 1)).as_vec3() * CELL)).count();
                    if corners == 8 && (!only_air || !was.solid()) {
                        self.set(cell, b);
                        continue;
                    }
                    for i in 0..SPLIT * SPLIT * SPLIT {
                        let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
                        if inside((bit.as_vec3() + 0.5) * BIT) && (!only_air || !self.bit(bit).solid()) {
                            self.set_bit(bit, b);
                        }
                    }
                }
            }
        }
    }

    /// Whether a point in metres is inside something solid, to the bit.
    pub fn solid_at(&self, point: Vec3) -> bool {
        self.bit((point / BIT).floor().as_ivec3()).solid()
    }
    /// Sets a column of cells from `from` up to, not including, `to`, each to what `block`
    /// gives for its height. Much quicker than a `set` a cell: the chunk is found once.
    pub fn set_column(&mut self, x: i32, z: i32, from: i32, to: i32, mut block: impl FnMut(i32) -> Block) {
        let mut y = from;
        while y < to {
            let (c, l) = Self::split(IVec3::new(x, y, z));
            let chunk = self.chunks.entry(c).or_default();
            for ly in l.y..CHUNK.min(l.y + to - y) {
                chunk.set(IVec3::new(l.x, ly, l.z), block(y));
                y += 1;
            }
        }
    }

    /// Sets every cell in a box given in metres: from `lo` up to, not including, `hi`.
    pub fn fill(&mut self, lo: Vec3, hi: Vec3, b: Block) {
        let (lo, hi) = ((lo / CELL).round().as_ivec3(), (hi / CELL).round().as_ivec3());
        for y in lo.y..hi.y {
            for z in lo.z..hi.z {
                for x in lo.x..hi.x {
                    self.set(IVec3::new(x, y, z), b);
                }
            }
        }
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
            match self.get(cell) {
                // Through a broken cell in steps of half a bit, to the first solid bit.
                Block::Broken => {
                    let leaves = next.x.min(next.y).min(next.z).min(reach);
                    let mut t = at;
                    while t < leaves {
                        if self.solid_at(from + dir * (t + 1e-4)) {
                            return Some(t);
                        }
                        t += BIT / 2.0;
                    }
                }
                block if block.solid() => return Some(at),
                _ => {}
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
        assert_eq!(w.ground_height(-0.5 * CELL, 33.5 * CELL), 41.0 * CELL);
    }

    #[test]
    fn a_ray_stops_at_the_first_solid_face() {
        let mut w = World::default();
        w.fill(Vec3::new(3.0, 1.0, 0.0), Vec3::new(3.5, 1.5, 0.5), Block::Stone);
        let from = Vec3::new(0.25, 1.25, 0.25);
        assert_eq!(w.raycast(from, Vec3::X, 10.0), Some(2.75));
        assert_eq!(w.raycast(from, Vec3::X, 2.0), None);
        assert_eq!(w.raycast(from, Vec3::NEG_X, 10.0), None);
        let slant = w.raycast(Vec3::new(0.25, 4.0, 0.25), Vec3::new(3.0, -2.75, 0.0), 10.0).unwrap();
        // It meets the face at x = 3.0, eleven twelfths of the way along (3, -2.75).
        assert!((slant - 3.0f32.hypot(2.75) * 11.0 / 12.0).abs() < 1e-3, "{slant}");
    }

    #[test]
    fn a_cell_breaks_into_bits_and_mends() {
        let mut w = World::default();
        let cell = IVec3::new(3, 2, -1);
        w.set(cell, Block::Stone);
        let corner = cell * SPLIT;
        // One bit dug out: the cell is broken, the rest of it still stone.
        w.set_bit(corner, Block::Air);
        assert_eq!((w.get(cell), w.bit(corner), w.bit(corner + IVec3::X)), (Block::Broken, Block::Air, Block::Stone));
        assert!(w.get(cell).solid() && !w.get(cell).opaque());
        // A whole cell is 6 faces. Broken, each side is 25 little ones; the corner bit's three
        // are gone and the three it uncovered inside are drawn.
        let faces = |w: &World| mesh::mesh_chunk(w, World::split(cell).0, false).indices.len() / 6;
        assert_eq!(faces(&w), 6 * 25 - 3 + 3);
        // Put back, it is one cell again and nothing is kept for it.
        w.set_bit(corner, Block::Stone);
        assert_eq!((w.get(cell), w.bits.len(), faces(&w)), (Block::Stone, 0, 6));
        // A ray into a dug cell stops at the stone behind the hole, not at the cell's face.
        w.set_bit(corner, Block::Air);
        let from = (corner.as_vec3() + Vec3::new(-10.0, 0.5, 0.5)) * BIT;
        let hit = w.raycast(from, Vec3::X, 5.0).unwrap();
        assert!((hit - 11.0 * BIT).abs() < BIT * 0.6, "{hit}");
    }

    #[test]
    fn a_ball_dug_out_breaks_only_the_cells_its_surface_cuts() {
        let mut w = World::default();
        w.fill(Vec3::splat(-2.0), Vec3::splat(2.0), Block::Stone);
        w.ball(Vec3::ZERO, 0.6, Block::Air);
        assert_eq!(w.get(IVec3::ZERO), Block::Air);
        assert!(!w.solid_at(Vec3::new(0.55, 0.0, 0.0)) && w.solid_at(Vec3::new(0.65, 0.0, 0.0)));
        // A ball 1.2 m across is about 58 cells; the shell that broke is fewer than twice that.
        assert!(w.bits.len() > 30 && w.bits.len() < 110, "{}", w.bits.len());
        assert!(w.bits.iter().all(|(cell, _)| w.get(*cell) == Block::Broken));
    }

    #[test]
    fn a_lone_block_meshes_to_six_faces() {
        let mut w = World::default();
        w.set(IVec3::new(3, 3, 3), Block::Stone);
        let m = mesh::mesh_chunk(&w, IVec3::ZERO, false);
        assert_eq!(m.positions.len(), 24);
        assert_eq!(m.indices.len(), 36);
    }

}
