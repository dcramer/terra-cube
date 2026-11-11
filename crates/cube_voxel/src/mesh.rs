//! Chunk meshing: one quad per visible face, with ambient occlusion baked into vertex colour,
//! the block's colour in it too, and a place in the atlas of grains (`texture`) for the paint.
//! Not greedy yet: see docs/engine.md for the order of meshing work.

use crate::texture::{Grain, MEAN, SPAN};
use crate::{Block, CELL, CHUNK, Chunk, SPLIT, World};
use glam::{IVec3, Vec3};
use std::collections::HashMap;

#[derive(Default)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    /// Where each corner is in the atlas of grains (`texture`).
    pub uvs: Vec<[f32; 2]>,
    /// Which way the atlas's `u` runs along the surface at each corner, and the handedness
    /// of its `v`: what the relief in the atlas is turned by.
    pub tangents: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

/// A face: its normal, then its two in-plane axes (u, v) such that u × v = normal.
const FACES: [(IVec3, IVec3, IVec3); 6] = [
    (IVec3::X, IVec3::Y, IVec3::Z),
    (IVec3::NEG_X, IVec3::Z, IVec3::Y),
    (IVec3::Y, IVec3::Z, IVec3::X),
    (IVec3::NEG_Y, IVec3::X, IVec3::Z),
    (IVec3::Z, IVec3::X, IVec3::Y),
    (IVec3::NEG_Z, IVec3::Y, IVec3::X),
];

/// Where a corner of a face is in the atlas. `cube` is the least corner of the cube the face
/// belongs to and `corner` the face's corner, both in units of which there are `per` to a
/// cell (1 for cells, 5 for bits). The picture lies flat on tops and stands upright on sides,
/// and runs on unbroken from one cube to the next.
fn uv(grain: Grain, n: IVec3, cube: IVec3, corner: IVec3, per: i32) -> [f32; 2] {
    let along = |c: i32, p: i32| (c.rem_euclid(SPAN * per) + (p - c)) as f32 / per as f32;
    if n.y != 0 {
        grain.uv(along(cube.x, corner.x), along(cube.z, corner.z))
    } else {
        let across = if n.x != 0 { along(cube.z, corner.z) } else { along(cube.x, corner.x) };
        grain.uv(across, SPAN as f32 - along(cube.y, corner.y))
    }
}

/// How far across one repeat of the water's ripples is, metres.
pub const WATER_TILE: f32 = 4.0;

/// How free a leaf is to move in the wind, of the 1 that the tip of a blade of grass is.
const LEAF: f32 = 0.4;

/// The tangent of a face that looks along `n`, to match [`uv`]: `u` runs along x on tops and
/// on faces that look along z, along z on faces that look along x; `v` runs down a side.
fn tangent(n: IVec3) -> [f32; 4] {
    if n.x != 0 { [0.0, 0.0, 1.0, n.x as f32] } else if n.z != 0 { [1.0, 0.0, 0.0, -n.z as f32] } else { [1.0, 0.0, 0.0, -n.y as f32] }
}

/// How much one cube differs in shade from the next: little, so a wall is its grain and not a
/// chequer of cubes.
fn shade(p: IVec3) -> f32 {
    (0.96 + 0.08 * hash(p)) / MEAN
}

fn hash(p: IVec3) -> f32 {
    let mut h = (p.x as u32).wrapping_mul(0x9E3779B1) ^ (p.y as u32).wrapping_mul(0x85EBCA77) ^ (p.z as u32).wrapping_mul(0xC2B2AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// A slow drift of colour over the ground, 0 to 1: patches some metres across, so a meadow is
/// not one green.
fn patch(p: IVec3) -> f32 {
    let at = |x: i32, z: i32| hash(IVec3::new(x, 77, z));
    // Six metres from one patch to the next.
    let n = crate::cells(6.0);
    let (gx, gz) = (p.x.div_euclid(n), p.z.div_euclid(n));
    let (fx, fz) = (p.x.rem_euclid(n) as f32 / n as f32, p.z.rem_euclid(n) as f32 / n as f32);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let a = at(gx, gz) * (1.0 - sx) + at(gx + 1, gz) * sx;
    let b = at(gx, gz + 1) * (1.0 - sx) + at(gx + 1, gz + 1) * sx;
    a * (1.0 - sz) + b * sz
}

/// How rough the country is at a place, 0 smooth to 1 broken: regions some twenty metres over.
fn rough(p: IVec3) -> f32 {
    let at = |x: i32, z: i32| hash(IVec3::new(x, 191, z));
    let n = crate::cells(18.0);
    let (gx, gz) = (p.x.div_euclid(n), p.z.div_euclid(n));
    let (fx, fz) = (p.x.rem_euclid(n) as f32 / n as f32, p.z.rem_euclid(n) as f32 / n as f32);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let v = (at(gx, gz) * (1.0 - sx) + at(gx + 1, gz) * sx) * (1.0 - sz) + (at(gx, gz + 1) * (1.0 - sx) + at(gx + 1, gz + 1) * sx) * sz;
    // Most of the land is one or the other, with little between.
    ((v - 0.35) / 0.3).clamp(0.0, 1.0)
}

/// The chunk and the 26 round it, so a cell's neighbours are read without looking the chunk
/// up each time: meshing reads about thirty cells for every face it draws.
struct Near<'a>([Option<&'a Chunk>; 27]);

impl<'a> Near<'a> {
    fn of(world: &'a World, chunk: IVec3) -> Self {
        let mut near = Near([None; 27]);
        for (i, slot) in near.0.iter_mut().enumerate() {
            let i = i as i32;
            *slot = world.chunks.get(&(chunk + IVec3::new(i % 3 - 1, i / 9 - 1, i / 3 % 3 - 1)));
        }
        near
    }

    /// The block at a position local to the middle chunk, from -32 to 63 on each axis.
    #[inline]
    fn get(&self, local: IVec3) -> Block {
        let c = local.div_euclid(IVec3::splat(CHUNK));
        // Beyond the chunks round this one nothing is known: air.
        if c.abs().max_element() > 1 {
            return Block::Air;
        }
        match self.0[((c.x + 1) + (c.z + 1) * 3 + (c.y + 1) * 9) as usize] {
            Some(chunk) => chunk.get(local - c * CHUNK),
            None => Block::Air,
        }
    }
}

