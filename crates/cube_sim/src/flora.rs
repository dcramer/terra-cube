//! What grows, as data. The kinds of tree are a table in `assets/world/flora.toml`, read
//! once: where each grows and how common it is there, how tall, how thick, of what bark and
//! leaf, and which habit it grows in. Adding a kind of tree is adding a table to that file;
//! the land picks from the table (`terrain`), the habits are built in `plants`, and whoever
//! draws a tree asks its kind here what it is. Until 2026-11-11 the kinds were five names in
//! the code, their sizes in one file, their shares of a wood in another and their shapes far
//! off in a third.

use cube_voxel::Block;
use serde::Deserialize;
use std::sync::OnceLock;

/// The file, built in: the simulation needs no folder to stand in, and a test, a server and
/// the game all grow the same trees.
const FILE: &str = include_str!("../../../assets/world/flora.toml");

/// How a tree grows: the shape `plants::tree` builds.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Habit {
    Spreading,
    Slender,
    Whorled,
    Weeping,
    Dead,
}

/// One kind of tree.
#[derive(Deserialize, Clone, Debug)]
pub struct Species {
    pub name: String,
    pub habit: Habit,
    /// Metres, from one just grown to an old one.
    pub height: [f32; 2],
    pub girth: f32,
    #[serde(default = "one")]
    pub spread: f32,
    pub reach: f32,
    bark: String,
    leaf: String,
    #[serde(default)]
    old_leaf: Option<String>,
    #[serde(default)]
    pub giant: bool,
    #[serde(default)]
    pub low: f32,
    #[serde(default)]
    pub high: f32,
    #[serde(default)]
    pub waterside: f32,
}

fn one() -> f32 {
    1.0
}

/// A block by the name the files use for it.
pub fn block(name: &str) -> Block {
    match name {
        "wood" => Block::Wood,
        "birch" => Block::Birch,
        "leaves" => Block::Leaves,
        "leaves_pale" => Block::LeavesPale,
        "leaves_deep" => Block::LeavesDeep,
        "needles" => Block::Needles,
        other => panic!("assets/world/flora.toml: no block `{other}` (wood, birch, leaves, leaves_pale, leaves_deep, needles)"),
    }
}

impl Species {
    /// How tall one of this kind stands at an age from 0 (just grown) to 1 (old); a giant is
    /// older than old.
    pub fn tall(&self, age: f32) -> f32 {
        self.height[0] + (self.height[1] - self.height[0]) * age
    }
    pub fn bark(&self) -> Block {
        block(&self.bark)
    }
    pub fn leaf(&self) -> Block {
        block(&self.leaf)
    }
    /// What most old ones' leaf turns to; its usual leaf, if nothing is said.
    pub fn old_leaf(&self) -> Block {
        self.old_leaf.as_deref().map_or_else(|| self.leaf(), block)
    }
    pub fn needles(&self) -> bool {
        self.leaf() == Block::Needles
    }
}

#[derive(Deserialize)]
struct File {
    tree: Vec<Species>,
}

/// Every kind of tree, in the order of the file.
pub fn trees() -> &'static [Species] {
    static TREES: OnceLock<Vec<Species>> = OnceLock::new();
    TREES.get_or_init(|| {
        let file: File = toml::from_str(FILE).unwrap_or_else(|e| panic!("assets/world/flora.toml: {e}"));
        for tree in &file.tree {
            // (Asked for now, so a wrong name is found at the start and not in a wood.)
            let _ = (tree.bark(), tree.leaf(), tree.old_leaf());
        }
        file.tree
    })
}

/// A kind of tree: its place in the table.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Tree(pub u8);

impl Tree {
    pub fn named(name: &str) -> Option<Tree> {
        trees().iter().position(|t| t.name == name).map(|i| Tree(i as u8))
    }
    pub fn species(self) -> &'static Species {
        &trees()[self.0 as usize]
    }
}

/// Where a tree stands, for choosing its kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Site {
    Low,
    High,
    Waterside,
}

/// The kind of tree that stands at a site, by a number from 0 to 1: each kind as often as
/// the file says it is found there. A site nothing is said to grow at takes the low
/// ground's trees.
pub fn pick(site: Site, roll: f32) -> Tree {
    let share = |t: &Species, site: Site| match site {
        Site::Low => t.low,
        Site::High => t.high,
        Site::Waterside => t.waterside,
    };
    let site = if trees().iter().any(|t| share(t, site) > 0.0) { site } else { Site::Low };
    let all: f32 = trees().iter().map(|t| share(t, site)).sum();
    let mut left = roll * all;
    for (i, t) in trees().iter().enumerate() {
        left -= share(t, site);
        if left < 0.0 && share(t, site) > 0.0 {
            return Tree(i as u8);
        }
    }
    Tree(trees().iter().rposition(|t| share(t, site) > 0.0).unwrap_or(0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_in_the_file_is_whole_and_some_grow_everywhere() {
        assert!(trees().len() >= 5);
        for t in trees() {
            assert!(t.height[1] > t.height[0] && t.height[0] > 2.0, "{}", t.name);
            assert!(t.girth > 0.005 && t.girth < 0.1 && t.reach > 0.0 && t.reach <= 0.6, "{}", t.name);
        }
        for site in [Site::Low, Site::High, Site::Waterside] {
            // Every roll picks a kind that is said to grow there.
            for i in 0..50 {
                let tree = pick(site, i as f32 / 50.0).species();
                assert!(match site { Site::Low => tree.low, Site::High => tree.high, Site::Waterside => tree.waterside } > 0.0, "{} at {site:?}", tree.name);
            }
        }
        assert_eq!(Tree::named("oak").map(|t| t.species().habit), Some(Habit::Spreading));
    }
}
