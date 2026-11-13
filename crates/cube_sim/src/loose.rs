//! What a cut leaves standing on nothing comes loose and falls. A world that can be cut
//! anywhere must answer for what was resting on what was cut: a tree whose trunk is cut
//! through goes over, an overhang whose root is dug out drops, and each lies afterwards
//! where it fell, in cells again, to be cut again.
//!
//! How, in order:
//! 1. **Cut** ([`Loose::cut`]): a ball is dug out of the world, to the bit. A tree or a
//!    boulder it bites into is first made *wrought* (`Block::wrought`): they are drawn as
//!    whole shapes while they stand untouched (`cube_game::flora`), and a shape cannot show a
//!    notch or lie where it fell. Which limbs and leaf are the same tree is what `plants`
//!    noted as it grew them (`Limb::tree`): trees in a wood touch, and by the cells alone
//!    one tree cannot be told from the next. All of it becomes cells that are drawn as
//!    cells, and whoever draws shapes is told which ([`Cut::trees`], [`Cut::stones`]) so as
//!    to stop drawing it.
//! 2. **What is left unsupported** is looked for round the cut: from each solid cell beside
//!    it, through solid neighbours. Reaching more than [`HELD`] cells, it is held (the land
//!    is one great piece, and so is anything that stands on it). Fewer, and closed, it is
//!    loose. Leaves bear nothing, and nor does anything still a whole shape (an untouched
//!    tree is scenery: nothing hangs from it): two trees that touch do not hold each other
//!    up, and a loose piece takes with it the leaves that are its own.
//! 3. **A piece** ([`Piece`]) is those cells taken out of the world and kept as they were,
//!    moved as one body: tall on a small base (a tree) it goes over about its foot, away
//!    from the cut; anything else drops.
//! 4. **Landing**: when any of it meets the world it is put back, cell for cell where each
//!    now lies (turned as it fell), and the piece is gone. Stone and earth that come down
//!    hard do not land whole: they **shatter** into lumps that are thrown out, turn as
//!    they go, and land each on its own, as rubble. Wood does not shatter.
//!
//! **A tree's fall is staged** (the developer, 2026-11-12: what is cut down should behave in
//! a specific way, so it looks interesting): it goes the way it leans, more than the way it
//! was pushed; it hangs a moment and then goes faster and faster; and as its crown strikes,
//! its limbs break from it and are thrown a little way, each to lie on its own, while the
//! trunk comes down whole along the line it fell.
//!
//! No engine types and nothing random: the same cut fells the same tree. `cargo world fell`
//! judges it on a real tree with no window.

use crate::items::Stuff;
use crate::plants::Form;
use cube_voxel::{BIT, Bits, Block, CELL, SPLIT, World};
use glam::{IVec3, Quat, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};

/// How many cells a piece may be and still be looked through to its end: what reaches
/// further is held to be standing on the land. A great oak with its crown is some 12,000.
pub const HELD: usize = 30_000;
/// And how many cells of the land itself (soil, sand, stone in its beds): a lump of earth
/// bigger than this, some sixty cubic metres, is the land. Far fewer than [`HELD`], since
/// every spadeful is a search that ends here: at 30,000 a spadeful took 13 ms.
pub const LAND: usize = 4_000;
/// How far from its wood a piece's own leaves are gathered, cells.
const LEAFY: u8 = 10;
/// How fast stone or earth must come down to break up, metres a second: a drop of about
/// half a metre. And how fast a tree's top must be going, as it strikes, for its limbs to
/// break from it.
const SHATTERS: f32 = 4.5;
const CRASHES: f32 = 8.0;
/// Metres a second a second that a loose thing falls at (as a body does: `body`).
const FALL: f32 = 22.0;

/// One cell of a piece: whole, or its bits.
#[derive(Clone)]
pub enum Part {
    Whole(Block),
    Bits(Box<Bits>),
}

/// Something loose: its cells where they were in the world (cell positions), and how it has
/// moved since. A point of it that was at `p` (metres) is now at [`Piece::now`]`(p)`.
#[derive(Clone)]
pub struct Piece {
    pub id: u32,
    pub cells: HashMap<IVec3, Part>,
    /// What it turns about (where it stood on the ground, for what goes over), how far that
    /// has moved, and how it is turned.
    pub pivot: Vec3,
    pub shift: Vec3,
    pub turn: Quat,
    vel: Vec3,
    /// Going over like a tree: which way, how far it has tipped (radians) and how fast.
    over: Option<(Vec3, f32, f32)>,
    /// How it turns as it flies, radians a second about each axis (a lump thrown from
    /// something shattered).
    spin: Vec3,
    /// It will break up if it lands hard: stone or earth, not yet broken.
    brittle: bool,
    /// How tall it is, metres, and the places of it that are tried against the world.
    tall: f32,
    feel: Vec<Vec3>,
    /// Seconds it has been loose.
    pub age: f32,
}

impl Piece {
    pub fn now(&self, was: Vec3) -> Vec3 {
        self.pivot + self.shift + self.turn * (was - self.pivot)
    }
    fn was(&self, now: Vec3) -> Vec3 {
        self.pivot + self.turn.inverse() * (now - self.pivot - self.shift)
    }
}