/// The ground drawn as slopes, not stairs. The cells are not changed: digging, building and
/// what a body stands on are still cubes. Only where a corner of the ground's surface is
/// drawn moves, up or down to the mean height of the ground in the sixteen columns round it.
/// A staircase of cubes becomes the ramp through its middle, a gentle rise becomes a gentle
/// rise, and anything up to eight up for one along a smooth slope. A column counts for no
/// more than [`Eased::reach`] cells above or below the corner, so a real cliff keeps its face
/// and only its lip and foot are rounded. (At a reach of three, every slope steeper than one
/// in one broke into sawteeth; it is twenty-four, six metres, over a
/// mountain's ledges.)
///
/// The move is worked out from the cells alone, the same for a corner whichever chunk or
/// level of detail asks, so neighbouring meshes meet. Ground cover and the bits of a broken
/// cell (a rock, the rim of a hole) are drawn moved by as much as the ground under them.
pub struct Eased<'a> {
    near: &'a Near<'a>,
    reach: i32,
    lifts: HashMap<IVec3, (f32, [f32; 3])>,
    /// For each column looked at, every height where its ground ends with none over it.
    tops: HashMap<(i32, i32), Vec<i32>>,
    /// Where the chunk's corner is in the world, cells: what is rough and what is fine goes
    /// by place.
    origin: IVec3,
}

impl Eased<'_> {
    /// How far the corner at `c` (local cells) is drawn above where it is, in cells.
    fn lift(&mut self, c: IVec3) -> f32 {
        self.corner(c).0
    }

    /// That, and the colour of the ground there: the mean of the tops of the four columns that
    /// meet at the corner, so grass runs into sand and litter into turf without a sawn edge.
    fn corner(&mut self, c: IVec3) -> (f32, [f32; 3]) {
        if let Some(&known) = self.lifts.get(&c) {
            return known;
        }
        let r = self.reach;
        let (mut sum, mut colour, mut tops) = (0, [0.0f32; 3], 0.0);
        for dz in -2..2 {
            for dx in -2..2 {
                // Where this column's ground ends, from the corner: the first cell from the
                // top that is ground with none over it.
                let near = self.near;
                let ends = self.tops.entry((c.x + dx, c.z + dz)).or_insert_with(|| {
                    let ground = |y: i32| near.get(IVec3::new(c.x + dx, y, c.z + dz)).ground();
                    (1 - CHUNK..2 * CHUNK).filter(|&y| ground(y - 1) && !ground(y)).collect()
                });
                let top = if near.get(c + IVec3::new(dx, r, dz)).ground() { r } else { ends.iter().rev().find(|&&y| y <= c.y + r).map_or(-r, |y| (y - c.y).max(-r)) };
                sum += top;
                if (-1..=0).contains(&dx) && (-1..=0).contains(&dz) && top > -r {
                    let block = self.near.get(c + IVec3::new(dx, top - 1, dz));
                    // Its top if it lies open, its side if it is the face of a bank.
                    let (open, side) = block.colors();
                    let shown = if top.abs() <= 2 { open } else { side };
                    colour = [colour[0] + shown[0], colour[1] + shown[1], colour[2] + shown[2]];
                    tops += 1.0;
                }
            }
        }
        let lift = sum as f32 / 16.0;
        let known = (lift, if tops > 0.0 { colour.map(|v| v / tops) } else { [0.0; 3] });
        self.lifts.insert(c, known);
        known
    }

    /// The ground as blocks of the size its slope calls for: how far above the level `t` the
    /// top of the column at `x`, `z` is drawn, in cells.
    ///
    /// Nearly level ground is cut fine, in steps of a bit (5 cm), so a meadow rolls. A slope
    /// is whole cells, the plain cube. Steep ground is blocks half a metre a side, and a rock
    /// face is blocks of a metre: the land gets blockier as it gets wilder, and no two
    /// hillsides are the same staircase. The heights come from the eased ground, rounded to
    /// the size of the block.
    fn step(&mut self, x: i32, t: i32, z: i32) -> f32 {
        let rough = rough(self.origin + IVec3::new(x, 0, z));
        let mut lift = |x: i32, z: i32| self.lift(IVec3::new(x, t, z));
        // And by place as well as slope: the land has rough country, where even a modest slope
        // is broken into half-metre and metre blocks, and smooth country, cut fine. With slope
        // alone, everything in one view was one size.
        for (size, steep) in [(4, 2.4 - 2.0 * rough), (2, 1.1 - 0.95 * rough)] {
            // The block this column is in, by the world's grid, and the slope across it.
            let (bx, bz, half) = (x.div_euclid(size) * size + size / 2, z.div_euclid(size) * size + size / 2, size / 2);
            let slope = (lift(bx + half, bz) - lift(bx - half, bz)).hypot(lift(bx, bz + half) - lift(bx, bz - half)) / size as f32;
            if slope >= steep {
                return ((t as f32 + lift(bx, bz)) / size as f32).round() * size as f32 - t as f32;
            }
        }
        let (a, b, c, d) = (lift(x, z), lift(x + 1, z), lift(x, z + 1), lift(x + 1, z + 1));
        let (mean, slope) = ((a + b + c + d) / 4.0, ((b + d - a - c) / 2.0).hypot((c + d - a - b) / 2.0));
        let size = if slope < 0.3 && rough < 0.6 { 1.0 / SPLIT as f32 } else { 1.0 };
        (mean / size).round() * size
    }

    /// Whether the column at `x`, `z` with its top at the level `t` is part of the land: at
    /// least a quarter of the columns round it have ground near that level. A block standing
    /// alone, a pillar, a wall one cell thick are not.
    fn land(&mut self, x: i32, t: i32, z: i32) -> bool {
        let lifts = self.lift(IVec3::new(x, t, z)) + self.lift(IVec3::new(x + 1, t, z)) + self.lift(IVec3::new(x, t, z + 1)) + self.lift(IVec3::new(x + 1, t, z + 1));
        lifts / 4.0 > -0.75 * self.reach as f32
    }

    /// How high the top of the column at `x`, `z` is drawn (local cells), taking the end of its
    /// ground nearest the level `t`. Worked out from that column's own level, so two columns
    /// side by side agree which is the higher and one of them draws the riser between: asked
    /// from the other's level across a tall cliff, they did not, and the cliff had holes.
    fn top(&mut self, x: i32, t: i32, z: i32) -> f32 {
        let near = self.near;
        let ends = self.tops.entry((x, z)).or_insert_with(|| {
            let ground = |y: i32| near.get(IVec3::new(x, y, z)).ground();
            (1 - CHUNK..2 * CHUNK).filter(|&y| ground(y - 1) && !ground(y)).collect()
        });
        let Some(&level) = ends.iter().min_by_key(|&&y| (y - t).abs()) else { return (t - self.reach) as f32 };
        level as f32 + if self.land(x, level, z) { self.step(x, level, z) } else { 0.0 }
    }

    /// The same for a group of cells `size` a side seen from far off, whose middle is the
    /// corner `x`, `z`: the eased height there, in blocks the group's size where the land is
    /// steep and left as it is where it is not. A far hillside is then the near one with
    /// broader treads, where drawn as plain cubes of its own size it was a different hill.
    fn far_top(&mut self, x: i32, t: i32, z: i32, size: i32) -> f32 {
        let near = self.near;
        let ends = self.tops.entry((x, z)).or_insert_with(|| {
            let ground = |y: i32| near.get(IVec3::new(x, y, z)).ground();
            (1 - CHUNK..2 * CHUNK).filter(|&y| ground(y - 1) && !ground(y)).collect()
        });
        let Some(&level) = ends.iter().min_by_key(|&&y| (y - t).abs()) else { return (t - self.reach) as f32 };
        let mut lift = |x: i32, z: i32| self.lift(IVec3::new(x, level, z));
        let (here, half) = (lift(x, z), size / 2);
        let slope = (lift(x + half, z) - lift(x - half, z)).hypot(lift(x, z + half) - lift(x, z - half)) / size as f32;
        let high = level as f32 + here;
        if slope >= 1.1 { (high / size as f32).round() * size as f32 } else { high }
    }

    /// For a ground cell, the level its column's ground ends at over it and how far above
    /// that the column's top is drawn, if the column is land. Stone standing alone or built
    /// up is not: its top would be drawn far from where it is, so it is drawn as its cubes.
    fn column(&mut self, c: IVec3) -> Option<(i32, f32)> {
        let near = self.near;
        let ends = self.tops.entry((c.x, c.z)).or_insert_with(|| {
            let ground = |y: i32| near.get(IVec3::new(c.x, y, c.z)).ground();
            (1 - CHUNK..2 * CHUNK).filter(|&y| ground(y - 1) && !ground(y)).collect()
        });
        let level = *ends.iter().find(|&&y| y > c.y)?;
        self.land(c.x, level, c.z).then(|| (level, self.step(c.x, level, c.z)))
    }

    /// How far above itself whatever stands on the ground at the cell `c` is drawn, in cells:
    /// ground cover, the bits of a broken cell.
    fn under(&mut self, c: IVec3) -> f32 {
        match ground() {
            Ground::Cubes => 0.0,
            Ground::Stepped if self.land(c.x, c.y, c.z) => self.step(c.x, c.y, c.z),
            Ground::Stepped => 0.0,
            Ground::Smooth => (0..4).map(|i| self.lift(c + IVec3::new(i & 1, 0, i >> 1))).sum::<f32>() / 4.0,
        }
    }

    /// Which way the eased ground faces at a corner.
    fn normal(&mut self, c: IVec3) -> Vec3 {
        let (x, z) = (self.lift(c + IVec3::X) - self.lift(c - IVec3::X), self.lift(c + IVec3::Z) - self.lift(c - IVec3::Z));
        Vec3::new(-x * 0.5, 1.0, -z * 0.5).normalize()
    }
}

