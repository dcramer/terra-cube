//! A made world in numbers, so it is judged before anyone looks at it: how much is land, what
//! grows where, how high it goes, and how much of it can be walked to from the start.

use crate::scale;
use crate::terrain::{Biome, Land, SEA};
use cube_voxel::{CELL, cells};
use std::collections::VecDeque;

pub struct Survey {
    /// The part of all columns in each biome, in `Biome::ALL` order.
    pub share: [f32; 6],
    pub trees: usize,
    /// Metres from the sea to the highest ground.
    pub highest: f32,
    /// The part of dry land that is cliff: steeper than three up for one along.
    pub cliffs: f32,
    /// The part of dry land reached from the start on foot: steps, jumps and wading.
    pub on_foot: f32,
    /// The same when swimming is allowed.
    pub swimming: f32,
    /// Scale, each against a bar in `scale`: the grown tree in the middle of the count, in
    /// people; the tallest, metres; the largest rock over the smallest; and the part of
    /// meadow and forest that is easy ground.
    pub tree: f32,
    pub tallest: f32,
    pub rocks: f32,
    pub easy: f32,
}

/// Whether a creature gets from one column to the next: up a metre (a jump), down three,
/// and through water no deeper than half a metre unless it swims.
fn passable(land: &Land, from: (i32, i32), to: (i32, i32), swim: bool) -> bool {
    let wade = SEA - cells(0.5);
    let (a, b) = (land.height(from.0, from.1).max(wade), land.height(to.0, to.1));
    if b < wade {
        return swim;
    }
    b - a <= cells(1.0) && a - b <= cells(3.0)
}

/// Which columns are reached from the origin.
pub fn reached(land: &Land, swim: bool) -> Vec<bool> {
    let side = land.span * 2;
    let index = |x: i32, z: i32| ((x + land.span) + (z + land.span) * side) as usize;
    let mut seen = vec![false; (side * side) as usize];
    let mut queue = VecDeque::from([(0, 0)]);
    seen[index(0, 0)] = true;
    while let Some((x, z)) = queue.pop_front() {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, nz) = (x + dx, z + dz);
            if nx < -land.span || nx >= land.span || nz < -land.span || nz >= land.span || seen[index(nx, nz)] {
                continue;
            }
            if passable(land, (x, z), (nx, nz), swim) {
                seen[index(nx, nz)] = true;
                queue.push_back((nx, nz));
            }
        }
    }
    seen
}

pub fn survey(land: &Land) -> Survey {
    let span = land.span;
    let (mut share, mut dry, mut cliffs, mut top) = ([0.0f32; 6], 0usize, 0usize, SEA);
    let (foot, swim) = (reached(land, false), reached(land, true));
    let (mut on_foot, mut swimming) = (0usize, 0usize);
    let (mut soft, mut easy) = (0usize, 0usize);
    let mut i = 0;
    for z in -span..span {
        for x in -span..span {
            let biome = land.biome(x, z);
            share[biome as usize] += 1.0;
            let h = land.height(x, z);
            if matches!(biome, Biome::Meadow | Biome::Forest) && h < SEA + cells(scale::CONIFER_LINE) {
                soft += 1;
                easy += (grade(land, x, z) == 0) as usize;
            }
            if biome != Biome::Water {
                dry += 1;
                cliffs += (land.slope(x, z) >= 3) as usize;
                on_foot += foot[i] as usize;
                swimming += swim[i] as usize;
                top = top.max(land.height(x, z));
            }
            i += 1;
        }
    }
    let all = (span * span * 4) as f32;
    let dry = dry.max(1) as f32;
    let mut grown = land.grown.clone();
    grown.sort_by(f32::total_cmp);
    let tree = grown.get(grown.len() / 2).map_or(0.0, |t| t / scale::PERSON);
    let tallest = grown.last().copied().unwrap_or(0.0);
    let (least, most) = land.rocks.iter().fold((f32::MAX, 0.0f32), |(lo, hi), &r| (lo.min(r), hi.max(r)));
    let rocks = if land.rocks.is_empty() { 0.0 } else { most / least };
    let easy = easy as f32 / soft.max(1) as f32;
    Survey { tree, tallest, rocks, easy, share: share.map(|s| s / all), trees: land.trees.len(), highest: (top - SEA) as f32 * CELL, cliffs: cliffs as f32 / dry, on_foot: on_foot as f32 / dry, swimming: swimming as f32 / dry }
}

/// How steep the ground is, by height over the sea: for each band of dry land, how many
/// columns it has and the part of them that is easy (to 0.75 m up or down over two metres
/// along), steep (to 2 m) and cliff. Where the staircases are, when `easy` falls short.
pub fn grades(land: &Land, bands: &[f32]) -> Vec<(usize, [f32; 3])> {
    let mut out = vec![(0usize, [0.0f32; 3]); bands.len()];
    for z in -land.span..land.span {
        for x in -land.span..land.span {
            if land.biome(x, z) == Biome::Water {
                continue;
            }
            let over = (land.height(x, z) - SEA) as f32 * CELL;
            let band = bands.iter().position(|&top| over < top).unwrap_or(bands.len() - 1);
            out[band].0 += 1;
            out[band].1[grade(land, x, z)] += 1.0;
        }
    }
    for (n, parts) in &mut out {
        *parts = parts.map(|p| p / (*n).max(1) as f32);
    }
    out
}

/// A column's ground: 0 easy, 1 steep, 2 cliff, by the most it rises or falls to the columns
/// two metres from it.
pub fn grade(land: &Land, x: i32, z: i32) -> usize {
    let (far, h) = (cells(2.0), land.height(x, z));
    let most = [(far, 0), (-far, 0), (0, far), (0, -far)].into_iter().map(|(dx, dz)| (land.height(x + dx, z + dz) - h).abs()).max().unwrap_or(0);
    if most <= cells(0.75) { 0 } else if most <= cells(2.0) { 1 } else { 2 }
}
