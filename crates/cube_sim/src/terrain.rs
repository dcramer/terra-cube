//! Making a world from a seed: an island of meadow, forest, rock and snow, cut by a river,
//! with trees, undergrowth, boulders and ground cover on it (the shapes are in `plants`). The same seed always makes the same world.
//!
//! Two passes. [`Land::new`] works out every column (its height and what grows there) from
//! layered noise; [`generate`] fills the cells and plants things. `Land` is kept beside the
//! world: it is how creatures are placed and how the world is measured (`survey`).

use crate::plants::{self, Tree};
use crate::rng::Rng;
use cube_voxel::{Block, CELL, CHUNK, World, cells};
use glam::{IVec3, Vec3};

/// 0 to 1 from two whole numbers and a seed.
pub fn hash2(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E3779B1) ^ (z as u32).wrapping_mul(0x85EBCA77) ^ seed.wrapping_mul(0xC2B2AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

fn value_noise(x: f32, z: f32, seed: u32) -> f32 {
    let (x0, z0) = (x.floor(), z.floor());
    let (fx, fz) = (x - x0, z - z0);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let (xi, zi) = (x0 as i32, z0 as i32);
    let a = hash2(xi, zi, seed) * (1.0 - sx) + hash2(xi + 1, zi, seed) * sx;
    let b = hash2(xi, zi + 1, seed) * (1.0 - sx) + hash2(xi + 1, zi + 1, seed) * sx;
    a * (1.0 - sz) + b * sz
}

/// Four layers of noise, each twice as fine and half as strong: 0 to 1, mostly near 0.5.
fn fbm(x: f32, z: f32, seed: u32) -> f32 {
    (0..4).map(|o| value_noise(x * (1 << o) as f32, z * (1 << o) as f32, seed + o) / (1 << o) as f32).sum::<f32>() / 1.875
}

fn smooth(lo: f32, hi: f32, v: f32) -> f32 {
    let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Water level: metres over the bottom of the world, and the same in cells.
pub const SEA_LEVEL: f32 = 8.0;
pub const SEA: i32 = (SEA_LEVEL / CELL) as i32;
/// The world the game starts in, and the one `cargo world` judges: its seed, and how many
/// chunks it runs out from the origin (96 m each way).
pub const SEED: u32 = 6;
pub const RADIUS: i32 = (96.0 / (CELL * CHUNK as f32)) as i32;

/// What grows on a column, and so who lives there.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Biome {
    /// Under the sea, a lake or the river.
    Water,
    Shore,
    Meadow,
    Forest,
    Rock,
    Snow,
}

impl Biome {
    pub const ALL: [Biome; 6] = [Biome::Water, Biome::Shore, Biome::Meadow, Biome::Forest, Biome::Rock, Biome::Snow];
    pub fn name(self) -> &'static str {
        ["water", "shore", "meadow", "forest", "rock", "snow"][self as usize]
    }
    pub fn named(name: &str) -> Option<Biome> {
        Biome::ALL.into_iter().find(|b| b.name() == name)
    }
}

/// The lie of the land: every column's height and biome, and where the trees stand.
pub struct Land {
    pub seed: u32,
    /// Cells from the origin to the world's edge.
    pub span: i32,
    heights: Vec<i32>,
    biomes: Vec<Biome>,
    /// The foot of each tree's trunk.
    pub trees: Vec<IVec3>,
}

impl Land {
    pub fn new(seed: u32, radius: i32) -> Self {
        let span = radius * CHUNK;
        let side = (span * 2) as usize;
        let mut heights = vec![0; side * side];
        for z in -span..span {
            for x in -span..span {
                let metres = column((x as f32 + 0.5) * CELL, (z as f32 + 0.5) * CELL, seed, span as f32 * CELL);
                heights[((x + span) + (z + span) * span * 2) as usize] = cells(metres).max(cells(1.0));
            }
        }
        let mut land = Land { seed, span, heights, biomes: Vec::new(), trees: Vec::new() };
        let mut biomes = Vec::with_capacity(side * side);
        for z in -span..span {
            for x in -span..span {
                biomes.push(land.biome_of(x, z));
            }
        }
        land.biomes = biomes;
        land
    }