/// The height the ground is drawn at under a point standing on it (metres), if what it stands
/// on is ground and it stands on top of a cell of it: where to draw feet, since a body stands
/// on the cubes and the cubes are drawn as a slope.
pub fn eased_under(world: &World, feet: Vec3) -> Option<f32> {
    let level = (feet.y / CELL).round();
    let cell = IVec3::new((feet.x / CELL).floor() as i32, level as i32, (feet.z / CELL).floor() as i32);
    if (feet.y / CELL - level).abs() > 0.02 || !world.get(cell - IVec3::Y).ground() || ground() == Ground::Cubes {
        return None;
    }
    let chunk = cell.div_euclid(IVec3::splat(CHUNK));
    let near = Near::of(world, chunk);
    let mut eased = Eased { near: &near, reach: REACH, lifts: HashMap::new(), tops: HashMap::new(), origin: chunk * CHUNK };
    let local = cell - chunk * CHUNK;
    if ground() == Ground::Stepped {
        return eased.land(local.x, local.y, local.z).then(|| (level + eased.step(local.x, local.y, local.z)) * CELL);
    }
    let (fx, fz) = (feet.x / CELL - cell.x as f32, feet.z / CELL - cell.z as f32);
    let mut lift = |dx: i32, dz: i32| eased.lift(local + IVec3::new(dx, 0, dz));
    let (a, b) = (lift(0, 0) * (1.0 - fx) + lift(1, 0) * fx, lift(0, 1) * (1.0 - fx) + lift(1, 1) * fx);
    Some((level + a * (1.0 - fz) + b * fz) * CELL)
}

