//! The calendar and the sky: what time it is, and where that puts the sun and the moon. Kept
//! in the simulation, not with the drawing, because days and seasons are rules of the world:
//! what sleeps, what hunts, what grows and what a person can see by follow them. The game
//! only draws the sky this describes.
//!
//! Time is one number, `when`: days since the world began, with the hour as its fraction
//! (0.5 is noon of the first day). A day lasts 24 minutes, a minute to the hour. Seven days
//! are a season and four seasons a year, starting with spring; the moon goes through its
//! phases once a season.

use glam::Vec3;
use std::f32::consts::TAU;

/// How long a day lasts, seconds: a minute to the hour.
pub const DAY: f32 = 1440.0;
/// Days in a season, seasons in a year, and days from one new moon to the next.
pub const SEASON: f32 = 7.0;
pub const YEAR: f32 = SEASON * 4.0;
pub const MONTH: f32 = 7.0;
/// When the world starts: the middle of spring (day and night equal), mid afternoon, the
/// moon waxing toward full.
pub const START: f32 = 3.0 + 15.0 / 24.0;
/// How far the sun's path leans from overhead at the equinox, radians: at noon it stands 62
/// degrees up, to the south.
const LEAN: f32 = 0.49;
/// How far the sun swings north and south over the year, radians: summer noons are 81
/// degrees up and the day thirteen and a half hours; winter noons 43 degrees and the day
/// ten and a half.
const SWING: f32 = 0.33;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

/// `when` after `seconds` more.
pub fn later(when: f32, seconds: f32) -> f32 {
    when + seconds / DAY
}

/// The hour of the day, 0 to 24.
pub fn hour(when: f32) -> f32 {
    when.rem_euclid(1.0) * 24.0
}

/// `when` at an hour of a day.
pub fn at(day: u32, hour: f32) -> f32 {
    day as f32 + hour / 24.0
}

pub fn season(when: f32) -> Season {
    [Season::Spring, Season::Summer, Season::Autumn, Season::Winter][(when.rem_euclid(YEAR) / SEASON) as usize % 4]
}

/// How far through the year, 0 to 1, from the first day of spring.
pub fn year(when: f32) -> f32 {
    when.rem_euclid(YEAR) / YEAR
}

/// How warm the season is, -1 in the middle of winter to 1 in the middle of summer.
pub fn warmth(when: f32) -> f32 {
    ((year(when) - 0.125) * TAU).sin()
}

/// Which way a body is that crosses the sky as the sun does, at an hour angle (0 at its
/// highest) and so far north (up) of the sun's equinox path.
fn across(turn: f32, north: f32) -> Vec3 {
    let path = Vec3::new(-turn.sin(), turn.cos() * LEAN.cos(), turn.cos() * LEAN.sin());
    let pole = Vec3::new(0.0, LEAN.sin(), -LEAN.cos());
    (path * north.cos() + pole * north.sin()).normalize()
}

/// Which way the sun is, from the ground: up in the east (+x), highest at noon to the south,
/// down in the west, under the world at night; higher and longer up in summer.
pub fn sun(when: f32) -> Vec3 {
    across((hour(when) - 12.0) / 24.0 * TAU, SWING * warmth(when))
}

/// How much of day it is, 0 in full night to 1 in full day, passing through twilight while
/// the sun is within six degrees of the horizon.
pub fn daylight(when: f32) -> f32 {
    let t = ((sun(when).y + 0.105) / 0.21).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far the moon is through its phases, 0 to 1: new at 0, full at a half.
pub fn month(when: f32) -> f32 {
    when.rem_euclid(MONTH) / MONTH
}

/// Which way the moon is. It trails the sun across the sky by as much as it is through its
/// phases: beside the sun when new, opposite when full (so a full moon rises at sunset), and
/// it rides high when the sun rides low.
pub fn moon(when: f32) -> Vec3 {
    across((hour(when) - 12.0) / 24.0 * TAU - month(when) * TAU, -SWING * 0.6 * warmth(when))
}

/// How much of the moon's face is lit, 0 new to 1 full.
pub fn moon_lit(when: f32) -> f32 {
    (1.0 - (month(when) * TAU).cos()) / 2.0
}

/// How much light the moon gives the land, 0 to 1: by how full it is, and nothing when it is
/// down. A full moon is far more than twice a half one, as in life.
pub fn moonlight(when: f32) -> f32 {
    let up = ((moon(when).y + 0.02) / 0.12).clamp(0.0, 1.0);
    moon_lit(when).powf(1.6) * up
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sun_is_up_by_day_and_down_by_night() {
        let d = 3.0; // mid spring: the equinox
        assert!(sun(at(3, 12.0)).y > 0.85 && sun(at(3, 0.0)).y < -0.85);
        assert!(sun(d + 0.25).y.abs() < 0.02 && sun(d + 0.25).x > 0.99 && sun(d + 0.75).x < -0.99);
        assert_eq!((daylight(at(3, 12.0)), daylight(at(3, 0.0))), (1.0, 0.0));
        // Morning only gets lighter, evening only darker.
        assert!((0..60).all(|i| daylight(at(3, 4.0 + i as f32 * 0.1)) <= daylight(at(3, 4.1 + i as f32 * 0.1))));
        assert!((0..60).all(|i| daylight(at(3, 15.0 + i as f32 * 0.1)) >= daylight(at(3, 15.1 + i as f32 * 0.1))));
    }

    #[test]
    fn the_year_has_four_seasons_of_seven_days() {
        assert_eq!([0.0, 6.9, 7.0, 14.0, 21.0, 27.9, 28.0].map(season), [Season::Spring, Season::Spring, Season::Summer, Season::Autumn, Season::Winter, Season::Winter, Season::Spring]);
        // Summer days are long and the noon sun high; winter days short and the sun low.
        let lit = |day: u32| (0..240).filter(|i| sun(at(day, *i as f32 * 0.1)).y > 0.0).count() as f32 * 0.1;
        assert!(lit(10) > 13.0 && lit(24) < 11.0 && (lit(3) - 12.0).abs() < 0.6, "{} {} {}", lit(10), lit(24), lit(3));
        assert!(sun(at(10, 12.0)).y > sun(at(3, 12.0)).y && sun(at(3, 12.0)).y > sun(at(24, 12.0)).y);
        assert!(warmth(10.5) > 0.99 && warmth(24.5) < -0.99);
    }

    #[test]
    fn the_moon_goes_through_its_phases_once_a_season() {
        // New: beside the sun and dark. Full: opposite the sun, up at midnight, and bright.
        assert!(moon_lit(0.0) < 0.01 && moon(0.0).dot(sun(0.0)) > 0.9);
        let full = 3.5;
        assert!(moon_lit(full) > 0.99 && moon(full).dot(sun(full)) < -0.8);
        assert!(moon(at(4, 0.0)).y > 0.5 && moonlight(at(4, 0.0)) > 0.8);
        // Waxing: each night brighter than the last, to the full.
        assert!(moon_lit(1.0) < moon_lit(2.0) && moon_lit(2.0) < moon_lit(3.0) && moon_lit(5.0) > moon_lit(6.0));
        // No moon in the sky, no moonlight, however full.
        assert_eq!(moonlight(at(3, 12.0) + 0.02), 0.0);
        assert!((month(7.0 + 1.0) - month(1.0)).abs() < 1e-5);
    }

    #[test]
    fn time_goes_on() {
        assert!((later(START, DAY) - START - 1.0).abs() < 1e-4);
        assert!((hour(START) - 15.0).abs() < 1e-3);
    }
}
