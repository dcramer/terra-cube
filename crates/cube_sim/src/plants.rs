//! What grows: trees by kind, young trees, bushes, logs and stumps, each built into the world
//! out of cells. Sizes are in metres and real: an oak is an oak's size.
//!
//! A tree is not a pole with a ball on it. It has a trunk that flares into roots and tapers
//! upward, leaning a little; limbs that leave it and fork; and foliage in separate clumps at
//! the ends of the limbs, so sky shows through and no two are alike. Every choice comes from a
//! seeded [`Rng`], so the same seed grows the same forest.

use crate::rng::Rng;
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
                    if (d <= r || c == middle) && matches!(world.get(c), Block::Air | Block::Leaves | Block::Needles | Block::Tuft | Block::Flower) {
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
/// old one.
pub fn tree(world: &mut World, foot: IVec3, kind: Tree, age: f32, rng: &mut Rng) {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    let lean = |rng: &mut Rng, most: f32| Vec3::new(rng.between(-most, most), 0.0, rng.between(-most, most));
    // Foliage starts three and a half metres up, so the largest creature walks under it.
    let clear = foot.y + 3.5;
    match kind {
        Tree::Oak => {
            let base = 0.3 + 0.3 * age;
            let fork = foot + Vec3::Y * (3.6 + 2.6 * age) + lean(rng, 0.5);
            bole(world, foot, fork, base, base * 0.75, Block::Wood, rng);
            let limbs = 3 + rng.below(3);
            let turn = rng.between(0.0, TAU);
            for i in 0..limbs {
                let way = turn + i as f32 / limbs as f32 * TAU + rng.between(-0.35, 0.35);
                let (rise, long) = (rng.between(0.55, 1.1), 2.6 + 2.6 * age + rng.between(-0.5, 0.8));
                let end = fork + Vec3::new(way.cos() * rise.cos(), rise.sin(), way.sin() * rise.cos()) * long;
                // A limb bends: out first, then up.
                let elbow = fork.lerp(end, 0.55) + Vec3::Y * -0.25 * long * rise.cos() * 0.5;
                limb(world, fork, elbow, base * 0.55, base * 0.38, Block::Wood);
                limb(world, elbow, end, base * 0.38, 0.1, Block::Wood);
                let size = 1.5 + 0.9 * age + rng.between(-0.2, 0.4);
                clump(world, end + Vec3::Y * size * 0.25, Vec3::new(size, size * 0.72, size), Block::Leaves, clear, rng);
                // A side shoot off the elbow, with a smaller clump.
                if rng.chance(0.7) {
                    let off = way + rng.between(0.7, 1.3) * if rng.chance(0.5) { 1.0 } else { -1.0 };
                    let twig = elbow + Vec3::new(off.cos(), 0.6, off.sin()) * (1.2 + 1.2 * age);
                    limb(world, elbow, twig, base * 0.3, 0.08, Block::Wood);
                    clump(world, twig, Vec3::new(1.1, 0.8, 1.1) * (1.0 + 0.5 * age), Block::Leaves, clear, rng);
                }
            }
            // The leader, and the top of the crown over it.
            let top = fork + Vec3::Y * (2.2 + 2.0 * age) + lean(rng, 0.6);
            limb(world, fork, top, base * 0.5, 0.1, Block::Wood);
            let size = 1.7 + 1.0 * age;
            clump(world, top, Vec3::new(size, size * 0.75, size), Block::Leaves, clear, rng);
        }
        Tree::Birch => {
            let base = 0.14 + 0.1 * age;
            let tall = 8.0 + 5.0 * age;
            let bend = lean(rng, 0.9);
            let (mid, top) = (foot + Vec3::Y * tall * 0.5 + bend * 0.7, foot + Vec3::Y * tall + bend);
            limb(world, foot - Vec3::Y * 0.4, mid, base * 1.3, base, Block::Birch);
            limb(world, mid, top, base, 0.06, Block::Birch);
            // Short limbs sweeping up from the upper half, each with a light clump.
            let n = 5 + rng.below(4);
            for i in 0..n {
                let up = 0.45 + 0.5 * i as f32 / n as f32;
                let from = if up < 0.5 { foot.lerp(mid, up * 2.0) } else { mid.lerp(top, up * 2.0 - 1.0) };
                let way = rng.between(0.0, TAU);
                let long = (1.0 + 1.2 * (1.0 - up)) * (0.8 + 0.5 * age);
                let end = from + Vec3::new(way.cos() * 0.6, 0.8, way.sin() * 0.6) * long;
                limb(world, from, end, base * 0.5, 0.05, Block::Birch);
                let size = 0.8 + 0.5 * (1.0 - up) + 0.3 * age;
                clump(world, end, Vec3::new(size, size * 0.8, size), Block::Leaves, clear, rng);
            }
            clump(world, top, Vec3::new(0.9, 1.1, 0.9), Block::Leaves, clear, rng);
        }
        Tree::Pine => {
            let base = 0.25 + 0.2 * age;
            let tall = 12.0 + 12.0 * age;
            let top = foot + Vec3::Y * tall + lean(rng, 0.4);
            bole(world, foot, top, base, 0.08, Block::Wood, rng);
            let spread = 2.2 + 1.8 * age;
            let first = (tall * 0.28).max(3.6);
            // Dead stubs under the green.
            for _ in 0..2 + rng.below(3) {
                let (up, way) = (rng.between(1.6, first), rng.between(0.0, TAU));
                let from = foot.lerp(top, up / tall);
                limb(world, from, from + Vec3::new(way.cos(), -0.15, way.sin()) * rng.between(0.5, 1.1), 0.1, 0.05, Block::Wood);
            }
            // Whorls of boughs, each drooping, wide at the bottom and short at the top.
            let mut up = first;
            let mut turn = rng.between(0.0, TAU);
            while up < tall - 0.8 {
                let part = (up - first) / (tall - first);
                let long = spread * (1.0 - part).powf(0.8) + 0.4;
                let boughs = 4 + rng.below(3);
                for i in 0..boughs {
                    let way = turn + i as f32 / boughs as f32 * TAU + rng.between(-0.25, 0.25);
                    let from = foot.lerp(top, up / tall);
                    let long = long * rng.between(0.75, 1.1);
                    let end = from + Vec3::new(way.cos(), -0.22, way.sin()) * long;
                    limb(world, from, end, 0.1, 0.05, Block::Wood);
                    // Needles in a flat spray along the outer part of the bough.
                    let spray = from.lerp(end, 0.62);
                    clump(world, spray, Vec3::new(long * 0.5 + 0.3, 0.38, long * 0.5 + 0.3) * Vec3::new(0.55 + 0.45 * way.cos().abs(), 1.0, 0.55 + 0.45 * way.sin().abs()), Block::Needles, clear.min(foot.y + first - 0.5), rng);
                }
                up += rng.between(1.1, 1.7);
                turn += 0.6;
            }
            clump(world, top - Vec3::Y * 0.3, Vec3::new(0.6, 1.1, 0.6), Block::Needles, clear, rng);
        }
        Tree::Willow => {
            let base = 0.4 + 0.25 * age;
            let fork = foot + Vec3::Y * (2.6 + 1.2 * age) + lean(rng, 0.5);
            bole(world, foot, fork, base, base * 0.8, Block::Wood, rng);
            let limbs = 4 + rng.below(3);
            let turn = rng.between(0.0, TAU);
            for i in 0..limbs {
                let way = turn + i as f32 / limbs as f32 * TAU + rng.between(-0.3, 0.3);
                let long = 2.8 + 1.6 * age;
                let high = fork + Vec3::new(way.cos() * 0.6, 0.9, way.sin() * 0.6) * long * 0.6;
                let end = high + Vec3::new(way.cos(), -0.1, way.sin()) * long * 0.6;
                limb(world, fork, high, base * 0.5, base * 0.3, Block::Wood);
                limb(world, high, end, base * 0.3, 0.08, Block::Wood);
                clump(world, high.lerp(end, 0.5) + Vec3::Y * 0.3, Vec3::new(1.8, 1.0, 1.8), Block::Leaves, foot.y + 1.2, rng);
                // Curtains: strands hanging from the limb's end almost to the ground.
                for _ in 0..7 {
                    let at = end + Vec3::new(rng.between(-1.2, 1.2), 0.0, rng.between(-1.2, 1.2));
                    let hang = rng.between(1.6, (at.y - foot.y - 0.8).max(1.7));
                    let c = cell(at);
                    for down in 0..(hang / CELL) as i32 {
                        let strand = c - IVec3::Y * down;
                        if world.get(strand) == Block::Air {
                            world.set(strand, Block::Leaves);
                        }
                    }
                }
            }
            clump(world, fork + Vec3::Y * 2.2, Vec3::new(2.2, 1.3, 2.2), Block::Leaves, foot.y + 1.5, rng);
        }
        Tree::Snag => {
            let base = 0.28 + 0.2 * age;
            let top = foot + Vec3::Y * (4.0 + 5.0 * age) + lean(rng, 0.7);
            bole(world, foot, top, base, base * 0.45, Block::Wood, rng);
            for _ in 0..2 + rng.below(2) {
                let (up, way) = (rng.between(0.45, 0.9), rng.between(0.0, TAU));
                let from = foot.lerp(top, up);
                limb(world, from, from + Vec3::new(way.cos(), 0.5, way.sin()) * rng.between(1.0, 2.4), base * 0.35, 0.06, Block::Wood);
            }
        }
    }
}

/// A young tree, two to five metres, its crown low: what stands at eye level in a wood.
pub fn young(world: &mut World, foot: IVec3, needles: bool, rng: &mut Rng) {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    let tall = rng.between(2.2, 4.8);
    let top = foot + Vec3::Y * tall + Vec3::new(rng.between(-0.3, 0.3), 0.0, rng.between(-0.3, 0.3));
    limb(world, foot - Vec3::Y * 0.3, top, 0.1, 0.05, Block::Wood);
    if needles {
        let mut up = 0.8;
        while up < tall {
            let wide = (1.0 - up / tall) * 1.1 + 0.25;
            clump(world, foot.lerp(top, up / tall), Vec3::new(wide, 0.3, wide), Block::Needles, foot.y + 0.5, rng);
            up += 0.7;
        }
    } else {
        let size = 0.7 + tall * 0.18;
        clump(world, top - Vec3::Y * size * 0.3, Vec3::new(size, size * 0.9, size), Block::Leaves, foot.y + 1.0, rng);
        clump(world, top - Vec3::Y * size + Vec3::new(rng.between(-0.5, 0.5), 0.0, rng.between(-0.5, 0.5)), Vec3::splat(size * 0.7), Block::Leaves, foot.y + 0.9, rng);
    }
}

/// A bush: two or three clumps low on the ground.
pub fn bush(world: &mut World, foot: IVec3, rng: &mut Rng) {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    for _ in 0..2 + rng.below(2) {
        let at = foot + Vec3::new(rng.between(-0.5, 0.5), rng.between(0.3, 0.6), rng.between(-0.5, 0.5));
        clump(world, at, Vec3::new(rng.between(0.5, 0.9), rng.between(0.4, 0.7), rng.between(0.5, 0.9)), Block::Leaves, foot.y, rng);
    }
}

/// A fallen trunk lying on the ground, and the stump it broke from when it is thick.
pub fn log(world: &mut World, foot: IVec3, rng: &mut Rng) {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    let (way, long, thick) = (rng.between(0.0, TAU), rng.between(2.0, 5.5), rng.between(0.18, 0.36));
    let along = Vec3::new(way.cos(), 0.0, way.sin());
    limb(world, foot + Vec3::Y * thick * 0.7, foot + along * long + Vec3::Y * thick * 0.5, thick, thick * 0.7, Block::Wood);
    if thick > 0.26 {
        stump(world, cell(foot - along * (thick + 0.6)), rng);
    }
}

/// What is left of a felled or fallen tree.
pub fn stump(world: &mut World, foot: IVec3, rng: &mut Rng) {
    let foot = (foot.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * CELL;
    let thick = rng.between(0.25, 0.45);
    bole(world, foot, foot + Vec3::Y * rng.between(0.35, 0.8), thick, thick * 0.9, Block::Wood, rng);
}
