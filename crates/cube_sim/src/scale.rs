//! How big everything in the world is, in one place and in metres.
//!
//! The measure is a person, 1.75 m. A world reads as full size when the things in it keep the
//! ratios to a person that they have outside: a grown tree eight to twenty of them tall, a
//! hill some tens of them across, ground that is a grade underfoot and not a staircase. It
//! reads as a model when any of those is off, however good each thing looks alone.
//!
//! So nothing else in the simulation carries a size of its own. `terrain` shapes the land from
//! the [`Landform`]s here, `plants` grows each kind of tree to its [`Span`] and derives every
//! other measure of the tree from that height, rocks come in the [`ROCKS`] classes, and
//! `survey` measures what was actually made against the bars at the bottom, so
//! `cargo world survey` fails when the world drifts out of scale. To make the world bigger or
//! smaller, change it here; the documentation of these numbers is docs/art-guide.md, "Scale".

/// The measure of everything.
pub const PERSON: f32 = 1.75;

/// A shape of the land: how far it is from one rise to the next, and how far from the lowest
/// it makes the ground to the highest.
pub struct Landform {
    pub across: f32,
    pub tall: f32,
    /// How many finer layers of the same shape are laid over it, each half the size. One is a
    /// smooth swell; four is rough down to an eighth of `across`.
    pub detail: u32,
}

impl Landform {
    /// Up for one along, at its steepest: `tall` gained over half of `across`.
    pub fn grade(&self) -> f32 {
        self.tall / (self.across * 0.5)
    }
}

/// The lie of the whole island: what is a plain and what is a plateau.
pub const LOWLAND: Landform = Landform { across: 200.0, tall: 22.0, detail: 2 };
/// Hills over the lowland. Smooth: a person walking one meets a slope, not lumps. Their
/// smallest swell is 35 m over, twenty people.
pub const HILL: Landform = Landform { across: 80.0, tall: 7.0, detail: 2 };
/// The mountain: one to an island, how far across its foot and how high its top. A bell, so
/// its outer third is foothills a person walks and its middle is climbing.
pub const MASSIF: Landform = Landform { across: 480.0, tall: 170.0, detail: 1 };
/// The lesser high ground, somewhere else on the island: downs, not peaks.
pub const HEIGHTS: Landform = Landform { across: 260.0, tall: 60.0, detail: 1 };
/// Ridges on the high ground: how far from one to the next, and how deep the creases between.
pub const RIDGE: Landform = Landform { across: 110.0, tall: 26.0, detail: 2 };
/// A mountain breaks into ledges this high: a cliff of three people, with a shelf over it.
pub const LEDGE: f32 = 5.0;
/// How far apart the river's bends are, and how wide its valley and channel are cut (as parts
/// of that noise; about 22 m and 8 m).
pub const RIVER: f32 = 130.0;

/// Heights over the sea where the land changes.
pub const CONIFER_LINE: f32 = 30.0;
pub const ROCK_LINE: f32 = 55.0;
/// No tree grows above this, and they thin out for the last twenty metres under it: a peak
/// is bare.
pub const TREE_LINE: f32 = 70.0;
pub const MOSS_LINE: f32 = 60.0;
pub const SNOW_LINE: f32 = 105.0;

/// From the smallest to the largest of a thing, metres.
#[derive(Clone, Copy)]
pub struct Span(pub f32, pub f32);

impl Span {
    /// `t` of the way from the smallest (0) to the largest (1).
    pub fn at(self, t: f32) -> f32 {
        self.0 + (self.1 - self.0) * t
    }
}

/// (How tall each kind of tree stands is in `assets/world/flora.toml`, with the rest of what
/// a kind is. Raised 2026-11-11: the developer found the trees small. A grown tree is a big
/// thing, and the kinds differ.)
/// How far past its kind's tallest a giant stands (of the span), and how many are giants.
pub const GIANT: f32 = 1.25;
pub const GIANTS: f32 = 0.05;
/// A young tree: what stands at eye level and just over in a wood.
pub const YOUNG: Span = Span(3.0, 7.5);
/// A grown tree has this much ground to itself; small things, this much.
pub const TREE_ROOM: f32 = 6.5;
pub const SMALL_ROOM: f32 = 3.5;
/// Foliage of a grown tree starts this far up, so the largest creature (3.6 m) walks under.
pub const UNDER_CROWN: f32 = 4.5;

/// A class of rock: how far across, and how many of every hundred rocks are of it.
pub struct Rocks {
    pub name: &'static str,
    pub across: Span,
    pub share: f32,
}

/// Stones underfoot, boulders to walk round, and the rare great one that is a landmark:
/// taller than a person and seen across a meadow.
pub const ROCKS: [Rocks; 3] = [
    Rocks { name: "stone", across: Span(0.3, 0.8), share: 0.58 },
    Rocks { name: "boulder", across: Span(1.0, 2.6), share: 0.34 },
    Rocks { name: "great", across: Span(3.2, 6.0), share: 0.08 },
];

/// The bars `cargo world survey` holds the made world to.
///
/// A grown tree in the middle of the count is at least this many people tall.
pub const BAR_TREE: f32 = 8.0;
/// The largest rock is at least this many times the smallest.
pub const BAR_ROCKS: f32 = 10.0;
/// At least this part of the lowland's meadow and forest (under the conifer line) is easy
/// ground: no more than 0.75 m up or down over two metres along, a slope and not a climb.
pub const BAR_EASY: f32 = 0.65;