/// Whether what grows (wood, leaf) is drawn by someone else as shapes (`cube_game::flora`),
/// and so not here as cubes. The cells are there all the same.
pub static MODELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn modelled(block: Block) -> bool {
    (block.foliage() || matches!(block, Block::Wood | Block::Birch | Block::Rock)) && MODELLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Whether a block, as it is drawn, hides what is behind it: one drawn elsewhere as a shape
/// (a trunk in the ground, a log lying on it) hides nothing here, or the ground under it is a
/// hole.
fn hides(block: Block) -> bool {
    block.opaque() && !modelled(block)
}

/// How the ground is drawn. The cells are the same in all three; this is only the picture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Ground {
    /// Every cell the cube it is: the look to 2026-11-11, every slope the same staircase.
    Cubes,
    /// Blocks of the size the slope calls for (the default): see [`Eased::step`].
    Stepped,
    /// Eased into smooth slopes. Tried and set aside: smooth land under cube trees was two
    /// games in one picture.
    Smooth,
}

pub static GROUND: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(Ground::Smooth as u8);

fn ground() -> Ground {
    [Ground::Cubes, Ground::Stepped, Ground::Smooth][GROUND.load(std::sync::atomic::Ordering::Relaxed) as usize]
}

/// Whether a block's faces are drawn eased into slopes.
fn eases(block: Block) -> bool {
    block.ground() && ground() == Ground::Smooth
}

/// Cells above or below a corner that a column counts for, up close.
const REACH: i32 = 24;

