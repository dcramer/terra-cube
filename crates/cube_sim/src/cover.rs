//! What the land is made of and what grows on it, as rules in a file: `assets/world/land.toml`.
//! How thick the trees, undergrowth and rocks stand in each biome; what the undergrowth is;
//! what the top of the soil is at a place; and what stands in the cell over it. `terrain`
//! asks here and places what it is told. A rule is a list of tests of a place (how worn by a
//! trail, how far under a crown, how damp, what it stands on, a patch of slow noise, a
//! chance) and what to put where they all hold; the first rule that fits is taken.
//!
//! Until 2026-11-11 each of these was a `match` in `terrain::fill` with its numbers in it.

use crate::terrain::{Biome, fbm, hash2};
use cube_voxel::{Block, CELL};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

const FILE: &str = include_str!("../../../assets/world/land.toml");

/// A place, as the rules see it.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub biome: Biome,
    /// Its column, in cells, and the world's seed.
    pub x: i32,
    pub z: i32,
    pub seed: u32,
    /// How far under a crown (0 outside, 1 at the trunk), whether that crown is a conifer's,
    /// and whether this is the foot of a trunk.
    pub shade: f32,
    pub needles: bool,
    pub foot: bool,
    /// How worn by a trail, 0 to 1.
    pub trail: f32,
    pub damp: f32,
    /// At or under the water's level; the most it rises or falls to the next column, cells;
    /// metres over the sea.
    pub wet: bool,
    pub slope: i32,
    pub over: f32,
    /// What it stands on (for cover).
    pub on: Block,
}

#[derive(Deserialize, Clone, Copy, Debug)]
struct Patch {
    size: f32,
    seed: u32,
    over: f32,
    #[serde(default)]
    shade_eases: f32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
struct Thick {
    size: f32,
    seed: u32,
    minus: f32,
    times: f32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
struct Bloom {
    size: f32,
    seed: u32,
    minus: f32,
    times: f32,
    adds: f32,
}

#[derive(Deserialize, Clone, Debug)]
struct Rule {
    biomes: Vec<String>,
    put: String,
    trail: Option<[f32; 2]>,
    shade: Option<[f32; 2]>,
    foot: Option<bool>,
    needles: Option<bool>,
    damp: Option<[f32; 2]>,
    on: Option<String>,
    wet: Option<bool>,
    slope: Option<i32>,
    below: Option<f32>,
    patch: Option<Patch>,
    chance: Option<f32>,
    thick: Option<Thick>,
    pick: Option<f32>,
    bloom: Option<Bloom>,
    scatter: Option<f32>,
    tall: Option<[i32; 2]>,
}

/// How thick the standing things are in a biome (`land.toml`).
#[derive(Deserialize, Clone, Copy, Debug, Default)]
pub struct Density {
    pub trees: f32,
    pub under: f32,
    pub rocks: f32,
}

/// What the undergrowth is: the share of it that is each kind.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Undergrowth {
    pub young: f32,
    pub bush: f32,
    pub log: f32,
    pub stump: f32,
    pub young_conifers: f32,
}

/// A rule for which biome a place is (`[[biome]]`).
#[derive(Deserialize, Clone, Debug)]
struct Kind {
    is: String,
    below: Option<f32>,
    upto: Option<f32>,
    above: Option<f32>,
    steep: Option<i32>,
    moist: Option<[f32; 2]>,
}

#[derive(Deserialize)]
struct File {
    biome: Vec<Kind>,
    density: BTreeMap<String, Density>,
    undergrowth: Undergrowth,
    ground: Vec<Rule>,
    cover: Vec<Rule>,
}

/// The rules as they are used: names turned to what they name, once.
struct Ready {
    rule: Rule,
    biomes: Vec<Biome>,
    put: Option<Block>,
    on: Option<Block>,
}

struct Land {
    biomes: Vec<(Biome, Kind)>,
    density: [Density; 6],
    undergrowth: Undergrowth,
    ground: Vec<Ready>,
    cover: Vec<Ready>,
}

/// A block by the name the files use for it: its own name, in small letters.
pub fn block(name: &str) -> Block {
    match name {
        "grass" => Block::Grass,
        "dirt" => Block::Dirt,
        "stone" => Block::Stone,
        "sand" => Block::Sand,
        "snow" => Block::Snow,
        "turf" => Block::Turf,
        "gravel" => Block::Gravel,
        "moss" => Block::Moss,
        "litter" => Block::Litter,
        "slate" => Block::Slate,
        "chalk" => Block::Chalk,
        "tuft" => Block::Tuft,
        "flower" => Block::Flower,
        "reed" => Block::Reed,
        "fern" => Block::Fern,
        "mushroom" => Block::Mushroom,
        other => panic!("assets/world/land.toml: no block `{other}`"),
    }
}

fn land() -> &'static Land {
    static LAND: OnceLock<Land> = OnceLock::new();
    LAND.get_or_init(|| {
        let file: File = toml::from_str(FILE).unwrap_or_else(|e| panic!("assets/world/land.toml: {e}"));
        let ready = |rules: Vec<Rule>| -> Vec<Ready> {
            rules
                .into_iter()
                .map(|rule| Ready {
                    biomes: rule.biomes.iter().map(|b| Biome::named(b).unwrap_or_else(|| panic!("assets/world/land.toml: no biome `{b}`"))).collect(),
                    put: (rule.put != "none").then(|| block(&rule.put)),
                    on: rule.on.as_deref().map(block),
                    rule,
                })
                .collect()
        };
        let mut density = [Density::default(); 6];
        for (name, d) in &file.density {
            density[Biome::named(name).unwrap_or_else(|| panic!("assets/world/land.toml: no biome `{name}`")) as usize] = *d;
        }
        let biomes = file.biome.iter().map(|k| (Biome::named(&k.is).unwrap_or_else(|| panic!("assets/world/land.toml: no biome `{}`", k.is)), k.clone())).collect();
        Land { biomes, density, undergrowth: file.undergrowth, ground: ready(file.ground), cover: ready(file.cover) }
    })
}

