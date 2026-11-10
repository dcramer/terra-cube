//! A small deterministic test world: rolling hills with a pond, a beach, rock and a snowy rise.
//! Stands in for real world generation, which belongs to the simulation.

use crate::{Block, CHUNK, World};
use glam::IVec3;

fn hash2(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E3779B1) ^ (z as u32).wrapping_mul(0x85EBCA77) ^ seed.wrapping_mul(0xC2B2AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B3C6D);
    h ^= h >> 12;
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

fn fbm(x: f32, z: f32, seed: u32) -> f32 {
    (0..4).map(|o| value_noise(x * (1 << o) as f32, z * (1 << o) as f32, seed + o) / (1 << o) as f32).sum::<f32>() / 1.875
}

/// Water level, in cells.
pub const SEA: i32 = 10;

/// The surface height of a column, in cells. Flat near the origin, where creatures are shown.
pub fn height(x: i32, z: i32, seed: u32) -> i32 {
    let (fx, fz) = (x as f32, z as f32);
    let rolling = fbm(fx / 48.0, fz / 48.0, seed) * 22.0 - 4.0;
    let mountain = ((fx + fz) / 90.0 - 0.9).clamp(0.0, 1.0).powi(2) * 40.0;
    let pond = (1.0 - ((fx + 44.0).powi(2) + (fz - 30.0).powi(2)).sqrt() / 26.0).clamp(0.0, 1.0) * 24.0;
    let wild = (rolling + mountain - pond).round();
    let flat = 1.0 - ((fx * fx + fz * fz).sqrt() / 40.0 - 0.6).clamp(0.0, 1.0);
    ((SEA as f32 + 4.0 + wild * (1.0 - flat)).round() as i32).max(3)
}

/// A world `radius` chunks out from the origin in x and z.
pub fn generate(seed: u32, radius: i32) -> World {
    let mut world = World::default();
    let span = radius * CHUNK;
    for z in -span..span {
        for x in -span..span {
            let h = height(x, z, seed);
            for y in 0..h.max(SEA) {
                let depth = h - 1 - y;
                let block = if y >= h {
                    Block::Water
                } else if h > SEA + 34 && depth < 2 {
                    Block::Snow
                } else if h > SEA + 24 || depth > 4 {
                    Block::Stone
                } else if h <= SEA + 1 {
                    Block::Sand
                } else if depth == 0 {
                    Block::Grass
                } else {
                    Block::Dirt
                };
                world.set(IVec3::new(x, y, z), block);
            }
        }
    }
    world
}
