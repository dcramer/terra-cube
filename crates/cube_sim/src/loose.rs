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
//! was pushed; it hangs a moment and then goes faster and faster; and it comes down whole,
//! trunk and limbs, along the line it fell, to be cut into logs where it lies. Only the
//! outermost twigs of its crown snap off as it strikes, and drop where they are. Then it
//! **settles**: its top is down and its butt is still up at the height of the stump, so it
//! turns back about its top and slides a little way on, until its butt is on the ground
//! beside the stump and it lies flat. (Stopped where its top first struck, it lay as a ramp
//! from stump to tip, and a length cut out of its middle hung in the air.) The same cut
//! fells the same tree the same way: nothing is thrown about.
//!
//! **What is cut is still a thing** ([`Thing`]; the developer, 2026-11-12: a tree is to be
//! known as a tree still, since a world this size is made and dropped as it is walked and
//! cannot keep every box of it). Each tree or boulder cut into is noted: the shape it grew
//! (its limbs and leaf, a few hundred numbers), every bite taken out of it, and where it
//! lies if it has come down. That is all that need be kept of it: its cells can be made
//! again from the shape, put where it lies, and the bites taken again. Nothing keeps or
//! restores from it yet; it is what saving and dropping a region will use.
//!
//! **The world is its seed and what was done to it** ([`Edit`], [`Loose::journal`],
//! [`replay`]): every cut and every thing put down is noted with the moment it was done (in
//! steps of what was falling), a few numbers each. Done again in the same order to the land
//! as the seed makes it, they give the same world, cell for cell: nothing here is random and
//! a fall is the same fall. So a part of the world that has been changed need not be kept:
//! it can be dropped and made again. (Not yet used so by the game, which keeps every
//! changed region: `cargo world replay` proves it.)
//!
//! **Water runs into what is opened to it** ([`flow`]): from water beside a cut, sideways and
//! down into air and up no higher than it stood, so a hole dug at a lake's edge fills to the lake's level and a
//! trench leads it downhill. By the cell; a lake is not drained by it (the sea is the sea).
//!
//! No engine types and nothing random: the same cut fells the same tree. `cargo world fell`
//! judges it on a real tree with no window.

use crate::items::Stuff;
use crate::plants::Form;
use cube_voxel::{BIT, Bits, Block, CELL, SPLIT, World};
use glam::{IVec3, Quat, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};

/// How many blows of an axe a standing tree takes before it is through, by how thick its
/// trunk is where it is cut (metres, its radius): one to every 15 cm across, two at the
/// least and eight at the most. (The developer was not sure of this, 2026-11-13: "it might
/// need some kind of like.. durability so its not one chop in some cases".)
#[allow(non_snake_case)]
pub fn BLOWS(thick: f32) -> u32 {
    ((thick * 2.0 / 0.15).ceil() as u32).clamp(2, 8)
}
/// How long the logs are that a bare trunk breaks into, metres.
pub const LOG: f32 = 2.0;

/// How many cells a side a piece to carry is: half a metre, of wood ([`Block::Billet`]) or of
/// stone ([`Block::Cobble`]). One size for both, and the one place it is said. (Called a hunk
/// here: a chunk is already sixteen cells of the world.) The developer, 2026-11-13:
/// "resources are effectively these chunks, that are kind of just larger voxels ... all of
/// similar size ... you have to carry them in your hands, probably one at a time"; the
/// size was left to us.
pub const HUNK: i32 = 2;
/// How much of a log's wood comes away as hunks, of 1: the rest is chips. (A tree gave 20 to
/// 30 at all of it, and the developer, having felled one: "it changed into too many".)
pub const YIELD: f32 = 0.5;
/// How many blows of a pick a boulder takes before it breaks into hunks, by how far across
/// it is at its widest (metres, its radius): three to five ("maybe that takes 3-5 chips
/// depending on size of rock").
#[allow(non_snake_case)]
pub fn CHIPS(wide: f32) -> u32 {
    3 + (wide > 0.8) as u32 + (wide > 1.5) as u32
}

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
    /// Hunks and nothing else: it comes down where they lie two cells apart, and does not roll.
    hunk: bool,
    /// Which thing it is the fallen part of, if it is a tree gone over.
    thing: Option<u32>,
    /// How it was turned when it began the turn it is making (nothing, going over; as its
    /// top struck, settling), and whether it is settling. And the part of it that is not
    /// tried against the world: what is within so far of a place in it (its foot on the
    /// stump, going over; its top on the ground, settling).
    base: Quat,
    settling: bool,
    spare: (Vec3, f32),
    /// How tall it is, metres, and the places of it that are tried against the world.
    tall: f32,
    feel: Vec<Vec3>,
    /// Seconds it has been loose.
    pub age: f32,
    /// Times a lump has rolled off what it came down on ([`Loose::step`]).
    rolls: u8,
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
    /// Whole cells of what bears (wood, stone) that found no room where it came down.
    pub lost: usize,
    /// Metres a second it was going.
    pub speed: f32,
    /// A tree coming to rest after its top has already struck: that was the crash, and this
    /// is not heard again nor a second blow.
    pub settled: bool,
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

/// A tree or a boulder that has been cut into, as little as says all of it: what it grew as,
/// what has been cut out of it, and where it lies if it has come down.
#[derive(Clone)]
pub struct Thing {
    pub id: u32,
    /// The shape it grew: a tree's limbs and leaf, or a boulder's stone.
    pub limbs: Vec<crate::plants::Limb>,
    pub clumps: Vec<crate::plants::Clump>,
    pub stone: Option<crate::plants::Stone>,
    /// Every bite taken out of it: where, how far across, and of what stuff.
    pub cuts: Vec<(Vec3, f32, Option<Stuff>)>,
    /// Come down: what it turned about, how far that moved, and how it is turned (a point of
    /// it that stood at `p` lies at pivot + shift + turn (p - pivot), as [`Piece::now`]).
    pub lies: Option<(Vec3, Vec3, Quat)>,
    /// Blows of an axe taken standing (or of a pick, a boulder), and whether its boughs and
    /// leaf have been knocked off since it fell ([`Loose::chop`]). From then on it is
    /// timber, and nothing more is kept of it: [`Loose::timber`].
    pub blows: u32,
    pub limbed: bool,
}

impl Thing {
    /// Whether a ball reaches it as it stood.
    fn reached(&self, middle: Vec3, radius: f32) -> bool {
        self.limbs.iter().any(|l| from_line(middle, l.a, l.b) < radius + l.ra.max(l.rb) * if l.roots > 0 { 2.6 } else { 1.0 } + CELL) || self.stone.as_ref().is_some_and(|s| ((middle - s.middle) / (s.radii + radius + CELL)).length_squared() < 1.0)
    }
    /// Where it stands on the ground: the foot of its rooted trunk, or of its lowest limb.
    fn foot(&self) -> Option<Vec3> {
        self.limbs.iter().find(|l| l.roots > 0).or_else(|| self.limbs.iter().min_by(|a, b| a.a.y.total_cmp(&b.a.y))).map(|l| l.a)
    }
}

/// Something done to the world by hand: a cut, or a thing put down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edit {
    Cut { middle: Vec3, radius: f32, push: Vec3, stuff: Option<Stuff> },
    Put { middle: Vec3, radius: f32, block: Block },
    Chop { at: Vec3, push: Vec3 },
    Chip { at: Vec3, push: Vec3 },
    /// A hunk taken up, and one put down.
    Lift { at: Vec3 },
    Lay { at: Vec3, block: Block },
}

impl Edit {
    /// Where it was done.
    pub fn at(&self) -> Vec3 {
        match *self {
            Edit::Cut { middle, .. } | Edit::Put { middle, .. } => middle,
            Edit::Chop { at, .. } | Edit::Chip { at, .. } | Edit::Lift { at } | Edit::Lay { at, .. } => at,
        }
    }
}

/// Does a journal's edits to a world again, each at its moment, and lets what falls come to
/// rest. `forms` are the shapes of what grows there, as the land was made.
pub fn replay(world: &mut World, forms: &[&Form], journal: &[(u64, Edit)]) -> Loose {
    let mut again = Replay::default();
    while !again.advance(world, forms, journal, std::time::Duration::MAX) {}
    again.loose
}

/// A journal being done again, a little at a time: for whoever cannot spare the whole of it
/// in one go (the game, on a frame: a tree felled again is some tens of milliseconds).
#[derive(Default)]
pub struct Replay {
    pub loose: Loose,
    next: usize,
    settled: u32,
}

impl Replay {
    /// Does as much more of it as `budget` allows. Says whether it is all done.
    pub fn advance(&mut self, world: &mut World, forms: &[&Form], journal: &[(u64, Edit)], budget: std::time::Duration) -> bool {
        let began = std::time::Instant::now();
        while let Some((at, edit)) = journal.get(self.next) {
            while self.loose.steps < *at {
                self.loose.step(world, crate::DT);
                if began.elapsed() > budget {
                    return false;
                }
            }
            match *edit {
                Edit::Cut { middle, radius, push, stuff } => drop(self.loose.cut(world, middle, radius, push, forms, stuff)),
                Edit::Put { middle, radius, block } => drop(self.loose.put(world, middle, radius, block)),
                Edit::Chop { at, push } => drop(self.loose.chop(world, at, push, forms)),
                Edit::Chip { at, push } => drop(self.loose.chip(world, at, push, forms)),
                Edit::Lift { at } => drop(self.loose.lift(world, at)),
                Edit::Lay { at, block } => drop(self.loose.lay(world, at, block)),
            }
            self.next += 1;
            if began.elapsed() > budget {
                return false;
            }
        }
        while !self.loose.pieces.is_empty() && self.settled < 60 * crate::TICKS {
            self.loose.step(world, crate::DT);
            self.settled += 1;
            if began.elapsed() > budget {
                return false;
            }
        }
        true
    }
}

/// Everything loose in a world, and what has landed since it was last asked for.
#[derive(Default, Clone)]
pub struct Loose {
    pub pieces: Vec<Piece>,
    /// For the length of one felling cut: no body is too large to be looked through to its
    /// end ([`HELD`]). A tree is known to be a tree.
    whole: bool,
    pub landed: Vec<Landed>,
    /// The same, once the simulation has seen who was under each (`Sim::step`): for whoever
    /// makes the sound of it.
    pub heard: Vec<Landed>,
    /// For whoever shows and sounds it, to take as they are noted: each cut (where, and how
    /// much of earth, wood and stone it took), and each thing that came loose (where it
    /// stood, how many cells, and whether it is going over like a tree).
    pub cuts: Vec<(Vec3, [u32; 3])>,
    pub begun: Vec<(Vec3, usize, bool)>,
    /// What a blow knocked off a fallen tree, for whoever shows it to throw and let fade:
    /// where, and of what.
    pub shed: Vec<(Vec3, Block)>,
    /// Every tree and boulder that has been cut into ([`Thing`]).
    pub things: Vec<Thing>,
    /// Hunks that have come loose where they had just come to rest, by their corner, and
    /// how many times running: one that does it four times is left where it is
    /// ([`Loose::loosen`]).
    restless: HashMap<IVec3, u8>,
    /// Every edit made, with how many steps had been taken when it was ([`replay`]).
    pub journal: Vec<(u64, Edit)>,
    steps: u64,
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

/// What part of a trunk's wood must be left across a cut for it to go on standing: half.
/// (Chops are balls: three in a line through a trunk a metre across leave two fifths of it,
/// in two strips, and it must not stand on those.)
const HOLDS: f32 = 0.5;

/// Wood cut most of the way through gives way: if fewer than [`HOLDS`] of the trunk's fibres
/// run unbroken through the cut, what is left at the level with least wood goes too. (A hole chopped straight through a trunk left a sliver each side of
/// it, and the tree stood on them with daylight through it; every chop after went through
/// the hole.) Says how many bits went.
fn snap(world: &mut World, middle: Vec3, radius: f32) -> u32 {
    let reach = radius + 0.8;
    let (lo, hi) = (((middle - Vec3::new(reach, radius, reach)) / BIT).floor().as_ivec3(), ((middle + Vec3::new(reach, radius, reach)) / BIT).floor().as_ivec3());
    let level = |world: &World, y: i32| -> Vec<IVec3> { (lo.z..=hi.z).flat_map(|z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z))).filter(|b| world.bit(*b).wood()).collect() };
    // What holds is wood that runs unbroken from over the cut to under it: of the trunk's
    // fibres just above the cut, those with wood at every level down through it. (Judged
    // by the least wood at any one level, a tunnel chopped aslant through a trunk never
    // felled it: no level had lost half, and nine fibres in ten were cut.)
    let over = level(world, hi.y + 2);
    let whole = over.len();
    let unbroken = over.iter().filter(|b| (lo.y..=hi.y).all(|y| world.bit(b.with_y(y)).wood())).count();
    let Some(thin) = (lo.y..=hi.y).map(|y| level(world, y)).min_by_key(|l| l.len()) else { return 0 };
    if whole == 0 || thin.is_empty() || unbroken as f32 >= whole as f32 * HOLDS {
        return 0;
    }
    // (All of the cell that level is in, a hand's breadth of wood: what is joined to what is
    // told by the cell, and a slit one bit high through a cell parts nothing.)
    let floor = thin[0].y.div_euclid(SPLIT) * SPLIT;
    let mut gone = 0;
    for y in floor..floor + SPLIT {
        for bit in level(world, y) {
            world.set_bit(bit, Block::Air);
            gone += 1;
        }
    }
    gone
}

/// How many cells water may run into from one cut: a pit or a length of trench, not a valley.
const FLOWS: usize = 6_000;

