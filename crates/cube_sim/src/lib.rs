//! The simulation: the rules of the game, stepped at a fixed tick with no window and no
//! engine types, so it can run on a server and be tested in milliseconds.
//!
//! Map: `terrain` (a world from a seed) · `plants` (the trees and what else grows) · `survey` (that world in numbers) · `body` (how a
//! creature or the player moves through voxels: walking, steps, jumping, falling, floating) ·
//! `life` (the creatures in the world: who they are, what each is doing, one tick of all of
//! them) · `mind` (what a creature decides to do) · `rng` (seeded randomness).

pub mod body;
pub mod ecology;
pub mod items;
pub mod cover;
pub mod flora;
pub mod life;
pub mod matter;
pub mod loose;
pub mod mind;
pub mod plants;
pub mod rng;
pub mod scale;
pub mod sky;
pub mod survey;
pub mod terrain;

/// Simulation steps a second. Everything in this crate advances by [`DT`].
pub const TICKS: u32 = 60;
pub const DT: f32 = 1.0 / TICKS as f32;
