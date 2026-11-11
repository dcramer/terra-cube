//! What a cube's faces are painted with: one small picture of every surface, made here in
//! code from a seed, so there is no art file to keep and no two builds differ.
//!
//! A picture holds grain, not colour: how much lighter or darker each point of a surface is
//! than the block's colour, which still comes from [`Block::colors`] through the mesh. So a
//! new block needs a colour and the name of a grain it shares, not a painting.
//!
//! Painted, not pixelled: the grain is soft shapes the size of brush strokes (a hand to a
//! forearm across), drawn smooth at any distance. Square texels the size of a bit were tried
//! first and were Minecraft's look exactly. A tile is [`SPAN`] cells (two metres) each way
//! and repeats, so the pattern on a wall is larger than a cube and the cube grid is not what
//! the eye picks out. Every tile's mean is [`MEAN`], and the mesher divides colours by it, so
//! a surface seen from far off (where the picture has blurred to its mean) is the block's
//! colour.

use crate::Block;

/// Cells a tile covers each way before it repeats.
pub const SPAN: i32 = 8;
/// Texels along a tile, and along the border round it that repeats the tile's far side, so
/// a smooth sample at a tile's edge, and the smaller copies of the atlas, never take colour
/// from the tile beside it.
pub const TILE: usize = 64;
pub const BORDER: usize = 8;
const ROOM: usize = TILE + 2 * BORDER;
/// Tiles along the atlas, and how many smaller copies of it there are (each half the last,
/// down to eight texels a tile and one of border).
pub const ACROSS: usize = 4;
pub const LEVELS: usize = 4;
/// The brightness every tile averages, of 1.
pub const MEAN: f32 = 0.8;

/// A grain: one tile of the atlas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grain {
    /// Nothing: water, ground cover.
    Plain,
    Grass,
    Soil,
    /// Stone seen from above: blotches and cracks.
    Rock,
    /// Stone seen from the side: beds lying one on another.
    Strata,
    Sand,
    Snow,
    Pebbles,
    Bark,
    BirchBark,
    /// The cut end of wood.
    EndGrain,
    Leaves,
    Needles,
    /// Fallen leaves.
    Litter,
    Moss,
}

impl Grain {
    pub const ALL: [Grain; 15] = [
        Grain::Plain, Grain::Grass, Grain::Soil, Grain::Rock, Grain::Strata, Grain::Sand, Grain::Snow, Grain::Pebbles,
        Grain::Bark, Grain::BirchBark, Grain::EndGrain, Grain::Leaves, Grain::Needles, Grain::Litter, Grain::Moss,
    ];

    /// The grain of a block's top (and underside) or of its sides.
    pub fn of(block: Block, top: bool) -> Grain {
        match block {
            Block::Grass | Block::Turf => Grain::Grass,
            Block::Dirt => Grain::Soil,
            Block::Stone | Block::Slate | Block::Chalk | Block::Broken => if top { Grain::Rock } else { Grain::Strata },
            Block::Sand => Grain::Sand,
            Block::Snow => Grain::Snow,
            Block::Gravel => Grain::Pebbles,
            Block::Wood => if top { Grain::EndGrain } else { Grain::Bark },
            Block::Birch => if top { Grain::EndGrain } else { Grain::BirchBark },
            Block::Leaves | Block::LeavesPale | Block::LeavesDeep => Grain::Leaves,
            Block::Needles => Grain::Needles,
            Block::Litter => if top { Grain::Litter } else { Grain::Soil },
            Block::Moss => if top { Grain::Moss } else { Grain::Strata },
            Block::Air | Block::Water | Block::Tuft | Block::Flower | Block::Reed | Block::Fern | Block::Mushroom => Grain::Plain,
        }
    }

