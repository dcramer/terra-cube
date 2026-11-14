//! What lies at rest as itself ([`Rested`]): put into the world as where it is, taken out
//! again, heaved along, and let go to fall as itself.

use super::*;

/// A piece come to rest that is still itself: its own cells, as it was made, and how it lies
/// (a point of it that was at `p` is at pivot + shift + turn (p - pivot), as a [`Piece`]'s).
/// The world is a grid, and what is put into it is asked cell by cell where it is: a log
/// lying across the grid came out as steps, or was turned to lie along it. So what should
/// keep its shape is not put into the world. It stays as it is, and whoever draws it draws
/// it so; the world gets only the cells it is in, of a kind that is not drawn
/// ([`Block::whole`]), so that it stops a body, bears what is put on it and falls when what
/// is under it goes. (The developer, 2026-11-13: "one issue we have in the game right now
/// things cant have angles ... it'd be nice if those logs could just lay at an angle ...
/// we still wanna rely on voxels but forcing everything to be exact on the grid is not
/// amazing visually".)
#[derive(Clone)]
pub struct Rested {
    pub id: u32,
    pub cells: HashMap<IVec3, Part>,
    pub pivot: Vec3,
    pub shift: Vec3,
    pub turn: Quat,
    /// The cells of the world it is in.
    pub(super) placed: Vec<IVec3>,
}

impl Rested {
    pub fn now(&self, was: Vec3) -> Vec3 {
        self.pivot + self.shift + self.turn * (was - self.pivot)
    }

    /// Where in it, as it was made, a place in the world is.
    pub fn was(&self, now: Vec3) -> Vec3 {
        self.pivot + self.turn.inverse() * (now - self.pivot - self.shift)
    }

    /// Whether a place in the world is in it as it lies.
    pub fn has(&self, now: Vec3) -> bool {
        let was = self.was(now);
        match self.cells.get(&(was / CELL).floor().as_ivec3()) {
            Some(Part::Whole(_)) => true,
            Some(Part::Bits(bits)) => {
                let l = ((was / BIT).floor().as_ivec3()).rem_euclid(IVec3::splat(SPLIT));
                bits[(l.x + l.z * SPLIT + l.y * SPLIT * SPLIT) as usize].solid()
            }
            None => false,
        }
    }
}

/// As much of a piece may lie in something else when it comes to rest, of 1 ([`sunk`]).
pub(super) const SUNK: f32 = 0.03;

/// How much of a piece is where something already is, of 1: at every 5 cm of it.
pub(super) fn sunk(world: &World, piece: &Piece) -> f32 {
    let mut inside = 0;
    for cell in piece.cells.keys() {
        for i in 0..SPLIT * SPLIT * SPLIT {
            let was = (*cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT)).as_vec3() * BIT + BIT / 2.0;
            let b = world.bit((piece.now(was) / BIT).floor().as_ivec3());
            inside += (bears(b) || b == Block::Rock) as u32;
        }
    }
    inside as f32 / (piece.cells.len() as i32 * SPLIT * SPLIT * SPLIT).max(1) as f32
}

/// Puts into the world the cells a piece that keeps its shape is in, where there is room:
/// of what it is, not drawn. Gives them.
pub(super) fn rest(world: &mut World, piece: &Piece) -> Vec<IVec3> {
    let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
    for cell in piece.cells.keys() {
        for corner in 0..8 {
            let p = piece.now((*cell + IVec3::new(corner & 1, corner >> 1 & 1, corner >> 2 & 1)).as_vec3() * CELL);
            (lo, hi) = (lo.min(p), hi.max(p));
        }
    }
    let (lo, hi) = ((lo / CELL).floor().as_ivec3(), (hi / CELL).floor().as_ivec3());
    let mut placed = Vec::new();
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let cell = IVec3::new(x, y, z);
                // To the bit, 5 cm: what lies at an angle, asked cell by cell, stood a body as
                // much as 13 cm over or in the log that is drawn, and a fifth of it bore
                // nothing (`cargo world lying`). A cell wholly in it is one cell.
                let mut bits = [Block::Air; (SPLIT * SPLIT * SPLIT) as usize];
                let mut full = 0;
                for (i, slot) in bits.iter_mut().enumerate() {
                    let i = i as i32;
                    let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
                    let was = piece.was((bit.as_vec3() + 0.5) * BIT);
                    *slot = match piece.cells.get(&(was / CELL).floor().as_ivec3()) {
                        Some(Part::Whole(b)) => *b,
                        Some(Part::Bits(bits)) => {
                            let l = ((was / BIT).floor().as_ivec3()).rem_euclid(IVec3::splat(SPLIT));
                            bits[(l.x + l.z * SPLIT + l.y * SPLIT * SPLIT) as usize]
                        }
                        None => Block::Air,
                    };
                    full += slot.solid() as i32;
                }
                if full == SPLIT * SPLIT * SPLIT && bits.iter().all(|b| *b == bits[0]) && room(world, cell) {
                    world.set(cell, bits[0].veiled());
                    placed.push(cell);
                } else if full > 0 {
                    let mut any = false;
                    for (i, block) in bits.iter().enumerate().filter(|b| b.1.solid()) {
                        let i = i as i32;
                        let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
                        // (Leaf gives way to it, as to a cell of it: where only what was free
                        // of a bush's leaf was taken, one bit of a log lay alone in the bush.)
                        let there = world.bit(bit);
                        if !there.solid() || there.foliage() {
                            world.set_bit(bit, block.veiled());
                            any = true;
                        }
                    }
                    if any {
                        placed.push(cell);
                    }
                }
            }
        }
    }
    placed
}

