//! The day: what hour it is and where that puts the sun. Kept in the simulation, not with the
//! drawing, because the day is a rule of the world: what sleeps, what hunts and what a person
//! can see by will follow it. The game only draws the sky this describes.

use glam::Vec3;
use std::f32::consts::TAU;

/// How long a day lasts, seconds: a minute to the hour. Long enough that the light is not
/// seen to move, short enough that an hour's play meets dusk and dawn.
pub const DAY: f32 = 1440.0;
/// The hour the world starts at: mid afternoon, the sun 39 degrees up and shadows long.
pub const START: f32 = 15.0;
/// How far the sun's path leans from overhead, radians: at noon it stands 62 degrees up, to
/// the south. Upright, noon would have no shadows at all.
const LEAN: f32 = 0.49;

/// The hour after `seconds` more of a day at `hour`.
pub fn later(hour: f32, seconds: f32) -> f32 {
    (hour + seconds * 24.0 / DAY).rem_euclid(24.0)
}

/// Which way the sun is, from the ground, at an hour of 0 to 24: up in the east (+x) at six,
/// highest at noon, down in the west at eighteen, under the world at night.
pub fn sun(hour: f32) -> Vec3 {
    let turn = (hour - 12.0) / 24.0 * TAU;
    Vec3::new(-turn.sin(), turn.cos() * LEAN.cos(), turn.cos() * LEAN.sin()).normalize()
}

/// How much of day it is, 0 in full night to 1 in full day, passing through twilight while
/// the sun is within six degrees of the horizon.
pub fn daylight(hour: f32) -> f32 {
    let t = ((sun(hour).y + 0.105) / 0.21).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sun_is_up_by_day_and_down_by_night() {
        assert!(sun(12.0).y > 0.85 && sun(0.0).y < -0.85);
        assert!(sun(6.0).y.abs() < 0.01 && sun(6.0).x > 0.99 && sun(18.0).x < -0.99);
        assert_eq!((daylight(12.0), daylight(0.0)), (1.0, 0.0));
        assert!(daylight(18.0) > 0.4 && daylight(18.0) < 0.6);
        // Morning only gets lighter, evening only darker.
        assert!((0..60).all(|i| daylight(4.0 + i as f32 * 0.1) <= daylight(4.1 + i as f32 * 0.1)));
        assert!((0..60).all(|i| daylight(15.0 + i as f32 * 0.1) >= daylight(15.1 + i as f32 * 0.1)));
    }

    #[test]
    fn a_day_comes_round() {
        assert!((later(23.5, DAY / 24.0) - 0.5).abs() < 1e-4);
        assert!((later(START, DAY) - START).abs() < 1e-3);
    }
}