    /// Where a point of this grain's tile is in the atlas, 0 to 1 each way: `u` and `v` are in
    /// cells along the face, any whole number or part of one (the tile repeats every [`SPAN`]).
    pub fn uv(self, u: f32, v: f32) -> [f32; 2] {
        let (tile, span, room) = (self as usize, SPAN as f32, ROOM as f32);
        let within = |t: f32| (BORDER as f32 + t / span * TILE as f32) / room;
        [((tile % ACROSS) as f32 + within(u)) / ACROSS as f32, ((tile / ACROSS) as f32 + within(v)) / ACROSS as f32]
    }
}

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E3779B1) ^ (y as u32).wrapping_mul(0x85EBCA77) ^ seed.wrapping_mul(0xC2B2AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Smooth noise that repeats every tile: `size` texels from one lump to the next (a divisor
/// of [`TILE`]), stretched by `sx` and `sy`.
fn lumps(x: usize, y: usize, size: (usize, usize), seed: u32) -> f32 {
    let (nx, ny) = ((TILE / size.0) as i32, (TILE / size.1) as i32);
    let (fx, fy) = (x as f32 / size.0 as f32, y as f32 / size.1 as f32);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let at = |dx: i32, dy: i32| hash((x0 as i32 + dx).rem_euclid(nx), (y0 as i32 + dy).rem_euclid(ny), seed);
    let (a, b) = (at(0, 0) * (1.0 - sx) + at(1, 0) * sx, at(0, 1) * (1.0 - sx) + at(1, 1) * sx);
    a * (1.0 - sy) + b * sy
}

/// How bright one texel of a grain is, about 1: lighter over, darker under. `y` runs down
/// the tile, so on a block's side it is height. Shapes, not speckle: the smallest thing here
/// is four texels (12 cm) over, and most are larger.
fn texel(grain: Grain, x: usize, y: usize) -> f32 {
    let seed = grain as u32 * 16;
    let lump = |size: (usize, usize), n: u32| lumps(x, y, size, seed + n) - 0.5;
    // A soft dark line where a noise crosses its middle: a crack, a groove.
    let line = |size: (usize, usize), n: u32, wide: f32| {
        let t = (1.0 - lump(size, n).abs() / wide).max(0.0);
        t * t * (3.0 - 2.0 * t)
    };
    // A noise pushed toward two tones with a soft edge between: a dab of the brush.
    let dab = |size: (usize, usize), n: u32, edge: f32| (lump(size, n) / edge).clamp(-1.0, 1.0) * 0.5;
    match grain {
        Grain::Plain => 1.0,
        // Tufts: long soft strokes leaning up the tile, over broad lighter and darker patches.
        Grain::Grass => 1.0 + dab((4, 16), 1, 0.2) * 0.22 + lump((16, 16), 2) * 0.3 + lump((32, 32), 3) * 0.2,
        Grain::Soil => 1.0 + dab((8, 8), 1, 0.25) * 0.2 + lump((16, 16), 2) * 0.3 + dab((4, 4), 3, 0.12).max(0.3) * 0.25 - 0.075,
        Grain::Rock => 1.0 + dab((16, 16), 1, 0.3) * 0.22 + lump((8, 8), 2) * 0.18 - line((32, 32), 3, 0.03) * 0.35 - line((16, 32), 4, 0.02) * 0.2,
        // Beds: long flat bands of their own shade, with a dark seam here and there.
        Grain::Strata => 1.0 + dab((64, 8), 1, 0.2) * 0.26 + lump((32, 4), 2) * 0.16 + lump((16, 16), 3) * 0.12 - line((32, 8), 4, 0.03) * 0.3,
        Grain::Sand => 1.0 + lump((32, 8), 1) * 0.14 + lump((8, 4), 2) * 0.07,
        Grain::Snow => 1.0 + lump((32, 32), 1) * 0.07 + lump((8, 8), 2) * 0.03,
        // Stones: rounded lumps of their own shade with shadow between.
        Grain::Pebbles => 1.0 + dab((8, 8), 1, 0.15) * 0.3 + lump((16, 16), 2) * 0.15 - line((8, 8), 1, 0.06) * 0.3,
        // Ridges running up the trunk with deep grooves between.
        Grain::Bark => 1.0 + dab((8, 32), 1, 0.25) * 0.3 + lump((4, 16), 2) * 0.2 - line((8, 64), 3, 0.05) * 0.4,
        // Pale, with dark eyes and dashes lying across it.
        Grain::BirchBark => 1.0 + lump((32, 16), 1) * 0.08 - (dab((16, 4), 2, 0.05) + 0.5 - 0.82).max(0.0) * 3.4,
        // Rings.
        Grain::EndGrain => 1.0 + ((((x as f32 - 32.0).powi(2) + (y as f32 - 32.0).powi(2)).sqrt() * 0.8).sin() * 0.12) + lump((16, 16), 1) * 0.15,
        // Leaves: clusters catching the light, with pools of shade between.
        Grain::Leaves => 1.0 + dab((8, 8), 1, 0.18) * 0.34 + lump((16, 16), 2) * 0.25 - line((8, 8), 3, 0.04) * 0.2,
        // Sprays: finer, hanging.
        Grain::Needles => 1.0 + dab((4, 8), 1, 0.2) * 0.3 + lump((16, 16), 2) * 0.25,
        // Leaves lying every way, a few pale.
        Grain::Litter => 1.0 + dab((8, 4), 1, 0.15) * 0.25 + dab((4, 8), 2, 0.15) * 0.2 + lump((16, 16), 3) * 0.2,
        // Cushions.
        Grain::Moss => 1.0 + dab((8, 8), 1, 0.3) * 0.3 + lump((16, 16), 2) * 0.2 + lump((4, 4), 3) * 0.1,
    }
}

/// The atlas: every grain's tile in a square, then smaller copies of the whole, each half the
/// size, as sRGB RGBA bytes one after another. A smaller copy is each four texels averaged,
/// and a tile and its border halve with it.
pub struct Atlas {
    /// Texels along the largest copy.
    pub size: usize,
    pub levels: usize,
    pub rgba: Vec<u8>,
}

pub fn atlas() -> Atlas {
    let size = ROOM * ACROSS;
    let mut first = vec![MEAN; size * size];
    for (i, &grain) in Grain::ALL.iter().enumerate() {
        let mut tile: Vec<f32> = (0..TILE * TILE).map(|t| texel(grain, t % TILE, t / TILE).max(0.05)).collect();
        // Every tile to the same mean, so far off it is its block's colour and no other.
        // (Nothing is brighter than 1, so a pale tile with dark marks takes a few rounds.)
        for _ in 0..12 {
            let mean = tile.iter().sum::<f32>() / tile.len() as f32;
            tile.iter_mut().for_each(|v| *v = (*v * MEAN / mean).min(1.0));
        }
        // The tile with its border: the border is the tile carried round from its far side.
        let (ox, oy) = (i % ACROSS * ROOM, i / ACROSS * ROOM);
        for t in 0..ROOM * ROOM {
            let (x, y) = ((t % ROOM + TILE - BORDER) % TILE, (t / ROOM + TILE - BORDER) % TILE);
            first[ox + t % ROOM + (oy + t / ROOM) * size] = tile[x + y * TILE];
        }
    }
    let mut rgba = Vec::new();
    let (mut level, mut side) = (first, size);
    for n in 0..LEVELS {
        // Linear light to sRGB, as the picture is read.
        rgba.extend(level.iter().flat_map(|v| {
            let s = (if *v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 } * 255.0).round() as u8;
            [s, s, s, 255]
        }));
        if n + 1 < LEVELS {
            let half = side / 2;
            level = (0..half * half).map(|i| {
                let (x, y) = (i % half * 2, i / half * 2);
                (level[x + y * side] + level[x + 1 + y * side] + level[x + (y + 1) * side] + level[x + 1 + (y + 1) * side]) / 4.0
            }).collect();
            side = half;
        }
    }
    Atlas { size, levels: LEVELS, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tile_averages_the_mean_at_every_level() {
        let atlas = atlas();
        let (mut at, mut side) = (0, atlas.size);
        for _ in 0..atlas.levels {
            let (room, border) = (side / ACROSS, side / ACROSS * BORDER / ROOM);
            let tile = room - 2 * border;
            for i in 0..Grain::ALL.len() {
                let (ox, oy) = (i % ACROSS * room + border, i / ACROSS * room + border);
                let sum: f32 = (0..tile * tile).map(|t| ((atlas.rgba[at + (ox + t % tile + (oy + t / tile) * side) * 4] as f32 / 255.0 + 0.055) / 1.055).powf(2.4)).sum();
                let mean = sum / (tile * tile) as f32;
                assert!((mean - MEAN).abs() < 0.03, "{:?} at {side}: {mean}", Grain::ALL[i]);
            }
            at += side * side * 4;
            side /= 2;
        }
        assert_eq!(at, atlas.rgba.len());
    }

    #[test]
    fn a_face_stays_inside_its_tile_and_off_its_border() {
        for (i, grain) in Grain::ALL.into_iter().enumerate() {
            let (lo, hi) = (grain.uv(0.0, 0.0), grain.uv(SPAN as f32, SPAN as f32));
            let room = 1.0 / ACROSS as f32;
            let edge = (i % ACROSS) as f32 * room;
            assert!(lo[0] > edge && hi[0] < edge + room && hi[1] < 1.0, "{grain:?}");
            assert!((hi[0] - lo[0] - room * TILE as f32 / ROOM as f32).abs() < 1e-6);
        }
    }
}
