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
pub const TILE: usize = 512;
pub const BORDER: usize = 64;
const ROOM: usize = TILE + 2 * BORDER;
/// Tiles along the atlas, and how many smaller copies of it there are (each half the last,
/// down to eight texels a tile and one of border).
pub const ACROSS: usize = 4;
pub const LEVELS: usize = 6;
/// How strongly the grain shows: 1 is as the shapes are written.
pub const DEPTH: f32 = 1.45;
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
            Block::Stone | Block::Slate | Block::Chalk | Block::Broken | Block::Rock => if top { Grain::Rock } else { Grain::Strata },
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

/// Smooth noise that repeats every tile. The shapes are written on a tile 64 across (a unit
/// is 3 cm) whatever its texels: `size` units from one lump to the next (a divisor of 64),
/// each way.
fn lumps(x: f32, y: f32, size: (usize, usize), seed: u32) -> f32 {
    let (nx, ny) = ((64 / size.0) as i32, (64 / size.1) as i32);
    let (fx, fy) = (x / size.0 as f32, y / size.1 as f32);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let at = |dx: i32, dy: i32| hash((x0 as i32 + dx).rem_euclid(nx), (y0 as i32 + dy).rem_euclid(ny), seed);
    let (a, b) = (at(0, 0) * (1.0 - sx) + at(1, 0) * sx, at(0, 1) * (1.0 - sx) + at(1, 1) * sx);
    a * (1.0 - sy) + b * sy
}

/// What colour a point of a grain leans to, as a multiplier of the block's colour: grass has
/// dry blades and lush ones, fallen leaves are russet and gold and brown, rock carries lichen
/// and rust, bark carries moss. Brightness is [`texel`]'s; this is hue. Without it a surface
/// was one colour made lighter and darker, which reads as plastic.
fn hue(grain: Grain, x: f32, y: f32) -> [f32; 3] {
    let seed = grain as u32 * 16 + 9;
    let lump = |size: (usize, usize), n: u32| lumps(x, y, size, seed + n) - 0.5;
    // Toward yellow (over 0) or toward blue-green (under).
    let warm = |w: f32| [1.0 + w, 1.0 + w * 0.25, 1.0 - w * 1.3];
    // Where a noise runs high, a patch of something else.
    let patch = |size: (usize, usize), n: u32, over: f32, soft: f32| ((lump(size, n) - over) / soft).clamp(0.0, 1.0);
    let blend = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
    match grain {
        Grain::Plain | Grain::Snow => [1.0; 3],
        Grain::Grass => blend(warm(lump((8, 16), 1) * 0.5 + lump((2, 8), 2) * 0.35), [1.35, 1.12, 0.6], patch((4, 8), 3, 0.2, 0.08) * 0.7),
        Grain::Soil => blend(warm(lump((16, 16), 1) * 0.25), [0.8, 0.82, 0.9], patch((4, 4), 2, 0.22, 0.05) * 0.7),
        Grain::Rock | Grain::Strata => {
            let rust = blend(warm(lump((32, 16), 1) * 0.12), [1.18, 0.95, 0.74], patch((8, 32), 2, 0.2, 0.1) * 0.55);
            blend(rust, [0.82, 1.08, 0.62], patch((16, 16), 3, 0.17, 0.06) * 0.75)
        }
        Grain::Sand => warm(lump((16, 8), 1) * 0.18),
        Grain::Pebbles => warm(lump((8, 8), 1) * 0.3),
        Grain::Bark => blend(warm(lump((8, 32), 1) * 0.2), [0.7, 1.15, 0.55], patch((16, 16), 2, 0.16, 0.07) * 0.8),
        Grain::BirchBark => blend([1.0; 3], [0.8, 1.0, 0.7], patch((16, 16), 2, 0.2, 0.08) * 0.5),
        Grain::EndGrain => warm(lump((16, 16), 1) * 0.2),
        Grain::Leaves => warm(lump((8, 8), 1) * 0.6 + lump((4, 4), 2) * 0.3),
        Grain::Needles => warm(lump((8, 8), 1) * 0.3 - 0.04),
        Grain::Litter => {
            let russet = blend([0.97, 0.97, 1.0], [1.3, 0.97, 0.6], patch((8, 4), 1, 0.08, 0.12));
            blend(russet, [1.45, 1.25, 0.6], patch((4, 8), 2, 0.24, 0.05) * 0.8)
        }
        Grain::Moss => warm(lump((8, 8), 1) * 0.6),
    }
}