/// The mesh of one chunk, in metres, positioned in world space: its solid blocks, or with
/// `liquid` its water, which is drawn apart because it is seen through.
pub fn mesh_chunk(world: &World, chunk: IVec3, liquid: bool) -> MeshData {
    let mut m = MeshData::default();
    let Some(data) = world.chunks.get(&chunk) else { return m };
    let near = Near::of(world, chunk);
    let origin = chunk * CHUNK;
    // Whether there is any water in this chunk or beside it (only then is the bank looked at).
    let wet = liquid && ground() == Ground::Smooth && (-1..=1).any(|dz| (-1..=1).any(|dx| world.chunks.get(&(chunk + IVec3::new(dx, 0, dz))).is_some_and(|c| c.has(Block::Water))));
    let mut eased = Eased { near: &near, reach: REACH, lifts: HashMap::new(), tops: HashMap::new(), origin: chunk * CHUNK };
    for y in 0..CHUNK {
        for z in 0..CHUNK {
            for x in 0..CHUNK {
                let local = IVec3::new(x, y, z);
                let block = data.get(local);
                // The water's surface runs on two cells under the bank beside it. The ground is
                // drawn eased, so where the two meet is then a smooth shoreline; ending the
                // water at the last cell of water sawed the edge of every river into steps.
                if liquid && block != Block::Water && wet {
                    let at = |dx: i32, dz: i32, dy: i32| near.get(local + IVec3::new(dx, dy, dz));
                    let beside = (-2..=2).any(|dz| (-2..=2).any(|dx| at(dx, dz, 0) == Block::Water && at(dx, dz, 1) != Block::Water));
                    if beside {
                        let cell = origin + local;
                        let start = m.positions.len() as u32;
                        let colour = Block::Water.colors().0.map(|c| (c + (0.36 - c) * 0.25) / MEAN);
                        for (dx, dz) in [(0, 0), (0, 1), (1, 1), (1, 0)] {
                            let corner = cell + IVec3::new(dx, 1, dz);
                            m.positions.push((corner.as_vec3() * CELL).to_array());
                            m.normals.push([0.0, 1.0, 0.0]);
                            m.tangents.push(tangent(IVec3::Y));
                            m.uvs.push([corner.x as f32 * CELL / WATER_TILE, corner.z as f32 * CELL / WATER_TILE]);
                            m.colors.push([colour[0], colour[1], colour[2], 0.8]);
                        }
                        m.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
                    }
                }
                if block == Block::Air || (block == Block::Water) != liquid || modelled(block) {
                    continue;
                }
                let cell = origin + local;
                if block == Block::Broken {
                    let (from, lift) = (m.positions.len(), eased.under(local) * CELL);
                    bits(world, cell, &mut m);
                    m.positions[from..].iter_mut().for_each(|p| p[1] += lift);
                    continue;
                }
                if block.sprig() {
                    // It stands on the ground as the ground is drawn.
                    let (from, lift) = (m.positions.len(), eased.under(local) * CELL);
                    sprig(cell, block, &mut m);
                    m.positions[from..].iter_mut().for_each(|p| p[1] += lift);
                    continue;
                }
                let (top, side) = block.colors();
                let mut tint = shade(cell);
                let mut warm = 0.0;
                // Water hides what is under it by how much of it there is: a ford shows its
                // bed, the sea does not.
                let depth = if liquid { (1..=crate::cells(4.0)).take_while(|d| near.get(local - IVec3::Y * *d) == Block::Water).count() } else { 0 };
                // Of what is solid, alpha is how fast it is held against the wind: leaves give.
                // (Gently: a large step in opacity for each cell of depth drew the lake bed's
                // contours across the water as bands.)
                // Shallows are clear and pale, a hand's depth at the very edge near white where
                // the water breaks on the shore; a metre down it is the lake's own colour.
                let shallow = if liquid { (1.0 - depth as f32 * CELL / 1.0).clamp(0.0, 1.0) } else { 0.0 };
                let (top, side) = if liquid {
                    let foam = if depth == 0 { 0.12 } else { 0.0 };
                    let pale = |c: [f32; 3]| [c[0] + (0.2 - c[0]) * shallow * 0.25 + foam * 0.5, c[1] + (0.36 - c[1]) * shallow * 0.25 + foam * 0.5, c[2] + (0.38 - c[2]) * shallow * 0.25 + foam * 0.45];
                    (pale(top), pale(side))
                } else {
                    (top, side)
                };
                let alpha = if liquid { 0.8 + 0.06 * (1.0 - shallow) + depth as f32 * CELL * 0.045 } else if block.foliage() { 1.0 - LEAF } else { 1.0 };
                if matches!(block, Block::Grass | Block::Turf | Block::Moss | Block::Litter) || block.foliage() {
                    // Patches: lighter and yellower here, deeper there.
                    let drift = patch(cell) - 0.5;
                    tint *= 1.0 + drift * 0.35;
                    warm = drift * 0.25;
                }
                if block.foliage() {
                    // A crown has depth: leaves under leaves are in their shade, the ones on
                    // top catch the sun. Without it a tree is one flat green.
                    let over = (1..=3).filter(|up| near.get(local + IVec3::Y * *up).foliage()).count();
                    tint *= [1.14, 0.94, 0.8, 0.68][over];
                }
                if let (true, Ground::Stepped, Some((level, high))) = (block.ground(), ground(), eased.column(local)) {
                    // Only the top cell of a column draws: its top, and a riser down to each
                    // neighbour that stands lower.
                    if level != y + 1 {
                        continue;
                    }
                    for (n, u, v) in FACES {
                        if n == IVec3::NEG_Y || (n == IVec3::Y && hides(near.get(local + n))) {
                            continue;
                        }
                        let low = if n == IVec3::Y { high } else { eased.top(x + n.x, level, z + n.z) - level as f32 };
                        if n != IVec3::Y && low > high - 1e-3 {
                            continue;
                        }
                        // A low riser is the turf turning over an edge; a high one is the bank under it.
                        let low_riser = n != IVec3::Y && high - low < 0.99;
                        let (base, grain) = (if n == IVec3::Y { top } else if low_riser { top.map(|c| c * 0.92) } else { side }, Grain::of(block, n == IVec3::Y || low_riser));
                        // A riser is drawn in lengths that end where the grain's tile does.
                        let mut from = low;
                        loop {
                            let to = if n == IVec3::Y { high } else { high.min((((cell.y + 1) as f32 + from) / SPAN as f32 + 1e-4).floor() * SPAN as f32 + SPAN as f32 - (cell.y + 1) as f32) };
                            let start = m.positions.len() as u32;
                            for (su, sv) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                                let corner = cell + n.max(IVec3::ZERO) + u * su + v * sv;
                                let foot = n != IVec3::Y && corner.y == cell.y;
                                let height = (cell.y + 1) as f32 + if foot { from } else { to };
                                m.positions.push([corner.x as f32 * CELL, height * CELL, corner.z as f32 * CELL]);
                                // A low riser is lit as the ground it is a fold of. Lit as a wall it
                                // takes the warm light of the horizon while the ground takes the
                                // blue of the sky overhead, and a forest floor is a grid of tiles.
                                let facing = if low_riser { Vec3::Y } else { n.as_vec3() };
                                m.normals.push(facing.to_array());
                                m.tangents.push(tangent(n));
                                let flat = uv(grain, n, cell, corner, 1);
                                m.uvs.push(if n == IVec3::Y {
                                    flat
                                } else {
                                    // The foot of a length may lie on the tile's lower edge and its
                                    // head on the upper: each is read from inside the length.
                                    let inside = height + if foot { 1e-4 } else { -1e-4 };
                                    [flat[0], grain.uv(0.0, SPAN as f32 - inside.rem_euclid(SPAN as f32))[1]]
                                });
                                // A riser is in its own shade toward its foot.
                                let k = tint * if foot && to - from > 0.3 { 0.7 } else { 1.0 };
                                m.colors.push([base[0] * k * (1.0 + warm), base[1] * k, base[2] * k * (1.0 - warm), 1.0]);
                            }
                            m.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
                            if to >= high - 1e-3 {
                                break;
                            }
                            from = to;
                        }
                    }
                    continue;
                }
                for (n, u, v) in FACES {
                    // The underside of the world is never seen.
                    if cell.y == 0 && n == IVec3::NEG_Y {
                        continue;
                    }
                    let other = near.get(local + n);
                    let hidden = if block == Block::Water { other != Block::Air } else { hides(other) };
                    if hidden {
                        continue;
                    }
                    let base = if n == IVec3::Y { top } else { side };
                    let grain = Grain::of(block, n.y != 0);
                    let start = m.positions.len() as u32;
                    let mut ao = [0.0f32; 4];
                    let mut at = [Vec3::ZERO; 4];
                    // What stands on the ground and is not ground (a trunk) runs a little
                    // way into it, since the ground may be drawn lower than its cubes.
                    let rooted = !block.ground() && n.y == 0 && near.get(local - IVec3::Y).ground() && ground() != Ground::Cubes;
                    for (i, (su, sv)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
                        let corner = cell + n.max(IVec3::ZERO) + u * su + v * sv;
                        let (du, dv) = (u * (su * 2 - 1), v * (sv * 2 - 1));
                        // On eased ground a step one cube high casts no shade: it is not there.
                        let occ = |d: IVec3| {
                            let by = near.get(local + n + d);
                            (hides(by) && !(eases(block) && n == IVec3::Y && by.ground() && !near.get(local + n + d + IVec3::Y).opaque())) as i32
                        };
                        let (a, b, c) = (occ(du), occ(dv), occ(du + dv));
                        let level = if a + b == 2 { 0 } else { 3 - (a + b + c) };
                        ao[i] = 0.45 + 0.55 * level as f32 / 3.0;
                        let k = ao[i] * tint;
                        let lift = if eases(block) {
                            eased.lift(corner - origin)
                        } else if rooted && corner.y == cell.y {
                            -0.75
                        } else {
                            0.0
                        };
                        at[i] = (corner.as_vec3() + Vec3::Y * lift) * CELL;
                        // Every face of the ground is lit as the slope it is part of, a riser too.
                        let facing = if eases(block) && n != IVec3::NEG_Y { eased.normal(corner - origin) } else { n.as_vec3() };
                        m.positions.push(at[i].to_array());
                        m.normals.push(facing.to_array());
                        m.tangents.push(tangent(n));
                        // Water is not painted from the atlas: its ripples lie by where it is in
                        // the world, four metres to a repeat (`WATER_TILE`).
                        m.uvs.push(if liquid { [corner.x as f32 * CELL / WATER_TILE, corner.z as f32 * CELL / WATER_TILE] } else { uv(grain, n, cell, corner, 1) });
                        let base = if eases(block) && n == IVec3::Y { eased.corner(corner - origin).1 } else { base };
                        m.colors.push([base[0] * k * (1.0 + warm), base[1] * k, base[2] * k * (1.0 - warm), alpha]);
                    }
                    // A riser between two treads has closed to nothing: it is not drawn.
                    if (at[2] - at[0]).cross(at[3] - at[1]).length_squared() < 1e-10 {
                        m.positions.truncate(start as usize);
                        m.normals.truncate(start as usize);
                        m.uvs.truncate(start as usize);
                        m.colors.truncate(start as usize);
                        m.tangents.truncate(start as usize);
                        continue;
                    }
                    // Split the quad along the diagonal that keeps the AO gradient smooth.
                    if ao[0] + ao[2] > ao[1] + ao[3] {
                        m.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
                    } else {
                        m.indices.extend([start + 1, start + 2, start + 3, start + 1, start + 3, start]);
                    }
                }
            }
        }
    }
    m
}