/// What has come down: where, how many cells of it were put back, and how many it had.
#[derive(Clone, Copy, Debug)]
pub struct Landed {
    pub at: Vec3,
    pub cells: usize,
    pub put: usize,
    /// Metres a second it was going.
    pub speed: f32,
}

/// What a cut did: the chunks that changed; and whatever whole shapes it bit into, which are
/// now cells drawn as cells: each tree (which of the forms given, and its number there) and
/// each boulder (which form, and which of its stones).
#[derive(Default)]
pub struct Cut {
    pub changed: HashSet<IVec3>,
    pub trees: Vec<(usize, u32)>,
    pub stones: Vec<(usize, usize)>,
    /// How many cells were made wrought.
    pub worked: usize,
    /// How much was cut away, in bits: of earth, of wood and of stone.
    pub taken: [u32; 3],
}

/// Everything loose in a world, and what has landed since it was last asked for.
#[derive(Default, Clone)]
pub struct Loose {
    pub pieces: Vec<Piece>,
    pub landed: Vec<Landed>,
    /// The same, once the simulation has seen who was under each (`Sim::step`): for whoever
    /// makes the sound of it.
    pub heard: Vec<Landed>,
    /// For whoever shows and sounds it, to take as they are noted: each cut (where, and how
    /// much of earth, wood and stone it took), and each thing that came loose (where it
    /// stood, how many cells, and whether it is going over like a tree).
    pub cuts: Vec<(Vec3, [u32; 3])>,
    pub begun: Vec<(Vec3, usize, bool)>,
    next: u32,
}

/// Bears weight: solid, not leaf, and not part of a whole shape still untouched.
fn bears(b: Block) -> bool {
    b.solid() && !b.foliage() && !b.shaped()
}

/// Whether a cell has anything in it that bears weight, or (with `leaf`) any leaf.
fn has(world: &World, cell: IVec3, leaf: bool) -> bool {
    let want = |b: Block| if leaf { b.foliage() && !b.shaped() } else { bears(b) };
    match world.get(cell) {
        Block::Broken => world.bits_at(cell).is_some_and(|bits| bits.iter().any(|b| want(*b))),
        whole => want(whole),
    }
}

/// Takes out of the world what of a ball is `stuff` (or all of it), to the bit. Says how many
/// bits went, of earth, wood and stone.
fn carve(world: &mut World, middle: Vec3, radius: f32, stuff: Option<Stuff>) -> [u32; 3] {
    let mut taken = [0; 3];
    let goes = |b: Block| b != Block::Air && b != Block::Water && stuff.is_none_or(|s| Stuff::of(b) == Some(s));
    let tally = |taken: &mut [u32; 3], b: Block, bits: u32| {
        if let Some(of) = Stuff::of(b).filter(|_| !b.foliage()) {
            taken[of as usize] += bits;
        }
    };
    let inside = |p: Vec3| p.distance_squared(middle) < radius * radius;
    let (lo, hi) = (((middle - radius) / CELL).floor().as_ivec3(), ((middle + radius) / CELL).floor().as_ivec3());
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let cell = IVec3::new(x, y, z);
                let was = world.get(cell);
                if was == Block::Air {
                    continue;
                }
                let all = (0..8).all(|i| inside((cell + IVec3::new(i & 1, i >> 1 & 1, i >> 2 & 1)).as_vec3() * CELL));
                if all && was != Block::Broken {
                    if goes(was) {
                        world.set(cell, Block::Air);
                        tally(&mut taken, was, (SPLIT * SPLIT * SPLIT) as u32);
                    }
                    continue;
                }
                for i in 0..SPLIT * SPLIT * SPLIT {
                    let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
                    if inside((bit.as_vec3() + 0.5) * BIT) {
                        let b = world.bit(bit);
                        if goes(b) {
                            world.set_bit(bit, Block::Air);
                            tally(&mut taken, b, 1);
                        }
                    }
                }
            }
        }
    }
    taken
}

/// How far a point is from the line between two others.
fn from_line(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let along = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
    p.distance(a + (b - a) * along)
}

