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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tree {
    /// Broad, heavy-limbed, the crown wider than it is tall.
    Oak,
    /// Slender and pale, its foliage light and high.
    Birch,
    /// A straight tapering trunk with whorls of drooping boughs.
    Pine,
    /// Short and thick by water, its crown hanging in curtains.
    Willow,
    /// Dead: a broken trunk and a few bare limbs.
    Snag,
}

fn cell(p: Vec3) -> IVec3 {
    (p / CELL).floor().as_ivec3()
}

/// Wood from `a` to `b` (metres), `ra` thick at one end and `rb` at the other. It grows through
/// air and foliage, not through the ground.
fn limb(world: &mut World, a: Vec3, b: Vec3, ra: f32, rb: f32, bark: Block) {
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
    limb(world, foot - Vec3::Y * 0.5, top, base, tip, bark);
    // The flare, and roots running out along the ground and into it.
    limb(world, foot - Vec3::Y * 0.3, foot + Vec3::Y * base * 1.5, base * 1.5, base, bark);
    let roots = 3 + rng.below(3);
    let turn = rng.between(0.0, TAU);
    for i in 0..roots {
        let way = turn + i as f32 / roots as f32 * TAU + rng.between(-0.4, 0.4);
        let out = Vec3::new(way.cos(), 0.0, way.sin()) * (base * 2.2 + rng.between(0.2, 0.7));
        limb(world, foot + Vec3::Y * base * 0.8, foot + out - Vec3::Y * 0.2, base * 0.5, CELL * 0.4, bark);
    }
}