/// The faces of one broken cell: each solid bit as a little cube, by the same rules as a
/// cell. Slower a face than whole cells (every neighbour is looked up in the world), which is
/// fine while broken cells are few.
fn bits(world: &World, cell: IVec3, m: &mut MeshData) {
    let Some(data) = world.bits.get(&cell) else { return };
    // A neighbour inside this cell is read from it; only the bits at its faces ask the world.
    let near = |at: IVec3| {
        let local = at - cell * SPLIT;
        if local.cmpge(IVec3::ZERO).all() && local.cmplt(IVec3::splat(SPLIT)).all() { data[(local.x + local.z * SPLIT + local.y * SPLIT * SPLIT) as usize] } else { world.bit(at) }
    };
    for (i, &block) in data.iter().enumerate() {
        if !block.solid() || modelled(block) {
            continue;
        }
        let i = i as i32;
        let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
        let (top, side) = block.colors();
        let tint = shade(bit);
        for (n, u, v) in FACES {
            if near(bit + n).solid() {
                continue;
            }
            let base = if n == IVec3::Y { top } else { side };
            let grain = Grain::of(block, n.y != 0);
            let start = m.positions.len() as u32;
            let mut ao = [0.0f32; 4];
            for (i, (su, sv)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
                let corner = bit + n.max(IVec3::ZERO) + u * su + v * sv;
                let (du, dv) = (u * (su * 2 - 1), v * (sv * 2 - 1));
                let occ = |d: IVec3| near(bit + n + d).solid() as i32;
                let (a, b, c) = (occ(du), occ(dv), occ(du + dv));
                let level = if a + b == 2 { 0 } else { 3 - (a + b + c) };
                ao[i] = 0.45 + 0.55 * level as f32 / 3.0;
                let k = ao[i] * tint;
                m.positions.push((corner.as_vec3() * CELL / SPLIT as f32).to_array());
                m.normals.push(n.as_vec3().to_array());
                m.tangents.push(tangent(n));
                m.uvs.push(uv(grain, n, bit, corner, SPLIT));
                m.colors.push([base[0] * k, base[1] * k, base[2] * k, 1.0]);
            }
            if ao[0] + ao[2] > ao[1] + ao[3] {
                m.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
            } else {
                m.indices.extend([start + 1, start + 2, start + 3, start + 1, start + 3, start]);
            }
        }
    }
}

/// One box, in bits from a cell's corner: five faces (not its underside), one colour.
fn blade(m: &mut MeshData, cell: IVec3, lo: IVec3, hi: IVec3, colour: [f32; 3]) {
    let at = |p: IVec3| ((cell * SPLIT + p).as_vec3() * CELL / SPLIT as f32).to_array();
    // A single blade (one bit across) is two planes crossed on its diagonals, seen from both
    // sides: two faces where a box is five, and finer to look at. A meadow of boxes was more
    // faces than all the land under it.
    if hi.x - lo.x == 1 && hi.z - lo.z == 1 {
        for (a, b) in [(IVec3::new(lo.x, 0, lo.z), IVec3::new(hi.x, 0, hi.z)), (IVec3::new(hi.x, 0, lo.z), IVec3::new(lo.x, 0, hi.z))] {
            let start = m.positions.len() as u32;
            for (foot, y) in [(a, lo.y), (b, lo.y), (b, hi.y), (a, hi.y)] {
                let corner = IVec3::new(foot.x, y, foot.z);
                m.positions.push(at(corner));
                // Lit as the ground it grows from, not as a wall.
                m.normals.push([0.0, 1.0, 0.0]);
                m.tangents.push([1.0, 0.0, 0.0, 1.0]);
                let along = |c: i32, p: i32| (c.rem_euclid(SPAN) as f32 + p as f32 / SPLIT as f32).min(SPAN as f32 - 0.01);
                m.uvs.push(Grain::Grass.uv(along(cell.x, corner.x + corner.y), along(cell.z, corner.z + corner.y)));
                let shade = if corner.y == 0 { 0.5 } else { 0.85 + 0.3 * (corner.y as f32 / SPLIT as f32).min(1.0) };
                m.colors.push([colour[0] * shade / MEAN, colour[1] * shade / MEAN, colour[2] * shade / MEAN, 1.0 - (corner.y as f32 / SPLIT as f32).min(1.0)]);
            }
            m.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
        }
        return;
    }
    for (n, u, v) in FACES {
        if n == IVec3::NEG_Y {
            continue;
        }
        let size = hi - lo;
        // The face's corner on this side of the box, then along its two axes.
        let from = lo + n.max(IVec3::ZERO) * size;
        let start = m.positions.len() as u32;
        let shade = if n == IVec3::Y { 1.0 } else { 0.78 + 0.12 * (n.x + n.z) as f32 };
        for (su, sv) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
            let corner = from + u * size * su + v * size * sv;
            m.positions.push(at(corner));
            m.normals.push(n.as_vec3().to_array());
            m.tangents.push(tangent(n));
            // Painted with the grass's own grain, by where it stands, and dark at the foot where
            // its neighbours shade it: a flat-coloured box reads as a toy brick.
            let along = |c: i32, p: i32| (c.rem_euclid(SPAN) as f32 + p as f32 / SPLIT as f32).min(SPAN as f32 - 0.01);
            m.uvs.push(Grain::Grass.uv(along(cell.x, corner.x + corner.y), along(cell.z, corner.z + corner.y)));
            let shade = shade * if corner.y == 0 { 0.5 } else { 0.85 + 0.3 * (corner.y as f32 / SPLIT as f32).min(1.0) };
            // How free it is to move in the wind is in the alpha: the foot is held, the tip is not.
            m.colors.push([colour[0] * shade / MEAN, colour[1] * shade / MEAN, colour[2] * shade / MEAN, 1.0 - (corner.y as f32 / SPLIT as f32).min(1.0)]);
        }
        m.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
    }
}

