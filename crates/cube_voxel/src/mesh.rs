//! Chunk meshing: one quad per visible face, with ambient occlusion baked into vertex colour
//! and a little per-cell colour variation so flat ground does not read as one tile.
//! Not greedy yet: see docs/engine.md for the order of meshing work.

use crate::{Block, CELL, CHUNK, World};
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

/// The mesh of one chunk, in metres, positioned in world space.
pub fn mesh_chunk(world: &World, chunk: IVec3) -> MeshData {
    let mut m = MeshData::default();
    let Some(data) = world.chunks.get(&chunk) else { return m };
    let origin = chunk * CHUNK;
    for y in 0..CHUNK {
        for z in 0..CHUNK {
            for x in 0..CHUNK {
                let local = IVec3::new(x, y, z);
                let block = data.get(local);
                if block == Block::Air {
                    continue;
                }
                let cell = origin + local;
                let (top, side) = block.colors();
                let tint = 0.9 + 0.2 * hash(cell);
                for (n, u, v) in FACES {
                    let other = world.get(cell + n);
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
                        let occ = |d: IVec3| world.get(cell + n + d).opaque() as i32;
                        let (a, b, c) = (occ(du), occ(dv), occ(du + dv));
                        let level = if a + b == 2 { 0 } else { 3 - (a + b + c) };
                        ao[i] = 0.45 + 0.55 * level as f32 / 3.0;
                        let k = ao[i] * tint;
                        m.positions.push((corner.as_vec3() * CELL).to_array());
                        m.normals.push(n.as_vec3().to_array());
                        m.colors.push([base[0] * k, base[1] * k, base[2] * k, if block == Block::Water { 0.75 } else { 1.0 }]);
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
