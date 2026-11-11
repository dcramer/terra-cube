//! Chunk meshing: one quad per visible face, with ambient occlusion baked into vertex colour,
//! the block's colour in it too, and a place in the atlas of grains (`texture`) for the paint.
//! Not greedy yet: see docs/engine.md for the order of meshing work.

use crate::texture::{Grain, MEAN, SPAN};
use crate::{Block, CELL, CHUNK, Chunk, SPLIT, World};
use glam::IVec3;

#[derive(Default)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    /// Where each corner is in the atlas of grains (`texture`).
    pub uvs: Vec<[f32; 2]>,
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

/// Where a corner of a face is in the atlas. `cube` is the least corner of the cube the face
/// belongs to and `corner` the face's corner, both in units of which there are `per` to a
/// cell (1 for cells, 5 for bits). The picture lies flat on tops and stands upright on sides,
/// and runs on unbroken from one cube to the next.
fn uv(grain: Grain, n: IVec3, cube: IVec3, corner: IVec3, per: i32) -> [f32; 2] {
    let along = |c: i32, p: i32| (c.rem_euclid(SPAN * per) + (p - c)) as f32 / per as f32;
    if n.y != 0 {
        grain.uv(along(cube.x, corner.x), along(cube.z, corner.z))
    } else {
        let across = if n.x != 0 { along(cube.z, corner.z) } else { along(cube.x, corner.x) };
        grain.uv(across, SPAN as f32 - along(cube.y, corner.y))
    }
}

/// How much one cube differs in shade from the next: little, so a wall is its grain and not a
/// chequer of cubes.
fn shade(p: IVec3) -> f32 {
    (0.96 + 0.08 * hash(p)) / MEAN
}

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
                if block == Block::Broken {
                    bits(world, cell, &mut m);
                    continue;
                }
                if block.sprig() {
                    sprig(cell, block, &mut m);
                    continue;
                }
                let (top, side) = block.colors();
                let mut tint = shade(cell);
                let mut warm = 0.0;
                // Water hides what is under it by how much of it there is: a ford shows its
                // bed, the sea does not.
                let depth = if liquid { (1..=crate::cells(4.0)).take_while(|d| near.get(local - IVec3::Y * *d) == Block::Water).count() } else { 0 };
                let alpha = if liquid { 0.42 + depth as f32 * CELL * 0.14 } else { 1.0 };
                if matches!(block, Block::Grass | Block::Turf | Block::Moss | Block::Litter) || block.foliage() {
                    // Patches: lighter and yellower here, deeper there.
                    let drift = patch(cell) - 0.5;
                    tint *= 1.0 + drift * 0.35;
                    warm = drift * 0.25;
                }
                if block.foliage() {
                    // A crown has depth: leaves under leaves are in their shade, the ones on
                    // top catch the sun. Without it a tree is one flat green.
                    let over = (1..=3).filter(|up| near.get(local + IVec3::Y * *up).foliage()).count();
                    tint *= [1.14, 0.94, 0.8, 0.68][over];
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
                    let grain = Grain::of(block, n.y != 0);
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
                        m.uvs.push(uv(grain, n, cell, corner, 1));
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

/// The faces of one broken cell: each solid bit as a little cube, by the same rules as a
/// cell. Slower a face than whole cells (every neighbour is looked up in the world), which is
/// fine while broken cells are few.
fn bits(world: &World, cell: IVec3, m: &mut MeshData) {
    let Some(data) = world.bits.get(&cell) else { return };
    // A neighbour inside this cell is read from it; only the bits at its faces ask the world.
    let near = |at: IVec3| {
        let local = at - cell * SPLIT;
        if local.cmpge(IVec3::ZERO).all() && local.cmplt(IVec3::splat(SPLIT)).all() { data[(local.x + local.z * SPLIT + local.y * SPLIT * SPLIT) as usize] } else { world.bit(at) }
    };
    for (i, &block) in data.iter().enumerate() {
        if !block.solid() {
            continue;
        }
        let i = i as i32;
        let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
        let (top, side) = block.colors();
        let tint = shade(bit);
        for (n, u, v) in FACES {
            if near(bit + n).solid() {
                continue;
            }
            let base = if n == IVec3::Y { top } else { side };
            let grain = Grain::of(block, n.y != 0);
            let start = m.positions.len() as u32;
            let mut ao = [0.0f32; 4];
            for (i, (su, sv)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
                let corner = bit + n.max(IVec3::ZERO) + u * su + v * sv;
                let (du, dv) = (u * (su * 2 - 1), v * (sv * 2 - 1));
                let occ = |d: IVec3| near(bit + n + d).solid() as i32;
                let (a, b, c) = (occ(du), occ(dv), occ(du + dv));
                let level = if a + b == 2 { 0 } else { 3 - (a + b + c) };
                ao[i] = 0.45 + 0.55 * level as f32 / 3.0;
                let k = ao[i] * tint;
                m.positions.push((corner.as_vec3() * CELL / SPLIT as f32).to_array());
                m.normals.push(n.as_vec3().to_array());
                m.uvs.push(uv(grain, n, bit, corner, SPLIT));
                m.colors.push([base[0] * k, base[1] * k, base[2] * k, 1.0]);
            }
            if ao[0] + ao[2] > ao[1] + ao[3] {
                m.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
            } else {
                m.indices.extend([start + 1, start + 2, start + 3, start + 1, start + 3, start]);
            }
        }
    }
}

/// One box, in bits from a cell's corner: five faces (not its underside), one colour.
fn blade(m: &mut MeshData, cell: IVec3, lo: IVec3, hi: IVec3, colour: [f32; 3]) {
    let at = |p: IVec3| ((cell * SPLIT + p).as_vec3() * CELL / SPLIT as f32).to_array();
    for (n, u, v) in FACES {
        if n == IVec3::NEG_Y {
            continue;
        }
        let size = hi - lo;
        // The face's corner on this side of the box, then along its two axes.
        let from = lo + n.max(IVec3::ZERO) * size;
        let start = m.positions.len() as u32;
        let shade = if n == IVec3::Y { 1.0 } else { 0.78 + 0.12 * (n.x + n.z) as f32 };
        for (su, sv) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
            m.positions.push(at(from + u * size * su + v * size * sv));
            m.normals.push(n.as_vec3().to_array());
            m.uvs.push(Grain::Plain.uv(SPAN as f32 * 0.5, SPAN as f32 * 0.5));
            m.colors.push([colour[0] * shade / MEAN, colour[1] * shade / MEAN, colour[2] * shade / MEAN, 1.0]);
        }
        m.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
    }
}

