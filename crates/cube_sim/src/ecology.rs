//! Where things live, and when: the one system that decides what life a place has.
//!
//! A [`Place`] is what a spot of the island is like to something living there: its biome, how
//! far it is from water, how much tree cover stands over it, how high it is. A [`Niche`] is
//! what a kind of life wants of a place and at what hours it is about. [`Niche::fit`] puts the
//! two together, 0 (never here) to 1 (at home). Everything that puts life into the world asks
//! it: the animals of the simulation when the world is populated (`life::Sim::populate`), and
//! the small life the game draws round the camera (butterflies, birds, fish, fireflies).
//!
//! So a firefly is not "a light near the camera at night": it is in damp woodland within
//! reach of water, at dusk and after, and near the camera only if the camera is somewhere
//! like that. Change where a thing lives here, or in its `creature.toml`, not in the code
//! that draws it. `cargo world life` prints how much of the island suits each kind.

use crate::sky;
use crate::terrain::{Biome, Land, SEA};
use cube_voxel::CELL;
use serde::Deserialize;
use std::collections::VecDeque;

/// What a spot is like to live in.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub biome: Biome,
    /// Metres to the nearest water, to at most [`FAR`]; 0 on it.
    pub water: f32,
    /// How deep the water is here, metres; 0 on land.
    pub depth: f32,
    /// Tree cover, 0 open sky to 1 deep wood: the trees standing within eight metres.
    pub cover: f32,
    /// Metres over the sea.
    pub high: f32,
}

/// Beyond this, water is simply far.
pub const FAR: f32 = 60.0;
/// How far apart the island is measured, metres.
const STEP: f32 = 2.0;

/// The island measured for living in: how far every spot is from water and how wooded.
pub struct Ecology {
    side: i32,
    water: Vec<f32>,
    cover: Vec<f32>,
}

impl Ecology {
    pub fn new(land: &Land) -> Self {
        let step = (STEP / CELL) as i32;
        let side = land.span * 2 / step;
        let at = |gx: i32, gz: i32| (gx + gz * side) as usize;
        let cell = |g: i32| g * step - land.span + step / 2;
        // Water: outward from every wet spot, a step at a time.
        let mut water = vec![FAR; (side * side) as usize];
        let mut queue = VecDeque::new();
        for gz in 0..side {
            for gx in 0..side {
                if land.biome(cell(gx), cell(gz)) == Biome::Water {
                    water[at(gx, gz)] = 0.0;
                    queue.push_back((gx, gz));
                }
            }
        }
        while let Some((gx, gz)) = queue.pop_front() {
            let next = water[at(gx, gz)] + STEP;
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, nz) = (gx + dx, gz + dz);
                if nx >= 0 && nz >= 0 && nx < side && nz < side && next < water[at(nx, nz)] {
                    water[at(nx, nz)] = next;
                    queue.push_back((nx, nz));
                }
            }
        }
        // Cover: each tree shades the ground eight metres round its foot, most at the foot.
        let mut cover = vec![0.0f32; (side * side) as usize];
        let reach = (8.0 / STEP) as i32;
        for tree in &land.trees {
            let (tx, tz) = ((tree.x + land.span) / step, (tree.z + land.span) / step);
            for dz in -reach..=reach {
                for dx in -reach..=reach {
                    let (gx, gz) = (tx + dx, tz + dz);
                    let away = ((dx * dx + dz * dz) as f32).sqrt() / reach as f32;
                    if gx >= 0 && gz >= 0 && gx < side && gz < side && away < 1.0 {
                        cover[at(gx, gz)] += (1.0 - away) * 0.45;
                    }
                }
            }
        }
        cover.iter_mut().for_each(|c| *c = c.min(1.0));
        Ecology { side, water, cover }
    }

    /// What the spot at `x`, `z` (metres) is like.
    pub fn place(&self, land: &Land, x: f32, z: f32) -> Place {
        let (cx, cz) = ((x / CELL).floor() as i32, (z / CELL).floor() as i32);
        let step = (STEP / CELL) as i32;
        let (gx, gz) = (((cx + land.span) / step).clamp(0, self.side - 1), ((cz + land.span) / step).clamp(0, self.side - 1));
        let i = (gx + gz * self.side) as usize;
        let h = land.height(cx, cz);
        let biome = land.biome(cx, cz);
        // (The measure is two metres coarse; a wet spot is on the water whatever it says.)
        Place { biome, water: if biome == Biome::Water { 0.0 } else { self.water[i].max(STEP * 0.5) }, depth: ((SEA - h) as f32 * CELL).max(0.0), cover: self.cover[i], high: (h - SEA) as f32 * CELL }
    }
}