/// Makes wrought every tree and boulder of `forms` that a ball reaches: all of each tree, its
/// every limb and its leaf, as `plants` grew it. Says which, and the chunks changed.
fn work(world: &mut World, middle: Vec3, radius: f32, forms: &[&Form], stuff: Option<Stuff>) -> Cut {
    let mut cut = Cut::default();
    let mut cells: HashSet<IVec3> = HashSet::new();
    let (trees, stones) = (stuff.is_none_or(|s| s == Stuff::Wood), stuff.is_none_or(|s| s == Stuff::Stone));
    for (n, form) in forms.iter().enumerate() {
        // (A rooted trunk swells to twice its thickness and more at its foot.)
        let reach = |l: &crate::plants::Limb| radius + l.ra.max(l.rb) * if l.roots > 0 { 2.6 } else { 1.0 } + CELL;
        let mut trees: Vec<u32> = form.limbs.iter().filter(|l| trees && from_line(middle, l.a, l.b) < reach(l)).map(|l| l.tree).collect();
        trees.sort_unstable();
        trees.dedup();
        for tree in trees {
            for limb in form.limbs.iter().filter(|l| l.tree == tree) {
                limb.fill(CELL, |c| {
                    cells.insert(c);
                });
            }
            for clump in form.clumps.iter().filter(|c| c.tree == tree) {
                let (lo, hi) = (((clump.middle - clump.radii) / CELL).floor().as_ivec3(), ((clump.middle + clump.radii) / CELL).floor().as_ivec3());
                for y in lo.y..=hi.y {
                    for z in lo.z..=hi.z {
                        for x in lo.x..=hi.x {
                            let c = IVec3::new(x, y, z);
                            if world.get(c).foliage() {
                                cells.insert(c);
                            }
                        }
                    }
                }
            }
            cut.trees.push((n, tree));
        }
        for (k, stone) in form.stones.iter().enumerate().filter(|(_, s)| stones && ((middle - s.middle) / (s.radii + radius + CELL)).length_squared() < 1.0) {
            let reach = Vec3::splat(stone.radii.max_element());
            let (lo, hi) = (((stone.middle - reach) / CELL).floor().as_ivec3(), ((stone.middle + reach) / CELL).floor().as_ivec3());
            for y in lo.y..=hi.y {
                for z in lo.z..=hi.z {
                    for x in lo.x..=hi.x {
                        cells.insert(IVec3::new(x, y, z));
                    }
                }
            }
            cut.stones.push((n, k));
        }
    }
    for cell in &cells {
        let was = world.get(*cell);
        if was == Block::Broken || was.shaped() {
            world.map(*cell, Block::wrought);
            cut.worked += 1;
            cut.changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        }
    }
    cut
}

const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];

/// Every cell round a cell: by a face, an edge or a corner. What bears is joined through any
/// of them: a limb a hand thick that climbs aslant passes from cell to cell by their edges,
/// and by faces alone half the limbs of a wood hang in the air unjoined.
fn round() -> impl Iterator<Item = IVec3> {
    (-1..=1).flat_map(|y| (-1..=1).flat_map(move |z| (-1..=1).map(move |x| IVec3::new(x, y, z)))).filter(|d| *d != IVec3::ZERO)
}

impl Loose {
    /// Digs a ball out of the world and looses what that leaves unsupported, pushed the way
    /// `push` points (the way whoever cut it was facing). Gives the chunks that changed.
    ///
    /// `forms` are the shapes of what grows round the cut, as `plants` noted them: a tree or
    /// a boulder among them that the ball reaches is made wrought first.
    ///
    /// Only `stuff` is cut, where one is given (what a tool cuts: an axe takes the wood out
    /// of the ball and leaves the earth); with none, everything in the ball goes.
    pub fn cut(&mut self, world: &mut World, middle: Vec3, radius: f32, push: Vec3, forms: &[&Form], stuff: Option<Stuff>) -> Cut {
        let (lo, hi) = (((middle - radius) / CELL).floor().as_ivec3() - 1, ((middle + radius) / CELL).floor().as_ivec3() + 1);
        // (A tree is made wrought only by what cuts wood, a boulder by what cuts stone.)
        let mut cut = work(world, middle, radius, forms, stuff);
        cut.taken = carve(world, middle, radius, stuff);
        if cut.taken.iter().any(|t| *t > 0) {
            self.cuts.push((middle, cut.taken));
        }
        let mut changed = std::mem::take(&mut cut.changed);
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                changed.insert(IVec3::new(x, lo.y, z).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                changed.insert(IVec3::new(x, hi.y, z).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            }
        }
        changed.extend(self.loosen(world, lo, hi, push));
        Cut { changed, ..cut }
    }

