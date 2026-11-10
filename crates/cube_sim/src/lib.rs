//! The simulation: the rules of the game, stepped at a fixed tick with no window and no
//! engine types, so it can run on a server and be tested in milliseconds.
//!
//! Map: `body` (how a creature or the player moves through the voxel world: walking, steps,
//! jumping, falling, floating).

pub mod body;

/// Simulation steps a second. Everything in this crate advances by [`DT`].
pub const TICKS: u32 = 60;
pub const DT: f32 = 1.0 / TICKS as f32;