/// Water beside or in a box of cells runs into the air it can reach: sideways, down, and up
/// no higher than it stood. What grows low (grass, a flower) is drowned. Gives the chunks
/// changed.
pub fn flow(world: &mut World, lo: IVec3, hi: IVec3) -> HashSet<IVec3> {
    let mut queue: VecDeque<IVec3> = VecDeque::new();
    for y in lo.y - 1..=hi.y + 1 {
        for z in lo.z - 1..=hi.z + 1 {
            for x in lo.x - 1..=hi.x + 1 {
                let cell = IVec3::new(x, y, z);
                if world.get(cell) == Block::Water {
                    queue.push_back(cell);
                }
            }
        }
    }
    // (It finds its own level: up, too, as far as the highest of the water it came from. A
    // pit joined to a pool below the pool's top fills to the top.)
    let level = queue.iter().map(|c| c.y).max().unwrap_or(i32::MIN);
    let (mut changed, mut filled) = (HashSet::new(), 0);
    while let Some(cell) = queue.pop_front() {
        for side in [IVec3::NEG_Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::Y] {
            let next = cell + side;
            let there = world.get(next);
            if filled < FLOWS && next.y <= level && (there == Block::Air || there.sprig()) {
                world.set(next, Block::Water);
                changed.insert(next.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                queue.push_back(next);
                filled += 1;
            }
        }
    }
    changed
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
/// How far a point is outside a limb's wood (inside it, less than nothing): from its line,
/// less how thick it is there. (By its thickest anywhere, a great tree that tapers from a
/// metre to a hand's breadth owned every cell within a metre of its top, a birch's trunk
/// beside it among them: the birch's own top was no longer joined to its foot and came
/// down by itself when the birch was first cut into, 2026-11-13.)
fn outside(p: Vec3, limb: &(Vec3, Vec3, f32, f32)) -> f32 {
    let (a, b, ra, rb) = *limb;
    let along = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
    p.distance(a + (b - a) * along) - (ra + (rb - ra) * along)
}

fn from_line(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let along = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
    p.distance(a + (b - a) * along)
}

/// Makes wrought every tree and boulder of `forms` that a ball reaches: all of each tree, its
/// every limb and its leaf, as `plants` grew it. Says which, and the chunks changed.
fn work(world: &mut World, middle: Vec3, radius: f32, forms: &[&Form], stuff: Option<Stuff>, known: &[Vec3]) -> Cut {
    let mut cut = Cut::default();
    // (Each cell made wrought as it is come to, not gathered first: making the same cell
    // wrought twice is nothing, and a great oak is a quarter of a million cells, which took
    // 50 ms to gather into a set and then change.)
    let turn = |world: &mut World, cut: &mut Cut, cell: IVec3| {
        let was = world.get(cell);
        if was == Block::Broken || was.shaped() {
            world.map(cell, Block::wrought);
            cut.worked += 1;
            cut.changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        }
    };
    let (trees, stones) = (stuff.is_none_or(|s| s == Stuff::Wood), stuff.is_none_or(|s| s == Stuff::Stone));
    for (n, form) in forms.iter().enumerate() {
        // (A rooted trunk swells to twice its thickness and more at its foot.)
        let reach = |l: &crate::plants::Limb| radius + l.ra.max(l.rb) * if l.roots > 0 { 2.6 } else { 1.0 } + CELL;
        let mut trees: Vec<u32> = form.limbs.iter().filter(|l| trees && from_line(middle, l.a, l.b) < reach(l)).map(|l| l.tree).collect();
        trees.sort_unstable();
        trees.dedup();
        trees.retain(|t| form.limbs.iter().find(|l| l.tree == *t).is_none_or(|l| !known.contains(&l.a)));
        for tree in trees {
            // (Low down a tree's wood is written to the bit, in the cubes it is drawn in
            // near the eye (`plants::wood`), and those reach cells that its shape at the
            // size of a cell does not: a root's flare, the edge of a leaning trunk. Left as
            // they grew they were wood nobody draws and nothing rests on: a birch stood with
            // a band of air in its trunk, and its top was held by nothing, 2026-11-13. So
            // near its foot the cells beside each cell are looked at too.)
            let low = form.limbs.iter().filter(|l| l.tree == tree && l.roots > 0).map(|l| l.a.y).fold(f32::MAX, f32::min) + 4.0;
            for limb in form.limbs.iter().filter(|l| l.tree == tree) {
                limb.fill(CELL, |c| {
                    turn(world, &mut cut, c);
                    if (c.y as f32) * CELL < low {
                        // (Only what has this limb's own wood in it and is no further from
                        // it than its foot swells: every broken cell beside it, a sapling's
                        // and a bush's among them, was 13,000 cells more on one tree, and
                        // the tree then came down on the sapling.)
                        let swell = limb.ra.max(limb.rb) * if limb.roots > 0 { 2.6 } else { 1.0 } + CELL;
                        for side in round() {
                            let next = c + side;
                            if world.get(next) == Block::Broken && from_line((next.as_vec3() + 0.5) * CELL, limb.a, limb.b) < swell && world.bits_at(next).is_some_and(|bits| bits.contains(&limb.bark)) {
                                turn(world, &mut cut, next);
                            }
                        }
                    }
                });
            }
            for clump in form.clumps.iter().filter(|c| c.tree == tree) {
                let (lo, hi) = (((clump.middle - clump.radii) / CELL).floor().as_ivec3(), ((clump.middle + clump.radii) / CELL).floor().as_ivec3());
                for y in lo.y..=hi.y {
                    for z in lo.z..=hi.z {
                        for x in lo.x..=hi.x {
                            let c = IVec3::new(x, y, z);
                            if world.get(c).foliage() {
                                turn(world, &mut cut, c);
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
                        turn(world, &mut cut, IVec3::new(x, y, z));
                    }
                }
            }
            cut.stones.push((n, k));
        }
    }
    cut
}

/// `LOOSE_TRACE=1`: a line of what was done, for whoever is finding out why.
fn trace(what: std::fmt::Arguments) {
    if std::env::var_os("LOOSE_TRACE").is_some() {
        eprintln!("loose: {what}");
    }
}

/// Whether a hunk's cell may go where this is: into air, water, what grows low, and leaf,
/// which gives way under it.
fn room(world: &World, cell: IVec3) -> bool {
    match world.get(cell) {
        // (A cell part filled with leaf and nothing else is leaf: a hunk could neither go
        // into one, which was taken, nor lie on one, which bears nothing, and was lifted
        // over it and let fall into it for as long as anyone watched. The developer saw
        // one "infinitely falling on itself", 2026-11-13.)
        Block::Broken => world.bits_at(cell).is_some_and(|bits| bits.iter().all(|b| !b.solid() || b.foliage())),
        there => !there.solid() || there.foliage(),
    }
}

/// The cells of a hunk, from its lowest corner.
fn hunk() -> impl Iterator<Item = IVec3> {
    (0..HUNK).flat_map(|y| (0..HUNK).flat_map(move |z| (0..HUNK).map(move |x| IVec3::new(x, y, z))))
}

/// The hunk a cell is part of: its lowest corner, and what it is of. Nothing is kept of
/// where hunks are: a hunk is the cells it is. It lies on the squares two cells a side that
/// start at even cells, and at whatever height it came to rest, counted in hunks from the
/// lowest of its kind under it (one put on another is one hunk higher).
pub fn hunk_at(world: &World, cell: IVec3) -> Option<(IVec3, Block)> {
    let block = world.get(cell);
    if !block.hunk() {
        return None;
    }
    let (x, z) = (cell.x.div_euclid(HUNK) * HUNK, cell.z.div_euclid(HUNK) * HUNK);
    let low = (1..64).map(|d| cell.y - d).find(|y| world.get(IVec3::new(x, *y, z)) != block).map_or(cell.y, |y| y + 1);
    let corner = IVec3::new(x, low + (cell.y - low).div_euclid(HUNK) * HUNK, z);
    hunk().all(|d| world.get(corner + d) == block).then_some((corner, block))
}

/// Every cell joined to one, by faces, edges or corners, that `holds` says is of the same
/// stuff: the piece a blow has struck. No further than 6,000 cells.
fn joined(world: &World, first: IVec3, holds: impl Fn(&World, IVec3) -> bool) -> Vec<IVec3> {
    let (mut body, mut queue, mut seen) = (vec![first], VecDeque::from([first]), HashSet::from([first]));
    while let Some(cell) = queue.pop_front() {
        for side in round() {
            let next = cell + side;
            if body.len() < 6_000 && holds(world, next) && seen.insert(next) {
                body.push(next);
                queue.push_back(next);
            }
        }
    }
    body
}

/// Puts hunks down: as many as asked, at the first of the places given (each a lowest
/// corner, with what it is ordered by) that have room. Says how many found room.
fn strew(world: &mut World, mut places: Vec<(f32, IVec3)>, many: usize, block: Block, changed: &mut HashSet<IVec3>) -> usize {
    places.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| (a.1.y, a.1.z, a.1.x).cmp(&(b.1.y, b.1.z, b.1.x))));
    let mut put = 0;
    for (_, corner) in places {
        if put == many {
            break;
        }
        if hunk().all(|d| room(world, corner + d)) {
            for d in hunk() {
                world.set(corner + d, block);
                changed.insert((corner + d).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            }
            put += 1;
        }
    }
    put
}

/// Sets one hunk down at a place: on the square it is over, at that height or the least
/// above it that has room, as high as three more hunks. Gives its corner, if it found room.
fn set_down(world: &mut World, at: Vec3, block: Block, changed: &mut HashSet<IVec3>) -> Option<IVec3> {
    let cell = (at / CELL).floor().as_ivec3();
    let (x, z) = (cell.x.div_euclid(HUNK) * HUNK, cell.z.div_euclid(HUNK) * HUNK);
    let corner = (0..=3 * HUNK).map(|up| IVec3::new(x, cell.y + up, z)).find(|c| hunk().all(|d| room(world, *c + d)))?;
    strew(world, vec![(0.0, corner)], 1, block, changed);
    Some(corner)
}

/// Takes what a tool's stuff is out of a box of cells, where `of` says a place is its own:
/// whole cells of it, and broken cells that hold nothing else. Says how much, in bits.
fn emptied(world: &mut World, lo: IVec3, hi: IVec3, stuff: impl Fn(Block) -> bool, of: impl Fn(Vec3) -> bool, changed: &mut HashSet<IVec3>) -> u32 {
    let mut bits = 0;
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let cell = IVec3::new(x, y, z);
                if !of((cell.as_vec3() + 0.5) * CELL) {
                    continue;
                }
                // (A broken cell that holds something else too gives up only its bits of this.)
                if world.get(cell) == Block::Broken && world.bits_at(cell).is_some_and(|b| b.iter().any(|b| stuff(*b)) && !b.iter().all(|b| *b == Block::Air || stuff(*b))) {
                    for i in 0..SPLIT * SPLIT * SPLIT {
                        let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / SPLIT % SPLIT, i / (SPLIT * SPLIT));
                        if stuff(world.bit(bit)) {
                            world.set_bit(bit, Block::Air);
                            bits += 1;
                        }
                    }
                    changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                    continue;
                }
                let held = match world.get(cell) {
                    Block::Broken => world.bits_at(cell).filter(|b| b.iter().all(|b| *b == Block::Air || stuff(*b))).map_or(0, |b| b.iter().filter(|b| stuff(**b)).count() as u32),
                    b if stuff(b) => (SPLIT * SPLIT * SPLIT) as u32,
                    _ => 0,
                };
                if held > 0 {
                    world.set(cell, Block::Air);
                    changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                    bits += held;
                }
            }
        }
    }
    bits
}

const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];

/// Every cell round a cell: by a face, an edge or a corner. What bears is joined through any
/// of them: a limb a hand thick that climbs aslant passes from cell to cell by their edges,
/// and by faces alone half the limbs of a wood hang in the air unjoined.
fn round() -> impl Iterator<Item = IVec3> {
    (-1..=1).flat_map(|y| (-1..=1).flat_map(move |z| (-1..=1).map(move |x| IVec3::new(x, y, z)))).filter(|d| *d != IVec3::ZERO)
}

/// Where a lump of earth let go at `middle` comes to rest: straight down onto what is under
/// it, and from there down any side that falls away by more than `STEEP`, a lump's width at
/// a time, until it lies on ground no steeper. From the cells alone, in a fixed order: the
/// same put on the same world rests in the same place, which the journal's replay needs.
/// (Until 2026-11-13 it stayed where it was let go: each lump on the one before, earth
/// could be built out into the air like stone.)
fn heaped(world: &World, middle: Vec3, radius: f32) -> Vec3 {
    const STEEP: f32 = 0.45;
    let under = |at: Vec3| world.raycast(at, Vec3::NEG_Y, 48.0);
    let Some(mut drop) = under(middle) else { return middle };
    let mut at = middle;
    for _ in 0..24 {
        let sides = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z].map(|d| at + d * radius * 2.0);
        // (A side it can get to: not through what stands beside it.)
        let lower = sides.into_iter().filter(|s| !world.get((*s / CELL).floor().as_ivec3()).solid()).filter_map(|s| under(s).map(|d| (d, s))).filter(|(d, _)| *d > drop + STEEP).max_by(|a, b| a.0.total_cmp(&b.0));
        let Some((d, side)) = lower else { break };
        (drop, at) = (d, side);
    }
    // (Sunk a third into what it lies on, so it is one with it.)
    at - Vec3::Y * (drop - radius * 0.6).max(0.0)
}

impl Loose {
    /// Notes each tree and boulder a cut has made cells of, once, as a [`Thing`].
    fn note(&mut self, cut: &Cut, forms: &[&Form]) {
        for (n, tree) in &cut.trees {
            let limbs: Vec<crate::plants::Limb> = forms[*n].limbs.iter().filter(|l| l.tree == *tree).copied().collect();
            // (Once: whoever keeps the shapes is to drop it from them, and may not have yet.)
            if self.things.iter().any(|t| t.limbs.first().map(|l| l.a) == limbs.first().map(|l| l.a)) {
                continue;
            }
            self.next += 1;
            self.things.push(Thing { id: self.next, limbs, clumps: forms[*n].clumps.iter().filter(|c| c.tree == *tree).copied().collect(), stone: None, cuts: Vec::new(), lies: None, blows: 0, limbed: false });
        }
        for (n, stone) in &cut.stones {
            if self.things.iter().any(|t| t.stone.as_ref().map(|s| s.middle) == forms[*n].stones.get(*stone).map(|s| s.middle)) {
                continue;
            }
            self.next += 1;
            self.things.push(Thing { id: self.next, limbs: Vec::new(), clumps: Vec::new(), stone: forms[*n].stones.get(*stone).cloned(), cuts: Vec::new(), lies: None, blows: 0, limbed: false });
        }
    }