    /// Looks through what is solid in a box of cells for anything no longer held, and takes
    /// each such thing out of the world as a piece.
    pub fn loosen(&mut self, world: &mut World, lo: IVec3, hi: IVec3, push: Vec3) -> HashSet<IVec3> {
        let mut changed = HashSet::new();
        // Every cell looked through, and in which search: one that meets an earlier search's
        // cells is of the same body, and that body was held (a loose one is out of the world).
        let mut seen: HashMap<IVec3, u32> = HashMap::new();
        let mut search = 0;
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    let from = IVec3::new(x, y, z);
                    if seen.contains_key(&from) || !has(world, from, false) {
                        continue;
                    }
                    search += 1;
                    // Through what bears weight, until it is plainly the land's or it ends.
                    let (mut own, mut queue, mut held, mut land) = (vec![from], VecDeque::from([from]), false, 0);
                    seen.insert(from, search);
                    'through: while let Some(cell) = queue.pop_front() {
                        // (Earth is joined to earth by its faces, which is enough for a
                        // thing with no thin parts and a quarter of the looking.)
                        let earth = world.get(cell).ground();
                        land += earth as usize;
                        if land > LAND {
                            held = true;
                            break;
                        }
                        for side in round().filter(|d| !earth || d.abs().element_sum() == 1) {
                            let next = cell + side;
                            if !has(world, next, false) {
                                continue;
                            }
                            match seen.get(&next) {
                                Some(by) if *by == search => continue,
                                Some(_) => {
                                    held = true;
                                    break 'through;
                                }
                                None => {}
                            }
                            seen.insert(next, search);
                            own.push(next);
                            queue.push_back(next);
                        }
                        if own.len() > HELD {
                            held = true;
                            break;
                        }
                    }
                    if !held {
                        changed.extend(self.take(world, own, push));
                    }
                }
            }
        }
        changed
    }

    /// Takes cells out of the world as one piece, with the leaves that are theirs.
    fn take(&mut self, world: &mut World, mut own: Vec<IVec3>, push: Vec3) -> HashSet<IVec3> {
        // Its leaves: what foliage is within a few cells of it, through foliage.
        let mut near: HashMap<IVec3, u8> = own.iter().map(|c| (*c, 0)).collect();
        let mut queue: VecDeque<IVec3> = own.iter().copied().collect();
        while let Some(cell) = queue.pop_front() {
            let far = near[&cell];
            if far >= LEAFY {
                continue;
            }
            for side in SIDES {
                let next = cell + side;
                if !near.contains_key(&next) && has(world, next, true) && !has(world, next, false) {
                    near.insert(next, far + 1);
                    own.push(next);
                    queue.push_back(next);
                }
            }
        }
        let mut cells = HashMap::with_capacity(own.len());
        let mut changed = HashSet::new();
        for cell in own {
            let part = match world.get(cell) {
                Block::Broken => world.bits_at(cell).map(|b| Part::Bits(Box::new(*b))),
                Block::Air => None,
                whole => Some(Part::Whole(whole)),
            };
            if let Some(part) = part {
                cells.insert(cell, part);
                world.set(cell, Block::Air);
                changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            }
        }
        // (A crumb of a few cells is gone, not a thing that falls: what a cut or a landing
        // chips off.)
        if cells.len() < 4 {
            return changed;
        }
        // How it stands: its height, and how wide it is at its foot (what bears, lowest).
        let bearing: Vec<IVec3> = cells.iter().filter(|(_, p)| match p { Part::Whole(b) => bears(*b), Part::Bits(b) => b.iter().any(|b| bears(*b)) }).map(|(c, _)| *c).collect();
        let of = if bearing.is_empty() { cells.keys().copied().collect() } else { bearing };
        let (low, high) = of.iter().fold((i32::MAX, i32::MIN), |(l, h), c| (l.min(c.y), h.max(c.y)));
        let foot: Vec<Vec3> = of.iter().filter(|c| c.y <= low + 1).map(|c| (c.as_vec3() + 0.5) * CELL).collect();
        let middle = foot.iter().sum::<Vec3>() / foot.len() as f32;
        let wide = foot.iter().map(|p| (*p - middle).with_y(0.0).length()).fold(0.0, f32::max) * 2.0 + CELL;
        let tall = (high - low + 1) as f32 * CELL;
        let pivot = middle.with_y(low as f32 * CELL);
        // Which way it goes: the way it was pushed, and more the way it leans (where the
        // middle of its wood is from over its foot: a leaning tree goes the way it leans
        // whoever cuts it).
        let heart = of.iter().map(|c| (c.as_vec3() + 0.5) * CELL).sum::<Vec3>() / of.len() as f32;
        let lean = (heart - middle).with_y(0.0) / (tall * 0.08).max(0.1);
        let way = (push.with_y(0.0).normalize_or_zero() + lean.clamp_length_max(2.5)).try_normalize().unwrap_or(Vec3::X);
        // (Slowly at first: it hangs on its stump a moment and then goes.)
        let over = (tall > 2.0 && tall > wide * 2.5).then_some((way, 0.05, 0.16));
        // Where it is tried against the world: some hundreds of its cells, spread through it.
        // (Its wood and stone, not its leaves: a crown goes through another crown, and
        // into the ground a little, before the wood under it is stopped.)
        let stride = (of.len() / 300).max(1);
        let mut order: Vec<IVec3> = of.clone();
        order.sort_by_key(|c| (c.y, c.z, c.x));
        let feel = order.iter().step_by(stride).map(|c| (c.as_vec3() + 0.5) * CELL).collect();
        // Stone and earth break; wood (and what is mostly wood) does not.
        let brittle = of.iter().filter(|c| match &cells[*c] { Part::Whole(b) => !b.wood(), Part::Bits(b) => !b.iter().any(|b| b.wood()) }).count() * 2 > of.len();
        self.next += 1;
        self.begun.push((pivot, cells.len(), over.is_some()));
        self.pieces.push(Piece { id: self.next, cells, pivot, shift: Vec3::ZERO, turn: Quat::IDENTITY, vel: Vec3::ZERO, over, spin: Vec3::ZERO, brittle, tall, feel, age: 0.0 });
        changed
    }

    /// Breaks a piece that has come down hard: what of it `keeps` says stays one piece (given
    /// back); the rest goes in lumps some three cells across, each thrown out from the
    /// middle and up (`throw` of how fast the whole came down), turning, to land on its own.
    /// What is too small to be a lump is dust, and gone.
    fn shatter(&mut self, mut piece: Piece, speed: f32, throw: f32, keeps: impl Fn(IVec3) -> bool) -> Piece {
        let mut lumps: HashMap<IVec3, HashMap<IVec3, Part>> = HashMap::new();
        let all = std::mem::take(&mut piece.cells);
        for (cell, part) in all {
            if keeps(cell) {
                piece.cells.insert(cell, part);
                continue;
            }
            // (Lumps of uneven size: which lump a cell is of goes by a grid that is itself
            // shifted from place to place.)
            let shove = IVec3::new((cell.y / 2).rem_euclid(2), (cell.z / 3).rem_euclid(2), (cell.x / 2).rem_euclid(2));
            lumps.entry((cell + shove).div_euclid(IVec3::splat(3))).or_default().insert(cell, part);
        }
        let heart = piece.pivot + Vec3::Y * piece.tall / 2.0;
        let mut keys: Vec<IVec3> = lumps.keys().copied().collect();
        keys.sort_by_key(|k| (k.y, k.z, k.x));
        for key in keys {
            let cells = lumps.remove(&key).unwrap_or_default();
            // (A lump of leaf alone is not a thing that falls: it is leaves, shed.)
            if cells.len() < 4 || !cells.values().any(|p| match p { Part::Whole(b) => bears(*b), Part::Bits(b) => b.iter().any(|b| bears(*b)) }) {
                continue;
            }
            let middle = cells.keys().map(|c| (c.as_vec3() + 0.5) * CELL).sum::<Vec3>() / cells.len() as f32;
            // (Nothing random: how each is thrown goes by where it was.)
            let odd = |n: i32| ((key.x * 73 + key.y * 179 + key.z * 283 + n * 97).rem_euclid(200) as f32 / 100.0) - 1.0;
            let out = (middle - heart).with_y(0.0).try_normalize().unwrap_or(Vec3::new(odd(1), 0.0, odd(2)).normalize_or(Vec3::X));
            let feel = cells.iter().filter(|(_, p)| match p { Part::Whole(b) => bears(*b), Part::Bits(b) => b.iter().any(|b| bears(*b)) }).map(|(c, _)| (c.as_vec3() + 0.5) * CELL).collect();
            self.next += 1;
            self.pieces.push(Piece {
                id: self.next,
                cells,
                // (Turning about its own middle, from where the whole had brought it.)
                pivot: middle,
                shift: piece.pivot + piece.shift + piece.turn * (middle - piece.pivot) - middle,
                turn: piece.turn,
                vel: piece.turn * (out * (0.18 + 0.1 * odd(3)) + Vec3::Y * (0.22 + 0.08 * odd(4))) * speed.min(14.0) * throw,
                over: None,
                spin: Vec3::new(odd(5), odd(6), odd(7)) * 5.0 * throw.min(1.0),
                brittle: false,
                tall: 3.0 * CELL,
                feel,
                age: 0.0,
            });
        }
        piece
    }

    /// One moment of everything loose: each piece falls or goes further over, and what
    /// meets the world is put back into it. Gives the chunks that changed.
    pub fn step(&mut self, world: &mut World, dt: f32) -> HashSet<IVec3> {
        let mut changed = HashSet::new();
        let mut down = Vec::new();
        for (n, piece) in self.pieces.iter_mut().enumerate() {
            piece.age += dt;
            let before = (piece.shift, piece.turn);
            let speed;
            match &mut piece.over {
                Some((way, tipped, rate)) => {
                    // As a pole on its end goes over: faster the further it has gone.
                    *rate += 1.5 * FALL / piece.tall.max(1.0) * tipped.sin() * dt;
                    *tipped += *rate * dt;
                    piece.turn = Quat::from_axis_angle(Vec3::Y.cross(*way).normalize_or(Vec3::Z), *tipped);
                    speed = *rate * piece.tall;
                    // (Gone right over with nothing under it: it was at an edge, and drops.)
                    if *tipped > 1.9 {
                        piece.vel = Vec3::NEG_Y * speed * 0.5;
                        piece.over = None;
                    }
                }
                None => {
                    piece.vel.y -= FALL * dt;
                    piece.shift += piece.vel * dt;
                    piece.turn = (Quat::from_scaled_axis(piece.spin * dt) * piece.turn).normalize();
                    speed = piece.vel.length();
                }
            }
            // What of it is tried: all of it as it drops; as it goes over, what is well
            // above its foot (its foot stands on the stump it was cut from).
            let foot = piece.pivot.y + piece.tall * 0.25;
            let going_over = piece.over.is_some();
            // (Stopped by the ground and by what has itself been cut or has fallen; not by
            // leaf, and not by a tree still standing whole: in a wood every tree's limbs are
            // in its neighbours', and stopped by those nothing cut ever came down. It goes
            // through them, and where it lies across a standing trunk it is not put.)
            let stops = |at: Vec3| bears(world.bit((at / BIT).floor().as_ivec3()));
            let meets = |piece: &Piece| piece.feel.iter().filter(|p| !going_over || p.y > foot).any(|p| stops(piece.now(*p)));
            let met = meets(piece);
            if met {
                // Back to where it last was clear, as near to touching as six halvings find:
                // put down where it met the world it would be a step's depth inside it, and
                // what was inside would be lost (half a slab, dropped two metres).
                let after = (piece.shift, piece.turn);
                let (mut clear, mut inside) = (0.0f32, 1.0f32);
                for _ in 0..6 {
                    let half = (clear + inside) / 2.0;
                    (piece.shift, piece.turn) = (before.0.lerp(after.0, half), before.1.slerp(after.1, half));
                    if meets(piece) { inside = half } else { clear = half }
                }
                (piece.shift, piece.turn) = (before.0.lerp(after.0, clear), before.1.slerp(after.1, clear));
            }
            if met || piece.age > 20.0 {
                down.push((n, speed));
            }
        }
        for (n, speed) in down.into_iter().rev() {
            let piece = self.pieces.swap_remove(n);
            let mut piece = piece;
            if piece.brittle && speed > SHATTERS && piece.cells.len() >= 8 {
                self.landed.push(Landed { at: piece.now(piece.pivot + Vec3::Y * piece.tall / 2.0), cells: piece.cells.len(), put: 0, speed });
                self.shatter(piece, speed, 1.0, |_| false);
                continue;
            }
            // A tree gone over: its trunk (what was within a stride of the line it stood
            // on) lands whole, and what stood out from it breaks off as it strikes.
            if piece.over.is_some() && speed > CRASHES {
                let (foot, stem) = (piece.pivot, (piece.tall * 0.05).clamp(0.5, 0.9));
                piece = self.shatter(piece, speed, 0.22, |c| ((c.as_vec3() + 0.5) * CELL - foot).with_y(0.0).length() < stem);
            }
            let (put, touched, lo, hi) = land(world, &piece);
            changed.extend(touched);
            // And what of it has come down on nothing (a limb left sticking out over a
            // hollow, a part cut off from the rest by what it fell across) falls in its turn.
            changed.extend(self.loosen(world, lo - 1, hi + 1, Vec3::X));
            self.landed.push(Landed { at: piece.now(piece.pivot + Vec3::Y * piece.tall / 2.0), cells: piece.cells.len(), put, speed });
        }
        changed
    }
}

