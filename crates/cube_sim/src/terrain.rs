//! Making a world from a seed: an island of meadow, forest, rock and snow, cut by a river,
//! with trees, undergrowth, boulders and ground cover on it (the shapes are in `plants`). The same seed always makes the same world.
//!
//! Two halves. [`Land::new`] works out the lie of the whole island from layered noise: every
//! column's height and biome, and where every tree and rock stands. [`fill`] makes the cells
//! of any part of it, the same whenever and in whatever order it is asked. `Land` is kept beside the
//! world: it is how creatures are placed and how the world is measured (`survey`).

use crate::plants::{self, Tree};
use crate::rng::Rng;
use crate::scale::{self, Landform};
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
pub(crate) fn fbm(x: f32, z: f32, seed: u32) -> f32 {
    layers(x, z, seed, 4)
}

/// The same with `n` layers: one is a smooth swell, each more adds roughness half the size.
fn layers(x: f32, z: f32, seed: u32, n: u32) -> f32 {
    (0..n).map(|o| value_noise(x * (1 << o) as f32, z * (1 << o) as f32, seed + o) / (1 << o) as f32).sum::<f32>() / (2.0 - 1.0 / (1 << (n - 1)) as f32)
}

/// A landform's noise at a point in metres: 0 to 1, mostly near 0.5.
fn form(f: &Landform, x: f32, z: f32, seed: u32) -> f32 {
    layers(x / f.across, z / f.across, seed, f.detail)
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
/// The world the game starts in: its seed, and how many chunks it runs out from the origin
/// (512 m each way: an island a kilometre across).
pub const SEED: u32 = 6;
pub const RADIUS: i32 = (512.0 / (CELL * CHUNK as f32)) as i32;
/// How much of the island is made into cells at once, chunks from the middle of a part each
/// way: what the game keeps round the player, and what `cargo world` judges round the start
/// (160 m each way). The lie of the land is known for all of it always; its cells are not.
pub const NEAR: i32 = (160.0 / (CELL * CHUNK as f32)) as i32;
/// A region: the piece of the world made and dropped as one, cells a side (32 m).
pub const REGION: i32 = CHUNK * 4;
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

/// Something that stands on the land: where its foot is, what it is, and the seed it is built
/// from, so it is the same thing whoever builds it and however often.
struct Thing {
    foot: IVec3,
    what: What,
    seed: u64,
}

#[derive(Clone, Copy)]
enum What {
    /// A grown tree: its kind, and its age from 0 to 1.
    Tree(Tree, f32),
    /// A young tree; whether it is a conifer.
    Young(bool),
    Bush,
    Log,
    Stump,
    /// A rock, so many metres across.
    Rock(f32),
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
    /// How tall each of those trees stands, metres, and what kind it is.
    pub grown: Vec<f32>,
    pub sorts: Vec<Tree>,
    /// How far across each rock is, metres.
    pub rocks: Vec<f32>,
}

impl Land {
    pub fn new(seed: u32, radius: i32) -> Self {
        let span = radius * CHUNK;
        let side = (span * 2) as usize;
        // A row at a time, on every core: a kilometre of island is seventeen million columns.
        let rows = |f: &(dyn Fn(i32, i32) -> i32 + Sync)| -> Vec<i32> {
            let mut out = vec![0; side * side];
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
            let per = side.div_ceil(threads).max(1);
            std::thread::scope(|scope| {
                for (band, part) in out.chunks_mut(per * side).enumerate() {
                    scope.spawn(move || {
                        for (i, v) in part.iter_mut().enumerate() {
                            *v = f((i % side) as i32 - span, (band * per + i / side) as i32 - span);
                        }
                    });
                }
            });
            out
        };
        let heights = rows(&|x, z| cells(column((x as f32 + 0.5) * CELL, (z as f32 + 0.5) * CELL, seed, span as f32 * CELL)).max(cells(1.0)));
        let mut land = Land { seed, span, heights, biomes: Vec::new(), trees: Vec::new(), grown: Vec::new(), sorts: Vec::new(), rocks: Vec::new() };
        let biomes = rows(&|x, z| land.biome_of(x, z) as i32);
        land.biomes = biomes.into_iter().map(|b| Biome::ALL[b as usize]).collect();
        // What stands on it is decided here, from the land alone, so it is known for all the
        // island whether or not its cells have been made: where the trees are and how tall,
        // how big the rocks.
        for (pass, room) in [scale::TREE_ROOM, scale::SMALL_ROOM].into_iter().enumerate() {
            let grid = cells(room);
            for gz in -span / grid..span / grid {
                for gx in -span / grid..span / grid {
                    match land.thing(pass, gx, gz).map(|t| (t.foot, t.what)) {
                        Some((foot, What::Tree(kind, age))) => {
                            land.trees.push(foot);
                            land.grown.push(plants::height(kind, age));
                            land.sorts.push(kind);
                        }
                        Some((_, What::Rock(across))) => land.rocks.push(across),
                        _ => {}
                    }
                }
            }
        }
        land
    }

    /// What stands in a square of the planting grid, if anything: trees on the coarse grid
    /// (`pass` 0), one to a square at a spot and with a chance that come from the seed, so
    /// nothing crowds and nothing lines up; on the finer grid (`pass` 1) the small things under
    /// and between them. How much room each has, and how big it grows, is in `scale`.
    fn thing(&self, pass: usize, gx: i32, gz: i32) -> Option<Thing> {
        let (land, seed) = (self, self.seed);
        let grid = cells([scale::TREE_ROOM, scale::SMALL_ROOM][pass]);
        let roll = |n: u32| hash2(gx, gz, seed + 40 + pass as u32 * 10 + n);
        let (x, z) = (gx * grid + (roll(0) * grid as f32) as i32, gz * grid + (roll(1) * grid as f32) as i32);
        let (h, biome) = (land.height(x, z), land.biome(x, z));
        let foot = IVec3::new(x, h, z);
        let near_start = Vec3::new(x as f32, 0.0, z as f32).length() * CELL < CLEARING;
        if land.slope(x, z) >= 2 || near_start || land.index(x, z).is_none() {
            return None;
        }
        // Nothing stands in a trail: that is what keeps it one.
        if matches!(biome, Biome::Meadow | Biome::Forest) && worn(x as f32 * CELL, z as f32 * CELL, seed) > 0.0 {
            return None;
        }
        let high = h > SEA + cells(scale::CONIFER_LINE);
        let conifers = high || matches!(biome, Biome::Rock | Biome::Snow);
        // How thick the trees, the undergrowth and the rocks stand here (`land.toml`).
        let crate::cover::Density { trees: tree, under, rocks: rock } = crate::cover::density(biome);
        let pick = roll(2);
        // Up a mountain the trees thin and stop: nothing stands on a peak.
        let over = (h - SEA) as f32 * CELL;
        let tree = tree * ((scale::TREE_LINE - over) / 20.0).clamp(0.0, 1.0);
        let what = if pass == 0 {
            if pick >= tree {
                return None;
            }
            // By water: within six metres of it and little above it.
            let wet = h <= SEA + cells(1.5) && (-3..=3).any(|dz| (-3..=3).any(|dx| land.biome(x + dx * cells(2.0), z + dz * cells(2.0)) == Biome::Water));
            // Which kind: as the table of trees says is found at such a place.
            let site = if wet && !conifers { crate::flora::Site::Waterside } else if conifers { crate::flora::Site::High } else { crate::flora::Site::Low };
            let kind = crate::flora::pick(site, roll(3));
            let giant = kind.species().giant && roll(6) < scale::GIANTS;
            What::Tree(kind, if giant { scale::GIANT } else { roll(4).sqrt() })
        } else if pick < under {
            // What the undergrowth is, by its shares (`land.toml`).
            let of = crate::cover::undergrowth();
            match roll(3) * (of.young + of.bush + of.log + of.stump) {
                r if r < of.young => What::Young(conifers || roll(5) < of.young_conifers),
                r if r < of.young + of.bush => What::Bush,
                r if r < of.young + of.bush + of.log => What::Log,
                _ => What::Stump,
            }
        } else if pick < under + rock {
            // Which class of rock: the shares are in `scale`, and bare rock and snow lean
            // toward the larger.
            let mut class = if matches!(biome, Biome::Rock | Biome::Snow) { 0.3 + roll(4) * 0.7 } else { roll(4) };
            let rocks = scale::ROCKS.iter().find(|r| {
                class -= r.share;
                class < 0.0
            });
            What::Rock(rocks.unwrap_or(&scale::ROCKS[2]).across.at(roll(5)))
        } else {
            return None;
        };
        Some(Thing { foot, what, seed: (seed as u64) << 40 ^ ((gx as u32 as u64) << 20) ^ gz as u32 as u64 ^ (pass as u64) << 60 })
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

    /// How worn the ground at a column is by a trail ([`worn`]): only where a trail can
    /// run, on meadow and forest floor that is not steep.
    pub fn trail(&self, x: i32, z: i32) -> f32 {
        if !matches!(self.biome(x, z), Biome::Meadow | Biome::Forest) || self.slope(x, z) > 1 {
            return 0.0;
        }
        worn(x as f32 * CELL, z as f32 * CELL, self.seed)
    }

    fn biome_of(&self, x: i32, z: i32) -> Biome {
        let h = self.height(x, z);
        let over_sea = (h - SEA) as f32 * CELL;
        let moist = fbm(x as f32 * CELL / DAMP, z as f32 * CELL / DAMP, self.seed + 21);
        if h < SEA {
            Biome::Water
        } else if over_sea <= 0.6 {
            Biome::Shore
        } else if over_sea > scale::SNOW_LINE {
            Biome::Snow
        } else if over_sea > scale::ROCK_LINE || self.slope(x, z) >= 3 {
            Biome::Rock
        } else if moist > 0.5 {
            Biome::Forest
        } else {
            Biome::Meadow
        }
    }
}

/// How worn by feet the ground is at a point (metres): 0 not at all, 1 the middle of a trail.
///
/// Animals walk the same lines until they are paths. The lines here are the level lines of
/// a slow, bent noise, so they wander, fork where two come near and never run straight;
/// one passes within a hundred metres or so of anywhere. A trail is about a metre of bare
/// earth with as much again of trodden grass each side. Where the land will not carry one
/// (water, rock, a steep bank) is for whoever asks to say: [`Land::trail`].
pub fn worn(x: f32, z: f32, seed: u32) -> f32 {
    let field = |x: f32, z: f32| {
        let (bx, bz) = (fbm(x / 31.0, z / 31.0, seed + 90) - 0.5, fbm(x / 31.0, z / 31.0, seed + 91) - 0.5);
        layers((x + bx * 22.0) / 110.0, (z + bz * 22.0) / 110.0, seed + 92, 2) - 0.5
    };
    let here = field(x, z);
    if here.abs() > 0.04 {
        return 0.0;
    }
    // How fast the noise changes here says how far off its level line is.
    let steep = (field(x + 1.0, z) - here).hypot(field(x, z + 1.0) - here);
    if steep < 0.004 {
        return 0.0;
    }
    (1.0 - here.abs() / steep / TRAIL).clamp(0.0, 1.0)
}

/// Half the width of a trail and its trodden edges, metres.
const TRAIL: f32 = 1.5;

/// How far apart damp ground and dry are, metres: forest grows on the damp.
const DAMP: f32 = 70.0;

/// The ground's height at a point, both in metres. Every size in it is a [`Landform`] from
/// `scale`: the lowland rolls a few metres over the sea in smooth hills; a mountain mask
/// raises rough ridges in part of the island, stepped into ledges; a river winds through
/// where a slow noise crosses its middle; the edge falls away into the sea; the origin is a
/// clearing. Nothing here knows the size of a cell.
fn column(x: f32, z: f32, seed: u32, span: f32) -> f32 {
    let sea = SEA_LEVEL;
    // Bent coordinates, so nothing runs straight.
    let (wx, wz) = (x + (layers(x / 90.0, z / 90.0, seed + 11, 2) - 0.5) * 44.0, z + (layers(x / 90.0, z / 90.0, seed + 12, 2) - 0.5) * 44.0);
    let hills = (form(&scale::HILL, wx, wz, seed + 3) - 0.5) * scale::HILL.tall;
    let mut h = sea + 4.0 + (form(&scale::LOWLAND, wx, wz, seed) - 0.45) * scale::LOWLAND.tall + hills;

    let from_origin = (x * x + z * z).sqrt();
    // Mountains are placed, not found: each massif stands at a spot the seed picks, well out
    // from the start, and rises as a bell from broad foothills to its top. So there is always
    // open lowland, always a mountain to see from it, and the ground steepens toward the
    // mountain instead of standing up from the plain like a wall.
    let ridge = 1.0 - (2.0 * form(&scale::RIDGE, wx, wz, seed + 5) - 1.0).abs();
    // To the north (-z), within forty degrees: the sun goes round by the south (`sky`), so a
    // mountain there is lit all day and never lies between the sun and the lowland. Placed
    // anywhere, seed 6 had it due west, and the start was in its shadow from mid afternoon.
    let turn = -std::f32::consts::FRAC_PI_2 + (hash2(1, 2, seed + 7) - 0.5) * 1.4;
    let mut bell = 0.0f32;
    // The first two stand near the start, the mountain to its north; a bigger island has
    // more of them, about one for every 384 m square of it, wherever the seed puts them and of
    // their own sizes.
    let count = 2 + (0.55 * ((span / 192.0).powi(2) - 1.0)).round().max(0.0) as i32;
    // Where two stand together the land is the higher of them, not one piled on the other.
    let mut lifted = 0.0f32;
    for i in 0..count {
        let massif = [&scale::MASSIF, &scale::HEIGHTS][(i % 2) as usize];
        let (way, out, size) = if i < 2 {
            (turn + i as f32 * (2.2 + hash2(3, 4, seed + 7) * 1.4), massif.across * 0.6, 1.0)
        } else {
            (hash2(i, 5, seed + 7) * std::f32::consts::TAU, span * (0.35 + 0.45 * hash2(i, 6, seed + 7)), 0.45 + 0.6 * hash2(i, 7, seed + 7))
        };
        let (cx, cz) = (way.cos() * out, way.sin() * out);
        let d = ((wx - cx).powi(2) + (wz - cz).powi(2)).sqrt() / (massif.across * size * 0.5);
        let rise = (1.0 - d * d).max(0.0).powi(2) * smooth(40.0, 110.0, from_origin);
        // Ridges and the creases between them, deepest where the mountain is highest.
        let tall = massif.tall * size.min(1.15);
        let crease = scale::RIDGE.tall.min(tall * 0.4);
        lifted = lifted.max(rise * (tall - crease * (1.0 - ridge)) * (1.0 + 0.25 * (1.0 - d).max(0.0)));
        bell = bell.max(rise * tall / scale::MASSIF.tall);
    }
    h += lifted;
    // Ledges: the height rounded to steps, only well up the mountain.
    // Not in rings round it: the shelves rise and dip along their length, and whole faces of
    // the mountain have none.
    let wobble = (fbm(x / 40.0, z / 40.0, seed + 17) - 0.5) * scale::LEDGE * 2.5;
    let ledged = ((h - sea + wobble) / scale::LEDGE).round() * scale::LEDGE + sea - wobble;
    let faces = smooth(0.38, 0.6, fbm(x / 80.0, z / 80.0, seed + 18));
    h = mix(h, ledged, smooth(0.3, 0.7, bell) * faces * 0.6);

    let wind = (fbm(wx / scale::RIVER, wz / scale::RIVER, seed + 9) - 0.5).abs();
    let valley = 1.0 - smooth(0.012, 0.13, wind);
    let channel = 1.0 - smooth(0.005, 0.04, wind);
    h = mix(h, h.min(sea + 1.2), valley);
    // The river is deep in places and a ford in others.
    let bed = mix(0.5, 2.25, smooth(0.42, 0.58, fbm(x / 45.0, z / 45.0, seed + 10)));
    h = mix(h, sea - bed, channel);

    let clearing = 1.0 - smooth(14.0, 40.0, from_origin);
    h = mix(h, sea + 3.4 + hills * 0.15, clearing);

    let out = from_origin / span + (fbm(x / 50.0, z / 50.0, seed + 15) - 0.5) * 0.25;
    mix(h, sea - 7.6, smooth(0.78, 0.99, out))
}

/// A whole world `radius` chunks out from the origin in x and z, and the land it was made
/// from: for tests and for small worlds. The game's island is too big to hold whole; it and
/// `cargo world` make the land, and then cells for the part they need ([`fill`]).
pub fn generate(seed: u32, radius: i32) -> (World, Land) {
    let land = Land::new(seed, radius);
    let mut world = World::default();
    fill(&mut world, &land, (-land.span, -land.span), (land.span, land.span));
    (world, land)
}

/// The cells of the part of the land round a spot (cells), `radius` chunks each way, kept
/// inside the island: what `cargo world` judges.
pub fn part(land: &Land, middle: (i32, i32), radius: i32) -> World {
    let mut world = World::default();
    let reach = radius * CHUNK;
    let clamp = |v: i32| v.clamp(-land.span, land.span).div_euclid(REGION) * REGION;
    fill(&mut world, land, (clamp(middle.0 - reach), clamp(middle.1 - reach)), (clamp(middle.0 + reach), clamp(middle.1 + reach)));
    world
}

/// How far from the start the first tree may stand, metres: past the reach of a great oak, whose limbs and leaf would else be where a person is put down.
const CLEARING: f32 = 22.0;

/// How far a tree reaches from its foot, cells: one whose foot is this near a part being
/// made is built with it, so no crown is cut off at the part's edge.
const CROWN: i32 = (19.0 / CELL) as i32;

thread_local! {
    /// Seconds `fill` has spent on this thread in each of its three parts: the ground, what
    /// stands on it, the ground cover (`fill_cost`).
    static SPENT: std::cell::Cell<[f64; 5]> = const { std::cell::Cell::new([0.0; 5]) };
}

/// What `fill` has cost on this thread since this was last asked, seconds: the ground, what
/// stands on it, the ground cover; and of what stands, its wood and its leaf. For `cargo
/// world cost`.
pub fn fill_cost() -> [f64; 5] {
    SPENT.replace([0.0; 5])
}

pub(crate) fn spent(part: usize, since: std::time::Instant) -> std::time::Instant {
    SPENT.with(|s| {
        let mut all = s.get();
        all[part] += since.elapsed().as_secs_f64();
        s.set(all);
    });
    std::time::Instant::now()
}

/// Makes the cells of the columns from `lo` to `hi` (cells, x and z; `hi` not included):
/// the ground, what stands on it, and the ground cover. The same call makes the same cells
/// whatever has been made round it and in whatever order, so the world can be made and
/// dropped a region at a time.
pub fn fill(world: &mut World, land: &Land, lo: (i32, i32), hi: (i32, i32)) {
    // Snow lies half a metre deep, soil two metres, and the river's bed is gravel where it is
    // over half a metre down.
    let (snow, soil, deep) = (cells(0.5), cells(2.0), cells(1.0));
    let seed = land.seed;
    let clock = std::time::Instant::now();
    // The trees whose crowns are over this ground: where each stands, how far its crown
    // reaches, and whether it drops needles. The ground under a tree is that tree's.
    let mut crowns: Vec<(f32, f32, f32, bool)> = Vec::new();
    let grid = cells(scale::TREE_ROOM);
    for gz in (lo.1 - CROWN).div_euclid(grid)..=(hi.1 + CROWN).div_euclid(grid) {
        for gx in (lo.0 - CROWN).div_euclid(grid)..=(hi.0 + CROWN).div_euclid(grid) {
            if let Some(Thing { foot, what: What::Tree(kind, age), .. }) = land.thing(0, gx, gz) {
                let reach = plants::height(kind, age) * kind.species().reach;
                crowns.push(((foot.x as f32 + 0.5) * CELL, (foot.z as f32 + 0.5) * CELL, reach, kind.species().needles()));
            }
        }
    }
    // For a column: how far under a crown it is (0 outside, 1 at the trunk), whether that
    // crown drops needles, and whether it is at the very foot of a trunk.
    let under = |x: i32, z: i32| {
        let (xm, zm) = ((x as f32 + 0.5) * CELL, (z as f32 + 0.5) * CELL);
        let (mut most, mut needles, mut foot) = (0.0f32, false, false);
        for (tx, tz, reach, pine) in &crowns {
            let away = (xm - tx).hypot(zm - tz);
            if away < *reach && 1.0 - away / reach > most {
                (most, needles) = (1.0 - away / reach, *pine);
            }
            foot |= away < 0.9;
        }
        (most, needles, foot)
    };
    for z in lo.1..hi.1 {
        for x in lo.0..hi.0 {
            let (h, biome) = (land.height(x, z), land.biome(x, z));
            let (xm, zm) = (x as f32 * CELL, z as f32 * CELL);
            // Stone lies in beds a metre or two thick that dip and roll: a dark band, a pale
            // one, the grey between. A cliff then shows how it was laid down.
            let dip = fbm(xm / 40.0, zm / 40.0, seed + 80) * 7.0 + xm * 0.04;
            let bed = |y: i32| match (((y as f32 * CELL + dip) / 1.4).floor() as i32).rem_euclid(7) {
                0 => Block::Slate,
                3 | 4 => Block::Chalk,
                _ => Block::Stone,
            };
            // On rock that is not steep: moss where it is low and damp, loose stone under a
            // cliff, bare stone elsewhere.
            let damp = fbm(xm / DAMP, zm / DAMP, seed + 21);
            let under_cliff = (-2..=2).any(|dz| (-2..=2).any(|dx| land.height(x + dx, z + dz) >= h + cells(1.5)));
            let cap = if biome != Biome::Rock || land.slope(x, z) > 1 {
                None
            } else if under_cliff {
                Some(Block::Gravel)
            } else if damp > 0.47 && h < SEA + cells(scale::MOSS_LINE) && fbm(xm / 5.0, zm / 5.0, seed + 81) < 0.6 {
                // (In patches a few metres over, bare stone between: cell by cell at random it
                // was a floor of grey tiles.)
                Some(Block::Moss)
            } else {
                None
            };
            // What the top of the soil is, from what is over it and what crosses it: the
            // rules of `land.toml` (`cover::ground`). A trail is bare earth; at the foot of a
            // trunk nothing grows; under a crown lie its leaves; and so on, each a rule there.
            // One rule a biome made every wood the same floor from edge to edge.
            let (shade, needles, foot) = under(x, z);
            let site = crate::cover::Site { biome, x, z, seed, shade, needles, foot, trail: land.trail(x, z), damp, wet: h <= SEA, slope: land.slope(x, z), over: (h - SEA) as f32 * CELL, on: Block::Air };
            let top = crate::cover::ground(&site).unwrap_or(Block::Sand);
            world.set_column(x, z, 0, h.max(SEA), |y| {
                let depth = h - 1 - y;
                if y >= h {
                    return Block::Water;
                }
                match biome {
                    Biome::Snow if depth < snow => Block::Snow,
                    Biome::Rock if depth == 0 && cap.is_some() => cap.unwrap(),
                    Biome::Rock | Biome::Snow => bed(y),
                    _ if depth > soil => bed(y),
                    Biome::Water if h < SEA - deep => Block::Gravel,
                    Biome::Water => Block::Sand,
                    Biome::Shore if depth > 0 => Block::Sand,
                    _ if depth == 0 => top,
                    _ => Block::Dirt,
                }
            });
        }
    }
    let clock = spent(0, clock);
    // What stands on it. Trees whose crowns reach in from outside are built too; the small
    // things only where their own ground has been made.
    for (pass, room) in [scale::TREE_ROOM, scale::SMALL_ROOM].into_iter().enumerate() {
        let (grid, reach) = (cells(room), if pass == 0 { CROWN } else { 0 });
        for gz in (lo.1 - reach).div_euclid(grid)..=(hi.1 + reach).div_euclid(grid) {
            for gx in (lo.0 - reach).div_euclid(grid)..=(hi.0 + reach).div_euclid(grid) {
                let Some(thing) = land.thing(pass, gx, gz) else { continue };
                let (foot, inside) = (thing.foot, |m: i32| thing.foot.x >= lo.0 - m && thing.foot.x < hi.0 + m && thing.foot.z >= lo.1 - m && thing.foot.z < hi.1 + m);
                if !inside(reach) {
                    continue;
                }
                let mut rng = Rng::new(thing.seed);
                match thing.what {
                    What::Tree(kind, age) => {
                        plants::only(lo, hi, || plants::tree(world, foot, kind, age, &mut rng));
                    }
                    // A trunk or a root is already here.
                    _ if world.get(foot) != Block::Air => {}
                    What::Young(needles) => plants::young(world, foot, needles, &mut rng),
                    What::Bush => plants::bush(world, foot, &mut rng),
                    What::Log => plants::log(world, foot, &mut rng),
                    What::Stump => plants::stump(world, foot, &mut rng),
                    What::Rock(across) => plants::rock(world, foot, across, &mut rng),
                }
            }
        }
    }
    let clock = spent(1, clock);
    // Ground cover: long grass and flowers in the open, ferns under trees, reeds at the
    // water's edge. In patches, not sprinkled: a slow noise says where it is thick.
    for z in lo.1..hi.1 {
        for x in lo.0..hi.0 {
            let (h, biome) = (land.height(x, z), land.biome(x, z));
            let on = IVec3::new(x, h, z);
            if world.get(on) != Block::Air || !world.get(on - IVec3::Y).solid() {
                continue;
            }
            // What grows here: the rules of `land.toml` (`cover::cover`).
            let (shade, needles, foot) = under(x, z);
            let damp = fbm(x as f32 * CELL / DAMP, z as f32 * CELL / DAMP, seed + 21);
            let site = crate::cover::Site { biome, x, z, seed, shade, needles, foot, trail: land.trail(x, z), damp, wet: h <= SEA, slope: land.slope(x, z), over: (h - SEA) as f32 * CELL, on: world.get(on - IVec3::Y) };
            if let Some((block, tall)) = crate::cover::cover(&site) {
                for up in 0..tall {
                    world.set(on + IVec3::Y * up, block);
                }
            }
        }
    }
    spent(2, clock);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_made_apart_in_any_order_are_the_land_made_at_once() {
        // Two regions side by side in a wood, where trees stand astride the line between.
        let land = Land::new(6, 48);
        let r = REGION;
        let edge = land.span / r - 1;
        let wooded = |i: i32, j: i32| (0..16).filter(|k| land.biome(i * r - r + k * 16, j * r + 64) == Biome::Forest).count() >= 12;
        let (i, j) = (-edge + 1..edge).flat_map(|j| (-edge + 1..edge).map(move |i| (i, j))).find(|&(i, j)| wooded(i, j)).expect("no wood two regions wide");
        let (at, row) = (i * r, j * r);
        let whole = {
            let mut w = World::default();
            fill(&mut w, &land, (at - r, row), (at + r, row + r));
            w
        };
        for order in [[0, 1], [1, 0]] {
            let mut apart = World::default();
            for i in order {
                fill(&mut apart, &land, (at - r + i * r, row), (at + i * r, row + r));
            }
            let mut wood = 0;
            for (place, chunk) in &whole.chunks {
                for y in 0..CHUNK {
                    for z in 0..CHUNK {
                        for x in 0..CHUNK {
                            let c = IVec3::new(x, y, z);
                            wood += (chunk.get(c) == Block::Wood) as u32;
                            assert_eq!(chunk.get(c), apart.chunks.get(place).map_or(Block::Air, |ch| ch.get(c)), "cell {c} of chunk {place}, order {order:?}");
                        }
                    }
                }
            }
            assert!(wood > 500 && apart.chunks.len() <= whole.chunks.len(), "no trees to speak of ({wood} cells of wood)");
        }
    }

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
        let (world, land) = generate(7, 6);
        assert_eq!(land.biome(0, 0), Biome::Meadow);
        assert!(land.trees.iter().all(|t| Vec3::new(t.x as f32, 0.0, t.z as f32).length() * CELL >= CLEARING));
        assert_eq!(world.get(IVec3::new(0, land.height(0, 0), 0)), Block::Air);
        assert_eq!(land.biome(land.span - 1, 0), Biome::Water);
    }
}
