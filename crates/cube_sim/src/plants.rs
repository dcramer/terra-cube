//! What grows: trees by kind, young trees, bushes, logs and stumps, each built into the world
//! out of cells. Sizes are in metres and real: an oak is an oak's size. How tall each kind
//! stands is in `scale`; every other measure of a tree here is a part of its height, so a
//! change there keeps each tree in proportion.
//!
//! A tree is not a pole with a ball on it. It has a trunk that flares into roots and tapers
//! upward, leaning a little; limbs that leave it and fork; and foliage in separate clumps at
//! the ends of the limbs, so sky shows through and no two are alike. Every choice comes from a
//! seeded [`Rng`], so the same seed grows the same forest.

use crate::rng::Rng;
use crate::scale;
use cube_voxel::{Block, CELL, World};
use glam::{IVec3, Vec3};
use std::f32::consts::TAU;

pub use crate::flora::Tree;
use crate::flora::Habit;

/// The shape of what was grown, for whoever draws it as a shape and not as cells: every limb
/// (from, to, how thick at each end, of what bark) and every mass of leaf (its middle, how
/// far it reaches each way, of what leaf).
#[derive(Clone, Default, Debug)]
pub struct Form {
    pub limbs: Vec<Limb>,
    pub clumps: Vec<Clump>,
    pub stones: Vec<Stone>,
}

/// A piece of rock: an ellipsoid (its middle, its reach along its own axes, turned so far
/// about the upright) with flat faces struck off it, each a plane in the ellipsoid's own
/// unit space (a direction, and how far out it lies), of one of the kinds of stone.
#[derive(Clone, Debug)]
pub struct Stone {
    pub middle: Vec3,
    pub radii: Vec3,
    pub turn: f32,
    pub faces: Vec<(Vec3, f32)>,
    pub kind: Block,
}