/// How bright one point of a grain is, about 1: lighter over, darker under. `y` runs down
/// the tile, so on a block's side it is height. Broad shapes first, then the fine: blades,
/// grit, the fibre of bark.
fn texel(grain: Grain, x: f32, y: f32) -> f32 {
    let seed = grain as u32 * 16;
    let lump = |size: (usize, usize), n: u32| lumps(x, y, size, seed + n) - 0.5;
    // A soft dark line where a noise crosses its middle: a crack, a groove.
    let line = |size: (usize, usize), n: u32, wide: f32| {
        let t = (1.0 - lump(size, n).abs() / wide).max(0.0);
        t * t * (3.0 - 2.0 * t)
    };
    // A noise pushed toward two tones with a soft edge between: a dab of the brush.
    let dab = |size: (usize, usize), n: u32, edge: f32| (lump(size, n) / edge).clamp(-1.0, 1.0) * 0.5;
    // The fine: what is seen from a step away.
    let fine = match grain {
        Grain::Plain | Grain::Snow => 0.0,
        Grain::Grass | Grain::Needles => dab((1, 4), 30, 0.25) * 0.3 + lump((2, 2), 31) * 0.12,
        Grain::Bark => dab((1, 8), 30, 0.3) * 0.3 + lump((2, 4), 31) * 0.15,
        Grain::Strata => lump((8, 1), 30) * 0.22 + lump((2, 2), 31) * 0.14,
        Grain::Leaves | Grain::Litter | Grain::Moss => dab((2, 2), 30, 0.2) * 0.3 + lump((1, 1), 31) * 0.12,
        _ => lump((2, 2), 30) * 0.2 + lump((1, 1), 31) * 0.16,
    };
    fine + match grain {
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
        Grain::EndGrain => 1.0 + ((((x - 32.0).powi(2) + (y - 32.0).powi(2)).sqrt() * 0.8).sin() * 0.12) + lump((16, 16), 1) * 0.15,
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
/// size, one after another. A smaller copy is each four texels averaged, and a tile and its
/// border halve with it. Two pictures the same shape: `rgba` is the paint (sRGB), `relief`
/// is which way each point of the surface faces (a normal map, linear), so a low sun rakes
/// across bark and stone and picks out their grain.
pub struct Atlas {
    /// Texels along the largest copy.
    pub size: usize,
    pub levels: usize,
    pub rgba: Vec<u8>,
    pub relief: Vec<u8>,
}

/// How steep the relief is for a given change of brightness.
const RELIEF: f32 = 2.0;

/// A surface as photographed: its colour at every texel of a tile (linear light, [`TILE`] a
/// side, repeating) and which way each point faces (a normal map, y up). Where the game has
/// one for a grain it takes the place of the grain made here; the block's colour still tints
/// it, since every tile is brought to the same mean.
pub struct Photo {
    pub colour: Vec<[f32; 3]>,
    pub facing: Vec<[f32; 3]>,
}

/// A crown's face, painted leaf by leaf: each a pointed blade with a midrib (or a needle),
/// laid at its own angle from the back of the crown to the front, the back ones in shade and
/// the front ones lit, some a little yellower or bluer. Where no leaf lies there is a gap.
/// Its colour at every texel, how high (which is how lit), and whether anything is there.
/// No scan of a crown was to be had; fallen leaves on the ground stood in for a while and
/// looked like what they were.
fn foliage(needles: bool) -> (Vec<[f32; 3]>, Vec<f32>, Vec<bool>) {
    let (mut colour, mut high, mut there) = (vec![[0.3f32; 3]; TILE * TILE], vec![0.0f32; TILE * TILE], vec![false; TILE * TILE]);
    // Leaves nine centimetres long; needles in sprays five long.
    let (count, long, wide) = if needles { (9000, 13.0f32, 1.3f32) } else { (2300, 12.0, 5.5) };
    let seed = if needles { 700 } else { 600 };
    for i in 0..count {
        let roll = |n: u32| hash(i, n as i32, seed);
        let (cx, cy, turn) = (roll(0) * TILE as f32, roll(1) * TILE as f32, if needles { 1.2 + roll(2) * 0.8 } else { roll(2) * std::f32::consts::TAU });
        let (sin, cos) = turn.sin_cos();
        let (a, b) = (long * (0.7 + 0.6 * roll(3)), wide * (0.75 + 0.5 * roll(4)));
        // Later ones lie over earlier ones and are nearer the light.
        let depth = i as f32 / count as f32;
        let lit = (0.32 + 0.85 * depth * depth) * (0.85 + 0.3 * roll(5));
        let warm = (roll(6) - 0.5) * 0.3;
        let reach = a.ceil() as i32 + 1;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (u, v) = (dx as f32 * cos + dy as f32 * sin, dy as f32 * cos - dx as f32 * sin);
                // A blade: widest a third along, pointed at both ends.
                let across = b * (1.0 - (u / a) * (u / a)).max(0.0) * (1.0 - 0.3 * u / a);
                if u.abs() >= a || v.abs() >= across {
                    continue;
                }
                let at = (cx as i32 + dx).rem_euclid(TILE as i32) as usize + (cy as i32 + dy).rem_euclid(TILE as i32) as usize * TILE;
                // Dark along the midrib and toward the edge, cupped.
                let shape = if needles { 1.0 } else { (1.0 - 0.25 * (v.abs() / across.max(0.5)).powi(2)) * if v.abs() < 0.7 { 0.78 } else { 1.0 } };
                let k = lit * shape;
                colour[at] = [k * (1.0 + warm), k, k * (1.0 - warm * 1.4)];
                high[at] = depth + 0.1 * (1.0 - v.abs() / across.max(0.5));
                there[at] = true;
            }
        }
    }
    (colour, high, there)
}