/// Ground cover in one cell: a few thin blades a bit across, of heights and places that
/// come from where the cell is. A flower is a stem with a head; reeds stand the cell's
/// height.
fn sprig(cell: IVec3, block: Block, m: &mut MeshData) {
    let (top, side) = block.colors();
    let roll = |n: i32| hash(cell * 7 + IVec3::splat(n));
    if block == Block::Mushroom {
        // A pale stem and a cap three bits across, one or two to a cell.
        for i in 0..1 + (roll(0) * 2.0) as i32 {
            let (x, z) = (1 + (roll(i * 3 + 1) * 3.0) as i32, 1 + (roll(i * 3 + 2) * 3.0) as i32);
            let tall = 1 + (roll(i + 9) * 2.0) as i32;
            blade(m, cell, IVec3::new(x, 0, z), IVec3::new(x + 1, tall, z + 1), side);
            blade(m, cell, IVec3::new(x - 1, tall, z - 1).max(IVec3::ZERO), IVec3::new(x + 2, tall + 1, z + 2).min(IVec3::splat(SPLIT)), top);
        }
        return;
    }
    if block == Block::Fern {
        // Fronds: low wide slabs fanning out from the middle, each a little higher than the last.
        for i in 0..4 {
            let (dx, dz) = [(1, 0), (-1, 0), (0, 1), (0, -1)][i as usize];
            let tall = 1 + (roll(i + 3) * 3.0) as i32;
            let tint = 0.85 + 0.3 * roll(i + 20);
            let (lo, hi) = (IVec3::new(2 + dx.min(0) * 2, tall - 1, 2 + dz.min(0) * 2), IVec3::new(3 + dx.max(0) * 2, tall, 3 + dz.max(0) * 2));
            blade(m, cell, lo, hi, top.map(|c| c * tint));
        }
        blade(m, cell, IVec3::new(2, 0, 2), IVec3::new(3, 2, 3), side);
        return;
    }
    // A tuft is a clump, not three sticks: five to eight blades.
    let blades = if block == Block::Flower { 4 } else { 5 + (roll(0) * 4.0) as i32 };
    for i in 0..blades {
        let (x, z) = ((roll(i * 3 + 1) * SPLIT as f32) as i32 % SPLIT, (roll(i * 3 + 2) * SPLIT as f32) as i32 % SPLIT);
        let tall = match block {
            Block::Reed => SPLIT,
            Block::Flower if i == 0 => 3,
            _ => 2 + (roll(i * 3 + 3) * 3.0) as i32,
        };
        let tint = 0.85 + 0.3 * roll(i + 20);
        blade(m, cell, IVec3::new(x, 0, z), IVec3::new(x + 1, tall, z + 1), side.map(|c| c * tint));
        if block == Block::Flower && i == 0 {
            // Its head: white, yellow or violet by where it grows.
            let head = [[0.85, 0.82, 0.74], top, [0.48, 0.30, 0.62]][(roll(40) * 3.0) as usize % 3];
            blade(m, cell, IVec3::new(x - 1, tall, z - 1).max(IVec3::ZERO), IVec3::new(x + 2, tall + 1, z + 2).min(IVec3::splat(SPLIT)), head);
        }
    }
}