/// Which biome a place is: how many metres it stands over the sea, how many cells it rises
/// or falls to the next column, how damp it is. The rules of `land.toml`, the first that
/// fits. Until 2026-11-11 this was a chain of `if`s in `terrain`, its numbers in `scale`.
pub fn biome(over: f32, slope: i32, moist: f32) -> Biome {
    land()
        .biomes
        .iter()
        .find(|(_, k)| k.below.is_none_or(|v| over < v) && k.upto.is_none_or(|v| over <= v) && k.above.is_none_or(|v| over > v) && k.steep.is_none_or(|v| slope >= v) && within(k.moist, moist))
        .map_or(Biome::Meadow, |(b, _)| *b)
}

pub fn density(biome: Biome) -> Density {
    land().density[biome as usize]
}

pub fn undergrowth() -> Undergrowth {
    land().undergrowth
}

fn within(range: Option<[f32; 2]>, v: f32) -> bool {
    range.is_none_or(|[lo, hi]| v >= lo && v <= hi)
}

/// Whether a rule fits a place.
fn fits(r: &Ready, s: &Site) -> bool {
    let rule = &r.rule;
    let (xm, zm) = (s.x as f32 * CELL, s.z as f32 * CELL);
    let noise = |size: f32, seed: u32| fbm(xm / size, zm / size, s.seed + seed);
    // The rolls of the place: the same three whatever rule asks.
    let roll = |n: u32| hash2(s.x, s.z, s.seed + 71 + n);
    let bloom = rule.bloom.map(|b| ((noise(b.size, b.seed) - b.minus) * b.times).clamp(0.0, 1.0));
    r.biomes.contains(&s.biome)
        && within(rule.trail, s.trail)
        && within(rule.shade, s.shade)
        && within(rule.damp, s.damp)
        && rule.foot.is_none_or(|f| f == s.foot)
        && rule.needles.is_none_or(|n| n == s.needles)
        && r.on.is_none_or(|b| b == s.on)
        && rule.wet.is_none_or(|w| w == s.wet)
        && rule.slope.is_none_or(|most| s.slope <= most)
        && rule.below.is_none_or(|most| s.over < most)
        && rule.patch.is_none_or(|p| noise(p.size, p.seed) > p.over - s.shade * p.shade_eases)
        && rule.chance.is_none_or(|c| roll(0) < c)
        && rule.thick.is_none_or(|t| roll(0) < (noise(t.size, t.seed) - t.minus) * t.times)
        && rule.pick.is_none_or(|p| roll(1) < p + rule.bloom.map_or(0.0, |b| b.adds) * bloom.unwrap_or(0.0))
        && rule.scatter.is_none_or(|c| roll(2) < c * bloom.unwrap_or(1.0))
}

