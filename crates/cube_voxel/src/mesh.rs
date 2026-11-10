//! Chunk meshing: one quad per visible face, with ambient occlusion baked into vertex colour
//! and a little per-cell colour variation so flat ground does not read as one tile.
//! Not greedy yet: see docs/engine.md for the order of meshing work.

use crate::{Block, CELL, CHUNK, Chunk, World};
use glam::IVec3;

#[derive(Default)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

/// A face: its normal, then its two in-plane axes (u, v) such that u × v = normal.
const FACES: [(IVec3, IVec3, IVec3); 6] = [
    (IVec3::X, IVec3::Y, IVec3::Z),
    (IVec3::NEG_X, IVec3::Z, IVec3::Y),
    (IVec3::Y, IVec3::Z, IVec3::X),
    (IVec3::NEG_Y, IVec3::X, IVec3::Z),
    (IVec3::Z, IVec3::X, IVec3::Y),
    (IVec3::NEG_Z, IVec3::Y, IVec3::X),
];

fn hash(p: IVec3) -> f32 {
    let mut h = (p.x as u32).wrapping_mul(0x9E3779B1) ^ (p.y as u32).wrapping_mul(0x85EBCA77) ^ (p.z as u32).wrapping_mul(0xC2B2AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// A slow drift of colour over the ground, 0 to 1: patches some metres across, so a meadow is
/// not one green.
fn patch(p: IVec3) -> f32 {
    let at = |x: i32, z: i32| hash(IVec3::new(x, 77, z));
    // Six metres from one patch to the next.
    let n = crate::cells(6.0);
    let (gx, gz) = (p.x.div_euclid(n), p.z.div_euclid(n));
    let (fx, fz) = (p.x.rem_euclid(n) as f32 / n as f32, p.z.rem_euclid(n) as f32 / n as f32);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let a = at(gx, gz) * (1.0 - sx) + at(gx + 1, gz) * sx;
    let b = at(gx, gz + 1) * (1.0 - sx) + at(gx + 1, gz + 1) * sx;
    a * (1.0 - sz) + b * sz
}

/// The chunk and the 26 round it, so a cell's neighbours are read without looking the chunk
/// up each time: meshing reads about thirty cells for every face it draws.
struct Near<'a>([Option<&'a Chunk>; 27]);

impl Near<'_> {
    /// The block at a position local to the middle chunk, from -32 to 63 on each axis.
    #[inline]
    fn get(&self, local: IVec3) -> Block {
        let c = local.div_euclid(IVec3::splat(CHUNK));
        match self.0[((c.x + 1) + (c.z + 1) * 3 + (c.y + 1) * 9) as usize] {
            Some(chunk) => chunk.get(local - c * CHUNK),
            None => Block::Air,
        }
    }
}

/// The mesh of one chunk, in metres, positioned in world space: its solid blocks, or with
/// `liquid` its water, which is drawn apart because it is seen through.
pub fn mesh_chunk(world: &World, chunk: IVec3, liquid: bool) -> MeshData {
    let mut m = MeshData::default();
    let Some(data) = world.chunks.get(&chunk) else { return m };
    let mut near = Near([None; 27]);
    for (i, slot) in near.0.iter_mut().enumerate() {
        let i = i as i32;
        *slot = world.chunks.get(&(chunk + IVec3::new(i % 3 - 1, i / 9 - 1, i / 3 % 3 - 1)));
    }
    let origin = chunk * CHUNK;
    for y in 0..CHUNK {
        for z in 0..CHUNK {
            for x in 0..CHUNK {
                let local = IVec3::new(x, y, z);
                let block = data.get(local);
                if block == Block::Air || (block == Block::Water) != liquid {
                    continue;
                }
                let cell = origin + local;
                let (top, side) = block.colors();
                let mut tint = 0.9 + 0.2 * hash(cell);
                let mut warm = 0.0;
                // Water hides what is under it by how much of it there is: a ford shows its
                // bed, the sea does not.
                let depth = if liquid { (1..=crate::cells(4.0)).take_while(|d| near.get(local - IVec3::Y * *d) == Block::Water).count() } else { 0 };
                let alpha = if liquid { 0.42 + depth as f32 * CELL * 0.14 } else { 1.0 };
                if matches!(block, Block::Grass | Block::Turf | Block::Leaves) {
                    // Patches: lighter and yellower here, deeper there.
                    let drift = patch(cell) - 0.5;
                    tint *= 1.0 + drift * 0.35;
                    warm = drift * 0.25;
                }
                for (n, u, v) in FACES {
                    // The underside of the world is never seen.
                    if cell.y == 0 && n == IVec3::NEG_Y {
                        continue;
                    }
                    let other = near.get(local + n);
                    let hidden = if block == Block::Water { other != Block::Air } else { other.opaque() };
                    if hidden {
                        continue;
                    }
                    let base = if n == IVec3::Y { top } else { side };
                    let start = m.positions.len() as u32;
                    let mut ao = [0.0f32; 4];
                    for (i, (su, sv)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
                        let corner = cell + n.max(IVec3::ZERO) + u * su + v * sv;
                        let (du, dv) = (u * (su * 2 - 1), v * (sv * 2 - 1));
                        let occ = |d: IVec3| near.get(local + n + d).opaque() as i32;
                        let (a, b, c) = (occ(du), occ(dv), occ(du + dv));
                        let level = if a + b == 2 { 0 } else { 3 - (a + b + c) };
                        ao[i] = 0.45 + 0.55 * level as f32 / 3.0;
                        let k = ao[i] * tint;
                        m.positions.push((corner.as_vec3() * CELL).to_array());
                        m.normals.push(n.as_vec3().to_array());
                        m.colors.push([base[0] * k * (1.0 + warm), base[1] * k, base[2] * k * (1.0 - warm), alpha]);
                    }
                    // Split the quad along the diagonal that keeps the AO gradient smooth.
                    if ao[0] + ao[2] > ao[1] + ao[3] {
                        m.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
                    } else {
                        m.indices.extend([start + 1, start + 2, start + 3, start + 1, start + 3, start]);
                    }
                }
            }
        }
    }
    m
}