/// How near water something keeps.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Water {
    /// It does not matter.
    #[default]
    Any,
    /// At the water's edge: within a few metres.
    Edge,
    /// Within a short walk: its drinking place.
    Near,
    /// Dry country, well away from it.
    Away,
    /// In it, where it is deep enough to swim.
    In,
}

/// How much tree cover something keeps under.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Cover {
    #[default]
    Any,
    /// Open ground under the sky.
    Open,
    /// The edge of a wood: some trees, not all.
    Edge,
    /// Deep under trees.
    Deep,
}

/// When something is about.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Hours {
    #[default]
    Any,
    Day,
    Night,
    /// Dawn and dusk, and on into the night.
    Dusk,
}

/// What a kind of life wants of a place, and when it is about.
#[derive(Clone, Copy, Debug)]
pub struct Niche {
    pub name: &'static str,
    /// The biomes it lives in, and how well each suits it (0 to 1).
    pub biomes: &'static [(Biome, f32)],
    pub water: Water,
    pub cover: Cover,
    pub hours: Hours,
    /// How much of it is about in the depth of winter, of what there is in summer: 0 for what
    /// sleeps the winter through or has flown south, 1 for what does not mind.
    pub cold: f32,
}

/// 1 inside `lo..hi`, falling to 0 over `soft` beyond either end.
fn band(v: f32, lo: f32, hi: f32, soft: f32) -> f32 {
    (1.0 - ((lo - v).max(v - hi).max(0.0) / soft)).clamp(0.0, 1.0)
}

impl Niche {
    /// How well a place suits it, whatever the hour: 0 never, 1 at home.
    pub fn suits(&self, p: &Place) -> f32 {
        let biome = self.biomes.iter().find(|(b, _)| *b == p.biome).map_or(0.0, |(_, w)| *w);
        let water = match self.water {
            Water::Any => 1.0,
            Water::Edge => band(p.water, 0.0, 6.0, 8.0),
            Water::Near => band(p.water, 0.0, 28.0, 20.0),
            Water::Away => band(p.water, 30.0, FAR, 14.0),
            Water::In => band(p.depth, 0.5, 100.0, 0.3),
        };
        let cover = match self.cover {
            Cover::Any => 1.0,
            Cover::Open => band(p.cover, 0.0, 0.15, 0.3),
            Cover::Edge => band(p.cover, 0.15, 0.6, 0.2),
            Cover::Deep => band(p.cover, 0.55, 1.0, 0.3),
        };
        biome * water * cover
    }

    /// How much it is about at an hour of the day, 0 to 1.
    pub fn about(&self, when: f32) -> f32 {
        let day = sky::daylight(when);
        // Only the depth of winter thins it: by early spring everything is back.
        let season = self.cold + (1.0 - self.cold) * ((sky::warmth(when) + 0.9) / 0.6).clamp(0.0, 1.0);
        season * match self.hours {
            Hours::Any => 1.0,
            Hours::Day => day,
            Hours::Night => 1.0 - day,
            Hours::Dusk => (4.0 * day * (1.0 - day)).max(0.8 * (1.0 - day)),
        }
    }

    /// Whether it is here now: the place, and the hour.
    pub fn fit(&self, p: &Place, when: f32) -> f32 {
        self.suits(p) * self.about(when)
    }
}