    /// Digs a ball out of the world and looses what that leaves unsupported, pushed the way
    /// `push` points (the way whoever cut it was facing). Gives the chunks that changed.
    ///
    /// `forms` are the shapes of what grows round the cut, as `plants` noted them: a tree or
    /// a boulder among them that the ball reaches is made wrought first.
    ///
    /// Only `stuff` is cut, where one is given (what a tool cuts: an axe takes the wood out
    /// of the ball and leaves the earth); with none, everything in the ball goes.
    pub fn cut(&mut self, world: &mut World, middle: Vec3, radius: f32, push: Vec3, forms: &[&Form], stuff: Option<Stuff>) -> Cut {
        self.journal.push((self.steps, Edit::Cut { middle, radius, push, stuff }));
        let began = std::time::Instant::now();
        let (lo, hi) = (((middle - radius) / CELL).floor().as_ivec3() - 1, ((middle + radius) / CELL).floor().as_ivec3() + 1);
        // (A tree is made wrought only by what cuts wood, a boulder by what cuts stone.)
        // (A tree already cut into is cells, whether or not whoever keeps the shapes has
        // dropped it from them yet.)
        let known: Vec<Vec3> = self.things.iter().filter_map(|t| t.limbs.first().map(|l| l.a)).collect();
        let mut cut = work(world, middle, radius, forms, stuff, &known);
        // Each thing made wrought is noted, and every bite out of a thing noted on it.
        self.note(&cut, forms);
        for thing in self.things.iter_mut().filter(|t| t.lies.is_none() && t.reached(middle, radius)) {
            thing.cuts.push((middle, radius, stuff));
        }
        cut.taken = carve(world, middle, radius, stuff);
        // A trunk cut most of the way through snaps.
        if cut.taken[Stuff::Wood as usize] > 0 {
            cut.taken[Stuff::Wood as usize] += snap(world, middle, radius);
        }
        if cut.taken.iter().any(|t| *t > 0) {
            self.cuts.push((middle, cut.taken));
        }
        let mut changed = std::mem::take(&mut cut.changed);
        // Stone dug from the land with a pick comes away as a hunk: a bite that took half
        // a hunk's worth or more leaves one, on the side the blow came from, to be carried
        // off. (By the bite alone, with nothing kept from one to the next: a region's
        // edits are done again on their own, `Replay`.) Not from a boulder, which is
        // chipped ([`Loose::chip`]).
        let boulder = self.things.iter().any(|t| t.stone.is_some() && t.reached(middle, radius));
        if stuff == Some(Stuff::Stone) && !boulder && cut.taken[Stuff::Stone as usize] * 2 >= (SPLIT * SPLIT * SPLIT * HUNK * HUNK * HUNK) as u32 {
            let back = middle - push.with_y(0.0).normalize_or_zero() * (radius + CELL);
            if let Some(corner) = set_down(world, back, Block::Cobble, &mut changed) {
                changed.extend(self.loosen(world, corner - 1, corner + HUNK, push));
            }
        }
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                changed.insert(IVec3::new(x, lo.y, z).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                changed.insert(IVec3::new(x, hi.y, z).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            }
        }
        changed.extend(self.loosen(world, lo, hi, push));
        changed.extend(flow(world, lo, hi));
        // `LOOSE_TRACE=1`: each cut in a line (where, what it took, what is loose after it).
        if std::env::var_os("LOOSE_TRACE").is_some() {
            let around = |y: f32| { let c = ((middle.with_y(y)) / CELL).floor().as_ivec3(); (-3..=3).flat_map(|z| (-3..=3).map(move |x| c + IVec3::new(x, 0, z))).map(|c| format!("{:?}", world.get(c)).chars().next().unwrap_or('?')).collect::<String>() };
            eprintln!("loose: cut at {middle:.2?} r {radius:.2} of {stuff:?} · took {:?} · made wrought {} in {:.1} ms · {} loose · cells at the cut {} · over it {}", cut.taken, cut.worked, began.elapsed().as_secs_f32() * 1000.0, self.pieces.len(), around(middle.y), around(middle.y + radius + 0.3));
        }
        let _ = began;
        Cut { changed, ..cut }
    }