/// A chunk seen from far off: its cells taken `step` at a time each way, so a mesh a quarter
/// or a sixteenth the size. A group of cells is solid if any of them is, and is the block
/// highest in it, so distant ground has no holes and trees keep their bulk; ground cover and
/// the fine shape of broken cells are left out. Water is meshed apart, as up close: a group
/// is water if any of it is.
pub fn mesh_chunk_far(world: &World, chunk: IVec3, liquid: bool, step: i32) -> MeshData {
    let mut m = MeshData::default();
    if !world.chunks.contains_key(&chunk) {
        return m;
    }
    let near = Near::of(world, chunk);
    let n = CHUNK / step;
    let side = (n + 2) as usize;
    // Every group in the chunk and one beyond each face, worked out once.
    let mut groups = vec![Block::Air; side * side * side];
    let at_group = |s: IVec3| ((s.x + 1) + (s.z + 1) * (n + 2) + (s.y + 1) * (n + 2) * (n + 2)) as usize;
    for sy in -1..=n {
        for sz in -1..=n {
            for sx in -1..=n {
                let s = IVec3::new(sx, sy, sz);
                let mut found = Block::Air;
                'group: for y in (0..step).rev() {
                    for z in 0..step {
                        for x in 0..step {
                            let block = near.get(s * step + IVec3::new(x, y, z));
                            // (A broken cell far off is a rock's edge or a hole's rim: neither is drawn from here.)
                            let wanted = if liquid { block == Block::Water } else { block.solid() && !modelled(block) && !(block == Block::Broken && MODELLED.load(std::sync::atomic::Ordering::Relaxed)) };
                            if wanted && (found == Block::Air || eases(block)) {
                                // The bits of a broken cell are not looked at from here: stone.
                                found = if block == Block::Broken { Block::Stone } else { block };
                                // Ground wins over what stands on it: a group with a trunk's
                                // foot in it is the ground the trunk stands on, or the eased
                                // ground round it leaves a hole.
                                if liquid || found.ground() {
                                    break 'group;
                                }
                            }
                        }
                    }
                }
                groups[at_group(s)] = found;
            }
        }
    }
    let origin = chunk * CHUNK;
    // A group's top is up to a group above the ground it stands for, so columns count further.
    let mut eased = Eased { near: &near, reach: REACH + step, lifts: HashMap::new(), tops: HashMap::new(), origin: chunk * CHUNK };
    for sy in 0..n {
        for sz in 0..n {
            for sx in 0..n {
                let s = IVec3::new(sx, sy, sz);
                let block = groups[at_group(s)];
                if block == Block::Air || modelled(block) {
                    continue;
                }
                let cell = origin + s * step;
                let (top, side_colour) = block.colors();
                let mut tint = shade(cell);
                let mut warm = 0.0;
                if matches!(block, Block::Grass | Block::Turf | Block::Moss | Block::Litter) || block.foliage() {
                    let drift = patch(cell) - 0.5;
                    tint *= 1.0 + drift * 0.35;
                    warm = drift * 0.25;
                }
                if block.ground() && ground() == Ground::Stepped {
                    // As up close: the top group of a column draws its top and a riser down
                    // to each neighbour that stands lower.
                    if groups[at_group(s + IVec3::Y)].ground() {
                        continue;
                    }
                    let (local, level, half) = (s * step, (s.y + 1) * step, step / 2);
                    let high = eased.far_top(local.x + half, level, local.z + half, step);
                    for (normal, u, v) in FACES {
                        if normal == IVec3::NEG_Y || (normal == IVec3::Y && groups[at_group(s + normal)] != Block::Air) {
                            continue;
                        }
                        let low = if normal == IVec3::Y { high } else { eased.far_top(local.x + half + normal.x * step, level, local.z + half + normal.z * step, step) };
                        if normal != IVec3::Y && low > high - 1e-3 {
                            continue;
                        }
                        let low_riser = normal != IVec3::Y && high - low < 0.99;
                        let (base, grain) = (if normal == IVec3::Y { top } else if low_riser { top.map(|c| c * 0.92) } else { side_colour }, Grain::of(block, normal == IVec3::Y || low_riser));
                        let start = m.positions.len() as u32;
                        for (su, sv) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                            let corner = cell + (normal.max(IVec3::ZERO) + u * su + v * sv) * step;
                            let foot = normal != IVec3::Y && corner.y == cell.y;
                            let height = origin.y as f32 + if foot { low } else { high };
                            m.positions.push([corner.x as f32 * CELL, height * CELL, corner.z as f32 * CELL]);
                            m.normals.push(if low_riser { [0.0, 1.0, 0.0] } else { normal.as_vec3().to_array() });
                            m.tangents.push(tangent(normal));
                            // Far off the grain has blurred to its mean: where in the tile does
                            // not matter up a riser, only that it stays inside it.
                            m.uvs.push(uv(grain, normal, cell, corner, 1));
                            let k = tint * if foot && high - low > 0.3 { 0.7 } else { 1.0 };
                            m.colors.push([base[0] * k * (1.0 + warm), base[1] * k, base[2] * k * (1.0 - warm), 1.0]);
                        }
                        m.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
                    }
                    continue;
                }
                for (normal, u, v) in FACES {
                    if (cell.y == 0 && normal == IVec3::NEG_Y) || groups[at_group(s + normal)] != Block::Air {
                        continue;
                    }
                    let base = if normal == IVec3::Y { top } else { side_colour };
                    let grain = Grain::of(block, normal.y != 0);
                    let start = m.positions.len() as u32;
                    let mut ao = [0.0f32; 4];
                    let mut at = [Vec3::ZERO; 4];
                    for (i, (su, sv)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
                        let corner = cell + (normal.max(IVec3::ZERO) + u * su + v * sv) * step;
                        let (du, dv) = (u * (su * 2 - 1), v * (sv * 2 - 1));
                        // Ground shades ground only up close.
                        let occ = |d: IVec3| (!liquid && !eases(block) && groups[at_group(s + normal + d)] != Block::Air) as i32;
                        let (a, b, c) = (occ(du), occ(dv), occ(du + dv));
                        let level = if a + b == 2 { 0 } else { 3 - (a + b + c) };
                        ao[i] = 0.45 + 0.55 * level as f32 / 3.0;
                        let k = ao[i] * tint;
                        let rooted = normal.y == 0 && corner.y == cell.y && groups[at_group(s - IVec3::Y)].ground() && ground() != Ground::Cubes;
                        let lift = if eases(block) {
                            eased.lift(corner - origin)
                        } else if rooted && !block.ground() {
                            -(step as f32)
                        } else {
                            0.0
                        };
                        at[i] = (corner.as_vec3() + Vec3::Y * lift) * CELL;
                        let facing = if eases(block) && normal != IVec3::NEG_Y { eased.normal(corner - origin) } else { normal.as_vec3() };
                        m.positions.push(at[i].to_array());
                        m.normals.push(facing.to_array());
                        m.tangents.push(tangent(normal));
                        m.uvs.push(if liquid { [corner.x as f32 * CELL / WATER_TILE, corner.z as f32 * CELL / WATER_TILE] } else { uv(grain, normal, cell, corner, 1) });
                        let base = if eases(block) && normal == IVec3::Y { eased.corner(corner - origin).1 } else { base };
                        m.colors.push([base[0] * k * (1.0 + warm), base[1] * k, base[2] * k * (1.0 - warm), if liquid { 0.8 } else if block.foliage() { 1.0 - LEAF } else { 1.0 }]);
                    }
                    if (at[2] - at[0]).cross(at[3] - at[1]).length_squared() < 1e-10 {
                        m.positions.truncate(start as usize);
                        m.normals.truncate(start as usize);
                        m.uvs.truncate(start as usize);
                        m.colors.truncate(start as usize);
                        m.tangents.truncate(start as usize);
                        continue;
                    }
                    if ao[0] + ao[2] > ao[1] + ao[3] {
                        m.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
                    } else {
                        m.indices.extend([start + 1, start + 2, start + 3, start + 1, start + 3, start]);
                    }
                }
            }
        }
    }
    m
}