/// Ground cover in one cell: a few thin blades a bit across, of heights and places that
/// come from where the cell is. A flower is a stem with a head; reeds stand the cell's
/// height.
fn sprig(cell: IVec3, block: Block, m: &mut MeshData) {
    let (top, side) = block.colors();
    let roll = |n: i32| hash(cell * 7 + IVec3::splat(n));
    if block == Block::Mushroom {
        // A pale stem and a cap three bits across, one or two to a cell.
        for i in 0..1 + (roll(0) * 2.0) as i32 {
            let (x, z) = (1 + (roll(i * 3 + 1) * 3.0) as i32, 1 + (roll(i * 3 + 2) * 3.0) as i32);
            let tall = 1 + (roll(i + 9) * 2.0) as i32;
            blade(m, cell, IVec3::new(x, 0, z), IVec3::new(x + 1, tall, z + 1), side);
            blade(m, cell, IVec3::new(x - 1, tall, z - 1).max(IVec3::ZERO), IVec3::new(x + 2, tall + 1, z + 2).min(IVec3::splat(SPLIT)), top);
        }
        return;
    }
    if block == Block::Fern {
        // Fronds: low wide slabs fanning out from the middle, each a little higher than the last.
        for i in 0..4 {
            let (dx, dz) = [(1, 0), (-1, 0), (0, 1), (0, -1)][i as usize];
            let tall = 1 + (roll(i + 3) * 3.0) as i32;
            let tint = 0.85 + 0.3 * roll(i + 20);
            let (lo, hi) = (IVec3::new(2 + dx.min(0) * 2, tall - 1, 2 + dz.min(0) * 2), IVec3::new(3 + dx.max(0) * 2, tall, 3 + dz.max(0) * 2));
            blade(m, cell, lo, hi, top.map(|c| c * tint));
        }
        blade(m, cell, IVec3::new(2, 0, 2), IVec3::new(3, 2, 3), side);
        return;
    }
    let blades = if block == Block::Flower { 2 } else { 3 + (roll(0) * 3.0) as i32 };
    for i in 0..blades {
        let (x, z) = ((roll(i * 3 + 1) * SPLIT as f32) as i32 % SPLIT, (roll(i * 3 + 2) * SPLIT as f32) as i32 % SPLIT);
        let tall = match block {
            Block::Reed => SPLIT,
            Block::Flower if i == 0 => 3,
            _ => 2 + (roll(i * 3 + 3) * 3.0) as i32,
        };
        let tint = 0.85 + 0.3 * roll(i + 20);
        blade(m, cell, IVec3::new(x, 0, z), IVec3::new(x + 1, tall, z + 1), side.map(|c| c * tint));
        if block == Block::Flower && i == 0 {
            // Its head: white, yellow or violet by where it grows.
            let head = [[0.85, 0.82, 0.74], top, [0.48, 0.30, 0.62]][(roll(40) * 3.0) as usize % 3];
            blade(m, cell, IVec3::new(x - 1, tall, z - 1).max(IVec3::ZERO), IVec3::new(x + 2, tall + 1, z + 2).min(IVec3::splat(SPLIT)), head);
        }
    }
}