    /// A blow of an axe on a tree. An axe does not dig wood as a shovel digs earth: it does
    /// to the tree it strikes what is next for that tree (the developer, 2026-11-13).
    ///
    /// 1. Standing, the blow goes into one notch a metre up its trunk, on the side it came
    ///    from, and the tree stands until it has taken as many as its girth asks ([`BLOWS`]):
    ///    the last cuts it through, and it falls.
    /// 2. Fallen, it is still a tree: a blow knocks off all its boughs and leaf, which are
    ///    noted in `shed` for whoever shows them.
    /// 3. Bare, it is timber ([`Loose::timber`]): a blow breaks what it strikes through at
    ///    the nearest of the places it breaks, one to each [`LOG`] of its length.
    /// 4. And a blow at a log splits it into hunks ([`HUNK`]): pieces of wood all of one
    ///    size, to be carried and built with.
    ///
    /// Says what changed as a cut does; a tree it made cells of is in `trees`.
    pub fn chop(&mut self, world: &mut World, at: Vec3, push: Vec3, forms: &[&Form]) -> Cut {
        self.journal.push((self.steps, Edit::Chop { at, push }));
        // The tree struck: one already cut into, as it stands or as it lies; or one still as
        // it grew, which is made cells and noted first.
        let reached = |t: &Thing| {
            let p = t.lies.map_or(at, |(pivot, shift, turn)| pivot + turn.inverse() * (at - pivot - shift));
            t.limbs.iter().map(|l| from_line(p, l.a, l.b) - l.ra.max(l.rb)).fold(f32::MAX, f32::min)
        };
        // (Not a stump or a log lying where it grew, which an axe does nothing to: one
        // beside a great tree was nearer the blow than the tree's own trunk, took every
        // blow, and the tree stood. Something with three metres of height to it.)
        let tree = |t: &Thing| t.lies.is_some() || t.limbs.iter().map(|l| l.a.y.max(l.b.y)).fold(f32::MIN, f32::max) - t.limbs.iter().map(|l| l.a.y.min(l.b.y)).fold(f32::MAX, f32::min) >= 3.0;
        let near = |things: &[Thing]| things.iter().enumerate().filter(|(_, t)| !t.limbs.is_empty() && tree(t) && !(t.lies.is_some() && t.limbed)).map(|(n, t)| (reached(t), n)).filter(|(d, _)| *d < 0.7).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, n)| n);
        let mut cut = Cut::default();
        let mut which = near(&self.things);
        // (No tree that stands or lies in its boughs: then it is at timber, if any is there.)
        if which.is_none() && let Some(cut) = self.timber(world, at, push) {
            return cut;
        }
        if which.is_none() {
            let known: Vec<Vec3> = self.things.iter().filter_map(|t| t.limbs.first().map(|l| l.a)).collect();
            cut = work(world, at, 0.5, forms, Some(Stuff::Wood), &known);
            self.note(&cut, forms);
            which = near(&self.things);
        }
        let Some(n) = which else { return cut };
        let thing = self.things[n].clone();
        let wood = |b: Block| b.wood() || b == Block::Broken;
        let mut gone = 0u32;
        let clear = |world: &mut World, cell: IVec3, cut: &mut Cut| {
            world.set(cell, Block::Air);
            cut.changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        };
        match thing.lies {
            None => {
                let Some(trunk) = thing.limbs.iter().find(|l| l.roots > 0).or(thing.limbs.first()).copied() else { return cut };
                let up = ((crate::plants::Limb::UNDER + 0.9) / (trunk.b.y - trunk.a.y).max(0.1)).min(0.8);
                let (middle, thick) = (trunk.a.lerp(trunk.b, up), trunk.ra + (trunk.rb - trunk.ra) * up);
                let needs = BLOWS(thick);
                // (Through already, and on its way down: a blow at where it stood is at nothing.)
                if thing.blows >= needs {
                    return cut;
                }
                let blows = thing.blows + 1;
                self.things[n].blows = blows;
                trace(format_args!("chop: standing, blow {blows} of {needs}"));
                let into = push.with_y(0.0).try_normalize().unwrap_or(Vec3::X);
                if blows < needs {
                    // The notch, a blow deeper: a wedge on the side the blows come from.
                    let deep = thick * 2.0 * blows as f32 / needs as f32 * 0.6;
                    let taken = carve(world, middle - into * (thick * 1.5 - deep) , thick * 0.75, Some(Stuff::Wood));
                    cut.taken[Stuff::Wood as usize] += taken[Stuff::Wood as usize];
                    let r = (thick * 3.0 / CELL).ceil() as i32 + 1;
                    let c = (middle / CELL).floor().as_ivec3();
                    // (Should a notch ever part it, it falls as if cut through.)
                    cut.changed.extend(self.loosen(world, c - r, c + r, push));
                } else {
                    // Through: a cell's height of its trunk, all the way across.
                    let (c, r) = ((middle / CELL).floor().as_ivec3(), ((thick * 2.4 + 0.2) / CELL).ceil() as i32);
                    for z in -r..=r {
                        for x in -r..=r {
                            for y in 0..=1 {
                                let cell = c + IVec3::new(x, y, z);
                                if x * x + z * z <= r * r && wood(world.get(cell)) {
                                    clear(world, cell, &mut cut);
                                    gone += 1;
                                }
                            }
                        }
                    }
                    // (A great tree is more cells than a body may be and still be looked
                    // through, and was taken for the land: cut clean through, it stood in
                    // the air. The developer saw one, 2026-11-13; of 35 trees tried then, the
                    // great ones did not come loose. Here it is known to be a tree.)
                    self.whole = true;
                    cut.changed.extend(self.loosen(world, c - IVec3::new(r, 1, r), c + IVec3::new(r, 2, r), push));
                    self.whole = false;
// What of its leaf did not go with it is shed where it hung: a piece takes the
                    // leaf near its wood, and a weeping tree's long strands were left hanging in
                    // the air where it had stood (seen in the game, 2026-11-13).
                    for clump in &thing.clumps {
                        let (bl, bh) = (((clump.middle - clump.radii) / CELL).floor().as_ivec3(), ((clump.middle + clump.radii) / CELL).floor().as_ivec3());
                        for y in bl.y..=bh.y {
                            for z in bl.z..=bh.z {
                                for x in bl.x..=bh.x {
                                    let cell = IVec3::new(x, y, z);
                                    let b = world.get(cell);
                                    // (Its own: within the mass it grew as, not all of the box round it.)
                                    if b.foliage() && !b.shaped() && (((cell.as_vec3() + 0.5) * CELL - clump.middle) / (clump.radii + CELL)).length_squared() < 1.0 {
                                        clear(world, cell, &mut cut);
                                        if (x + y * 3 + z * 7).rem_euclid(23) == 0 {
                                            self.shed.push(((cell.as_vec3() + 0.5) * CELL, b));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some((pivot, shift, turn)) => {
                let (now, was) = (|p: Vec3| pivot + shift + turn * (p - pivot), |p: Vec3| pivot + turn.inverse() * (p - pivot - shift));
                let stout = thing.limbs.iter().map(|l| l.ra.max(l.rb)).fold(0.0, f32::max) * 0.34;
                let heavy: Vec<crate::plants::Limb> = thing.limbs.iter().filter(|l| l.ra.max(l.rb) >= stout).copied().collect();
                let ends = thing.limbs.iter().flat_map(|l| [(l.a, l.ra.max(l.rb)), (l.b, l.ra.max(l.rb))]).chain(thing.clumps.iter().map(|c| (c.middle, c.radii.max_element())));
                let (mut from, mut to) = (Vec3::MAX, Vec3::MIN);
                for (p, r) in ends {
                    (from, to) = (from.min(now(p) - r - 0.5), to.max(now(p) + r + 0.5));
                }
                let (bl, bh) = ((from / CELL).floor().as_ivec3(), (to / CELL).floor().as_ivec3());
                let (lo, hi);
                {
                    // Its boughs and leaf: whatever of this tree is not its trunk or a heavy limb.
                    for y in bl.y..=bh.y {
                        for z in bl.z..=bh.z {
                            for x in bl.x..=bh.x {
                                let cell = IVec3::new(x, y, z);
                                let b = world.get(cell);
                                if !(b.wood() || b.foliage()) || b.shaped() {
                                    continue;
                                }
                                let here = (cell.as_vec3() + 0.5) * CELL;
                                let p = was(here);
                                // (Its wood there: leaf against its trunk goes with the rest.)
                                if b.wood() && heavy.iter().any(|l| from_line(p, l.a, l.b) < l.ra.max(l.rb) + CELL * 1.5) {
                                    continue;
                                }
                                let its = thing.limbs.iter().any(|l| from_line(p, l.a, l.b) < l.ra.max(l.rb) + CELL * 2.0) || thing.clumps.iter().any(|c| ((p - c.middle) / (c.radii + CELL * 2.0)).length_squared() < 1.0);
                                if !its {
                                    continue;
                                }
                                clear(world, cell, &mut cut);
                                gone += b.wood() as u32;
                                if (x + y * 3 + z * 7).rem_euclid(9) == 0 {
                                    self.shed.push((here, b));
                                }
                            }
                        }
                    }
                    // (All of them at the one blow, wherever on it the blow lands: "the first
                    // chop probably should cut away the branches", the developer, having
                    // felled one, 2026-11-13. By how far a blow reached it took two, and
                    // before that a willow was never bare.)
                    self.things[n].limbed = true;
                    trace(format_args!("chop: fallen, its boughs and leaf knocked off"));
                    (lo, hi) = (bl, bh);
                }
                // (What a blow leaves on nothing comes down in its turn: a bough's end, its trunk.)
                cut.changed.extend(self.loosen(world, lo - 1, hi + 1, push));
            }
        }
        // (One of timber to carry for each two cells of wood a blow takes, by a cut's own count.)
        cut.taken[Stuff::Wood as usize] += gone * (SPLIT * SPLIT * SPLIT) as u32 / 2;
        self.cuts.push((at, cut.taken));
        cut
    }

    /// A blow of an axe at timber: wood that lies cut, whatever tree it was and wherever it
    /// has come to rest since. The piece struck is all the cut wood joined to what the blow
    /// landed on. Longer than a log and a half, it is broken through at the nearest of its
    /// break points, one to each [`LOG`] from its end; a log, it is split into hunks, as
    /// many as it has wood for, which lie where it lay. Nothing, if no cut wood is there.
    ///
    /// (By what is there, not by where a tree's logs ought to be: asked of the tree, a log
    /// that had dropped a metre when its trunk was broken was not found by any blow, and
    /// half the wood of a thin tree could not be split. `cargo world hunks`, 2026-11-13.)
    fn timber(&mut self, world: &mut World, at: Vec3, push: Vec3) -> Option<Cut> {
        let holds = |world: &World, cell: IVec3| match world.get(cell) {
            Block::Broken => world.bits_at(cell).is_some_and(|b| b.iter().any(|b| b.wood() && !b.shaped())),
            b => b.wood() && !b.shaped(),
        };
        // (Not what stands: a tree cut into and not yet down is for the blows that fell it.)
        let stands = |p: Vec3| self.things.iter().any(|t| t.lies.is_none() && t.limbs.iter().any(|l| from_line(p, l.a, l.b) < l.ra.max(l.rb) * if l.roots > 0 { 2.6 } else { 1.0 } + CELL * 2.0));
        let c = (at / CELL).floor().as_ivec3();
        let first = (-2..=2).flat_map(|y| (-2..=2).flat_map(move |z| (-2..=2).map(move |x| c + IVec3::new(x, y, z)))).filter(|cell| holds(world, *cell)).map(|cell| (((cell.as_vec3() + 0.5) * CELL).distance(at), cell)).filter(|(d, _)| *d < 0.45).min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| (a.1.y, a.1.z, a.1.x).cmp(&(b.1.y, b.1.z, b.1.x))))?.1;
        if stands((first.as_vec3() + 0.5) * CELL) {
            return None;
        }
        // The piece: every cell of cut wood joined to that one.
        let body = joined(world, first, holds);
        // Its length: from the cell furthest from the blow to the one furthest from that.
        let middle = |c: &IVec3| (c.as_vec3() + 0.5) * CELL;
        let furthest = |from: Vec3| body.iter().map(middle).max_by(|a, b| a.distance(from).total_cmp(&b.distance(from)).then_with(|| (a.x, a.y, a.z).partial_cmp(&(b.x, b.y, b.z)).unwrap_or(std::cmp::Ordering::Equal))).unwrap_or(from);
        let a = furthest(at);
        let b = furthest(a);
        let (long, along) = (a.distance(b), (b - a).normalize_or(Vec3::X));
        let mut cut = Cut::default();
        let (mut lo, mut hi) = (IVec3::MAX, IVec3::MIN);
        let mut wood = 0u32;
        // (Its own wood out of a cell: all of a whole one, the bits of a broken one.)
        let mut empty = |world: &mut World, cell: IVec3, cut: &mut Cut| {
            if world.get(cell) == Block::Broken {
                for i in 0..SPLIT * SPLIT * SPLIT {
                    let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / SPLIT % SPLIT, i / (SPLIT * SPLIT));
                    if world.bit(bit).wood() {
                        world.set_bit(bit, Block::Air);
                        wood += 1;
                    }
                }
            } else {
                world.set(cell, Block::Air);
                wood += (SPLIT * SPLIT * SPLIT) as u32;
            }
            cut.changed.insert(cell.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
            (lo, hi) = (lo.min(cell), hi.max(cell));
        };
        let places = ((long - LOG * 0.5) / LOG).floor().max(0.0) as u32;
        if long > LOG * 1.5 && places > 0 {
            // Broken through, a cell's width of it, at the place nearest the blow.
            let k = ((at - a).dot(along) / LOG).round().clamp(1.0, places as f32);
            for cell in &body {
                if ((middle(cell) - a).dot(along) - k * LOG).abs() < CELL * 0.75 {
                    empty(world, *cell, &mut cut);
                    if (cell.x + cell.y + cell.z).rem_euclid(4) == 0 {
                        self.shed.push((middle(cell), Block::Timber));
                    }
                }
            }
            trace(format_args!("chop: timber {long:.1} m long, broken at place {k} of {places}"));
        } else {
            // A log: all of it into hunks, nearest its middle line first.
            let mut places: Vec<(f32, IVec3)> = body.iter().flat_map(|c| [0, -1].map(|down| IVec3::new(c.x.div_euclid(HUNK) * HUNK, c.y + down, c.z.div_euclid(HUNK) * HUNK))).collect::<HashSet<_>>().into_iter().map(|corner| (from_line((corner.as_vec3() + HUNK as f32 * 0.5) * CELL, a, b), corner)).collect();
            places.sort_by(|x, y| x.0.total_cmp(&y.0).then_with(|| (x.1.y, x.1.z, x.1.x).cmp(&(y.1.y, y.1.z, y.1.x))));
            for cell in &body {
                empty(world, *cell, &mut cut);
            }
            // (To the nearest hunk, and none from a scrap of under half of one: with one
            // at the least from anything, a tree's twigs' ends made a fifth more wood than
            // it had.)
            let many = (wood as f32 * YIELD / (SPLIT * SPLIT * SPLIT * HUNK * HUNK * HUNK) as f32).round() as usize;
            let put = strew(world, places, many, Block::Billet, &mut cut.changed);
            for k in 0..6 {
                self.shed.push((a.lerp(b, (k as f32 + 0.5) / 6.0) + Vec3::Y * 0.3, Block::Timber));
            }
            trace(format_args!("chop: a log {long:.1} m long split into {put} hunks of {many}"));
        }
        if lo.x <= hi.x {
            cut.changed.extend(self.loosen(world, lo - HUNK - 1, hi + HUNK + 1, push));
        }
        cut.taken[Stuff::Wood as usize] = wood;
        self.cuts.push((at, cut.taken));
        Some(cut)
    }

    /// A blow of a pick at loose stone: rubble, what a boulder or a cliff became when it came
    /// down, or what is left where a boulder broke. As timber to an axe: the piece struck
    /// is all the loose stone joined to what the blow landed on, and it breaks into hunks,
    /// as many as it has stone for; a scrap of under half a hunk is gone in chips. Nothing,
    /// if no loose stone is there, or it is more than a blow breaks (some 12 m3).
    fn rubble(&mut self, world: &mut World, at: Vec3, push: Vec3) -> Option<Cut> {
        let holds = |world: &World, cell: IVec3| match world.get(cell) {
            Block::Broken => world.bits_at(cell).is_some_and(|b| b.contains(&Block::Shard)),
            b => b == Block::Shard,
        };
        let c = (at / CELL).floor().as_ivec3();
        let first = (-2..=2).flat_map(|y| (-2..=2).flat_map(move |z| (-2..=2).map(move |x| c + IVec3::new(x, y, z)))).filter(|cell| holds(world, *cell)).map(|cell| (((cell.as_vec3() + 0.5) * CELL).distance(at), cell)).filter(|(d, _)| *d < 0.45).min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| (a.1.y, a.1.z, a.1.x).cmp(&(b.1.y, b.1.z, b.1.x))))?.1;
        let body = joined(world, first, holds);
        if body.len() > 800 {
            return None;
        }
        self.journal.push((self.steps, Edit::Chip { at, push }));
        let mut cut = Cut::default();
        let (lo, hi) = body.iter().fold((IVec3::MAX, IVec3::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
        let middle = body.iter().map(|c| (c.as_vec3() + 0.5) * CELL).sum::<Vec3>() / body.len() as f32;
        // (From the ground up and from its middle out, on the squares its cells are over.)
        let places: Vec<(f32, IVec3)> = body.iter().map(|c| IVec3::new(c.x.div_euclid(HUNK) * HUNK, c.y, c.z.div_euclid(HUNK) * HUNK)).collect::<HashSet<_>>().into_iter().map(|corner| (corner.y as f32 * 8.0 + ((corner.as_vec3() + HUNK as f32 * 0.5) * CELL - middle).with_y(0.0).length(), corner)).collect();
        let own: HashSet<IVec3> = body.iter().copied().collect();
        let stone = emptied(world, lo, hi, |b| b == Block::Shard, |p| own.contains(&(p / CELL).floor().as_ivec3()), &mut cut.changed);
        let many = (stone as f32 / (SPLIT * SPLIT * SPLIT * HUNK * HUNK * HUNK) as f32).round() as usize;
        let put = strew(world, places, many, Block::Cobble, &mut cut.changed);
        for k in 0..4 {
            self.shed.push((middle + Vec3::new(k as f32 * 0.15 - 0.2, 0.2, 0.0), Block::Shard));
        }
        trace(format_args!("chip: rubble of {} cells broke into {put} hunks of {many}", body.len()));
        cut.changed.extend(self.loosen(world, lo - HUNK - 1, hi + HUNK + 1, push));
        cut.taken[Stuff::Stone as usize] = stone;
        self.cuts.push((at, cut.taken));
        Some(cut)
    }

    /// A blow of a pick on a boulder. As an axe to a tree, so a pick to a boulder: it is
    /// chipped, a little off it at each blow, until it has taken as many as its size asks
    /// ([`CHIPS`]), and at the last it breaks into hunks, as many as it has stone for, lying
    /// where it lay. Nothing, if what is struck is no boulder (the land's own stone is dug).
    pub fn chip(&mut self, world: &mut World, at: Vec3, push: Vec3, forms: &[&Form]) -> Option<Cut> {
        // (By where the boulder lies, not by what is at the blow: a blow into the hollow an
        // earlier one left is at air, and was taken for a blow at no boulder.)
        // Every boulder the blow reaches is cells from now and noted, and the blow is at the
        // one it is deepest in that is not yet broken. (The first found that it reached: of
        // two that lay against each other the one already noted took every blow at the
        // other, which never broke: seed 5, 2026-11-13.)
        let cut = work(world, at, 0.3, forms, Some(Stuff::Stone), &[]);
        self.note(&cut, forms);
        let deep = |t: &Thing| t.stone.as_ref().filter(|s| t.blows < CHIPS(s.radii.max_element())).map(|s| ((at - s.middle) / (s.radii + 0.35)).length_squared()).filter(|d| *d < 1.0);
        let Some(n) = self.things.iter().enumerate().filter_map(|(n, t)| deep(t).map(|d| (d, n))).min_by(|a, b| a.0.total_cmp(&b.0)).map(|b| b.1) else {
            return self.rubble(world, at, push);
        };
        let mut cut = cut;
        let Some(stone) = self.things[n].stone.clone() else { return Some(cut) };
        let needs = CHIPS(stone.radii.max_element());
        self.journal.push((self.steps, Edit::Chip { at, push }));
        self.things[n].blows += 1;
        let (lo, hi);
        if self.things[n].blows < needs {
            cut.taken = carve(world, at, 0.22, Some(Stuff::Stone));
            (lo, hi) = (((at - 0.5) / CELL).floor().as_ivec3(), ((at + 0.5) / CELL).floor().as_ivec3());
            for k in 0..3 {
                self.shed.push((at + Vec3::new(k as f32 * 0.1 - 0.1, 0.1, 0.0), Block::Shard));
            }
        } else {
            let reach = stone.radii.max_element() + 0.3;
            (lo, hi) = (((stone.middle - reach) / CELL).floor().as_ivec3(), ((stone.middle + reach) / CELL).floor().as_ivec3());
            let its = |here: Vec3| here.distance(stone.middle) < reach;
            let held = emptied(world, lo, hi, |b| b == Block::Shard, its, &mut cut.changed);
            let many = ((held as f32 / (SPLIT * SPLIT * SPLIT * HUNK * HUNK * HUNK) as f32).round() as usize).max(1);
            // From the ground up and from its middle out: a heap where it lay.
            let mut places = Vec::new();
            for y in lo.y..=hi.y {
                for z in (lo.z.div_euclid(HUNK) * HUNK..=hi.z).step_by(HUNK as usize) {
                    for x in (lo.x.div_euclid(HUNK) * HUNK..=hi.x).step_by(HUNK as usize) {
                        let corner = IVec3::new(x, y, z);
                        let middle = (corner.as_vec3() + HUNK as f32 * 0.5) * CELL;
                        if its(middle) {
                            places.push((y as f32 * 8.0 + (middle - stone.middle).with_y(0.0).length(), corner));
                        }
                    }
                }
            }
            let put = if held > 0 { strew(world, places, many, Block::Cobble, &mut cut.changed) } else { 0 };
            trace(format_args!("chip: a boulder {:.1} m across broke at blow {needs} into {put} hunks of {many}", stone.radii.max_element() * 2.0));
            for k in 0..8 {
                let turn = k as f32 * 0.8;
                self.shed.push((stone.middle + Vec3::new(turn.cos(), 0.4, turn.sin()) * stone.radii * 0.7, Block::Shard));
            }
            cut.taken[Stuff::Stone as usize] = held;
        }
        cut.changed.extend(self.loosen(world, lo - 1, hi + 1, push));
        self.cuts.push((at, cut.taken));
        Some(cut)
    }

    /// Takes up the hunk that is at a place, if one is: it is gone from the world, and what
    /// lay on it comes down. Gives what it was of, and the chunks changed.
    pub fn lift(&mut self, world: &mut World, at: Vec3) -> Option<(Block, HashSet<IVec3>)> {
        let (corner, block) = hunk_at(world, (at / CELL).floor().as_ivec3())?;
        self.journal.push((self.steps, Edit::Lift { at }));
        self.restless.clear();
        let mut changed = HashSet::new();
        for d in hunk() {
            world.set(corner + d, Block::Air);
            changed.insert((corner + d).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        }
        changed.extend(self.loosen(world, corner - 1, corner + HUNK, Vec3::X));
        Some((block, changed))
    }

    /// Puts a hunk down at a place: on the square it is over, at that height or the least
    /// above it that has room (as high as three more hunks: what a person lifts to), and
    /// from there it comes down onto what is under it. Nothing, if there is no room; else
    /// the corner it was put at, and the chunks changed.
    pub fn lay(&mut self, world: &mut World, at: Vec3, block: Block) -> Option<(IVec3, HashSet<IVec3>)> {
        let mut changed = HashSet::new();
        let corner = set_down(world, at, block, &mut changed)?;
        self.journal.push((self.steps, Edit::Lay { at, block }));
        changed.extend(self.loosen(world, corner - 1, corner + HUNK, Vec3::X));
        Some((corner, changed))
    }

    /// (Earth does not stay where it is let go: it falls to what is under it and runs down
    /// off a heap, [`heaped`]. Stone and timber stay, which is building.)
    /// Puts a ball of something down, into air only (what is put down lies on what is there,
    /// not through it). Gives the chunks changed.
    pub fn put(&mut self, world: &mut World, middle: Vec3, radius: f32, block: Block) -> HashSet<IVec3> {
        self.journal.push((self.steps, Edit::Put { middle, radius, block }));
        let middle = if block == Block::Dirt { heaped(world, middle, radius) } else { middle };
        // A piece of stone or timber in hand (a lump's size) is put down as the whole cell
        // it is in: what is built is square and its faces meet, a wall and not a heap of
        // balls (seven pieces put one on another were one brown lump, 2026-11-13). Earth
        // is a lump, and anything larger is the ball asked for.
        if block != Block::Dirt && radius < CELL {
            let cell = (middle / CELL).floor().as_ivec3();
            if !world.get(cell).solid() || world.get(cell) == Block::Broken {
                world.set(cell, block);
            }
        } else {
            world.ellipsoid(middle, Vec3::splat(radius), block, true);
        }
        // (And the cells beside it, whose faces toward it are drawn or not by what it is.)
        let (lo, hi) = (((middle - radius) / CELL).floor().as_ivec3() - 1, ((middle + radius) / CELL).floor().as_ivec3() + 1);
        let mut changed: HashSet<IVec3> = (lo.y..=hi.y).flat_map(|y| (lo.z..=hi.z).flat_map(move |z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z).div_euclid(IVec3::splat(cube_voxel::CHUNK))))).collect();
        // (What is put where nothing holds it comes down like anything else cut loose: a
        // stone let go in the air hung there until 2026-11-13.)
        if block != Block::Dirt {
            changed.extend(self.loosen(world, lo, hi, Vec3::X));
        }
        changed
    }

    /// Looks through what is solid in a box of cells for anything no longer held, and takes
    /// each such thing out of the world as a piece.
    pub fn loosen(&mut self, world: &mut World, lo: IVec3, hi: IVec3, push: Vec3) -> HashSet<IVec3> {
        let mut changed = HashSet::new();
        // Every cell looked through, and in which search: one that meets an earlier search's
        // cells is of the same body, and that body was held (a loose one is out of the world).
        let mut seen: HashMap<IVec3, u32> = HashMap::new();
        let mut search = 0;
        // A tree holds only itself up. Where two that have been cut into stand close, a cell
        // is the one's whose wood it is nearest, and what is one tree's is not joined to
        // what is another's: a birch cut clean through hung by its twigs from the tree half
        // a metre off, which the same cut had made cells, and never fell (seed 1,
        // 2026-11-13). Only looked into where two such trees stand.
        let trees: Vec<(u32, Vec<(Vec3, Vec3, f32, f32)>)> = self.things.iter().filter(|t| t.lies.is_none() && !t.limbs.is_empty()).map(|t| (t.id, t.limbs.iter().map(|l| (l.a, l.b, l.ra, l.rb)).collect())).collect();
        let mut whose: HashMap<IVec3, u32> = HashMap::new();
        let mut of = |cell: IVec3| -> u32 {
            *whose.entry(cell).or_insert_with(|| {
                let p = (cell.as_vec3() + 0.5) * CELL;
                trees.iter().filter_map(|(id, limbs)| limbs.iter().map(|l| outside(p, l)).min_by(|x, y| x.total_cmp(y)).map(|d| (d, *id))).filter(|(d, _)| *d < 0.5).min_by(|x, y| x.0.total_cmp(&y.0)).map_or(0, |(_, id)| id)
            })
        };
        let rivals = trees.len() > 1;
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
                                // (What lies on a boulder nobody has cut lies on something:
                                // a boulder as it grew is no body to look through, and a
                                // hunk that came down on one was let go again the moment it
                                // landed, for as long as anyone waited. Seed 5, 2026-11-13.)
                                // (Not so a tree nobody has cut: a felled one would hang on
                                // its neighbour's boughs.)
                                // (A hunk does lie on one's foot: it is no part of any tree.)
                                // (Its outside is cells part filled: by what is in them.)
                                let under = |of: &dyn Fn(Block) -> bool| match world.get(next) {
                                    Block::Broken => world.bits_at(next).is_some_and(|bits| bits.iter().any(|b| of(*b))),
                                    b => of(b),
                                };
                                if side == IVec3::NEG_Y && (under(&|b| b == Block::Rock) || world.get(cell).hunk() && under(&|b| b.wood())) {
                                    held = true;
                                    break 'through;
                                }
                                continue;
                            }
                            // A hunk is a body of its own, held by what is under it and by
                            // nothing beside it: joined as cells are, one put against
                            // another that lay on the ground hung in the air (four of a
                            // tree's twelve did, `cargo world hunks`, 2026-11-13).
                            let (this, that) = (world.get(cell), world.get(next));
                            if (this.hunk() || that.hunk()) && side != IVec3::NEG_Y && !(this == that && hunk_at(world, cell).map(|h| h.0) == hunk_at(world, next).map(|h| h.0)) {
                                continue;
                            }
                            if rivals && !earth && !world.get(next).ground() {
                                let (mine, its) = (of(cell), of(next));
                                if mine != 0 && its != 0 && mine != its {
                                    continue;
                                }
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
                        if own.len() > HELD && !self.whole {
                            held = true;
                            break;
                        }
                    }
                    if !held {
                        // Each hunk of it falls as itself, apart from the rest: a log's end
                        // that lay on one came down with it as one piece, which landed as
                        // cells do, and the hunk's cells that met the ground were put
                        // wherever there was room (seed 7, 2026-11-13).
                        let mut apart: HashMap<Option<IVec3>, Vec<IVec3>> = HashMap::new();
                        for cell in own {
                            apart.entry(hunk_at(world, cell).map(|h| h.0)).or_default().push(cell);
                        }
                        let mut apart: Vec<(Option<IVec3>, Vec<IVec3>)> = apart.into_iter().collect();
                        apart.sort_by_key(|(corner, _)| corner.map(|c| (c.y, c.z, c.x)));
                        for (corner, cells) in apart {
                            // A hunk that comes loose again from where it has just come to
                            // rest, four times running, is left there: it is on something,
                            // whatever the rules above make of it, and let go it would fall
                            // onto the same place for ever (the developer saw one do so,
                            // 2026-11-13, and it has not been made to happen since:
                            // `LOOSE_TRACE=1` says what was under it).
                            if let Some(corner) = corner {
                                let times = self.restless.entry(corner).or_insert(0);
                                *times += 1;
                                if *times > 4 {
                                    if *times == 5 {
                                        trace(format_args!("a hunk at {corner} would not settle, and is left there: under it {:?}", hunk().filter(|d| d.y == 0).map(|d| world.get(corner + d - IVec3::Y)).collect::<Vec<_>>()));
                                    }
                                    continue;
                                }
                            }
                            changed.extend(self.take(world, cells, push));
                        }
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
        // (A hunk has none: one that came down beside leaf lying on the ground took it up,
        // was no longer hunks and nothing else, and landed as cells do. Seed 7, 2026-11-13.)
        let bare = own.iter().all(|c| world.get(*c).hunk());
        let mut queue: VecDeque<IVec3> = if bare { VecDeque::new() } else { own.iter().copied().collect() };
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
            // (Stone that comes loose is rubble from then on, not the land: the land is drawn
            // eased into the ground round it and takes its colour, and a lump of it lying
            // in a meadow was drawn half grass.)
            let rubble = |b: Block| if matches!(b, Block::Stone | Block::Slate | Block::Chalk) { Block::Shard } else { b };
            let part = match world.get(cell) {
                Block::Broken => world.bits_at(cell).map(|b| Part::Bits(Box::new(b.map(rubble)))),
                Block::Air => None,
                whole => Some(Part::Whole(rubble(whole))),
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
        // (Hunks one on another are not a pole: they come straight down.)
        let hunk = cells.values().all(|p| matches!(p, Part::Whole(b) if b.hunk()));
        let over = (!hunk && tall > 2.0 && tall > wide * 2.5).then_some((way, 0.05, 0.16));
        // Where it is tried against the world: some hundreds of its cells, spread through it.
        // (Its wood and stone, not its leaves: a crown goes through another crown, and
        // into the ground a little, before the wood under it is stopped.)
        let stride = (of.len() / 300).max(1);
        let mut order: Vec<IVec3> = of.clone();
        order.sort_by_key(|c| (c.y, c.z, c.x));
        let feel: Vec<Vec3> = order.iter().step_by(stride).map(|c| (c.as_vec3() + 0.5) * CELL).collect();
        // Stone and earth break; wood (and what is mostly wood) does not.
        let brittle = !hunk && of.iter().filter(|c| match &cells[*c] { Part::Whole(b) => !b.wood(), Part::Bits(b) => !b.iter().any(|b| b.wood()) }).count() * 2 > of.len();
        self.next += 1;
        self.begun.push((pivot, cells.len(), over.is_some()));
        // (A tree gone over is the thing whose trunk stood here.)
        let thing = over.and_then(|_| self.things.iter().find(|t| t.lies.is_none() && t.foot().is_some_and(|f| f.with_y(0.0).distance(pivot.with_y(0.0)) < 1.5)).map(|t| t.id));
        // A tree comes down onto its trunk and its heavy boughs, not onto its twigs: what of
        // it is thinner than a third of its trunk gives way under it, and is not felt for.
        // (Felt for like the rest, every felled tree lay propped on its own branches, its
        // trunk a metre and a half to two and a half up at its middle and its top three to
        // five: measured on eight islands after the developer saw one lying level in the
        // air over a bush, 2026-11-13. What gives way is crushed where it goes into the
        // ground, as `land` puts back only what finds room.)
        let feel: Vec<Vec3> = match thing.and_then(|id| self.things.iter().find(|t| t.id == id)) {
            Some(tree) => {
                let thick = tree.limbs.iter().map(|l| l.ra.max(l.rb)).fold(0.0, f32::max) * 0.6;
                let heavy: Vec<&crate::plants::Limb> = tree.limbs.iter().filter(|l| l.ra.max(l.rb) >= thick).collect();
                let kept: Vec<Vec3> = feel.iter().copied().filter(|p: &Vec3| heavy.iter().any(|l| from_line(*p, l.a, l.b) < l.ra.max(l.rb) + CELL)).collect();
                if kept.len() >= 8 { kept } else { feel }
            }
            None => feel,
        };
        self.pieces.push(Piece { id: self.next, cells, pivot, shift: Vec3::ZERO, turn: Quat::IDENTITY, vel: Vec3::ZERO, over, spin: Vec3::ZERO, brittle, hunk, thing, base: Quat::IDENTITY, settling: false, spare: (pivot, tall * 0.3), tall, feel, age: 0.0, rolls: 0 });
        changed
    }

    /// Breaks a piece that has come down hard: what of it `keeps` says stays one piece (given
    /// back); the rest goes in lumps some three cells across, each thrown out from the
    /// middle and up (`throw` of how fast the whole came down), turning, to land on its own.
    /// What is too small to be a lump goes with the lump beside it; with none beside it, it
    /// is a small stone of its own.
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
        // What is too small to be a lump goes with the largest lump beside it. (It was dust,
        // and gone: a boulder is mostly its outside, and a third to a half of one that came
        // down was no longer anywhere, 2026-11-13.) In a fixed order, so the same fall
        // breaks the same way.
        let mut small: Vec<IVec3> = lumps.iter().filter(|(_, c)| c.len() < 4).map(|(k, _)| *k).collect();
        small.sort_by_key(|k| (k.y, k.z, k.x));
        for key in small {
            let beside = round().map(|d| key + d).filter(|k| lumps.get(k).is_some_and(|c| c.len() >= 4)).max_by_key(|k| (lumps[k].len(), k.y, k.z, k.x));
            if let Some(to) = beside {
                let cells = lumps.remove(&key).unwrap_or_default();
                lumps.get_mut(&to).unwrap().extend(cells);
            }
        }
        let heart = piece.pivot + Vec3::Y * piece.tall / 2.0;
        let mut keys: Vec<IVec3> = lumps.keys().copied().collect();
        keys.sort_by_key(|k| (k.y, k.z, k.x));
        for key in keys {
            let cells = lumps.remove(&key).unwrap_or_default();
            // (A lump of leaf alone is not a thing that falls: it is leaves, shed.)
            // (And a scrap of stone with no lump beside it is a small stone, not nothing.)
            if !cells.values().any(|p| match p { Part::Whole(b) => bears(*b), Part::Bits(b) => b.iter().any(|b| bears(*b)) }) {
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
                hunk: false,
                cells,
                // (Turning about its own middle, from where the whole had brought it.)
                pivot: middle,
                shift: piece.pivot + piece.shift + piece.turn * (middle - piece.pivot) - middle,
                turn: piece.turn,
                vel: piece.turn * (out * (0.18 + 0.1 * odd(3)) + Vec3::Y * (0.22 + 0.08 * odd(4))) * speed.min(14.0) * throw,
                over: None,
                spin: Vec3::new(odd(5), odd(6), odd(7)) * 5.0 * throw.min(1.0),
                brittle: false,
                thing: None,
                base: Quat::IDENTITY,
                settling: false,
                spare: (middle, 0.0),
                tall: 3.0 * CELL,
                feel,
                age: 0.0,
                rolls: 0,
            });
        }
        piece
    }

    /// One moment of everything loose: each piece falls or goes further over, and what
    /// meets the world is put back into it. Gives the chunks that changed.
    pub fn step(&mut self, world: &mut World, dt: f32) -> HashSet<IVec3> {
        self.steps += 1;
        let mut changed = HashSet::new();
        let mut down = Vec::new();
        // (Every tree that has been cut into and still stands, for what falls to go through: below.)
        let standing: Vec<(u32, Vec<(Vec3, Vec3, f32, f32)>)> = if self.pieces.is_empty() { Vec::new() } else { self.things.iter().filter(|t| t.lies.is_none() && !t.limbs.is_empty()).map(|t| (t.id, t.limbs.iter().map(|l| (l.a, l.b, l.ra, l.rb)).collect())).collect() };
        for (n, piece) in self.pieces.iter_mut().enumerate() {
            piece.age += dt;
            let before = (piece.shift, piece.turn);
            let speed;
            match &mut piece.over {
                Some((way, tipped, rate)) if piece.settling => {
                    // Its top is down: it turns back about it, its butt coming down off the
                    // stump, and slides on a little the way it fell.
                    // (First on, a good stride and more, and only then down: a trunk's foot
                    // spreads into roots a metre out, and let down at once the butt came to
                    // rest on them, still in the air.)
                    if piece.age < 0.4 {
                        piece.shift += *way * 3.5 * dt;
                    } else {
                        *rate -= 5.0 * dt;
                        *tipped += *rate * dt;
                    }
                    piece.turn = Quat::from_axis_angle(Vec3::Y.cross(*way).normalize_or(Vec3::Z), *tipped) * piece.base;
                    speed = rate.abs() * piece.tall * 0.5;
                    if *tipped < -1.2 {
                        piece.age = 99.0;
                    }
                }
                Some((way, tipped, rate)) => {
                    // As a pole on its end goes over: faster the further it has gone.
                    *rate += 1.5 * FALL / piece.tall.max(1.0) * tipped.sin() * dt;
                    *tipped += *rate * dt;
                    piece.turn = Quat::from_axis_angle(Vec3::Y.cross(*way).normalize_or(Vec3::Z), *tipped);
                    speed = *rate * piece.tall;
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
            let going_over = piece.over.is_some();
            // (Stopped by the ground and by what has itself been cut or has fallen; not by
            // leaf, and not by a tree still standing whole: in a wood every tree's limbs are
            // in its neighbours', and stopped by those nothing cut ever came down. It goes
            // through them, and where it lies across a standing trunk it is not put.)
            // (Nor by another tree that stands though it has been cut into, which is cells
            // and would bear: a birch felled beside one lay 14 m up in its crown.)
            let (mine, lone) = (piece.thing, piece.hunk);
            let stops = |at: Vec3| {
                let b = world.bit((at / BIT).floor().as_ivec3());
                // (A boulder as it grew stops what falls on it, as one cut into does: until
                // it was cut a tree came down through it.)
                // (And a hunk by any wood: it is no tree, to go through a tree, and one let go
                // over a standing trunk went into it and was lost.)
                if lone && b.wood() {
                    return true;
                }
                (bears(b) || b == Block::Rock) && !(b.wood() && standing.iter().any(|(id, limbs)| Some(*id) != mine && limbs.iter().any(|l| outside(at, l) < 0.3)))
            };
            let tried = |piece: &Piece, p: &Vec3| !going_over || p.distance(piece.spare.0) > piece.spare.1;
            let meets = |piece: &Piece| piece.feel.iter().filter(|p| tried(piece, p)).any(|p| stops(piece.now(*p)));
            let met = meets(piece);
            // `LOOSE_TRACE=1`: what stopped it, each time it is stopped: where on it (as it
            // stood, from its foot) and by what in the world.
            if met && std::env::var_os("LOOSE_TRACE").is_some() {
                let hits: Vec<String> = piece.feel.iter().filter(|p| tried(piece, p)).filter(|p| stops(piece.now(**p))).take(4).map(|p| { let at = piece.now(*p); format!("{:.1} m up it, {:.1} m out, at {:.1} by {:?}", p.y - piece.pivot.y, (*p - piece.pivot).with_y(0.0).length(), at, world.bit((at / BIT).floor().as_ivec3())) }).collect();
                eprintln!("loose: piece {} of {} cells stopped (going over {going_over}, settling {}): {}", piece.id, piece.cells.len(), piece.settling, hits.join(" · "));
            }
            // (Where it struck, for a tree whose top has just come down: what it will settle about.)
            let struck = (met && going_over && !piece.settling).then(|| piece.feel.iter().filter(|p| tried(piece, p)).find(|p| stops(piece.now(**p))).copied()).flatten();
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
                down.push((n, speed, struck));
            }
        }
        let mut settling = Vec::new();
        let mut settling_rolls: Vec<Piece> = Vec::new();
        for (n, speed, struck) in down.into_iter().rev() {
            let piece = self.pieces.swap_remove(n);
            let mut piece = piece;
            // A lump that has come down on a heap rolls off it: where the ground beside it is
            // lower than where it lies by more than its own height, it goes that way and
            // falls again, three times at the most. (Left where they met, the lumps of one
            // broken stone stood on one another two metres high, 2026-11-13.) In a fixed
            // order of sides, so the same fall heaps the same way.
            // (Not wood: a thin log rolled from where its tree lay, and the axe, which asks
            // the tree where its logs are, could not find it to split. Seed 2, 2026-11-13.)
            let wooden = || piece.cells.values().any(|p| matches!(p, Part::Whole(b) if b.wood()));
            if piece.thing.is_none() && piece.over.is_none() && !piece.settling && !piece.hunk && piece.cells.len() < 80 && piece.rolls < 3 && !wooden() {
                let (middle, reach) = (piece.now(piece.pivot), piece.tall.max(2.0 * CELL));
                let below = |at: Vec3| world.raycast(at, Vec3::NEG_Y, 12.0);
                let sides = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z].map(|d| middle + d * reach);
                let lower = sides.into_iter().filter(|s| !bears(world.bit((*s / BIT).floor().as_ivec3()))).filter_map(|s| below(s).map(|d| (d, s))).filter(|(d, _)| *d > reach * 1.5).max_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, side)) = lower {
                    piece.shift += side - middle;
                    (piece.vel, piece.spin, piece.rolls) = (Vec3::ZERO, piece.spin * 0.5, piece.rolls + 1);
                    settling_rolls.push(piece);
                    continue;
                }
            }
            // A tree's top has struck: it is heard, its outermost twigs snap off, and it
            // settles about where it struck (below, once every piece has been looked at).
            if let (Some(top), Some((way, ..))) = (struck, piece.over) {
                self.landed.push(Landed { at: piece.now(top), cells: piece.cells.len(), put: 0, lost: 0, speed, settled: false });
                if speed > CRASHES {
                    let (foot, stem) = (piece.pivot, (piece.tall * 0.33).max(1.2));
                    piece = self.shatter(piece, speed, 0.04, |c| ((c.as_vec3() + 0.5) * CELL - foot).with_y(0.0).length() < stem);
                    piece.feel.retain(|p| piece.cells.contains_key(&(*p / CELL).floor().as_ivec3()));
                }
                (piece.shift, piece.pivot) = (piece.pivot + piece.shift + piece.turn * (top - piece.pivot) - top, top);
                (piece.base, piece.over, piece.settling, piece.spare, piece.age) = (piece.turn, Some((way, 0.0, 0.0)), true, (top, piece.tall * 0.3), 0.0);
                settling.push(piece);
                continue;
            }
            if piece.brittle && speed > SHATTERS && piece.cells.len() >= 8 {
                self.landed.push(Landed { at: piece.now(piece.pivot + Vec3::Y * piece.tall / 2.0), cells: piece.cells.len(), put: 0, lost: 0, speed, settled: false });
                self.shatter(piece, speed, 1.0, |_| false);
                continue;
            }
            // A lump or a thing dropped straight comes to rest square to the world: turned to
            // the nearest quarter turn each way and moved to the nearest whole cell (up, not
            // down into what it lies on). The world is cells, and what lies turned between
            // them is asked cell by cell what of it is there: a boulder's broken outside,
            // a bit thick, was mostly lost that way, a quarter of its stone (2026-11-13).
            // A tree lies as it fell: it is long, and squared it would jump.
            if piece.thing.is_none() && piece.over.is_none() && !piece.settling {
                square(&mut piece);
            }
            // (A hunk comes down whole or not at all: up, a cell at a time, until every cell
            // of it has room. Put where it met the ground a quarter of a cell in, one lost
            // two of its eight cells and was no hunk: seed 5, 2026-11-13.)
            if piece.hunk {
                for _ in 0..8 {
                    let by = (piece.shift / CELL).round().as_ivec3();
                    // (Leaf is room: it is crushed under what comes down on it, and a hunk
                    // set down on top of it lay on nothing that bears and fell again.)
                    if piece.cells.keys().all(|c| room(world, *c + by)) {
                        // (And what leaf is there is crushed under it.)
                        for c in piece.cells.keys() {
                            if world.get(*c + by) == Block::Broken {
                                world.set(*c + by, Block::Air);
                            }
                        }
                        break;
                    }
                    piece.shift.y += CELL;
                }
            }
            let (put, touched, lo, hi, lost) = land(world, &piece);
            changed.extend(touched);
            if let Some(thing) = self.things.iter_mut().find(|t| Some(t.id) == piece.thing) {
                thing.lies = Some((piece.pivot, piece.shift, piece.turn));
            }
            // And what of it has come down on nothing (a limb left sticking out over a
            // hollow, a part cut off from the rest by what it fell across) falls in its turn.
            changed.extend(self.loosen(world, lo - 1, hi + 1, Vec3::X));
            self.landed.push(Landed { at: piece.now(piece.pivot + Vec3::Y * piece.tall / 2.0), cells: piece.cells.len(), put, lost, speed, settled: piece.settling });
        }
        self.pieces.extend(settling);
        self.pieces.extend(settling_rolls);
        changed
    }
}