/// What a place sounds like at an hour, each 0 to 1: the wind, birdsong, the insects of the
/// night, and water. What the game plays is this and nothing else, so the sound of a place
/// follows from what the place is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ambience {
    pub wind: f32,
    pub birds: f32,
    pub night: f32,
    pub water: f32,
}

pub fn ambience(p: &Place, when: f32) -> Ambience {
    let (day, warm) = (sky::daylight(when), (sky::warmth(when) + 1.0) / 2.0);
    let land = if matches!(p.biome, Biome::Meadow | Biome::Forest | Biome::Shore) { 1.0 } else { 0.25 };
    Ambience {
        // More in the open and on high ground; a wood is sheltered.
        wind: (0.3 + 0.35 * (1.0 - p.cover) + p.high.max(0.0) / 90.0).min(1.0),
        // By day, most where there are trees to sing from, and little in winter.
        birds: day * land * (0.25 + 0.75 * (p.cover / 0.6).min(1.0)) * (0.2 + 0.8 * warm),
        // After dark, in the warm half of the year.
        night: (1.0 - day) * land * ((warm - 0.25) / 0.4).clamp(0.0, 1.0),
        water: band(p.water, 0.0, 4.0, 22.0),
    }
}

/// The small life the game draws, that the simulation does not know one by one.
pub const BUTTERFLY: Niche = Niche { name: "butterfly", biomes: &[(Biome::Meadow, 1.0), (Biome::Shore, 0.4), (Biome::Forest, 0.3)], water: Water::Any, cover: Cover::Open, hours: Hours::Day, cold: 0.0 };
pub const SONGBIRD: Niche = Niche { name: "songbird", biomes: &[(Biome::Forest, 1.0), (Biome::Meadow, 0.8)], water: Water::Any, cover: Cover::Edge, hours: Hours::Day, cold: 0.4 };
pub const SWALLOW: Niche = Niche { name: "swallow", biomes: &[(Biome::Meadow, 1.0), (Biome::Shore, 1.0), (Biome::Water, 1.0)], water: Water::Near, cover: Cover::Open, hours: Hours::Day, cold: 0.0 };
pub const HAWK: Niche = Niche { name: "hawk", biomes: &[(Biome::Rock, 1.0), (Biome::Meadow, 0.7), (Biome::Snow, 0.5)], water: Water::Any, cover: Cover::Open, hours: Hours::Day, cold: 0.7 };
pub const DRAGONFLY: Niche = Niche { name: "dragonfly", biomes: &[(Biome::Water, 1.0), (Biome::Shore, 1.0)], water: Water::Edge, cover: Cover::Any, hours: Hours::Day, cold: 0.0 };
pub const FIREFLY: Niche = Niche { name: "firefly", biomes: &[(Biome::Forest, 1.0), (Biome::Meadow, 0.6), (Biome::Shore, 0.8)], water: Water::Near, cover: Cover::Edge, hours: Hours::Dusk, cold: 0.0 };
pub const FISH: Niche = Niche { name: "fish", biomes: &[(Biome::Water, 1.0)], water: Water::In, cover: Cover::Any, hours: Hours::Any, cold: 0.6 };
pub const FALLING_LEAF: Niche = Niche { name: "falling leaf", biomes: &[(Biome::Forest, 1.0), (Biome::Meadow, 0.6)], water: Water::Any, cover: Cover::Deep, hours: Hours::Any, cold: 1.0 };
pub const POLLEN: Niche = Niche { name: "pollen", biomes: &[(Biome::Meadow, 1.0), (Biome::Forest, 0.5), (Biome::Shore, 0.3)], water: Water::Any, cover: Cover::Any, hours: Hours::Day, cold: 0.0 };

pub const SMALL_LIFE: [Niche; 9] = [BUTTERFLY, SONGBIRD, SWALLOW, HAWK, DRAGONFLY, FIREFLY, FISH, FALLING_LEAF, POLLEN];

#[cfg(test)]
mod tests {
    use super::*;