/// What the top of the soil is at a place: none where no rule speaks (the land's own stone,
/// sand or snow is left).
pub fn ground(site: &Site) -> Option<Block> {
    land().ground.iter().find(|r| fits(r, site)).and_then(|r| r.put)
}

/// What stands in the cell over the ground at a place, and how many cells tall.
pub fn cover(site: &Site) -> Option<(Block, i32)> {
    let r = land().cover.iter().find(|r| fits(r, site))?;
    let tall = r.rule.tall.map_or(1, |[lo, hi]| lo + (hash2(site.x, site.z, site.seed + 75) * (hi - lo + 1) as f32) as i32);
    r.put.map(|b| (b, tall.max(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(biome: Biome) -> Site {
        Site { biome, x: 0, z: 0, seed: 6, shade: 0.0, needles: false, foot: false, trail: 0.0, damp: 0.5, wet: false, slope: 0, over: 5.0, on: Block::Grass }
    }

    #[test]
    fn the_rules_read_and_say_what_a_place_is() {
        // A trail is bare earth and nothing grows in it; the foot of a trunk likewise.
        assert_eq!(ground(&Site { trail: 0.9, ..site(Biome::Meadow) }), Some(Block::Dirt));
        assert_eq!(cover(&Site { trail: 0.9, ..site(Biome::Meadow) }), None);
        assert_eq!(ground(&Site { foot: true, ..site(Biome::Forest) }), Some(Block::Dirt));
        // Every forest and meadow place has ground; a peak of rock is left as its stone.
        for x in 0..200 {
            assert!(matches!(ground(&Site { x, ..site(Biome::Forest) }), Some(Block::Turf | Block::Litter | Block::Moss)));
            assert!(matches!(ground(&Site { x, ..site(Biome::Meadow) }), Some(Block::Grass | Block::Dirt)));
        }
        assert_eq!(ground(&site(Biome::Rock)), None);
        // A meadow has grass and flowers on it in places, a forest ferns; reeds stand two or three tall.
        let found = |biome, wanted: Block, wet| (0..4000).any(|i| cover(&Site { x: i % 97, z: i / 97, wet, on: if wet { Block::Sand } else { Block::Grass }, ..site(biome) }).is_some_and(|c| c.0 == wanted));
        assert!(found(Biome::Meadow, Block::Tuft, false) && found(Biome::Meadow, Block::Flower, false) && found(Biome::Forest, Block::Fern, false));
        assert!((0..4000).any(|i| cover(&Site { x: i % 97, z: i / 97, wet: true, on: Block::Sand, ..site(Biome::Shore) }).is_some_and(|c| c.0 == Block::Reed && (2..=3).contains(&c.1))));
        assert!(density(Biome::Forest).trees > density(Biome::Meadow).trees && undergrowth().bush > 0.0);
        // And which biome a place is: under the sea, at its edge, the heights, a cliff, wet and dry.
        assert_eq!([biome(-1.0, 0, 0.9), biome(0.5, 0, 0.9), biome(120.0, 0, 0.2), biome(60.0, 0, 0.9), biome(10.0, 3, 0.9), biome(10.0, 0, 0.6), biome(10.0, 0, 0.4)], [Biome::Water, Biome::Shore, Biome::Snow, Biome::Rock, Biome::Rock, Biome::Forest, Biome::Meadow]);
    }
}