/// Turns a piece to the nearest of the ways a cube can lie and moves it to whole cells.
fn square(piece: &mut Piece) {
    let m = glam::Mat3::from_quat(piece.turn);
    let snap = |v: Vec3, not: Option<usize>| -> (usize, Vec3) {
        let a = v.abs().to_array();
        let i = (0..3).filter(|i| Some(*i) != not).max_by(|x, y| a[*x].total_cmp(&a[*y])).unwrap_or(0);
        let mut out = [0.0; 3];
        out[i] = v.to_array()[i].signum();
        (i, Vec3::from(out))
    };
    let (ix, x) = snap(m.x_axis, None);
    let (_, y) = snap(m.y_axis, Some(ix));
    let turn = Quat::from_mat3(&glam::Mat3::from_cols(x, y, x.cross(y))).normalize();
    // (Where the piece's own origin now is: that is what must be on a cell's corner.)
    let at = piece.pivot + piece.shift - turn * piece.pivot;
    // (Hunks to the squares hunks lie on: [`hunk_at`].)
    let wide = if piece.hunk { HUNK as f32 } else { 1.0 };
    let to = Vec3::new((at.x / CELL / wide).round() * wide, (at.y / CELL - 0.25).ceil(), (at.z / CELL / wide).round() * wide) * CELL;
    (piece.turn, piece.shift) = (turn, piece.shift + (to - at));
}