    fn place(biome: Biome, water: f32, cover: f32) -> Place {
        Place { biome, water, depth: if biome == Biome::Water { 2.0 } else { 0.0 }, cover, high: 5.0 }
    }

    #[test]
    fn each_kind_has_its_place_and_its_hour() {
        let (glade, deep_wood, lake, dry_meadow) = (place(Biome::Forest, 12.0, 0.4), place(Biome::Forest, 50.0, 0.95), place(Biome::Water, 0.0, 0.0), place(Biome::Meadow, 55.0, 0.0));
        // Fireflies: a damp glade at night, never at noon, never far from water.
        assert!(FIREFLY.fit(&glade, sky::at(3, 22.0)) > 0.6 && FIREFLY.fit(&glade, sky::at(3, 12.0)) < 0.05 && FIREFLY.fit(&dry_meadow, sky::at(3, 22.0)) < 0.05);
        // Butterflies: open meadow by day; not in deep wood, not at night.
        assert!(BUTTERFLY.fit(&dry_meadow, sky::at(3, 12.0)) > 0.9 && BUTTERFLY.fit(&deep_wood, sky::at(3, 12.0)) < 0.05 && BUTTERFLY.fit(&dry_meadow, sky::at(3, 0.0)) < 0.05);
        // Fish: in the lake at any hour, nowhere else.
        assert!(FISH.fit(&lake, sky::at(3, 3.0)) > 0.9 && FISH.fit(&glade, sky::at(3, 12.0)) == 0.0);
        // In the depth of winter: no butterflies, no fireflies; fish still, and fewer birds.
        let winter = sky::at(24, 12.0);
        assert!(BUTTERFLY.fit(&dry_meadow, winter) < 0.05 && FIREFLY.fit(&glade, sky::at(24, 22.0)) < 0.05 && FISH.fit(&lake, winter) > 0.5);
        assert!(SONGBIRD.about(winter) < SONGBIRD.about(sky::at(10, 12.0)) && SONGBIRD.about(winter) > 0.2);
        // Leaves fall under trees, not in the open.
        assert!(FALLING_LEAF.suits(&deep_wood) > 0.9 && FALLING_LEAF.suits(&dry_meadow) < 0.05);
    }

    #[test]
    fn a_place_sounds_like_what_it_is() {
        let (glade, lake_shore, peak) = (place(Biome::Forest, 40.0, 0.6), place(Biome::Shore, 2.0, 0.1), Place { high: 60.0, ..place(Biome::Rock, 55.0, 0.0) });
        let (noon, midnight, winter_noon) = (sky::at(3, 12.0), sky::at(10, 0.0), sky::at(24, 12.0));
        // Birds by day in the wood, not at night, few in winter; insects on a summer night.
        assert!(ambience(&glade, noon).birds > 0.5 && ambience(&glade, midnight).birds == 0.0 && ambience(&glade, winter_noon).birds < 0.3);
        assert!(ambience(&glade, midnight).night > 0.8 && ambience(&glade, noon).night == 0.0 && ambience(&glade, sky::at(24, 0.0)).night == 0.0);
        // Water where there is water; wind on the heights more than in the wood.
        assert!(ambience(&lake_shore, noon).water > 0.9 && ambience(&glade, noon).water == 0.0);
        assert!(ambience(&peak, noon).wind > ambience(&glade, noon).wind + 0.3);
    }

    #[test]
    fn the_island_is_measured() {
        let (_, land) = crate::terrain::generate(7, 4);
        let eco = Ecology::new(&land);
        let mut seen = (false, false, false);
        for z in (-60..60).step_by(4) {
            for x in (-60..60).step_by(4) {
                let p = eco.place(&land, x as f32, z as f32);
                assert!((p.water == 0.0) == (p.biome == Biome::Water), "{p:?}");
                seen = (seen.0 || p.water > 10.0, seen.1 || p.cover > 0.2, seen.2 || p.depth > 0.5);
            }
        }
        assert!(seen.0 && seen.1 && seen.2, "an island has dry ground, woods and deep water: {seen:?}");
    }
}