pub fn atlas() -> Atlas {
    atlas_with(|_| None)
}

pub fn atlas_with(photo: impl Fn(Grain) -> Option<Photo>) -> Atlas {
    let size = ROOM * ACROSS;
    let mut paint = vec![[MEAN; 3]; size * size];
    let mut facing = vec![[0.0, 0.0, 1.0f32]; size * size];
    // How much of each texel is there at all: 1 everywhere but in foliage, which has gaps.
    let mut cover = vec![[1.0f32; 3]; size * size];
    let unit = 64.0 / TILE as f32;
    for (i, &grain) in Grain::ALL.iter().enumerate() {
        let painted = matches!(grain, Grain::Leaves | Grain::Needles).then(|| foliage(grain == Grain::Needles));
        let taken = photo(grain).filter(|p| painted.is_none() && p.colour.len() == TILE * TILE && p.facing.len() == TILE * TILE);
        // The shapes are written gently and deepened here, in one place: at their written
        // strength the developer saw no texture at all.
        let bright: Vec<f32> = if let Some((_, high, _)) = &painted { high.iter().map(|h| h * 0.5).collect() } else if taken.is_some() { Vec::new() } else { (0..TILE * TILE).map(|t| (1.0 + (texel(grain, (t % TILE) as f32 * unit, (t / TILE) as f32 * unit) - 1.0) * DEPTH).max(0.05)).collect() };
        let mut tile: Vec<[f32; 3]> = match &taken {
            // Sand, gravel and snow are fine and even in life; as scanned they are harsh, and
            // a lake bed of them seen through water was white speckle. They keep half their grain.
            Some(p) => {
                let keep = if matches!(grain, Grain::Sand | Grain::Pebbles | Grain::Snow) { 0.45 } else { 1.0 };
                let mean: [f32; 3] = std::array::from_fn(|c| p.colour.iter().map(|v| v[c]).sum::<f32>() / p.colour.len() as f32);
                p.colour.iter().map(|v| std::array::from_fn(|c| mean[c] + (v[c] - mean[c]) * keep)).collect()
            }
            None if painted.is_some() => painted.as_ref().unwrap().0.clone(),
            None => (0..TILE * TILE).map(|t| hue(grain, (t % TILE) as f32 * unit, (t / TILE) as f32 * unit).map(|c| c * bright[t])).collect(),
        };
        // Every tile to the same mean in each colour, so far off it is its block's colour
        // and no other. (Nothing is brighter than 1, so a pale tile with dark marks takes a
        // few rounds.)
        for _ in 0..12 {
            for c in 0..3 {
                let mean = tile.iter().map(|v| v[c]).sum::<f32>() / tile.len() as f32;
                tile.iter_mut().for_each(|v| v[c] = (v[c] * MEAN / mean.max(1e-4)).min(1.0));
            }
        }
        let lean: Vec<[f32; 3]> = match taken {
            Some(p) => p.facing,
            None => {
                // Brighter is higher: the slope of brightness is the lean of the surface.
                let high = |x: usize, y: usize| bright[x % TILE + y % TILE * TILE];
                (0..TILE * TILE).map(|t| {
                    let (x, y) = (t % TILE + TILE, t / TILE + TILE);
                    let (dx, dy) = (high(x + 1, y) - high(x - 1, y), high(x, y + 1) - high(x, y - 1));
                    let n = glam::Vec3::new(-dx * RELIEF * 4.0, -dy * RELIEF * 4.0, 1.0).normalize();
                    [n.x, n.y, n.z]
                }).collect()
            }
        };
        // Sand, gravel and snow lie nearly flat: at their scanned relief a low sun rakes every
        // grain, and a lake bed of them was white speckle through the water.
        let flat = if matches!(grain, Grain::Sand | Grain::Pebbles | Grain::Snow) { 0.3 } else { 1.0 };
        let lean: Vec<[f32; 3]> = lean.into_iter().map(|n| { let v = glam::Vec3::new(n[0] * flat, n[1] * flat, n[2]).normalize(); [v.x, v.y, v.z] }).collect();
        // The tile with its border: the border is the tile carried round from its far side.
        let (ox, oy) = (i % ACROSS * ROOM, i / ACROSS * ROOM);
        for t in 0..ROOM * ROOM {
            let (x, y) = ((t % ROOM + TILE - BORDER) % TILE, (t / ROOM + TILE - BORDER) % TILE);
            paint[ox + t % ROOM + (oy + t / ROOM) * size] = tile[x + y * TILE];
            facing[ox + t % ROOM + (oy + t / ROOM) * size] = lean[x + y * TILE];
            cover[ox + t % ROOM + (oy + t / ROOM) * size] = [if painted.as_ref().is_some_and(|p| !p.2[x + y * TILE]) { 0.0 } else { 1.0 }; 3];
        }
    }
    let (mut rgba, mut relief) = (Vec::new(), Vec::new());
    let halve = |level: &[[f32; 3]], side: usize| -> Vec<[f32; 3]> {
        let half = side / 2;
        (0..half * half).map(|i| {
            let (x, y) = (i % half * 2, i / half * 2);
            let at = |dx: usize, dy: usize| level[x + dx + (y + dy) * side];
            std::array::from_fn(|c| (at(0, 0)[c] + at(1, 0)[c] + at(0, 1)[c] + at(1, 1)[c]) / 4.0)
        }).collect()
    };
    let mut side = size;
    for n in 0..LEVELS {
        // Linear light to sRGB, as the picture is read.
        rgba.extend(paint.iter().zip(&cover).flat_map(|(v, there)| {
            let s = v.map(|v| (if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 } * 255.0).round() as u8);
            [s[0], s[1], s[2], (there[0] * 255.0) as u8]
        }));
        relief.extend(facing.iter().flat_map(|v| {
            let n = glam::Vec3::from_array(*v).normalize();
            [n.x, n.y, n.z].map(|c| ((c * 0.5 + 0.5) * 255.0).round() as u8).into_iter().chain([255])
        }));
        if n + 1 < LEVELS {
            (paint, facing, cover) = (halve(&paint, side), halve(&facing, side), halve(&cover, side));
            side /= 2;
        }
    }
    Atlas { size, levels: LEVELS, rgba, relief }
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