/// Puts a piece back into the world where it now lies: every cell of the world it reaches is
/// asked what of the piece is there, so it goes back without gaps however it is turned.
/// Only into what is not solid already. Gives how many cells were written, their chunks, the
/// box of cells it reached, and how many whole cells of what bears found no room.
fn land(world: &mut World, piece: &Piece) -> (usize, HashSet<IVec3>, IVec3, IVec3, usize) {
    let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
    for cell in piece.cells.keys() {
        for corner in 0..8 {
            let p = piece.now((*cell + IVec3::new(corner & 1, corner >> 1 & 1, corner >> 2 & 1)).as_vec3() * CELL);
            (lo, hi) = (lo.min(p), hi.max(p));
        }
    }
    let (lo, hi) = ((lo / CELL).floor().as_ivec3(), (hi / CELL).floor().as_ivec3());
    let at = |now: Vec3| (piece.was(now) / CELL).floor().as_ivec3();
    let (mut put, mut changed, mut lost) = (0, HashSet::new(), 0);
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
                    // (Into a cell that is part filled, what of it there is room for, to the
                    // bit: the ground a felled trunk lies on is such cells, and a log cut
                    // from it that dropped a hand's width found no room for two thirds of
                    // itself, which was gone. Seed 2, 2026-11-13.)
                    Part::Whole(b) if there == Block::Broken && !b.hunk() => {
                        let mut any = false;
                        for i in 0..SPLIT * SPLIT * SPLIT {
                            let bit = cell * SPLIT + IVec3::new(i % SPLIT, i / (SPLIT * SPLIT), i / SPLIT % SPLIT);
                            if !world.bit(bit).solid() {
                                world.set_bit(bit, *b);
                                any = true;
                            }
                        }
                        put += any as usize;
                        lost += (!any && bears(*b)) as usize;
                    }
                    // (What bears and found no room: wood into wood, stone into the ground.
                    // Leaf that comes down into leaf or onto the ground is only crushed.)
                    // (It lies on what is there, in the first room over it: a boulder that
                    // came down lost a cell of stone for every cell of ground it met by
                    // less than half, a fifth of itself, 2026-11-13.)
                    Part::Whole(b) => {
                        if bears(*b) {
                            // (On what bears: over leaf it would be left in the air.)
                            match (1..=4).map(|up| cell + IVec3::Y * up).find(|c| world.get(*c) == Block::Air && has(world, *c - IVec3::Y, false) && !piece.cells.contains_key(&at((c.as_vec3() + 0.5) * CELL))) {
                                Some(room) => {
                                    world.set(room, *b);
                                    changed.insert(room.div_euclid(IVec3::splat(cube_voxel::CHUNK)));
                                    put += 1;
                                }
                                None => lost += 1,
                            }
                        }
                        continue;
                    }
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
                            } else if bears(block) && let Some(room) = (1..=2 * SPLIT).map(|up| bit + IVec3::Y * up).find(|b| !world.bit(*b).solid() && bears(world.bit(*b - IVec3::Y))) {
                                // (And a bit of stone that finds its place taken, the same.)
                                world.set_bit(room, block);
                                changed.insert(room.div_euclid(IVec3::splat(SPLIT)).div_euclid(IVec3::splat(cube_voxel::CHUNK)));
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
    (put, changed, lo, hi, lost)
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
        // (Come loose, stone is rubble.)
        let (low, up) = (count(&w, Vec3::new(-1.0, 0.0, -1.0), Vec3::new(4.0, 1.5, 2.0), Block::Shard), count(&w, Vec3::new(-1.0, 1.75, -1.0), Vec3::new(4.0, 4.0, 2.0), Block::Shard));
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
        // And it is still known as a tree: its shape, every bite, and (once down) where it lies.
        run(&mut loose, &mut w);
        assert_eq!(loose.things.len(), 1);
        let thing = &loose.things[0];
        assert!(thing.limbs.len() == 2 && thing.clumps.len() == 1 && thing.cuts.len() >= 2, "{} limbs, {} clumps, {} cuts", thing.limbs.len(), thing.clumps.len(), thing.cuts.len());
        let (_, _, turn) = thing.lies.expect("it came down and is not noted as lying");
        assert!((turn * Vec3::Y).y < 0.35, "noted as lying, but still upright: its up is {:?}", turn * Vec3::Y);
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
            let lies = if block == Block::Stone { Block::Shard } else { block };
            let down = count(&w, Vec3::new(-8.0, 0.0, -8.0), Vec3::new(10.0, 3.0, 10.0), lies);
            let block_shape = count(&w, Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 1.5, 2.0), lies);
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
    fn water_runs_into_a_hole_dug_beside_it_and_no_higher_than_it_stood() {
        let mut w = floor();
        // A pool in the floor, and a wall of earth between it and a dry pit, lower.
        w.fill(Vec3::new(0.0, -1.0, 0.0), Vec3::new(2.0, 0.0, 2.0), Block::Water);
        w.fill(Vec3::new(2.5, -1.0, 0.0), Vec3::new(4.5, 0.0, 2.0), Block::Air);
        assert_eq!(count(&w, Vec3::splat(-6.0), Vec3::splat(6.0), Block::Water), 256);
        let mut loose = Loose::default();
        loose.cut(&mut w, Vec3::new(2.25, -0.5, 1.0), 0.45, Vec3::X, &[], None);
        let pit = count(&w, Vec3::new(2.5, -1.0, 0.0), Vec3::new(4.5, 0.0, 2.0), Block::Water);
        let over = count(&w, Vec3::new(-6.0, 0.0, -6.0), Vec3::new(6.0, 4.0, 6.0), Block::Water);
        assert!(pit == 256 && over == 0, "{pit} of 256 cells of the pit filled, {over} cells of water above where it stood");
    }

    #[test]
    fn a_trunk_lying_on_the_ground_is_cut_into_lengths_that_stay_where_they_lie() {
        let mut w = floor();
        w.fill(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 6.0, 0.5), Block::Timber);
        let mut loose = Loose::default();
        loose.cut(&mut w, Vec3::new(0.25, 0.75, 0.25), 0.45, Vec3::X, &[], Some(Stuff::Wood));
        run(&mut loose, &mut w);
        // Where it lies: flat on the ground (none of it more than its own thickness up), from
        // one end to the other.
        let across = |w: &World, x: f32| count(w, Vec3::new(x - 0.1, 0.0, -1.0), Vec3::new(x + 0.1, 1.25, 1.5), Block::Timber);
        let along: Vec<f32> = (0..60).map(|i| 0.9 + i as f32 * 0.25).filter(|x| across(&w, *x) > 0).collect();
        let (from, to) = (along[0], along[along.len() - 1]);
        let (lying, up) = (count(&w, Vec3::new(0.8, 0.0, -1.0), Vec3::new(16.0, 0.75, 1.5), Block::Timber), count(&w, Vec3::new(0.8, 0.75, -1.0), Vec3::new(16.0, 8.0, 1.5), Block::Timber));
        assert!(to - from > 3.5 && up == 0 && lying > 60, "it lies from {from:.1} to {to:.1} m, {lying} cells low and {up} higher than its own thickness");
        // Two cuts across it with an axe, chop by chop, across it and down through it: three
        // lengths, each left where it lay, the wood cut away carried off and none of the
        // stone under it touched.
        let (first, second) = (from + (to - from) * 0.35, from + (to - from) * 0.68);
        let mut taken = 0;
        for x in [first, second] {
            for (y, z) in [(0.6, 0.0), (0.6, 0.25), (0.6, 0.5), (0.3, 0.0), (0.3, 0.25), (0.3, 0.5)] {
                let cut = loose.cut(&mut w, Vec3::new(x, y, z), 0.28, Vec3::X, &[], Some(Stuff::Wood));
                assert_eq!(cut.taken[Stuff::Stone as usize], 0, "an axe cut the ground under the log");
                taken += cut.taken[Stuff::Wood as usize];
            }
        }
        assert!(loose.pieces.is_empty(), "a length cut from a lying trunk came loose: {} pieces", loose.pieces.len());
        let thirds = [(from + first) / 2.0, (first + second) / 2.0, (second + to) / 2.0].map(|x| across(&w, x));
        assert!(across(&w, first) == 0 && across(&w, second) == 0 && thirds.iter().all(|n| *n > 0), "not three lengths: wood at the cuts {} and {}, between them {thirds:?}", across(&w, first), across(&w, second));
        let left = count(&w, Vec3::new(0.8, 0.0, -1.0), Vec3::new(16.0, 1.25, 1.5), Block::Timber);
        assert!(taken > 200 && left < lying && left as f32 > lying as f32 * 0.6, "took {taken} bits; {left} of {lying} cells of it left");
    }

    #[test]
    fn a_trunk_cut_most_of_the_way_through_snaps_and_one_only_notched_stands() {
        for (bites, falls) in [(1, false), (3, true)] {
            let mut w = floor();
            // A trunk a metre through, and chops in one line into its middle: a hole through
            // it, with wood left standing either side.
            w.fill(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 6.0, 0.5), Block::Timber);
            let mut loose = Loose::default();
            for n in 0..bites {
                loose.cut(&mut w, Vec3::new(-0.4 + n as f32 * 0.4, 1.0, 0.0), 0.36, Vec3::X, &[], Some(Stuff::Wood));
            }
            assert_eq!(!loose.pieces.is_empty(), falls, "after {bites} chops into a trunk a metre through");
        }
    }

    #[test]
    fn earth_let_go_heaps_and_stone_stays_on_the_ground_and_falls_from_the_air() {
        let mut w = floor();
        let mut loose = Loose::default();
        // Twenty lumps let go at one place two metres up: a heap on the ground, low and
        // wide, and nothing left in the air.
        for _ in 0..20 {
            loose.put(&mut w, Vec3::new(0.1, 2.0, 0.1), 0.155, Block::Dirt);
        }
        let earth: Vec<IVec3> = (-12..12).flat_map(|x| (-12..12).flat_map(move |z| (0..16).map(move |y| IVec3::new(x, y, z)))).filter(|c| w.get(*c) != Block::Air).collect();
        let (top, wide) = (earth.iter().map(|c| c.y).max().unwrap(), earth.iter().map(|c| c.x.abs().max(c.z.abs())).max().unwrap());
        assert!(top <= 3 && wide >= 2, "a heap {top} cells high and {wide} out, of {} cells", earth.len());
        // A stone put on the ground stays as it was put, which is building; one let go in
        // the air comes down, and from two metres it breaks.
        loose.put(&mut w, Vec3::new(4.1, 0.1, 0.1), 0.155, Block::Stone);
        assert!(loose.pieces.is_empty() && w.get(IVec3::new(16, 0, 0)) != Block::Air, "a stone on the ground did not stay");
        loose.put(&mut w, Vec3::new(8.1, 2.0, 0.1), 0.5, Block::Stone);
        assert_eq!(loose.pieces.len(), 1, "a stone let go in the air hangs there");
        run(&mut loose, &mut w);
        assert_eq!(w.get(IVec3::new(32, 8, 0)), Block::Air, "it is still in the air");
        assert!(loose.landed.iter().any(|l| l.speed > 4.5), "it did not come down hard: {:?}", loose.landed.iter().map(|l| l.speed).collect::<Vec<_>>());
    }

    #[test]
    fn the_same_edits_done_again_give_the_same_world() {
        use crate::plants::{Clump, Limb};
        let grown = || {
            let mut w = floor();
            let mut form = Form::default();
            let limb = Limb { a: Vec3::new(0.0, 0.0, 0.0), b: Vec3::new(0.2, 6.0, 0.0), ra: 0.3, rb: 0.2, bark: Block::Wood, roots: 0, tree: 1 };
            limb.fill(CELL, |c| w.set(c, Block::Wood));
            form.limbs.push(limb);
            w.ellipsoid(Vec3::new(0.2, 6.5, 0.0), Vec3::splat(1.5), Block::Leaves, true);
            form.clumps.push(Clump { middle: Vec3::new(0.2, 6.5, 0.0), radii: Vec3::splat(1.5), leaf: Block::Leaves, tree: 1 });
            (w, form)
        };
        // Played: a tree cut down, a hole dug while it is still falling, earth put down
        // after, and the fallen trunk cut across.
        let (mut w, form) = grown();
        let mut loose = Loose::default();
        for x in [-0.3, 0.0, 0.3] {
            loose.cut(&mut w, Vec3::new(x, 1.0, 0.0), 0.36, Vec3::X, &[&form], Some(Stuff::Wood));
        }
        for _ in 0..40 {
            loose.step(&mut w, crate::DT);
        }
        loose.cut(&mut w, Vec3::new(-3.0, -0.1, 2.0), 0.5, Vec3::X, &[&form], None);
        run(&mut loose, &mut w);
        loose.put(&mut w, Vec3::new(-3.0, 0.2, -2.0), 0.3, Block::Dirt);
        loose.cut(&mut w, Vec3::new(3.0, 0.4, 0.0), 0.4, Vec3::X, &[&form], Some(Stuff::Wood));
        run(&mut loose, &mut w);
        // Done again to the land as it grew.
        let (mut again, form) = grown();
        replay(&mut again, &[&form], &loose.journal);
        let (lo, hi) = (IVec3::splat(-48), IVec3::splat(48));
        let mut differ = 0;
        for y in lo.y..hi.y {
            for z in lo.z..hi.z {
                for x in lo.x..hi.x {
                    let c = IVec3::new(x, y, z);
                    differ += (w.get(c) != again.get(c) || w.bits_at(c).map(|b| b.to_vec()) != again.bits_at(c).map(|b| b.to_vec())) as usize;
                }
            }
        }
        assert!(loose.journal.len() == 6 && differ == 0, "{} edits; {differ} cells differ", loose.journal.len());
        assert!(count(&w, Vec3::splat(-12.0), Vec3::splat(12.0), Block::Timber) > 50, "nothing was felled to compare");
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

    #[test]
    fn a_tree_cut_through_falls_though_its_boughs_touch_another_that_was_cut_into() {
        use crate::plants::Limb;
        // Two trees a metre and a half apart, a bough of each reaching the other's trunk,
        // as `plants` grew them.
        let trees = || {
            let (mut w, mut form) = (floor(), Form::default());
            for (tree, x) in [(1, 0.0f32), (2, 1.5)] {
                let out = if tree == 1 { 1.5 } else { -1.5 };
                for (a, b, ra, roots) in [(Vec3::new(x, 0.0, 0.0), Vec3::new(x, 6.0, 0.0), 0.3, 1), (Vec3::new(x, 4.0, 0.0), Vec3::new(x + out, 4.5, 0.0), 0.12, 0)] {
                    let limb = Limb { a, b, ra, rb: ra * 0.8, bark: Block::Wood, roots, tree };
                    limb.fill(CELL, |c| w.set(c, Block::Wood));
                    form.limbs.push(limb);
                }
            }
            (w, form)
        };
        let (mut w, form) = trees();
        let mut loose = Loose::default();
        // A bite that reaches both, which makes both cells; then the first cut through.
        loose.cut(&mut w, Vec3::new(0.75, 1.0, 0.0), 0.5, Vec3::NEG_X, &[&form], Some(Stuff::Wood));
        assert_eq!(loose.things.len(), 2, "both were to be cut into");
        for x in [-0.3, 0.0, 0.3] {
            loose.cut(&mut w, Vec3::new(x, 1.0, 0.0), 0.4, Vec3::NEG_X, &[&form], Some(Stuff::Wood));
        }
        assert_eq!(loose.pieces.len(), 1, "it hangs by a bough from the tree beside it");
        // And comes down to the ground, through the other, which stands.
        run(&mut loose, &mut w);
        let lies = loose.landed.iter().map(|l| l.at.y).fold(f32::MAX, f32::min);
        assert!(lies < 1.5, "it lies {lies} m up");
        assert_ne!(w.get(IVec3::new(6, 12, 0)), Block::Air, "the other tree went with it");
    }

    #[test]
    fn a_wall_built_of_stone_stands_and_comes_down_when_its_foot_is_cut_away() {
        let mut w = floor();
        let mut loose = Loose::default();
        // Eight cells long and six high, a piece at a time, each put on what is there.
        for y in 0..6 {
            for x in 0..8 {
                loose.put(&mut w, (IVec3::new(x, y, 0).as_vec3() + 0.5) * CELL, 0.155, Block::Stone);
            }
        }
        run(&mut loose, &mut w);
        let standing = |w: &World| (0..6).flat_map(|y| (0..8).map(move |x| IVec3::new(x, y, 0))).filter(|c| has(w, *c, false)).count();
        assert!(loose.pieces.is_empty() && standing(&w) == 48, "{} of 48 cells stand, {} loose", standing(&w), loose.pieces.len());
        // A doorway picked through it leaves the wall standing over the gap.
        loose.cut(&mut w, Vec3::new(1.0, 0.25, 0.125), 0.3, Vec3::Z, &[], Some(Stuff::Stone));
        run(&mut loose, &mut w);
        assert!(standing(&w) >= 40 && w.get(IVec3::new(4, 5, 0)) == Block::Stone, "a hole brought it down: {} stand", standing(&w));
        // Its whole foot cut away, what was over it comes down, and its stone is on the ground.
        for x in 0..9 {
            loose.cut(&mut w, Vec3::new(x as f32 * CELL, 0.125, 0.125), 0.3, Vec3::Z, &[], Some(Stuff::Stone));
        }
        assert!(!loose.pieces.is_empty(), "the wall stands on nothing");
        run(&mut loose, &mut w);
        let top = (0..12).rev().find(|y| (-8..16).any(|x| (-8..8).any(|z| w.get(IVec3::new(x, *y, z)) != Block::Air))).unwrap_or(0);
        let stone: usize = (-12..20).flat_map(|x| (-12..12).flat_map(move |z| (0..12).map(move |y| IVec3::new(x, y, z)))).filter(|c| has(&w, *c, false)).count();
        assert!(top < 6 && stone >= 20, "it lies {top} cells high, {stone} cells of stone on the ground");
    }

    #[test]
    fn an_axe_fells_a_tree_by_its_blows_then_limbs_it_then_breaks_it_into_logs() {
        use crate::plants::{Clump, Limb};
        let (mut w, mut form) = (floor(), Form::default());
        // A trunk 8 m tall and 50 cm through, three boughs and their leaf.
        let mut grow = |a: Vec3, b: Vec3, ra: f32, roots: u8, w: &mut World| {
            let limb = Limb { a, b, ra, rb: ra * 0.8, bark: Block::Wood, roots, tree: 1 };
            limb.fill(CELL, |c| w.set(c, Block::Wood));
            form.limbs.push(limb);
        };
        grow(Vec3::new(0.1, 0.0, 0.1), Vec3::new(0.1, 8.0, 0.1), 0.25, 1, &mut w);
        for (y, out) in [(4.0, 1.6f32), (5.5, -1.4), (7.0, 1.2)] {
            grow(Vec3::new(0.1, y, 0.1), Vec3::new(0.1, y + 0.6, 0.1 + out), 0.06, 0, &mut w);
            w.ellipsoid(Vec3::new(0.1, y + 0.7, 0.1 + out), Vec3::splat(0.9), Block::Leaves, true);
            form.clumps.push(Clump { middle: Vec3::new(0.1, y + 0.7, 0.1 + out), radii: Vec3::splat(0.9), leaf: Block::Leaves, tree: 1 });
        }
        let mut loose = Loose::default();
        // Standing: it takes as many blows as its girth asks, and stands until the last.
        let needs = BLOWS(0.25);
        assert!((3..=5).contains(&needs), "{needs} blows for a trunk 50 cm through");
        for blow in 1..=needs {
            assert!(loose.pieces.is_empty() && loose.landed.is_empty(), "it was down after {} blows of {needs}", blow - 1);
            loose.chop(&mut w, Vec3::new(-0.2, 1.2, 0.1), Vec3::X, &[&form]);
        }
        assert_eq!(loose.pieces.len(), 1, "cut through, it stands");
        run(&mut loose, &mut w);
        let tree = &loose.things[0];
        let (pivot, shift, turn) = tree.lies.expect("it did not come down");
        let now = |p: Vec3| pivot + shift + turn * (p - pivot);
        assert!(now(Vec3::new(0.1, 5.0, 0.1)).y < 1.0, "its trunk lies {} m up", now(Vec3::new(0.1, 5.0, 0.1)).y);
        // Fallen: blows along it knock off its boughs and leaf, and leave its trunk.
        let count = |w: &World, of: &dyn Fn(Block) -> bool| (-60..60).flat_map(|x| (-40..40).flat_map(move |z| (0..24).map(move |y| IVec3::new(x, y, z)))).filter(|c| of(w.get(*c))).count();
        let trunk = count(&w, &|b| b.wood());
        assert!(count(&w, &|b| b.foliage()) > 20, "it has no leaf to lose");
        for blow in 0..6 {
            if loose.things[0].limbed {
                break;
            }
            loose.chop(&mut w, now(Vec3::new(0.1, 3.0 + blow as f32 * 1.2, 0.1)), Vec3::NEG_Y, &[&form]);
            run(&mut loose, &mut w);
        }
        assert!(loose.things[0].limbed && count(&w, &|b| b.foliage()) == 0, "{} cells of leaf left", count(&w, &|b| b.foliage()));
        assert!(!loose.shed.is_empty() && count(&w, &|b| b.wood()) * 10 > trunk * 7, "its trunk went with its boughs: {} of {trunk}", count(&w, &|b| b.wood()));
        // Bare, it is timber: a blow at what is longer than a log breaks it through, and a
        // blow at a log splits it into hunks. Blows all along it, until it is all hunks.
        let (after, mut breaks, mut splits) = (count(&w, &|b| b.wood()), 0, 0);
        for blow in 0..24 {
            let (wood, billets) = (count(&w, &|b| b.wood()), count(&w, &|b| b == Block::Billet));
            loose.chop(&mut w, now(Vec3::new(0.1, 1.5 + (blow * 5 % 13) as f32 * 0.5, 0.1)), Vec3::NEG_Y, &[&form]);
            run(&mut loose, &mut w);
            let split = count(&w, &|b| b == Block::Billet) > billets;
            (breaks, splits) = (breaks + (!split && count(&w, &|b| b.wood()) < wood) as u32, splits + split as u32);
        }
        assert!(breaks >= 1 && splits >= 2, "broken through {breaks} times and split {splits}");
        let (left, billets) = (count(&w, &|b| b.wood()), count(&w, &|b| b == Block::Billet));
        // (All of it but its stump, which stands where it stood.)
        let stump = (-4..=4).flat_map(|x| (-4..=4).flat_map(move |z| (0..8).map(move |y| IVec3::new(x, y, z)))).filter(|c| w.get(*c).wood()).count();
        let (left, after) = (left - stump, after - stump);
        assert!(left * 10 < after, "{left} cells of wood left in logs, of {after}");
        // (Half the wood that was in logs, `YIELD`, to the nearest hunk a log.)
        let share = billets as f32 / (after as f32 * YIELD);
        assert!(billets % 8 == 0 && (0.6..1.4).contains(&share), "{billets} cells of hunks from {after} of logs");
        // Every cell of them is a hunk that can be found, taken up, and put down elsewhere.
        let cells = |w: &World| -> Vec<IVec3> { (-60..60).flat_map(|x| (-40..40).flat_map(move |z| (0..24).map(move |y| IVec3::new(x, y, z)))).filter(|c| w.get(*c) == Block::Billet).collect() };
        let all = cells(&w);
        assert!(all.iter().all(|c| hunk_at(&w, *c).is_some()), "a cell of a hunk that is no hunk");
        assert!(all.iter().all(|c| c.y < 6), "a hunk left in the air at {:?}", all.iter().map(|c| c.y).max());
        let at = (all[0].as_vec3() + 0.5) * CELL;
        assert_eq!(loose.lift(&mut w, at).map(|l| l.0), Some(Block::Billet));
        run(&mut loose, &mut w);
        assert_eq!(cells(&w).len(), billets - 8);
        assert!(loose.lay(&mut w, Vec3::new(-5.1, 2.0, -5.1), Block::Billet).is_some());
        run(&mut loose, &mut w);
        let laid = hunk_at(&w, (Vec3::new(-5.1, 0.1, -5.1) / CELL).floor().as_ivec3());
        assert_eq!(laid.map(|l| l.0), Some(IVec3::new(-22, 0, -22)), "put down in the air, it came down onto the ground on its square");
        // Over a cell part filled with loose leaf it comes to rest too, on the ground under it.
        w.set_bit(IVec3::new(30, 2, 30) * SPLIT + IVec3::ONE, Block::Brush);
        assert_eq!(w.get(IVec3::new(30, 2, 30)), Block::Broken);
        assert!(loose.lay(&mut w, Vec3::new(7.6, 1.6, 7.6), Block::Billet).is_some());
        let steps = (0..600).take_while(|_| { loose.step(&mut w, crate::DT); !loose.pieces.is_empty() }).count();
        assert!(steps < 300, "a hunk over loose leaf was still falling after five seconds");
        assert_eq!(hunk_at(&w, IVec3::new(30, 1, 30)).map(|l| l.0), Some(IVec3::new(30, 0, 30)));
        let billets = billets + 8;
        // One put on it lies on it, and each is still its own hunk.
        assert!(loose.lay(&mut w, Vec3::new(-5.1, 0.1, -5.1), Block::Billet).is_some());
        run(&mut loose, &mut w);
        assert_eq!(hunk_at(&w, IVec3::new(-21, 3, -21)).map(|l| l.0), Some(IVec3::new(-22, 2, -22)));
        assert_eq!(cells(&w).len(), billets + 8);
    }

    #[test]
    fn a_pick_chips_a_boulder_until_it_breaks_into_hunks_and_digs_the_lands_own_stone() {
        let (mut w, mut form) = (floor(), Form::default());
        let stone = crate::plants::Stone { middle: Vec3::new(0.1, 0.6, 0.1), radii: Vec3::new(1.0, 0.8, 0.9), turn: 0.0, faces: Vec::new(), kind: Block::Rock };
        w.ellipsoid(stone.middle, stone.radii, Block::Rock, true);
        form.stones.push(stone);
        let count = |w: &World, of: &dyn Fn(Block) -> bool| (-40..40).flat_map(|x| (-40..40).flat_map(move |z| (0..24).map(move |y| IVec3::new(x, y, z)))).filter(|c| of(w.get(*c))).count();
        // (Three cubic metres of it: 24 hunks' worth, less what is under the ground.)
        let whole = count(&w, &|b| b.grown() == Block::Rock || b == Block::Broken);
        let (mut loose, needs) = (Loose::default(), CHIPS(1.0));
        assert_eq!(needs, 4);
        for blow in 1..needs {
            assert!(loose.chip(&mut w, Vec3::new(0.1, 1.3, 0.1), Vec3::NEG_Y, &[&form]).is_some());
            run(&mut loose, &mut w);
            let left = count(&w, &|b| b.grown() == Block::Rock || b == Block::Broken);
            assert!(left * 10 > whole * 8 && count(&w, &|b| b == Block::Cobble) == 0, "after {blow} blows of {needs}, {left} cells of {whole}");
        }
        assert!(loose.chip(&mut w, Vec3::new(0.1, 1.1, 0.1), Vec3::NEG_Y, &[&form]).is_some());
        run(&mut loose, &mut w);
        let (left, cobbles) = (count(&w, &|b| b.grown() == Block::Rock), count(&w, &|b| b == Block::Cobble));
        assert!(left == 0 && (20..=24).contains(&(cobbles / 8)) && cobbles % 8 == 0, "{left} cells of boulder left, {cobbles} of hunks");
        // They lie on the ground and on one another, each one that can be taken up.
        let all: Vec<IVec3> = (-40..40).flat_map(|x| (-40..40).flat_map(move |z| (0..24).map(move |y| IVec3::new(x, y, z)))).filter(|c| w.get(*c) == Block::Cobble).collect();
        assert!(all.iter().all(|c| hunk_at(&w, *c).is_some()));
        assert!(all.iter().all(|c| c.y == 0 || w.get(*c - IVec3::Y).solid()), "a hunk of stone in the air");
        // Rubble: stone that came down and broke is loose stone, and a blow at a lump of it
        // makes hunks of it, to the nearest one.
        w.fill(Vec3::new(6.0, 0.0, 6.0), Vec3::new(6.75, 0.75, 7.0), Block::Shard);
        assert!(loose.chip(&mut w, Vec3::new(6.3, 0.7, 6.3), Vec3::NEG_Y, &[&form]).is_some());
        run(&mut loose, &mut w);
        assert_eq!(count(&w, &|b| b == Block::Shard), 0);
        let (cobbles, all) = (cobbles + 40, (-40..40).flat_map(|x| (-40..40).flat_map(move |z| (0..24).map(move |y| IVec3::new(x, y, z)))).filter(|c| w.get(*c) == Block::Cobble).collect::<Vec<IVec3>>());
        assert_eq!(all.len(), cobbles, "36 cells of rubble are four and a half hunks: five, to the nearest");
        // A hunk is no boulder and is not cut: a pick does nothing to it, nor to the ground.
        let top = (all.iter().max_by_key(|c| c.y).unwrap().as_vec3() + 0.5) * CELL;
        assert!(loose.chip(&mut w, top, Vec3::NEG_Y, &[&form]).is_none());
        loose.cut(&mut w, top, 0.3, Vec3::NEG_Y, &[&form], Some(Stuff::Stone));
        assert_eq!(count(&w, &|b| b == Block::Cobble), cobbles);
        // Stone dug from the land comes away as a hunk, a bite a hunk.
        w.fill(Vec3::new(-8.0, 0.0, -8.0), Vec3::new(-5.0, 2.0, -5.0), Block::Stone);
        loose.cut(&mut w, Vec3::new(-6.5, 1.9, -6.5), 0.3, Vec3::NEG_Y, &[&form], Some(Stuff::Stone));
        run(&mut loose, &mut w);
        let cobbles = cobbles + 8;
        assert_eq!(count(&w, &|b| b == Block::Cobble), cobbles, "a bite of the land's stone left no hunk");
        // And the same blows on the same land give the same land.
        let (mut again, mut form2) = (floor(), Form::default());
        again.fill(Vec3::new(-8.0, 0.0, -8.0), Vec3::new(-5.0, 2.0, -5.0), Block::Stone);
        again.ellipsoid(Vec3::new(0.1, 0.6, 0.1), Vec3::new(1.0, 0.8, 0.9), Block::Rock, true);
        again.fill(Vec3::new(6.0, 0.0, 6.0), Vec3::new(6.75, 0.75, 7.0), Block::Shard);
        form2.stones.push(form.stones[0].clone());
        replay(&mut again, &[&form2], &loose.journal);
        assert_eq!(count(&again, &|b| b == Block::Cobble), cobbles);
    }
}
