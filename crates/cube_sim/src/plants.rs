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
    NOTED.with(|n| {
        if let Some(form) = n.borrow_mut().as_mut() {
            form.limbs.push(Limb { a, b, ra, rb, bark, roots: 0 });
        }
    });
    wood(world, a, b, ra, rb, bark);
}

/// The cells of a limb, and no note of its shape: for wood that whoever draws shapes draws
/// as part of something else (the flare at a trunk's foot).
fn wood(world: &mut World, a: Vec3, b: Vec3, ra: f32, rb: f32, bark: Block) {
    let steps = ((a.distance(b) / (CELL * 0.5)).ceil() as i32).max(1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let (at, r) = (a.lerp(b, t), ra + (rb - ra) * t);
        let reach = (r / CELL).ceil() as i32;
        let middle = cell(at);
        for y in -reach..=reach {
            for z in -reach..=reach {
                for x in -reach..=reach {
                    let c = middle + IVec3::new(x, y, z);
                    let d = ((c.as_vec3() + 0.5) * CELL).distance(at);
                    // Never thinner than one cell: a twig is still there.
                    if (d <= r || c == middle) && (world.get(c) == Block::Air || world.get(c).foliage() || world.get(c).sprig()) {
                        world.set(c, bark);
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
    let (lo, hi) = (cell(middle - radii), cell(middle + radii));
    let ragged = rng.bits() as u32;
    for y in lo.y..=hi.y {
        if (y as f32) * CELL < floor {
            continue;
        }
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let c = IVec3::new(x, y, z);
                let d = (((c.as_vec3() + 0.5) * CELL - middle) / radii).length_squared();
                if d < 1.0 - crate::terrain::hash2(x + y * 31, z - y * 17, ragged) * 0.35 && world.get(c) == Block::Air {
                    world.set(c, leaf);
                }
            }
        }
    }
}

/// A trunk from the ground at `foot` to `top` (metres), flaring into roots at its base.
fn bole(world: &mut World, foot: Vec3, top: Vec3, base: f32, tip: f32, bark: Block, rng: &mut Rng) {
    let roots = 4 + rng.below(3);
    NOTED.with(|n| {
        if let Some(form) = n.borrow_mut().as_mut() {
            form.limbs.push(Limb { a: foot - Vec3::Y * 0.5, b: top, ra: base, rb: tip, bark, roots: roots as u8 });
        }
    });
    wood(world, foot - Vec3::Y * 0.5, top, base, tip, bark);
    // The flare, and roots running out along the ground and into it.
    wood(world, foot - Vec3::Y * 0.3, foot + Vec3::Y * base * 1.5, base * 1.5, base, bark);
    let turn = rng.between(0.0, TAU);
    for i in 0..roots {
        let way = turn + i as f32 / roots as f32 * TAU + rng.between(-0.4, 0.4);
        let out = Vec3::new(way.cos(), 0.0, way.sin()) * (base * 2.2 + rng.between(0.2, 0.7));
        wood(world, foot + Vec3::Y * base * 0.8, foot + out - Vec3::Y * 0.2, base * 0.5, CELL * 0.4, bark);
    }
}

/// How tall a tree of a kind and age stands, metres: what [`tree`] builds.
pub fn height(kind: Tree, age: f32) -> f32 {
    kind.species().tall(age)
}

/// Grows a tree with its foot on the cell `foot`. `age` is 0 for one just grown, 1 for an
/// old one. Says how tall it stands, metres.
pub fn tree(world: &mut World, foot: IVec3, kind: Tree, age: f32, rng: &mut Rng) -> f32 {
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
                        if world.get(strand) == Block::Air {
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