/// Puts a piece back into the world where it now lies: every cell of the world it reaches is
/// asked what of the piece is there, so it goes back without gaps however it is turned.
/// Only into what is not solid already. Gives how many cells were written, and their chunks.
fn land(world: &mut World, piece: &Piece) -> (usize, HashSet<IVec3>, IVec3, IVec3) {
    let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
    for cell in piece.cells.keys() {
        for corner in 0..8 {
            let p = piece.now((*cell + IVec3::new(corner & 1, corner >> 1 & 1, corner >> 2 & 1)).as_vec3() * CELL);
            (lo, hi) = (lo.min(p), hi.max(p));
        }
    }
    let (lo, hi) = ((lo / CELL).floor().as_ivec3(), (hi / CELL).floor().as_ivec3());
    let at = |now: Vec3| (piece.was(now) / CELL).floor().as_ivec3();
    let (mut put, mut changed) = (0, HashSet::new());
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let cell = IVec3::new(x, y, z);
                let Some(part) = piece.cells.get(&at((cell.as_vec3() + 0.5) * CELL)) else { continue };
                let there = world.get(cell);
                match part {
                    // (Wood and stone go where leaf is: a limb fallen through a crown is not
                    // cut in two by it.)
                    Part::Whole(b) if (!there.solid() || (there.foliage() && !b.foliage())) && there != Block::Broken => {
                        world.set(cell, *b);
                        put += 1;
                    }
                    Part::Whole(_) => continue,
                    Part::Bits(_) => {
                        // To the bit: each bit of this cell is asked for on its own.
                        let mut any = false;
                        for i in 0..SPLIT * SPLIT * SPLIT {
                            let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
                            let was = piece.was((bit.as_vec3() + 0.5) * BIT);
                            let from = (was / CELL).floor().as_ivec3();
                            let block = match piece.cells.get(&from) {
                                Some(Part::Whole(b)) => *b,
                                Some(Part::Bits(bits)) => {
                                    let l = ((was / BIT).floor().as_ivec3()).rem_euclid(IVec3::splat(SPLIT));
                                    bits[(l.x + l.z * SPLIT + l.y * SPLIT * SPLIT) as usize]
                                }
                                None => Block::Air,
                            };
                            if block.solid() && !world.bit(bit).solid() {
                                world.set_bit(bit, block);
                                any = true;
                            }
                        }
                        put += any as usize;
                    }
                }
                changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            }
        }
    }
    (put, changed, lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A floor of stone far wider than [`HELD`] cells, its top at y = 0.
    fn floor() -> World {
        let mut w = World::default();
        w.fill(Vec3::new(-12.0, -1.0, -12.0), Vec3::new(12.0, 0.0, 12.0), Block::Stone);
        w
    }

    fn run(loose: &mut Loose, w: &mut World) -> f32 {
        let mut t = 0.0;
        while !loose.pieces.is_empty() && t < 30.0 {
            loose.step(w, crate::DT);
            t += crate::DT;
        }
        t
    }

    fn count(w: &World, lo: Vec3, hi: Vec3, b: Block) -> usize {
        let (lo, hi) = ((lo / CELL).floor().as_ivec3(), (hi / CELL).floor().as_ivec3());
        (lo.y..hi.y).flat_map(|y| (lo.z..hi.z).flat_map(move |z| (lo.x..hi.x).map(move |x| IVec3::new(x, y, z)))).filter(|c| w.get(*c) == b).count()
    }

    #[test]
    fn digging_a_hole_in_the_ground_looses_nothing() {
        let mut w = floor();
        let mut loose = Loose::default();
        loose.cut(&mut w, Vec3::new(0.0, -0.1, 0.0), 0.4, Vec3::X, &[], None);
        assert!(loose.pieces.is_empty(), "the land came loose: {} pieces", loose.pieces.len());
    }

    #[test]
    fn a_pole_cut_through_goes_over_and_lies_on_the_ground() {
        let mut w = floor();
        w.fill(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 6.0, 0.5), Block::Timber);
        let wood = count(&w, Vec3::splat(-12.0), Vec3::splat(12.0), Block::Timber);
        let mut loose = Loose::default();
        loose.cut(&mut w, Vec3::new(0.25, 0.75, 0.25), 0.45, Vec3::X, &[], None);
        assert_eq!(loose.pieces.len(), 1, "what was over the cut did not come loose");
        let (cells, standing) = (loose.pieces[0].cells.len(), count(&w, Vec3::splat(-12.0), Vec3::splat(12.0), Block::Timber));
        assert!(cells > 60 && standing < wood - cells + 1, "the piece has {cells} cells and {standing} of {wood} are left standing");
        let took = run(&mut loose, &mut w);
        let down = loose.landed[0];
        // It went over (a second or two, not at once and not for ever), it lies along the
        // way it was pushed and low, and nearly all of it is there.
        assert!((0.6..6.0).contains(&took), "it took {took:.1} s to come down");
        let lying = count(&w, Vec3::new(0.5, 0.0, -1.0), Vec3::new(8.0, 1.25, 1.5), Block::Timber);
        let high = count(&w, Vec3::new(-2.0, 2.0, -2.0), Vec3::new(8.0, 12.0, 2.0), Block::Timber);
        assert!(lying as f32 > cells as f32 * 0.8 && high == 0, "{lying} of {cells} cells lie along the ground, {high} are still up · {down:?}");
    }

    #[test]
    fn what_is_dug_out_from_under_drops_straight_and_stays_whole() {
        let mut w = floor();
        // A slab held up at one end by a post.
        w.fill(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 2.0, 0.5), Block::Stone);
        w.fill(Vec3::new(0.0, 2.0, 0.0), Vec3::new(3.0, 2.5, 1.0), Block::Stone);
        let mut loose = Loose::default();
        // (The whole post taken away: left a stub of it, the slab comes to rest on that.)
        w.fill(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 2.0, 0.5), Block::Air);
        loose.loosen(&mut w, IVec3::new(-1, -1, -1), IVec3::new(3, 9, 3), Vec3::Z);
        assert_eq!(loose.pieces.len(), 1);
        let cells = loose.pieces[0].cells.len();
        run(&mut loose, &mut w);
        let (low, up) = (count(&w, Vec3::new(-1.0, 0.0, -1.0), Vec3::new(4.0, 1.5, 2.0), Block::Stone), count(&w, Vec3::new(-1.0, 1.75, -1.0), Vec3::new(4.0, 4.0, 2.0), Block::Stone));
        assert!(low as f32 > cells as f32 * 0.85 && up == 0, "{low} of {cells} cells are down, {up} are still up");
    }

    #[test]
    fn a_tree_that_is_cut_into_is_cells_from_then_on_and_one_whose_limbs_touch_it_is_not() {
        use crate::plants::{Clump, Limb};
        let mut w = floor();
        // Two trees as `plants` notes them, a limb of each reaching into the other.
        let mut form = Form::default();
        for (tree, x) in [(1, 0.0f32), (2, 3.0)] {
            let reach = if tree == 1 { 2.0 } else { -2.0 };
            for limb in [Limb { a: Vec3::new(x, 0.0, 0.0), b: Vec3::new(x, 5.0, 0.0), ra: 0.25, rb: 0.2, bark: Block::Wood, roots: 0, tree }, Limb { a: Vec3::new(x, 3.0, 0.0), b: Vec3::new(x + reach, 4.0, 0.0), ra: 0.12, rb: 0.1, bark: Block::Wood, roots: 0, tree }] {
                limb.fill(CELL, |c| w.set(c, Block::Wood));
                form.limbs.push(limb);
            }
            w.ellipsoid(Vec3::new(x, 5.5, 0.0), Vec3::splat(1.2), Block::Leaves, true);
            form.clumps.push(Clump { middle: Vec3::new(x, 5.5, 0.0), radii: Vec3::splat(1.2), leaf: Block::Leaves, tree });
        }
        let count_all = |w: &World, b: Block| count(w, Vec3::splat(-12.0), Vec3::splat(12.0), b);
        let (wood, leaves) = (count_all(&w, Block::Wood), count_all(&w, Block::Leaves));
        let mut loose = Loose::default();
        // A notch, not a cut through: it stands, and all of it is wrought and the other is not.
        let cut = loose.cut(&mut w, Vec3::new(-0.3, 1.0, 0.0), 0.2, Vec3::X, &[&form], None);
        assert!(loose.pieces.is_empty(), "a notch felled it");
        assert_eq!(cut.trees, vec![(0, 1)]);
        let (timber, brush) = (count_all(&w, Block::Timber), count_all(&w, Block::Brush));
        assert!(timber > wood / 3 && brush > leaves / 3, "timber {timber} of {wood} wood, brush {brush} of {leaves} leaf");
        assert!(count_all(&w, Block::Wood) > wood / 3 && count_all(&w, Block::Leaves) > leaves / 3, "the tree beside it was made wrought too");
        // Cut through, it comes down though the other's limb is in its crown.
        for step in -2..=2 {
            loose.cut(&mut w, Vec3::new(step as f32 * 0.2, 1.0, 0.0), 0.4, Vec3::NEG_X, &[&form], None);
        }
        assert_eq!(loose.pieces.len(), 1, "cut through, it hangs by its neighbour");
    }

    #[test]
    fn stone_that_drops_shatters_into_rubble_and_wood_does_not() {
        for (block, breaks) in [(Block::Stone, true), (Block::Timber, false)] {
            let mut w = floor();
            w.fill(Vec3::new(0.0, 3.0, 0.0), Vec3::new(2.0, 4.5, 2.0), block);
            let mut loose = Loose::default();
            loose.loosen(&mut w, IVec3::new(-1, 11, -1), IVec3::new(9, 19, 9), Vec3::X);
            assert_eq!(loose.pieces.len(), 1);
            let cells = loose.pieces[0].cells.len();
            let mut most = 1;
            let mut t = 0.0;
            while !loose.pieces.is_empty() && t < 30.0 {
                loose.step(&mut w, crate::DT);
                most = most.max(loose.pieces.len());
                t += crate::DT;
            }
            let down = count(&w, Vec3::new(-8.0, 0.0, -8.0), Vec3::new(10.0, 3.0, 10.0), block);
            let block_shape = count(&w, Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 1.5, 2.0), block);
            if breaks {
                // Many lumps, thrown wider than the block was, most of it still there, all at rest.
                assert!(most >= 8, "it broke into {most}");
                assert!(down as f32 > cells as f32 * 0.6 && (block_shape as f32) < cells as f32 * 0.8, "{down} of {cells} cells came down, {block_shape} of them where the block fell");
            } else {
                assert!(most == 1 && block_shape as f32 > cells as f32 * 0.9, "wood broke into {most}: {block_shape} of {cells} where it fell");
            }
            assert!(loose.pieces.is_empty(), "something never came to rest");
        }
    }

    #[test]
    fn a_tool_cuts_its_own_stuff_and_leaves_the_rest() {
        let mut w = floor();
        w.fill(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 3.0, 0.5), Block::Timber);
        let mut loose = Loose::default();
        // An axe at the foot of a post: wood goes, the stone under it does not.
        let cut = loose.cut(&mut w, Vec3::new(0.25, 0.0, 0.25), 0.4, Vec3::X, &[], Some(Stuff::Wood));
        assert!(cut.taken[Stuff::Wood as usize] > 100 && cut.taken[Stuff::Stone as usize] == 0, "took {:?}", cut.taken);
        assert!(w.get(IVec3::new(0, -1, 0)) == Block::Stone, "an axe cut stone");
        // And a pick there takes the stone and leaves what wood is left.
        let wood = count(&w, Vec3::splat(-12.0), Vec3::splat(12.0), Block::Timber);
        let cut = loose.cut(&mut w, Vec3::new(1.0, 0.0, 0.25), 0.4, Vec3::X, &[], Some(Stuff::Stone));
        assert!(cut.taken[Stuff::Stone as usize] > 100 && cut.taken[Stuff::Wood as usize] == 0, "took {:?}", cut.taken);
        assert_eq!(count(&w, Vec3::splat(-12.0), Vec3::splat(12.0), Block::Timber), wood);
    }

    #[test]
    fn two_crowns_that_touch_do_not_hold_each_other_up() {
        let mut w = floor();
        // (Both cut into already: each is cells.)
        for x in [0.0, 3.0] {
            w.fill(Vec3::new(x, 0.0, 0.0), Vec3::new(x + 0.5, 4.0, 0.5), Block::Timber);
            w.fill(Vec3::new(x - 1.25, 4.0, -1.0), Vec3::new(x + 1.75, 5.5, 1.5), Block::Brush);
        }
        let mut loose = Loose::default();
        loose.cut(&mut w, Vec3::new(0.25, 0.75, 0.25), 0.45, Vec3::NEG_X, &[], None);
        assert_eq!(loose.pieces.len(), 1, "the cut tree hangs by its neighbour's leaves");
        // It took leaves with it, and left its neighbour its trunk.
        let piece = &loose.pieces[0];
        assert!(piece.cells.values().any(|p| matches!(p, Part::Whole(b) if b.foliage())), "it left all its leaves behind");
        assert!(w.get(IVec3::new(13, 8, 1)) == Block::Timber, "the neighbour's trunk went with it");
    }
}