/// A chunk seen from far off: its cells taken `step` at a time each way, so a mesh a quarter
/// or a sixteenth the size. A group of cells is solid if any of them is, and is the block
/// highest in it, so distant ground has no holes and trees keep their bulk; ground cover and
/// the fine shape of broken cells are left out. Water is meshed apart, as up close: a group
/// is water if any of it is.
pub fn mesh_chunk_far(world: &World, chunk: IVec3, liquid: bool, step: i32) -> MeshData {
    let mut m = MeshData::default();
    if !world.chunks.contains_key(&chunk) {
        return m;
    }
    let mut near = Near([None; 27]);
    for (i, slot) in near.0.iter_mut().enumerate() {
        let i = i as i32;
        *slot = world.chunks.get(&(chunk + IVec3::new(i % 3 - 1, i / 9 - 1, i / 3 % 3 - 1)));
    }
    let n = CHUNK / step;
    let side = (n + 2) as usize;
    // Every group in the chunk and one beyond each face, worked out once.
    let mut groups = vec![Block::Air; side * side * side];
    let at = |s: IVec3| ((s.x + 1) + (s.z + 1) * (n + 2) + (s.y + 1) * (n + 2) * (n + 2)) as usize;
    for sy in -1..=n {
        for sz in -1..=n {
            for sx in -1..=n {
                let s = IVec3::new(sx, sy, sz);
                let mut found = Block::Air;
                'group: for y in (0..step).rev() {
                    for z in 0..step {
                        for x in 0..step {
                            let block = near.get(s * step + IVec3::new(x, y, z));
                            let wanted = if liquid { block == Block::Water } else { block.solid() };
                            if wanted {
                                // The bits of a broken cell are not looked at from here: stone.
                                found = if block == Block::Broken { Block::Stone } else { block };
                                break 'group;
                            }
                        }
                    }
                }
                groups[at(s)] = found;
            }
        }
    }
    let origin = chunk * CHUNK;
    for sy in 0..n {
        for sz in 0..n {
            for sx in 0..n {
                let s = IVec3::new(sx, sy, sz);
                let block = groups[at(s)];
                if block == Block::Air {
                    continue;
                }
                let cell = origin + s * step;
                let (top, side_colour) = block.colors();
                let mut tint = shade(cell);
                let mut warm = 0.0;
                if matches!(block, Block::Grass | Block::Turf | Block::Moss | Block::Litter) || block.foliage() {
                    let drift = patch(cell) - 0.5;
                    tint *= 1.0 + drift * 0.35;
                    warm = drift * 0.25;
                }
                for (normal, u, v) in FACES {
                    if (cell.y == 0 && normal == IVec3::NEG_Y) || groups[at(s + normal)] != Block::Air {
                        continue;
                    }
                    let base = if normal == IVec3::Y { top } else { side_colour };
                    let grain = Grain::of(block, normal.y != 0);
                    let start = m.positions.len() as u32;
                    let mut ao = [0.0f32; 4];
                    for (i, (su, sv)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
                        let corner = cell + (normal.max(IVec3::ZERO) + u * su + v * sv) * step;
                        let (du, dv) = (u * (su * 2 - 1), v * (sv * 2 - 1));
                        let occ = |d: IVec3| (!liquid && groups[at(s + normal + d)] != Block::Air) as i32;
                        let (a, b, c) = (occ(du), occ(dv), occ(du + dv));
                        let level = if a + b == 2 { 0 } else { 3 - (a + b + c) };
                        ao[i] = 0.45 + 0.55 * level as f32 / 3.0;
                        let k = ao[i] * tint;
                        m.positions.push((corner.as_vec3() * CELL).to_array());
                        m.normals.push(normal.as_vec3().to_array());
                        m.uvs.push(uv(grain, normal, cell, corner, 1));
                        m.colors.push([base[0] * k * (1.0 + warm), base[1] * k, base[2] * k * (1.0 - warm), if liquid { 0.8 } else { 1.0 }]);
                    }
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