/// Grows a tree with its foot on the cell `foot`. `age` is 0 for one just grown, 1 for an
/// old one. Says how tall it stands, metres.
pub fn tree(world: &mut World, foot: IVec3, kind: Tree, age: f32, rng: &mut Rng) -> f32 {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    let lean = |rng: &mut Rng, most: f32| Vec3::new(rng.between(-most, most), 0.0, rng.between(-most, most));
    let clear = foot.y + scale::UNDER_CROWN;
    match kind {
        Tree::Oak => {
            // As wide as it is tall: the trunk forks a third of the way up into heavy limbs.
            let tall = scale::OAK.at(age);
            // An old oak's leaves are deep; a younger one's the common green.
            let leaf = if age > 0.55 && rng.chance(0.7) { Block::LeavesDeep } else { Block::Leaves };
            let base = tall * 0.027;
            let fork = foot + Vec3::Y * tall * 0.34 + lean(rng, tall * 0.03);
            bole(world, foot, fork, base, base * 0.75, Block::Wood, rng);
            let limbs = 4 + rng.below(3);
            let turn = rng.between(0.0, TAU);
            for i in 0..limbs {
                let way = turn + i as f32 / limbs as f32 * TAU + rng.between(-0.35, 0.35);
                let (rise, long) = (rng.between(0.5, 1.05), tall * rng.between(0.27, 0.36));
                let end = fork + Vec3::new(way.cos() * rise.cos(), rise.sin(), way.sin() * rise.cos()) * long;
                // A limb bends: out first, then up.
                let elbow = fork.lerp(end, 0.55) - Vec3::Y * 0.12 * long * rise.cos();
                limb(world, fork, elbow, base * 0.55, base * 0.38, Block::Wood);
                limb(world, elbow, end, base * 0.38, 0.1, Block::Wood);
                let size = tall * rng.between(0.11, 0.145);
                clump(world, end + Vec3::Y * size * 0.25, Vec3::new(size, size * 0.72, size), leaf, clear, rng);
                // A side shoot off the elbow, with a smaller clump.
                if rng.chance(0.75) {
                    let off = way + rng.between(0.7, 1.3) * if rng.chance(0.5) { 1.0 } else { -1.0 };
                    let twig = elbow + Vec3::new(off.cos(), 0.6, off.sin()) * tall * 0.13;
                    limb(world, elbow, twig, base * 0.3, 0.08, Block::Wood);
                    let size = tall * rng.between(0.075, 0.1);
                    clump(world, twig, Vec3::new(size, size * 0.75, size), leaf, clear, rng);
                }
            }
            // The leader, and the top of the crown over it.
            let size = tall * 0.13;
            let top = foot + Vec3::Y * (tall - size * 0.75) + lean(rng, tall * 0.04);
            limb(world, fork, top, base * 0.5, 0.1, Block::Wood);
            clump(world, top, Vec3::new(size, size * 0.75, size), leaf, clear, rng);
            tall
        }
        Tree::Birch => {
            // A thin pale trunk with a bend in it, and light clumps up its top half.
            let tall = scale::BIRCH.at(age);
            let base = tall * 0.016;
            let bend = lean(rng, tall * 0.07);
            let (mid, top) = (foot + Vec3::Y * tall * 0.5 + bend * 0.7, foot + Vec3::Y * tall * 0.93 + bend);
            limb(world, foot - Vec3::Y * 0.4, mid, base * 1.3, base, Block::Birch);
            limb(world, mid, top, base, 0.06, Block::Birch);
            let n = 6 + rng.below(5);
            for i in 0..n {
                let up = 0.42 + 0.53 * i as f32 / n as f32;
                let from = if up < 0.5 { foot.lerp(mid, up * 2.0) } else { mid.lerp(top, up * 2.0 - 1.0) };
                let way = rng.between(0.0, TAU);
                let long = tall * (0.07 + 0.11 * (1.0 - up));
                let end = from + Vec3::new(way.cos() * 0.6, 0.8, way.sin() * 0.6) * long;
                limb(world, from, end, base * 0.5, 0.05, Block::Birch);
                let size = tall * (0.055 + 0.05 * (1.0 - up));
                clump(world, end, Vec3::new(size, size * 0.8, size), Block::LeavesPale, clear, rng);
            }
            clump(world, top, Vec3::new(tall * 0.06, tall * 0.07, tall * 0.06), Block::LeavesPale, clear, rng);
            tall
        }
        Tree::Pine => {
            // A straight tapering trunk, bare for its first quarter, then whorls to a point.
            let tall = scale::PINE.at(age);
            let base = tall * 0.019;
            let top = foot + Vec3::Y * tall + lean(rng, tall * 0.025);
            bole(world, foot, top, base, 0.08, Block::Wood, rng);
            let spread = tall * 0.16;
            let first = (tall * 0.28).max(scale::UNDER_CROWN);
            // Dead stubs under the green.
            for _ in 0..2 + rng.below(3) {
                let (up, way) = (rng.between(1.6, first), rng.between(0.0, TAU));
                let from = foot.lerp(top, up / tall);
                limb(world, from, from + Vec3::new(way.cos(), -0.15, way.sin()) * rng.between(0.6, 1.6), 0.1, 0.05, Block::Wood);
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
                    let end = from + Vec3::new(way.cos(), -0.22, way.sin()) * long;
                    limb(world, from, end, tall * 0.006, 0.05, Block::Wood);
                    // Needles in a flat spray along the outer part of the bough.
                    let spray = from.lerp(end, 0.62);
                    let flat = Vec3::new(long * 0.5 + 0.3, tall * 0.014 + 0.2, long * 0.5 + 0.3);
                    clump(world, spray, flat * Vec3::new(0.55 + 0.45 * way.cos().abs(), 1.0, 0.55 + 0.45 * way.sin().abs()), Block::Needles, clear.min(foot.y + first - 0.5), rng);
                }
                up += tall * rng.between(0.06, 0.085);
                turn += 0.6;
            }
            clump(world, top - Vec3::Y * 0.3, Vec3::new(tall * 0.03, tall * 0.055, tall * 0.03), Block::Needles, clear, rng);
            tall
        }
        Tree::Willow => {
            // Short and thick, its limbs arching out and its leaves hanging to the ground.
            let tall = scale::WILLOW.at(age);
            let base = tall * 0.05;
            let fork = foot + Vec3::Y * tall * 0.3 + lean(rng, 0.5);
            bole(world, foot, fork, base, base * 0.8, Block::Wood, rng);
            let limbs = 4 + rng.below(3);
            let turn = rng.between(0.0, TAU);
            for i in 0..limbs {
                let way = turn + i as f32 / limbs as f32 * TAU + rng.between(-0.3, 0.3);
                let long = tall * 0.38;
                let high = fork + Vec3::new(way.cos() * 0.6, 0.9, way.sin() * 0.6) * long * 0.6;
                let end = high + Vec3::new(way.cos(), -0.1, way.sin()) * long * 0.6;
                limb(world, fork, high, base * 0.5, base * 0.3, Block::Wood);
                limb(world, high, end, base * 0.3, 0.08, Block::Wood);
                clump(world, high.lerp(end, 0.5) + Vec3::Y * 0.3, Vec3::new(tall * 0.2, tall * 0.11, tall * 0.2), Block::LeavesPale, foot.y + 1.6, rng);
                // Curtains: strands hanging from the limb's end almost to the ground.
                for _ in 0..10 {
                    let at = end + Vec3::new(rng.between(-1.0, 1.0), 0.0, rng.between(-1.0, 1.0)) * tall * 0.13;
                    let hang = rng.between(1.6, (at.y - foot.y - 0.8).max(1.7));
                    let c = cell(at);
                    for down in 0..(hang / CELL) as i32 {
                        let strand = c - IVec3::Y * down;
                        if world.get(strand) == Block::Air {
                            world.set(strand, Block::LeavesPale);
                        }
                    }
                }
            }
            let size = tall * 0.24;
            clump(world, foot + Vec3::Y * (tall - size * 0.6), Vec3::new(size, size * 0.6, size), Block::LeavesPale, foot.y + 2.0, rng);
            tall
        }
        Tree::Snag => {
            let tall = scale::SNAG.at(age);
            let base = tall * 0.035;
            let top = foot + Vec3::Y * tall + lean(rng, tall * 0.08);
            bole(world, foot, top, base, base * 0.45, Block::Wood, rng);
            for _ in 0..2 + rng.below(3) {
                let (up, way) = (rng.between(0.45, 0.9), rng.between(0.0, TAU));
                let from = foot.lerp(top, up);
                limb(world, from, from + Vec3::new(way.cos(), 0.5, way.sin()) * tall * rng.between(0.12, 0.28), base * 0.35, 0.06, Block::Wood);
            }
            tall
        }
    }
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
        let size = 0.6 + tall * 0.2;
        clump(world, top - Vec3::Y * size * 0.3, Vec3::new(size, size * 0.9, size), leaf, foot.y + 1.0, rng);
        clump(world, top - Vec3::Y * size + Vec3::new(rng.between(-0.5, 0.5), 0.0, rng.between(-0.5, 0.5)), Vec3::splat(size * 0.7), leaf, foot.y + 0.9, rng);
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
        let reach = Vec3::new(radii.x * long, radii.y, (radii.x * long).max(radii.z));
        world.shape(middle - reach, middle + reach, |p| {
            let d = p - middle;
            let q = Vec3::new((d.x * cos + d.z * sin) / (radii.x * long), d.y / radii.y, (d.z * cos - d.x * sin) / radii.z);
            q.length_squared() < 1.0 && faces.iter().all(|(n, d)| q.dot(*n) < *d)
        }, stone, true);
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