    fn index(&self, x: i32, z: i32) -> Option<usize> {
        let inside = x >= -self.span && x < self.span && z >= -self.span && z < self.span;
        inside.then(|| ((x + self.span) + (z + self.span) * self.span * 2) as usize)
    }

    /// The ground's height in cells (the first cell of air or water over it); 0 off the edge.
    pub fn height(&self, x: i32, z: i32) -> i32 {
        self.index(x, z).map_or(0, |i| self.heights[i])
    }

    pub fn biome(&self, x: i32, z: i32) -> Biome {
        self.index(x, z).map_or(Biome::Water, |i| self.biomes[i])
    }

    /// The most the ground rises or falls to a neighbouring column, in cells. Cells are as
    /// high as they are wide, so this is the same steepness whatever their size: 1 is a
    /// 45° stair, 3 and over a cliff.
    pub fn slope(&self, x: i32, z: i32) -> i32 {
        let h = self.height(x, z);
        [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().map(|(dx, dz)| (self.height(x + dx, z + dz) - h).abs()).max().unwrap_or(0)
    }

    fn biome_of(&self, x: i32, z: i32) -> Biome {
        let h = self.height(x, z);
        let over_sea = (h - SEA) as f32 * CELL;
        let moist = fbm(x as f32 * CELL / 35.0, z as f32 * CELL / 35.0, self.seed + 21);
        if h < SEA {
            Biome::Water
        } else if over_sea <= 0.5 {
            Biome::Shore
        } else if over_sea > 28.0 {
            Biome::Snow
        } else if over_sea > 17.0 || self.slope(x, z) >= 3 {
            Biome::Rock
        } else if moist > 0.5 {
            Biome::Forest
        } else {
            Biome::Meadow
        }
    }
}

/// The ground's height at a point, both in metres. Lowland rolls a few metres over the sea; a
/// mountain mask raises ridges in part of the island, stepped into ledges; a river winds
/// through where a slow noise crosses its middle; the edge falls away into the sea; the origin
/// is a clearing. Nothing here knows the size of a cell.
fn column(x: f32, z: f32, seed: u32, span: f32) -> f32 {
    // The shapes were tuned in half metres, and are kept so.
    let (x, z, span, sea) = (x * 2.0, z * 2.0, span * 2.0, SEA_LEVEL * 2.0);
    // Bent coordinates, so nothing runs straight.
    let (wx, wz) = (x + (fbm(x / 70.0, z / 70.0, seed + 11) - 0.5) * 44.0, z + (fbm(x / 70.0, z / 70.0, seed + 12) - 0.5) * 44.0);
    let base = fbm(wx / 120.0, wz / 120.0, seed);
    let hills = (fbm(wx / 38.0, wz / 38.0, seed + 3) - 0.5) * 12.0;
    let mut h = sea + 5.0 + (base - 0.45) * 30.0 + hills;

    let from_origin = (x * x + z * z).sqrt();
    // Mountains keep off the start: they are what is seen from it, not where it is.
    let mask = smooth(0.50, 0.66, fbm(wx / 150.0, wz / 150.0, seed + 7)) * smooth(50.0, 120.0, from_origin);
    let ridge = 1.0 - (2.0 * fbm(wx / 64.0, wz / 64.0, seed + 5) - 1.0).abs();
    let high = h + mask * (14.0 + ridge * ridge * 52.0);
    // Ledges: the height rounded to steps of three metres, most strongly where the mountain is.
    let ledged = (high / 6.0).round() * 6.0;
    h = mix(high, ledged, mask * 0.65);

    let wind = (fbm(wx / 130.0, wz / 130.0, seed + 9) - 0.5).abs();
    let valley = 1.0 - smooth(0.0, 0.085, wind);
    let channel = 1.0 - smooth(0.006, 0.03, wind);
    h = mix(h, h.min(sea + 2.0), valley * 0.85);
    // The river is deep in places and a ford in others.
    let bed = mix(1.0, 4.5, smooth(0.42, 0.58, fbm(x / 45.0, z / 45.0, seed + 10)));
    h = mix(h, sea - bed, channel);

    let clearing = 1.0 - smooth(14.0, 40.0, from_origin);
    h = mix(h, sea + 4.0 + hills * 0.15, clearing);

    let out = from_origin / span + (fbm(x / 50.0, z / 50.0, seed + 15) - 0.5) * 0.25;
    h = mix(h, sea - 9.0, smooth(0.68, 0.95, out));
    h * 0.5
}

/// A world `radius` chunks out from the origin in x and z, and the land it was made from.
pub fn generate(seed: u32, radius: i32) -> (World, Land) {
    let mut land = Land::new(seed, radius);
    let mut world = World::default();
    let span = land.span;
    // Snow lies half a metre deep, soil two metres, and the river's bed is gravel where it is
    // over half a metre down.
    let (snow, soil, deep) = (cells(0.5), cells(2.0), cells(1.0));
    for z in -span..span {
        for x in -span..span {
            let (h, biome) = (land.height(x, z), land.biome(x, z));
            world.set_column(x, z, 0, h.max(SEA), |y| {
                let depth = h - 1 - y;
                if y >= h {
                    return Block::Water;
                }
                match biome {
                    Biome::Snow if depth < snow => Block::Snow,
                    Biome::Rock | Biome::Snow => Block::Stone,
                    _ if depth > soil => Block::Stone,
                    Biome::Water if h < SEA - deep => Block::Gravel,
                    Biome::Water | Biome::Shore => Block::Sand,
                    Biome::Forest if depth == 0 => Block::Turf,
                    Biome::Meadow if depth == 0 => Block::Grass,
                    _ => Block::Dirt,
                }
            });
        }
    }
    plant(&mut world, &mut land);
    (world, land)
}

/// How far from the start the first tree may stand, metres.
const CLEARING: f32 = 10.0;

/// What grows and lies on the land. One thing may stand in each square of a coarse grid, at a
/// spot and with a chance that come from the seed, so nothing crowds and nothing lines up:
/// a tree of the kind that suits the place, or under and between the trees a young one, a
/// bush, a fallen log, a stump or a boulder. Then the ground cover, a column at a time.
fn plant(world: &mut World, land: &mut Land) {
    // A grown tree wants four metres to itself.
    let grid = cells(4.0);
    let (span, seed) = (land.span, land.seed);
    for gz in -span / grid..span / grid {
        for gx in -span / grid..span / grid {
            let roll = |n: u32| hash2(gx, gz, seed + 40 + n);
            let (x, z) = (gx * grid + (roll(0) * grid as f32) as i32, gz * grid + (roll(1) * grid as f32) as i32);
            let (h, biome) = (land.height(x, z), land.biome(x, z));
            let foot = IVec3::new(x, h, z);
            let near_start = Vec3::new(x as f32, 0.0, z as f32).length() * CELL < CLEARING;
            if land.slope(x, z) >= 2 || near_start || land.index(x, z).is_none() {
                continue;
            }
            let mut rng = Rng::new((seed as u64) << 40 ^ ((gx as u32 as u64) << 20) ^ gz as u32 as u64);
            let high = h > SEA + cells(10.0);
            // By water: within six metres of it and little above it.
            let wet = h <= SEA + cells(1.5) && (-3..=3).any(|dz| (-3..=3).any(|dx| land.biome(x + dx * cells(2.0), z + dz * cells(2.0)) == Biome::Water));
            let (tree, under, boulder) = match biome {
                Biome::Forest => (0.62, 0.3, 0.02),
                Biome::Meadow => (0.07, 0.1, 0.03),
                Biome::Rock => (0.08, 0.02, 0.16),
                Biome::Snow => (0.03, 0.0, 0.1),
                Biome::Shore => (0.0, 0.0, 0.03),
                Biome::Water => (0.0, 0.0, 0.0),
            };
            let pick = roll(2);
            let conifers = high || matches!(biome, Biome::Rock | Biome::Snow);
            if pick < tree {
                let kind = match roll(3) {
                    _ if wet && !conifers => Tree::Willow,
                    r if r < 0.06 => Tree::Snag,
                    r if conifers => if r < 0.9 { Tree::Pine } else { Tree::Birch },
                    r if r < 0.5 => Tree::Oak,
                    r if r < 0.78 => Tree::Birch,
                    _ => Tree::Pine,
                };
                plants::tree(world, foot, kind, roll(4), &mut rng);
                land.trees.push(foot);
            } else if pick < tree + under {
                match roll(3) {
                    r if r < 0.42 => plants::young(world, foot, conifers || roll(5) < 0.25, &mut rng),
                    r if r < 0.78 => plants::bush(world, foot, &mut rng),
                    r if r < 0.92 => plants::log(world, foot, &mut rng),
                    _ => plants::stump(world, foot, &mut rng),
                }
            } else if pick < tree + under + boulder {
                let on = foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
                let r = 0.6 + roll(4) * 1.1;
                blob(world, on + Vec3::Y * r * 0.25 / CELL, Vec3::new(r, r * (0.6 + roll(5) * 0.4), r * (0.7 + roll(6) * 0.5)) / CELL, Block::Stone, seed);
            }
        }
    }
    // Ground cover: long grass and flowers in the open, ferns under trees, reeds at the
    // water's edge. In patches, not sprinkled: a slow noise says where it is thick.
    for z in -span..span {
        for x in -span..span {
            let (h, biome) = (land.height(x, z), land.biome(x, z));
            let on = IVec3::new(x, h, z);
            if world.get(on) != Block::Air || !world.get(on - IVec3::Y).solid() {
                continue;
            }
            let (thick, lot) = (fbm(x as f32 * CELL / 9.0, z as f32 * CELL / 9.0, seed + 70), hash2(x, z, seed + 71));
            let cover = match biome {
                Biome::Meadow if lot < (thick - 0.35) * 0.9 => Some(if hash2(x, z, seed + 72) < 0.14 { Block::Flower } else { Block::Tuft }),
                Biome::Forest if lot < (thick - 0.42) * 0.5 => Some(Block::Tuft),
                Biome::Shore if lot < 0.22 && land.height(x, z) <= SEA => Some(Block::Reed),
                _ => None,
            };
            if let Some(block) = cover {
                world.set(on, block);
                // Reeds stand two or three cells tall.
                if block == Block::Reed {
                    for up in 1..2 + (lot * 9.0) as i32 % 2 {
                        world.set(on + IVec3::Y * up, Block::Reed);
                    }
                }
            }
        }
    }
}

/// Fills the air inside an ellipsoid, its edge chipped so it is not a ball. In cells.
fn blob(world: &mut World, middle: Vec3, radii: Vec3, block: Block, seed: u32) {
    let (lo, hi) = ((middle - radii).floor().as_ivec3(), (middle + radii).ceil().as_ivec3());
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let cell = IVec3::new(x, y, z);
                let d = ((cell.as_vec3() + 0.5 - middle) / radii).length_squared();
                if d < 1.0 - hash2(x + y * 31, z - y * 17, seed + 60) * 0.3 && world.get(cell) == Block::Air {
                    world.set(cell, block);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_makes_the_same_world() {
        let ((a, la), (b, lb)) = (generate(7, 3), generate(7, 3));
        assert_eq!(la.trees, lb.trees);
        for (at, chunk) in &a.chunks {
            for y in 0..CHUNK {
                let cell = IVec3::new(5, y, 9);
                assert_eq!(chunk.get(cell), b.chunks[at].get(cell));
            }
        }
        let (_, other) = generate(8, 3);
        assert!((-90..90).any(|x| other.height(x, 3) != la.height(x, 3)));
    }

    #[test]
    fn the_start_is_a_dry_clearing_and_the_edge_is_sea() {
        let (world, land) = generate(7, 5);
        assert_eq!(land.biome(0, 0), Biome::Meadow);
        assert!(land.trees.iter().all(|t| Vec3::new(t.x as f32, 0.0, t.z as f32).length() * CELL >= CLEARING));
        assert_eq!(world.get(IVec3::new(0, land.height(0, 0), 0)), Block::Air);
        assert_eq!(land.biome(land.span - 1, 0), Biome::Water);
    }
}