impl Loose {
    /// Takes out of the world what a thing at rest as itself had in it ([`rest`]): its own
    /// bits of each cell it is in, and no other's. Two things that lie against each other
    /// have bits in the same cells, and a cell is said to be one's (`owner`): what is left
    /// in a cell is then the other's, and the cell is said to be its. (All of a cell went
    /// with whichever was taken up first, and the other was left with a hole in it.)
    pub(super) fn unrest(&mut self, world: &mut World, rested: &Rested, changed: &mut HashSet<IVec3>) {
        for cell in &rested.placed {
            let (mut gone, mut left) = (false, false);
            match world.get(*cell) {
                Block::Broken => {
                    for i in 0..SPLIT * SPLIT * SPLIT {
                        let bit = *cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
                        if !world.bit(bit).whole() {
                            continue;
                        }
                        if rested.has((bit.as_vec3() + 0.5) * BIT) {
                            world.set_bit(bit, Block::Air);
                            gone = true;
                        } else {
                            left = true;
                        }
                    }
                }
                b if b.whole() => {
                    world.set(*cell, Block::Air);
                    gone = true;
                }
                _ => {}
            }
            if gone {
                changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            }
            if self.owner.get(cell) == Some(&rested.id) {
                match self.rested.iter().find(|r| left && r.id != rested.id && r.placed.contains(cell)) {
                    Some(other) => self.owner.insert(*cell, other.id),
                    None => self.owner.remove(cell),
                };
            }
        }
    }

    /// A place in the nearest thing at rest within reach that is light enough to heave: for
    /// a script, which has no eye to look at one with.
    pub fn shovable(&self, from: Vec3, reach: f32) -> Option<Vec3> {
        let light: HashSet<u32> = self.rested.iter().filter(|r| mass(&r.cells) <= crate::matter::SHOVED).map(|r| r.id).collect();
        self.owner.iter().filter(|(_, id)| light.contains(id)).map(|(cell, _)| (cell.as_vec3() + 0.5) * CELL).filter(|p| p.distance(from) < reach).min_by(|a, b| a.distance(from).total_cmp(&b.distance(from)).then((a.x, a.y, a.z).partial_cmp(&(b.x, b.y, b.z)).unwrap_or(std::cmp::Ordering::Equal)))
    }

    /// A thing at rest as itself, at `at`, heaved a hand's breadth the way of `push` with
    /// empty hands, if it is light enough ([`crate::matter::SHOVED`]); it comes to rest
    /// again as anything let go does. Gives the chunks changed, if it moved anything.
    /// (By hand and not by walking into it: what is light enough to shove is low enough to
    /// step over, so nobody is ever stopped by it.)
    pub fn shove(&mut self, world: &mut World, at: Vec3, push: Vec3) -> Option<HashSet<IVec3>> {
        let way = push.with_y(0.0).try_normalize()?;
        let id = self.owner.get(&(at / CELL).floor().as_ivec3()).copied()?;
        let n = self.rested.iter().position(|r| r.id == id).filter(|n| mass(&self.rested[*n].cells) <= crate::matter::SHOVED)?;
        self.journal.push((self.steps, Edit::Shove { at, push }));
        let rested = self.rested.remove(n);
        let mut changed = HashSet::new();
        self.unrest(world, &rested, &mut changed);
        let mut piece = self.made(rested.cells, rested.pivot, rested.turn);
        (piece.id, piece.shift, piece.vel) = (rested.id, rested.shift + way * 0.1 + Vec3::Y * BIT, way * 1.6 + Vec3::Y * 1.0);
        trace(format_args!("shove: piece {} of {:.0} kg heaved", piece.id, piece.mass));
        self.pieces.push(piece);
        Some(changed)
    }

    /// What lay at rest as itself is loose again, as itself: out of the world, and a piece.
    pub(super) fn rouse(&mut self, world: &mut World, id: u32) -> HashSet<IVec3> {
        let mut changed = HashSet::new();
        let Some(n) = self.rested.iter().position(|r| r.id == id) else { return changed };
        let rested = self.rested.remove(n);
        self.unrest(world, &rested, &mut changed);
        let mut piece = self.made(rested.cells, rested.pivot, rested.turn);
        (piece.id, piece.shift) = (rested.id, rested.shift);
        self.pieces.push(piece);
        changed
    }

    /// A piece made, not cut loose: these cells, turned so about a point, to fall from
    /// there and lie as itself where it comes to rest ([`Rested`]).
    pub(super) fn made(&mut self, cells: HashMap<IVec3, Part>, pivot: Vec3, turn: Quat) -> Piece {
        let (low, high) = cells.keys().fold((i32::MAX, i32::MIN), |(l, h), c| (l.min(c.y), h.max(c.y)));
        let mut feel: Vec<Vec3> = cells.keys().map(|c| (c.as_vec3() + 0.5) * CELL).collect();
        feel.sort_by(|a, b| (a.x, a.y, a.z).partial_cmp(&(b.x, b.y, b.z)).unwrap_or(std::cmp::Ordering::Equal));
        self.next += 1;
        Piece { id: self.next, mass: mass(&cells), cells, pivot, shift: Vec3::ZERO, turn, vel: Vec3::ZERO, over: None, spin: Vec3::ZERO, brittle: false, hunk: false, thing: None, base: Quat::IDENTITY, settling: false, spare: (pivot, 0.0), tall: (high - low + 1) as f32 * CELL, feel, age: 0.0, rolls: 0, keeps: true, tips: None }
    }
}