#[derive(Clone, Copy, Debug)]
pub struct Limb {
    pub a: Vec3,
    pub b: Vec3,
    pub ra: f32,
    pub rb: f32,
    pub bark: Block,
    /// A trunk standing in the ground: how many roots it spreads into at its foot (none,
    /// for any other limb). Whoever draws it draws them as one piece with the trunk.
    pub roots: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct Clump {
    pub middle: Vec3,
    pub radii: Vec3,
    pub leaf: Block,
}

thread_local! {
    /// While something is being grown inside [`recorded`], its shapes are noted here.
    static NOTED: std::cell::RefCell<Option<Form>> = const { std::cell::RefCell::new(None) };
}

thread_local! {
    /// The columns a tree being grown may write to (x and z, from and up to), where there is
    /// such a bound ([`only`]).
    static ONLY: std::cell::Cell<Option<[i32; 4]>> = const { std::cell::Cell::new(None) };
}

/// Runs `grow` with every tree it grows writing only the cells of the columns from `lo` up to
/// `hi`. A region being made grows the trees round it too, for the limbs and leaf that reach
/// in, and used to write each of those trees whole, into ground that was not its own: a tree
/// near an edge was written by every region near it, and four fifths of the time a region
/// took went on trees that were another's. Each cell is now written once, by its own region,
/// and a limb or a mass of leaf that does not reach the bound is passed over.
pub fn only<R>(lo: (i32, i32), hi: (i32, i32), grow: impl FnOnce() -> R) -> R {
    let before = ONLY.replace(Some([lo.0, lo.1, hi.0, hi.1]));
    let out = grow();
    ONLY.set(before);
    out
}

fn open(c: IVec3) -> bool {
    ONLY.get().is_none_or(|b| c.x >= b[0] && c.x < b[2] && c.z >= b[1] && c.z < b[3])
}

/// Runs `grow` and says, with what it gives back, the shape of everything it grew.
pub fn recorded<R>(grow: impl FnOnce() -> R) -> (R, Form) {
    NOTED.with(|n| *n.borrow_mut() = Some(Form::default()));
    let out = grow();
    (out, NOTED.with(|n| n.borrow_mut().take()).unwrap_or_default())
}

fn cell(p: Vec3) -> IVec3 {
    (p / CELL).floor().as_ivec3()
}

/// Wood from `a` to `b` (metres), `ra` thick at one end and `rb` at the other. It grows through
/// air and foliage, not through the ground.
fn limb(world: &mut World, a: Vec3, b: Vec3, ra: f32, rb: f32, bark: Block) {
    grow(world, Limb { a, b, ra, rb, bark, roots: 0 });
}

/// Notes a limb for whoever draws shapes, and fills its cells.
fn grow(world: &mut World, limb: Limb) {
    wood(world, &limb);
    NOTED.with(|n| {
        if let Some(form) = n.borrow_mut().as_mut() {
            form.limbs.push(limb);
        }
    });
}

/// The cells of a limb: the same shape whoever draws it draws ([`Limb::fill`]), at the size
/// of a cell. Wood grows through air, leaf and ground cover, not through the ground.
fn wood(world: &mut World, limb: &Limb) {
    let clock = std::time::Instant::now();
    let bound = ONLY.get();
    // A tree's wood is filled to the bit for as high as anything walks into it: a body is stopped by the wood that is drawn there, and not by a cell that
    // has it or has it not (`cargo world trunks`: by cells a body went 20 cm into wood it
    // could see and was stopped 16 cm short of it).
    // (Every limb of the tree that comes so low, not its foot alone: a trunk is several
    // limbs end to end, and a bough can hang at the height of a head. Not a twig.)
    let fine = FOOT.get().filter(|foot| limb.ra.max(limb.rb) >= 0.1 && limb.a.y.min(limb.b.y) < foot + FINE).map_or(f32::MIN, |foot| ((foot + FINE) / CELL).ceil() * CELL);
    if fine > f32::MIN {
        // In the very cubes it is drawn in near the eye, each written as the bits it covers:
        // so what stops a body is the wood that is seen, to the cube.
        let g = limb.near();
        let n = (g / cube_voxel::BIT).round() as i32;
        let cubes = bound.map(|b| [(b[0] as f32 * CELL / g).floor() as i32, (b[1] as f32 * CELL / g).floor() as i32, (b[2] as f32 * CELL / g).ceil() as i32, (b[3] as f32 * CELL / g).ceil() as i32]);
        limb.fill_span(g, cubes, (f32::MIN, fine), |cube| {
            for dy in 0..n {
                for dz in 0..n {
                    for dx in 0..n {
                        let bit = cube * n + IVec3::new(dx, dy, dz);
                        let cell = bit.div_euclid(IVec3::splat(cube_voxel::SPLIT));
                        if !open(cell) {
                            continue;
                        }
                        // (Grass in the cell goes, as it does where a whole cell turns to
                        // wood: left as the rest of a broken cell it was drawn as bits.)
                        if world.get(cell).sprig() {
                            world.set(cell, Block::Air);
                        }
                        let was = world.bit(bit);
                        if was == Block::Air || was.foliage() {
                            world.set_bit(bit, limb.bark);
                        }
                    }
                }
            }
        });
    }
    limb.fill_span(CELL, bound, (fine, f32::MAX), |c| {
        if !open(c) {
            return;
        }
        let was = world.get(c);
        if was == Block::Air || was.foliage() || was.sprig() {
            world.set(c, limb.bark);
        }
    });
    crate::terrain::spent(3, clock);
}

/// How far above its foot a tree is filled to the bit, metres: over a tall creature's head.
const FINE: f32 = 2.5;

thread_local! {
    /// The height of the foot of the tree being grown, while one is.
    static FOOT: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

fn mix(seed: u32, k: u32) -> f32 {
    let mut h = seed.wrapping_mul(0x9E3779B1) ^ k.wrapping_mul(0x85EBCA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

impl Limb {
    /// How far under the ground a rooted trunk begins (`bole`).
    pub const UNDER: f32 = 0.5;

    /// The shape of the limb, in cubes `g` metres a side on the world's own grid: `put` is
    /// given each cube it fills. It is round, tapers, wanders a little along its length and
    /// is ridged along its grain; a rooted trunk swells toward the ground and runs out in
    /// buttresses, each to its own length. Never thinner than one cube: a twig is still there.
    ///
    /// This is the one place a limb's shape is written. The simulation fills the world's
    /// cells from it and the game draws finer cubes from it, so what stops a body is what is
    /// seen, to the size of a cell. Before, cells were a plain cone and a few root sticks,
    /// the picture had the swell, the buttresses and the bends, and a person walked into
    /// wood he could see and was stopped by wood he could not.
    pub fn fill(&self, g: f32, put: impl FnMut(IVec3)) {
        self.fill_in(g, None, put)
    }

    /// [`Limb::fill`], passing over the lengths of the limb that reach no column from
    /// `only[0], only[1]` up to `only[2], only[3]` (cubes of `g`, x and z).
    pub fn fill_in(&self, g: f32, only: Option<[i32; 4]>, put: impl FnMut(IVec3)) {
        self.fill_span(g, only, (f32::MIN, f32::MAX), put)
    }

    /// The size of the cubes a limb is drawn in near the eye, metres: a trunk or a heavy limb
    /// in cubes of 10 cm, round to the cube (a great trunk, a metre through, in 15: at 10 its
    /// foot alone was twenty thousand faces); a lesser limb one line of cubes as thick as it
    /// is. Always a whole number of bits, since what is drawn near is also what a body meets.
    pub fn near(&self) -> f32 {
        let thick = self.ra.max(self.rb);
        if thick < 0.16 {
            ((thick * 2.0 / 0.05).round() * 0.05).clamp(0.05, 0.3)
        } else if thick > 0.5 {
            0.15
        } else {
            0.1
        }
    }

    /// [`Limb::fill_in`], of the cubes whose middles are from `span.0` up to `span.1` high
    /// (metres): a trunk is filled finely near the ground and coarsely above.
    pub fn fill_span(&self, g: f32, only: Option<[i32; 4]>, span: (f32, f32), mut put: impl FnMut(IVec3)) {
        let mut put = |c: IVec3| {
            let y = (c.y as f32 + 0.5) * g;
            if y >= span.0 && y < span.1 {
                put(c)
            }
        };
        let (a, b, ra, rb, roots) = (self.a, self.b, self.ra, self.rb, self.roots);
        let seed = a.x.to_bits() ^ a.z.to_bits().rotate_left(13) ^ b.y.to_bits().rotate_left(7) ^ b.x.to_bits().rotate_left(19);
        let thick = ra.max(rb);
        let along = (b - a).normalize_or(Vec3::Y);
        let side = along.any_orthonormal_vector();
        let other = along.cross(side);
        let length = a.distance(b);
        let (lobes, phase) = (if roots > 0 { roots as u32 } else { 3 + (mix(seed, 2) * 3.0) as u32 }, mix(seed, 3) * TAU);
        let slight = (ra / 0.35).clamp(0.3, 1.0);
        let high = |t: f32| ((t * length - Self::UNDER + 0.12) / ra.max(0.05)).max(0.0);
        let radius = |t: f32, turn: f32| {
            let r = (ra + (rb - ra) * t).max(g * 0.56);
            let mut ridge = 1.0 + 0.07 * (thick / 0.2).min(1.0) * (turn * lobes as f32 + phase).cos();
            if roots > 0 {
                let wave = (turn * lobes as f32 + phase + 0.5 * (turn * 2.0 + phase).sin()).cos();
                let back = wave.max(0.0).powf(2.2);
                let which = ((turn * lobes as f32 + phase) / TAU).round() as i32;
                let reach = (1.5 + 1.6 * mix(seed, 300 + which.rem_euclid(lobes as i32) as u32)) * slight;
                ridge = 1.0 + 0.45 * (-high(t) / 1.6).exp() + reach * back * (-high(t) / 0.75).exp() - 0.1 * (1.0 - back) * (-high(t) / 2.0).exp();
            }
            r * ridge
        };
        let most = |t: f32| (ra + (rb - ra) * t).max(g * 0.56) * if roots > 0 { 1.45 + 3.1 * slight * (-high(t) / 0.75).exp() } else { 1.1 };
        let wander = |t: f32| (side * (t * 4.0 + mix(seed, 20) * 6.0).sin() + other * (t * 3.1 + mix(seed, 40) * 6.0).sin()) * thick.min(0.35) * 0.3 * (t * std::f32::consts::PI).sin();
        let upright = along.y > 0.95;
        let steps = ((length / (g * 0.5)).ceil() as i32).max(1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            // (What is well under the ground is neither drawn nor in anyone's way.)
            if roots > 0 && t * length < Self::UNDER - 0.3 {
                continue;
            }
            let centre = a.lerp(b, t) + wander(t);
            let k = (most(t) / g).ceil() as i32 + 1;
            let m = (centre / g).floor().as_ivec3();
            if only.is_some_and(|b| m.x + k < b[0] || m.x - k >= b[2] || m.z + k < b[1] || m.z - k >= b[3]) || centre.y + (k + 1) as f32 * g < span.0 || centre.y - (k + 1) as f32 * g > span.1 {
                continue;
            }
            put(m);
            // (Well inside it is wood and well outside it is not, without working out its
            // ridges: only the skin of it needs them.)
            let (plain, widest) = ((ra + (rb - ra) * t).max(g * 0.56) * 0.9, most(t));
            // (A trunk that leans has its slices leaning with it: a cube far from the axis and
            // in this slice is higher or lower than the axis is here, by the lean. Three
            // layers were enough for a cell; in cubes of 10 cm a great leaning trunk was
            // hollow beyond a third of its radius.)
            let lean = ((k as f32) * (1.0 - along.y * along.y).max(0.0).sqrt() / along.y.max(0.5)).ceil() as i32;
            let deep = if upright { lean.max(1) } else { k };
            for dy in -deep..=deep {
                for dz in -k..=k {
                    for dx in -k..=k {
                        let c = m + IVec3::new(dx, dy, dz);
                        let p = (c.as_vec3() + 0.5) * g - centre;
                        let axial = p.dot(along);
                        if axial.abs() > g * 0.8 {
                            continue;
                        }
                        let flat = p - along * axial;
                        let out = flat.length();
                        if out <= widest && (out <= plain || out <= radius(t, flat.dot(other).atan2(flat.dot(side)))) {
                            put(c);
                        }
                    }
                }
            }
        }
    }
}

/// Foliage filling the air inside an ellipsoid (metres), its edge ragged, nothing lower than
/// `floor`.
fn clump(world: &mut World, middle: Vec3, radii: Vec3, leaf: Block, floor: f32, rng: &mut Rng) {
    NOTED.with(|n| {
        if let Some(form) = n.borrow_mut().as_mut() {
            // Not the part of it under its floor.
            let low = (middle.y - radii.y).max(floor);
            let high = middle.y + radii.y;
            if high > low {
                form.clumps.push(Clump { middle: middle.with_y((low + high) / 2.0), radii: radii.with_y((high - low) / 2.0), leaf });
            }
        }
    });
    let clock = std::time::Instant::now();
    let (mut lo, mut hi) = (cell(middle - radii), cell(middle + radii));
    let ragged = rng.bits() as u32;
    if let Some(b) = ONLY.get() {
        (lo.x, lo.z, hi.x, hi.z) = (lo.x.max(b[0]), lo.z.max(b[1]), hi.x.min(b[2] - 1), hi.z.min(b[3] - 1));
    }
    for y in lo.y..=hi.y {
        if (y as f32) * CELL < floor {
            continue;
        }
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let c = IVec3::new(x, y, z);
                let d = (((c.as_vec3() + 0.5) * CELL - middle) / radii).length_squared();
                // (Outside the mass no roll is needed, nor well inside it.)
                if d < 1.0 && (d < 0.65 || d < 1.0 - crate::terrain::hash2(x + y * 31, z - y * 17, ragged) * 0.35) && world.get(c) == Block::Air {
                    world.set(c, leaf);
                }
            }
        }
    }
    crate::terrain::spent(4, clock);
}

/// A trunk from the ground at `foot` to `top` (metres), flaring into roots at its base.
fn bole(world: &mut World, foot: Vec3, top: Vec3, base: f32, tip: f32, bark: Block, rng: &mut Rng) {
    // The swell and the roots are the trunk's own shape (`Limb::fill`): it is told only how
    // many roots it has.
    let roots = 4 + rng.below(3);
    grow(world, Limb { a: foot - Vec3::Y * Limb::UNDER, b: top, ra: base, rb: tip, bark, roots: roots as u8 });
}

/// How tall a tree of a kind and age stands, metres: what [`tree`] builds.
pub fn height(kind: Tree, age: f32) -> f32 {
    kind.species().tall(age)
}

/// Grows a tree with its foot on the cell `foot`. `age` is 0 for one just grown, 1 for an
/// old one. Says how tall it stands, metres.
pub fn tree(world: &mut World, foot: IVec3, kind: Tree, age: f32, rng: &mut Rng) -> f32 {
    FOOT.set(Some(foot.y as f32 * CELL));
    let tall = grown(world, foot, kind, age, rng);
    FOOT.set(None);
    tall
}

fn grown(world: &mut World, foot: IVec3, kind: Tree, age: f32, rng: &mut Rng) -> f32 {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    let lean = |rng: &mut Rng, most: f32| Vec3::new(rng.between(-most, most), 0.0, rng.between(-most, most));
    let clear = foot.y + scale::UNDER_CROWN;
    // Its kind says how big, of what and how thick (`flora`); its habit, which shape.
    let sp = kind.species();
    let (tall, bark, leaf) = (sp.tall(age), sp.bark(), sp.leaf());
    match sp.habit {
        Habit::Spreading => {
            // As wide as it is tall. A short thick trunk forks low into heavy limbs that run
            // out, then up; each throws side branches, and every shoot ends in a mass of leaf.
            // The masses overlap into one broad dome with the limbs showing under it.
            // An old oak's leaves are deep; a younger one's the common green.
            let leaf = if age > 0.55 && rng.chance(0.7) { sp.old_leaf() } else { leaf };
            let base = tall * sp.girth;
            let fork = foot + Vec3::Y * tall * 0.3 + lean(rng, tall * 0.03);
            bole(world, foot, fork, base, base * 0.78, bark, rng);
            let mass = |world: &mut World, at: Vec3, size: f32, rng: &mut Rng| clump(world, at + Vec3::Y * size * 0.2, Vec3::new(size, size * 0.7, size), leaf, clear, rng);
            let limbs = 5 + rng.below(3);
            let turn = rng.between(0.0, TAU);
            for i in 0..limbs {
                let way = turn + i as f32 / limbs as f32 * TAU + rng.between(-0.3, 0.3);
                let (rise, long) = (rng.between(0.3, 0.95), tall * sp.spread * rng.between(0.3, 0.42));
                let end = fork + Vec3::new(way.cos() * rise.cos(), rise.sin(), way.sin() * rise.cos()) * long;
                let first = fork.lerp(end, 0.35) - Vec3::Y * 0.06 * long * rise.cos();
                let second = fork.lerp(end, 0.7) + Vec3::Y * 0.04 * long;
                let end = end + Vec3::Y * 0.16 * long;
                limb(world, fork, first, base * 0.5, base * 0.4, bark);
                limb(world, first, second, base * 0.4, base * 0.27, bark);
                limb(world, second, end, base * 0.27, 0.07, bark);
                mass(world, end, tall * rng.between(0.15, 0.19), rng);
                for (from, thick, reach) in [(first, base * 0.3, 0.2), (second, base * 0.22, 0.16)] {
                    let off = way + rng.between(0.6, 1.3) * if rng.chance(0.5) { 1.0 } else { -1.0 };
                    let tip = from + Vec3::new(off.cos(), rng.between(0.5, 1.0), off.sin()).normalize() * tall * reach;
                    let mid = from.lerp(tip, 0.5) - Vec3::Y * tall * 0.012;
                    limb(world, from, mid, thick, thick * 0.6, bark);
                    limb(world, mid, tip, thick * 0.6, 0.05, bark);
                    mass(world, tip, tall * rng.between(0.11, 0.15), rng);
                }
            }
            // The leader, the top of the crown over it, and the heart of the crown round it.
            let size = tall * 0.16;
            let top = foot + Vec3::Y * (tall - size * 0.7) + lean(rng, tall * 0.04);
            let mid = fork.lerp(top, 0.5) + lean(rng, tall * 0.03);
            limb(world, fork, mid, base * 0.5, base * 0.3, bark);
            limb(world, mid, top, base * 0.3, 0.08, bark);
            mass(world, top, size, rng);
            mass(world, mid, tall * 0.2, rng);
            tall
        }
        Habit::Slender => {
            // A slender pale trunk that sways as it climbs. Its branches leave it steeply,
            // then tire and hang at their ends: a tall narrow crown of light leaf, open
            // enough to see the trunk through, longest in the middle and gathered at the top.
            let base = tall * sp.girth;
            let bend = lean(rng, tall * 0.06);
            let low = foot + Vec3::Y * tall * 0.34 + bend * 0.9;
            let mid = foot + Vec3::Y * tall * 0.66 - bend * 0.3;
            let top = foot + Vec3::Y * tall * 0.95 + bend * 0.5;
            bole(world, foot, low, base * 1.2, base * 0.85, bark, rng);
            limb(world, low, mid, base * 0.85, base * 0.5, bark);
            limb(world, mid, top, base * 0.5, 0.05, bark);
            let along = |up: f32| if up < 0.34 { foot.lerp(low, up / 0.34) } else if up < 0.66 { low.lerp(mid, (up - 0.34) / 0.32) } else { mid.lerp(top, (up - 0.66) / 0.29) };
            let n = 10 + rng.below(6);
            let mut way = rng.between(0.0, TAU);
            for i in 0..n {
                let up = 0.36 + 0.56 * (i as f32 + rng.between(0.0, 0.8)) / n as f32;
                // Longest two thirds of the way up the crown.
                let part = (up - 0.36) / 0.56;
                let long = tall * (0.07 + 0.15 * (1.0 - (part - 0.35).abs() * 1.5).max(0.0));
                way += 2.4 + rng.between(-0.5, 0.5);
                let out = Vec3::new(way.cos(), 0.0, way.sin());
                let from = along(up);
                let elbow = from + (out * 0.55 + Vec3::Y * 0.85) * long * 0.6;
                let tip = elbow + (out * 0.9 - Vec3::Y * 0.45) * long * 0.7;
                limb(world, from, elbow, base * 0.32 * (1.2 - part), 0.05, bark);
                limb(world, elbow, tip, 0.05, 0.03, bark);
                let size = long * rng.between(0.42, 0.55);
                clump(world, elbow.lerp(tip, 0.3), Vec3::new(size, size * 0.7, size), leaf, clear, rng);
                // The hanging end: longer down than across.
                clump(world, tip - Vec3::Y * size * 0.6, Vec3::new(size * 0.6, size * 1.1, size * 0.6), leaf, clear, rng);
            }
            clump(world, top, Vec3::new(tall * 0.055, tall * 0.075, tall * 0.055), leaf, clear, rng);
            tall
        }
        Habit::Whorled => {
            // A straight tapering trunk, bare for its first quarter, then whorls to a point.
            let base = tall * sp.girth;
            let top = foot + Vec3::Y * tall + lean(rng, tall * 0.025);
            bole(world, foot, top, base, 0.08, bark, rng);
            let spread = tall * 0.16 * sp.spread;
            let first = (tall * 0.28).max(scale::UNDER_CROWN);
            // Dead stubs under the green.
            for _ in 0..2 + rng.below(3) {
                let (up, way) = (rng.between(1.6, first), rng.between(0.0, TAU));
                let from = foot.lerp(top, up / tall);
                limb(world, from, from + Vec3::new(way.cos(), -0.15, way.sin()) * rng.between(0.6, 1.6), 0.1, 0.05, bark);
            }
            // Whorls of boughs, each drooping, wide at the bottom and short at the top.
            let mut up = first;
            let mut turn = rng.between(0.0, TAU);
            while up < tall - 1.0 {
                let part = (up - first) / (tall - first);
                let long = spread * (1.0 - part).powf(0.8) + 0.5;
                let boughs = 4 + rng.below(3);
                for i in 0..boughs {
                    let way = turn + i as f32 / boughs as f32 * TAU + rng.between(-0.25, 0.25);
                    let from = foot.lerp(top, up / tall);
                    let long = long * rng.between(0.75, 1.1);
                    let end = from + Vec3::new(way.cos(), -0.3, way.sin()) * long;
                    limb(world, from, end, tall * 0.006, 0.05, bark);
                    // Needles in a flat spray along the outer part of the bough.
                    let spray = from.lerp(end, 0.62);
                    let flat = Vec3::new(long * 0.5 + 0.3, tall * 0.014 + 0.3, long * 0.5 + 0.3);
                    clump(world, spray, flat * Vec3::new(0.55 + 0.45 * way.cos().abs(), 1.0, 0.55 + 0.45 * way.sin().abs()), leaf, clear.min(foot.y + first - 0.5), rng);
                }
                up += tall * rng.between(0.042, 0.06);
                turn += 0.6;
            }
            clump(world, top - Vec3::Y * 0.3, Vec3::new(tall * 0.03, tall * 0.055, tall * 0.03), leaf, clear, rng);
            tall
        }
        Habit::Weeping => {
            // Short and thick, its limbs arching out and its leaves hanging to the ground.
            let base = tall * sp.girth;
            let fork = foot + Vec3::Y * tall * 0.3 + lean(rng, 0.5);
            bole(world, foot, fork, base, base * 0.8, bark, rng);
            let limbs = 4 + rng.below(3);
            let turn = rng.between(0.0, TAU);
            for i in 0..limbs {
                let way = turn + i as f32 / limbs as f32 * TAU + rng.between(-0.3, 0.3);
                let long = tall * 0.38;
                let high = fork + Vec3::new(way.cos() * 0.6, 0.9, way.sin() * 0.6) * long * 0.6;
                let end = high + Vec3::new(way.cos(), -0.1, way.sin()) * long * 0.6;
                limb(world, fork, high, base * 0.5, base * 0.3, bark);
                limb(world, high, end, base * 0.3, 0.08, bark);
                clump(world, high.lerp(end, 0.5) + Vec3::Y * 0.3, Vec3::new(tall * 0.2, tall * 0.11, tall * 0.2), leaf, foot.y + 1.6, rng);
                // Curtains: strands hanging from the limb's end almost to the ground.
                for _ in 0..10 {
                    let at = end + Vec3::new(rng.between(-1.0, 1.0), 0.0, rng.between(-1.0, 1.0)) * tall * 0.13;
                    let hang = rng.between(1.6, (at.y - foot.y - 0.8).max(1.7));
                    // A strand, for whoever draws shapes: a thin hanging mass.
                    NOTED.with(|n| {
                        if let Some(form) = n.borrow_mut().as_mut() {
                            form.clumps.push(Clump { middle: at - Vec3::Y * hang * 0.5, radii: Vec3::new(0.35, hang * 0.5, 0.35), leaf });
                        }
                    });
                    let c = cell(at);
                    for down in 0..(hang / CELL) as i32 {
                        let strand = c - IVec3::Y * down;
                        if open(strand) && world.get(strand) == Block::Air {
                            world.set(strand, leaf);
                        }
                    }
                }
            }
            let size = tall * 0.24;
            clump(world, foot + Vec3::Y * (tall - size * 0.6), Vec3::new(size, size * 0.6, size), leaf, foot.y + 2.0, rng);
            tall
        }
        Habit::Dead => {
            let base = tall * sp.girth;
            let top = foot + Vec3::Y * tall + lean(rng, tall * 0.08);
            bole(world, foot, top, base, base * 0.45, bark, rng);
            for _ in 0..3 + rng.below(4) {
                let (up, way) = (rng.between(0.35, 0.92), rng.between(0.0, TAU));
                let from = foot.lerp(top, up);
                let out = Vec3::new(way.cos(), rng.between(0.2, 0.7), way.sin());
                // Some limbs are whole and forked, most broken off short.
                if rng.chance(0.4) {
                    let elbow = from + out * tall * rng.between(0.1, 0.18);
                    limb(world, from, elbow, base * 0.4, base * 0.22, bark);
                    for turn in [-0.7f32, 0.6] {
                        let fork = Vec3::new((way + turn).cos(), rng.between(0.3, 0.9), (way + turn).sin());
                        limb(world, elbow, elbow + fork * tall * rng.between(0.06, 0.13), base * 0.2, 0.04, bark);
                    }
                } else {
                    limb(world, from, from + out * rng.between(0.4, 1.1), base * 0.4, base * 0.3, bark);
                }
            }
            tall
        }
    }
}

/// Grows one thing by name at `foot`, to be looked at on its own (`--specimen`): any tree
/// of `assets/world/flora.toml` by its name, `young`, `sapling`, `bush`, `log`, `stump`,
/// `rock`. False if there is no such thing.
pub fn specimen(world: &mut World, foot: IVec3, name: &str, rng: &mut Rng) -> bool {
    let kind = Tree::named(name);
    match (kind, name) {
        (Some(kind), _) => {
            tree(world, foot, kind, 0.8, rng);
        }
        (_, "young") => young(world, foot, false, rng),
        (_, "sapling") => young(world, foot, true, rng),
        (_, "bush") => bush(world, foot, rng),
        (_, "log") => log(world, foot, rng),
        (_, "stump") => stump(world, foot, rng),
        (_, "rock") => rock(world, foot, 2.4, rng),
        _ => return false,
    }
    true
}

/// A young tree, its crown low: what stands at eye level in a wood.
pub fn young(world: &mut World, foot: IVec3, needles: bool, rng: &mut Rng) {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    let tall = scale::YOUNG.at(rng.between(0.0, 1.0));
    let top = foot + Vec3::Y * tall + Vec3::new(rng.between(-0.3, 0.3), 0.0, rng.between(-0.3, 0.3));
    limb(world, foot - Vec3::Y * 0.3, top, tall * 0.03, 0.05, Block::Wood);
    if needles {
        let mut up = 0.8;
        while up < tall {
            let wide = (1.0 - up / tall) * tall * 0.27 + 0.25;
            clump(world, foot.lerp(top, up / tall), Vec3::new(wide, 0.3, wide), Block::Needles, foot.y + 0.5, rng);
            up += 0.7;
        }
    } else {
        let leaf = [Block::Leaves, Block::LeavesPale, Block::LeavesDeep][rng.below(3)];
        let size = 0.45 + tall * 0.14;
        clump(world, top - Vec3::Y * size * 0.3, Vec3::new(size, size * 0.9, size), leaf, foot.y + 1.0, rng);
        let mut way = rng.between(0.0, TAU);
        for i in 0..4 + rng.below(3) {
            let up = 0.4 + 0.1 * i as f32;
            way += 2.3;
            let from = foot.lerp(top, up.min(0.9));
            let tip = from + Vec3::new(way.cos(), 0.7, way.sin()) * tall * (0.3 - 0.2 * up).max(0.08);
            limb(world, from, tip, tall * 0.012, 0.03, Block::Wood);
            clump(world, tip, Vec3::new(size, size * 0.75, size) * rng.between(0.6, 0.85), leaf, foot.y + 0.9, rng);
        }
    }
}

/// A bush: two to four clumps low on the ground, knee-high to head-high.
pub fn bush(world: &mut World, foot: IVec3, rng: &mut Rng) {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    let leaf = [Block::Leaves, Block::LeavesDeep][rng.below(2)];
    let big = rng.between(0.7, 1.7);
    for _ in 0..2 + rng.below(3) {
        let at = foot + Vec3::new(rng.between(-0.6, 0.6), rng.between(0.3, 0.6), rng.between(-0.6, 0.6)) * big;
        clump(world, at, Vec3::new(rng.between(0.5, 0.9), rng.between(0.4, 0.7), rng.between(0.5, 0.9)) * big, leaf, foot.y, rng);
    }
}

/// A fallen trunk lying on the ground, and the stump it broke from when it is thick.
pub fn log(world: &mut World, foot: IVec3, rng: &mut Rng) {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    let (way, long, thick) = (rng.between(0.0, TAU), rng.between(3.0, 9.0), rng.between(0.2, 0.5));
    let along = Vec3::new(way.cos(), 0.0, way.sin());
    limb(world, foot + Vec3::Y * thick * 0.7, foot + along * long + Vec3::Y * thick * 0.5, thick, thick * 0.7, Block::Wood);
    for _ in 0..1 + rng.below(4) {
        let at = foot + along * long * rng.between(0.25, 0.9) + Vec3::Y * thick * 0.6;
        let side = Vec3::new(-along.z, rng.between(0.4, 1.6), along.x * if rng.chance(0.5) { 1.0 } else { -1.0 }).normalize();
        limb(world, at, at + side * rng.between(0.35, 1.2), thick * 0.3, thick * 0.18, Block::Wood);
    }
    if thick > 0.3 {
        stump(world, cell(foot - along * (thick + 0.6)), rng);
    }
}

/// What is left of a felled or fallen tree.
pub fn stump(world: &mut World, foot: IVec3, rng: &mut Rng) {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    let thick = rng.between(0.3, 0.65);
    bole(world, foot, foot + Vec3::Y * rng.between(0.35, 1.1), thick, thick * 0.9, Block::Wood, rng);
}

/// Where the ground is under a point, metres: rocks sit on it, not at the height of the
/// place they were asked for.
fn ground(world: &World, at: Vec3) -> f32 {
    let mut c = cell(at) + IVec3::Y * 12;
    while c.y > 0 && !(world.get(c).solid() && !matches!(world.get(c), Block::Wood | Block::Birch) && !world.get(c).foliage()) {
        c.y -= 1;
    }
    (c.y + 1) as f32 * CELL
}

/// A rock `across` metres over, shaped to the bit (5 cm): stone with flat faces struck off
/// it at angles, as rock breaks. Not a ball, and not one shape at every size: it is a block,
/// a slab lying flat, a tooth standing up or a low hump mostly buried, with smaller pieces
/// lying against it and, round a large one, stones fallen from it.
pub fn rock(world: &mut World, foot: IVec3, across: f32, rng: &mut Rng) {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    let stone = [Block::Stone, Block::Stone, Block::Slate, Block::Chalk][rng.below(4)];
    let piece = |world: &mut World, at: Vec3, radii: Vec3, sunk: f32, rng: &mut Rng| {
        // Seven to ten faces: each a plane across the stone, at its own angle and depth.
        let faces: Vec<(Vec3, f32)> = (0..7 + rng.below(4))
            .map(|i| {
                let (turn, tilt) = (rng.between(0.0, TAU), if i == 0 { 1.4 } else { rng.between(-0.5, 1.2) });
                (Vec3::new(turn.cos() * tilt.cos(), tilt.sin(), turn.sin() * tilt.cos()), rng.between(0.45, 0.8))
            })
            .collect();
        // Longer one way than the other, turned any way round.
        let (turn, long) = (rng.between(0.0, TAU), rng.between(1.0, 1.5));
        let (sin, cos) = turn.sin_cos();
        let middle = Vec3::new(at.x, ground(world, at) + radii.y * (0.55 - sunk), at.z);
        NOTED.with(|n| {
            if let Some(form) = n.borrow_mut().as_mut() {
                form.stones.push(Stone { middle, radii: Vec3::new(radii.x * long, radii.y, radii.z), turn, faces: faces.clone(), kind: stone });
            }
        });
        let reach = Vec3::new(radii.x * long, radii.y, (radii.x * long).max(radii.z));
        world.shape(middle - reach, middle + reach, |p| {
            let d = p - middle;
            let q = Vec3::new((d.x * cos + d.z * sin) / (radii.x * long), d.y / radii.y, (d.z * cos - d.x * sin) / radii.z);
            q.length_squared() < 1.0 && faces.iter().all(|(n, d)| q.dot(*n) < *d)
        }, Block::Rock, true);
    };
    // The form: how tall and deep-set it is for its width.
    let half = across * 0.5;
    let (radii, sunk) = match rng.below(if across < 1.0 { 2 } else { 5 }) {
        // A block, about as tall as wide.
        0 | 1 => (Vec3::new(half, half * rng.between(0.7, 1.1), half * rng.between(0.75, 1.0)), rng.between(0.2, 0.5)),
        // A slab lying flat.
        2 => (Vec3::new(half * 1.15, half * rng.between(0.3, 0.45), half * rng.between(0.7, 1.0)), rng.between(0.1, 0.3)),
        // A tooth standing up.
        3 => (Vec3::new(half * 0.6, half * rng.between(1.3, 1.9), half * rng.between(0.45, 0.65)), rng.between(0.35, 0.5)),
        // A hump, most of it under the ground.
        _ => (Vec3::new(half * 1.1, half * rng.between(0.6, 0.8), half * rng.between(0.8, 1.1)), rng.between(0.7, 0.95)),
    };
    piece(world, foot, radii, sunk, rng);
    // Pieces lying against it.
    for _ in 0..rng.below(3) {
        let way = rng.between(0.0, TAU);
        let small = radii * rng.between(0.3, 0.55);
        piece(world, foot + Vec3::new(way.cos(), 0.0, way.sin()) * half * rng.between(0.8, 1.15), small, rng.between(0.2, 0.5), rng);
    }
    // Stones fallen round a large one, or keeping a small one company.
    let fallen = if across > 2.0 { 3 + rng.below(5) } else { rng.below(3) };
    for _ in 0..fallen {
        let (way, out) = (rng.between(0.0, TAU), half + rng.between(0.3, 2.5));
        let small = rng.between(0.12, 0.35);
        piece(world, foot + Vec3::new(way.cos(), 0.0, way.sin()) * out, Vec3::new(small, small * rng.between(0.6, 1.0), small * rng.between(0.7, 1.0)), rng.between(0.2, 0.5), rng);
    }
}
