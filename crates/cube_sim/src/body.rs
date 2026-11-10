//! A body moving through the voxel world: an upright box pushed by an [`Intent`], held out of
//! solid cells. It walks up a one-cell step without jumping (cells are half a metre, so hills
//! are staircases), stays on the ground down one, jumps, falls, and floats in water.
//!
//! The box is moved one axis at a time and, because a tick never moves it a whole cell, put
//! back against the cell face it crossed. No physics engine: this is all a block world needs.

use crate::DT;
use cube_voxel::{Block, CELL, World};
use glam::{IVec3, Vec2, Vec3};

/// How bodies move. One set for everything; a creature differs by the speed it asks for.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    /// Metres a second, squared, downward.
    pub gravity: f32,
    /// Upward speed a jump starts with. 6.9 with gravity 22 clears two cells (1.08 m).
    pub jump_speed: f32,
    /// How fast ground speed changes toward what is asked, m/s².
    pub ground_accel: f32,
    /// The same in the air: some steering, not much.
    pub air_accel: f32,
    /// The highest ledge walked up without jumping, metres.
    pub step: f32,
    /// Fastest fall, m/s. Kept under a cell a tick so the floor is never skipped.
    pub max_fall: f32,
    /// Seconds after walking off an edge in which a jump still counts.
    pub coyote: f32,
    /// Part of the asked speed kept when swimming.
    pub swim: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Self { gravity: 22.0, jump_speed: 6.9, ground_accel: 40.0, air_accel: 8.0, step: CELL + 0.05, max_fall: 25.0, coyote: 0.1, swim: 0.55 }
    }
}

/// What the player's keys or a creature's mind asks of its body this tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct Intent {
    /// Direction over the ground (x, z), at most 1 long; shorter asks for less speed.
    pub dir: Vec2,
    /// Metres a second at full `dir`.
    pub speed: f32,
    pub jump: bool,
}

/// How far up a body the water must come before it swims, and where it then floats: just
/// over the middle, so a figure lying forward to swim has its back out of the water.
const WAIST: f32 = 0.52;

#[derive(Clone, Copy, Debug)]
pub struct Body {
    /// The middle of the box's underside: where the feet are.
    pub pos: Vec3,
    pub vel: Vec3,
    /// Half the box's width and depth.
    pub half: f32,
    pub height: f32,
    pub grounded: bool,
    /// Waist deep in water: swimming.
    pub swimming: bool,
    since_ground: f32,
}

impl Body {
    pub fn new(pos: Vec3, half: f32, height: f32) -> Self {
        Self { pos, vel: Vec3::ZERO, half, height, grounded: false, swimming: false, since_ground: 1.0 }
    }

    /// Speed over the ground, m/s.
    pub fn ground_speed(&self) -> f32 {
        Vec2::new(self.vel.x, self.vel.z).length()
    }

    /// Whether the box, with its feet at `pos`, is in a solid cell.
    pub fn blocked(&self, world: &World, pos: Vec3) -> bool {
        const SKIN: f32 = 1e-4;
        let cell = |v: f32| (v / CELL).floor() as i32;
        let (x0, x1) = (cell(pos.x - self.half + SKIN), cell(pos.x + self.half - SKIN));
        let (y0, y1) = (cell(pos.y + SKIN), cell(pos.y + self.height - SKIN));
        let (z0, z1) = (cell(pos.z - self.half + SKIN), cell(pos.z + self.half - SKIN));
        (y0..=y1).any(|y| (z0..=z1).any(|z| (x0..=x1).any(|x| world.get(IVec3::new(x, y, z)).solid())))
    }

    /// The water's surface over the waist, in metres, when the waist is under water.
    fn water_over_waist(&self, world: &World) -> Option<f32> {
        let chest = self.pos + Vec3::Y * self.height * WAIST;
        let mut cell = (chest / CELL).floor().as_ivec3();
        if world.get(cell) != Block::Water {
            return None;
        }
        while world.get(cell + IVec3::Y) == Block::Water {
            cell.y += 1;
        }
        Some((cell.y + 1) as f32 * CELL)
    }

    /// One tick of [`DT`].
    pub fn step(&mut self, world: &World, intent: Intent, tuning: &Tuning) {
        let was_grounded = self.grounded;
        let surface = self.water_over_waist(world);
        self.swimming = surface.is_some();

        // Over the ground: toward the asked velocity, as fast as the footing allows.
        let dir = if intent.dir.length() > 1.0 { intent.dir.normalize() } else { intent.dir };
        let asked = dir * intent.speed * if self.swimming { tuning.swim } else { 1.0 };
        let accel = if self.grounded || self.swimming { tuning.ground_accel } else { tuning.air_accel };
        let flat = Vec2::new(self.vel.x, self.vel.z);
        let change = (asked - flat).clamp_length_max(accel * DT);
        (self.vel.x, self.vel.z) = (flat.x + change.x, flat.y + change.y);

        // Up and down: floating to the surface, a jump, or gravity.
        self.since_ground = if self.grounded { 0.0 } else { self.since_ground + DT };
        let mut jumped = false;
        if let Some(surface) = surface {
            let chest = self.pos.y + self.height * WAIST;
            let rise = ((surface - chest) * 6.0).clamp(-2.0, 2.0) + if intent.jump { 1.5 } else { 0.0 };
            self.vel.y += (rise - self.vel.y).clamp(-20.0 * DT, 20.0 * DT);
        } else {
            if intent.jump && self.since_ground <= tuning.coyote && self.vel.y <= 0.0 {
                self.vel.y = tuning.jump_speed;
                self.since_ground = tuning.coyote + 1.0;
                jumped = true;
            }
            self.vel.y = (self.vel.y - tuning.gravity * DT).max(-tuning.max_fall);
        }

        let start_y = self.pos.y;
        self.slide(world, 0, self.vel.x * DT, was_grounded && !jumped, tuning);
        self.slide(world, 2, self.vel.z * DT, was_grounded && !jumped, tuning);

        // Vertical, put back on the floor or under the ceiling it met.
        let mut to = self.pos;
        to.y += self.vel.y * DT;
        self.grounded = false;
        if self.blocked(world, to) {
            if self.vel.y <= 0.0 {
                self.pos.y = ((to.y / CELL).floor() + 1.0) * CELL;
                self.grounded = true;
            } else {
                self.pos.y = ((to.y + self.height) / CELL).floor() * CELL - self.height;
            }
            self.vel.y = 0.0;
        } else {
            self.pos = to;
        }

        // Walking off a one-cell ledge keeps the feet down instead of a short fall.
        if was_grounded && !self.grounded && !jumped && !self.swimming && self.pos.y <= start_y {
            let floor = ((start_y - tuning.step) / CELL).ceil() * CELL;
            let (on, under) = (self.pos.with_y(floor), self.pos.with_y(floor - 0.01));
            if !self.blocked(world, on) && self.blocked(world, under) {
                self.pos.y = floor;
                self.vel.y = 0.0;
                self.grounded = true;
            }
        }
    }

    /// Moves along x (0) or z (2). Against a wall it stops at the wall, unless the wall is a
    /// step low enough to walk up.
    fn slide(&mut self, world: &World, axis: usize, by: f32, may_step: bool, tuning: &Tuning) {
        if by == 0.0 {
            return;
        }
        let mut to = self.pos;
        to[axis] += by;
        if !self.blocked(world, to) {
            self.pos = to;
            return;
        }
        if may_step {
            // The top of the next cell up, if it is within a step.
            let up = ((self.pos.y / CELL + 1e-3).floor() + 1.0) * CELL;
            if up - self.pos.y <= tuning.step && !self.blocked(world, self.pos.with_y(up)) && !self.blocked(world, to.with_y(up)) {
                self.pos = to.with_y(up);
                return;
            }
        }
        // The face of the cell the leading edge went into.
        let edge = to[axis] + self.half * by.signum();
        let face = if by > 0.0 { (edge / CELL).floor() * CELL } else { ((edge / CELL).floor() + 1.0) * CELL };
        let stopped = face - self.half * by.signum();
        // Never past where it started: a box already against the wall stays put.
        self.pos[axis] = if by > 0.0 { stopped.max(self.pos[axis].min(stopped)) } else { stopped.min(self.pos[axis].max(stopped)) };
        self.vel[axis] = 0.0;
    }
}

/// Something a body cannot walk through that is not a body being moved: a creature standing
/// still. A pillar at `pos` (its foot), `half` wide and `height` tall.
#[derive(Clone, Copy, Debug)]
pub struct Pillar {
    pub pos: Vec3,
    pub half: f32,
    pub height: f32,
}

/// How far a pillar of `half` at `at` must move over the ground to stop overlapping `other`.
fn overlap(at: Vec3, half: f32, height: f32, other: &Pillar) -> Vec2 {
    if at.y >= other.pos.y + other.height || other.pos.y >= at.y + height {
        return Vec2::ZERO;
    }
    let apart = Vec2::new(at.x - other.pos.x, at.z - other.pos.z);
    let deep = half + other.half - apart.length();
    if deep <= 0.0 { Vec2::ZERO } else { apart.try_normalize().unwrap_or(Vec2::X) * deep }
}

/// Pushes bodies out of each other and out of `fixed` pillars, over the ground. Two bodies
/// share the push; a pillar gives none. The most in a tick is more than two runners close in
/// one, so nothing gets through, and little enough that a body dropped onto another eases off.
/// Bodies are treated as round here, which is what lets them slide past one another. The
/// voxel world still wins: a push never puts a body in a wall or up a step.
pub fn separate(world: &World, bodies: &mut [Body], fixed: &[Pillar]) {
    const MOST: f32 = 0.2;
    let pillar = |b: &Body| Pillar { pos: b.pos, half: b.half, height: b.height };
    for i in 0..bodies.len() {
        for j in i + 1..bodies.len() {
            let push = overlap(bodies[i].pos, bodies[i].half, bodies[i].height, &pillar(&bodies[j])).clamp_length_max(MOST) / 2.0;
            bodies[i].nudge(world, push);
            bodies[j].nudge(world, -push);
        }
        for other in fixed {
            let push = overlap(bodies[i].pos, bodies[i].half, bodies[i].height, other).clamp_length_max(MOST);
            bodies[i].nudge(world, push);
        }
    }
}

impl Body {
    /// Moves over the ground by `by` as far as the world allows, without changing speed.
    fn nudge(&mut self, world: &World, by: Vec2) {
        let vel = self.vel;
        let tuning = Tuning::default();
        self.slide(world, 0, by.x, false, &tuning);
        self.slide(world, 2, by.y, false, &tuning);
        self.vel = vel;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TICKS;
    use cube_voxel::terrain;

    /// A floor of stone with its top at y = 1 m, 40 m square.
    fn floor() -> World {
        let mut w = World::default();
        for z in -40..40 {
            for x in -40..40 {
                w.set(IVec3::new(x, 1, z), Block::Stone);
            }
        }
        w
    }

    fn player() -> Body {
        Body::new(Vec3::new(0.25, 1.0, 0.25), 0.3, 1.75)
    }

    fn run(body: &mut Body, world: &World, seconds: f32, intent: Intent) {
        for _ in 0..(seconds * TICKS as f32) as u32 {
            body.step(world, intent, &Tuning::default());
        }
    }

    const EAST: Intent = Intent { dir: Vec2::new(1.0, 0.0), speed: 3.5, jump: false };

    #[test]
    fn stands_and_walks_at_the_asked_speed() {
        let (w, mut b) = (floor(), player());
        run(&mut b, &w, 0.5, Intent::default());
        assert!(b.grounded && b.pos.y == 1.0, "{b:?}");
        run(&mut b, &w, 2.0, EAST);
        assert!((b.pos.x - 0.25 - 7.0).abs() < 0.3, "went {}", b.pos.x);
        assert!((b.ground_speed() - 3.5).abs() < 0.01);
    }

    #[test]
    fn walks_up_one_cell_and_stops_at_two() {
        let mut w = floor();
        for z in -4..4 {
            w.set(IVec3::new(4, 2, z), Block::Stone); // a step, top at 1.5
            for y in 2..4 {
                w.set(IVec3::new(8, y, z), Block::Stone); // a wall, top at 2.0
            }
        }
        let mut b = player();
        run(&mut b, &w, 0.2, Intent::default());
        run(&mut b, &w, 3.0, EAST);
        // Up the step, down the far side, and stopped against the wall at x = 4 m.
        assert!((b.pos.x - (4.0 - 0.3)).abs() < 1e-3 && b.pos.y == 1.0 && b.grounded, "{b:?}");
        assert!(!b.blocked(&w, b.pos));
    }

    #[test]
    fn stays_grounded_over_a_step_down_and_up() {
        let mut w = floor();
        for x in 4..8 {
            for z in -4..4 {
                w.set(IVec3::new(x, 2, z), Block::Stone);
            }
        }
        let mut b = player();
        run(&mut b, &w, 0.2, Intent::default());
        let mut top = 0.0f32;
        for _ in 0..(2.0 * TICKS as f32) as u32 {
            b.step(&w, EAST, &Tuning::default());
            assert!(b.grounded, "left the ground at {:?}", b.pos);
            top = top.max(b.pos.y);
        }
        assert_eq!((top, b.pos.y), (1.5, 1.0));
    }

    #[test]
    fn a_jump_clears_two_cells_and_lands() {
        let (w, mut b) = (floor(), player());
        run(&mut b, &w, 0.2, Intent::default());
        b.step(&w, Intent { jump: true, ..Intent::default() }, &Tuning::default());
        let mut top = 0.0f32;
        for _ in 0..TICKS {
            b.step(&w, Intent::default(), &Tuning::default());
            top = top.max(b.pos.y - 1.0);
        }
        assert!(top > 1.0 && top < 1.3, "rose {top}");
        assert!(b.grounded && b.pos.y == 1.0);
        // Held, it jumps again only once back on the ground, and a ceiling stops it.
        let mut w = w;
        w.set(IVec3::new(0, 6, 0), Block::Stone); // underside at 3.0: 0.25 m of headroom
        run(&mut b, &w, 1.0, Intent { jump: true, ..Intent::default() });
        assert!(b.pos.y <= 3.0 - 1.75 + 1e-3);
    }

    #[test]
    fn bodies_do_not_pass_through_each_other_or_a_pillar() {
        let w = floor();
        let step = |bodies: &mut [Body], fixed: &[Pillar], intents: &[Intent]| {
            separate(&w, bodies, fixed);
            for (b, i) in bodies.iter_mut().zip(intents) {
                b.step(&w, *i, &Tuning::default());
            }
        };
        // Head on: they meet and stop, neither inside the other.
        let mut pair = [player(), Body::new(Vec3::new(4.25, 1.0, 0.25), 0.3, 1.75)];
        let west = Intent { dir: Vec2::new(-1.0, 0.0), ..EAST };
        for _ in 0..TICKS * 3 {
            step(&mut pair, &[], &[EAST, west]);
        }
        let gap = pair[1].pos.x - pair[0].pos.x;
        // Each tick they step into each other and are put back, so they rest a step's worth in.
        assert!(gap > 0.45 && gap < 0.62, "gap {gap}");
        // Past a standing pillar a little off its line: it slides round and carries on.
        let mut one = [player()];
        let post = [Pillar { pos: Vec3::new(3.0, 1.0, 0.45), half: 0.3, height: 1.0 }];
        for _ in 0..TICKS * 3 {
            step(&mut one, &post, &[EAST]);
            let apart = Vec2::new(one[0].pos.x - 3.0, one[0].pos.z - 0.45).length();
            assert!(apart > 0.52, "inside the pillar: {apart}");
        }
        assert!(one[0].pos.x > 6.0 && one[0].pos.z < 0.0, "{:?}", one[0].pos);
    }

    #[test]
    fn floats_in_water() {
        let mut w = World::default();
        for z in -8..8 {
            for x in -8..8 {
                w.set(IVec3::new(x, 0, z), Block::Stone);
                for y in 1..9 {
                    w.set(IVec3::new(x, y, z), Block::Water); // surface at 4.5
                }
            }
        }
        let mut b = Body::new(Vec3::new(0.25, 6.0, 0.25), 0.3, 1.75);
        run(&mut b, &w, 6.0, Intent::default());
        let waist = b.pos.y + 1.75 * WAIST;
        assert!(b.swimming && (waist - 4.5).abs() < 0.15, "waist at {waist}");
    }

    #[test]
    fn never_ends_a_tick_inside_the_test_terrain() {
        let w = terrain::generate(7, 2);
        let mut b = Body::new(Vec3::new(0.25, w.ground_height(0.25, 0.25), 0.25), 0.3, 1.75);
        for tick in 0..TICKS * 60 {
            // A wandering heading that turns over the minute, jumping now and then.
            let a = tick as f32 * 0.004;
            let intent = Intent { dir: Vec2::new(a.cos(), (a * 1.7).sin()), speed: 5.0, jump: tick % 90 == 0 };
            b.step(&w, intent, &Tuning::default());
            assert!(!b.blocked(&w, b.pos), "inside the ground at tick {tick}: {b:?}");
            assert!(b.pos.y > 0.0, "fell out of the world at tick {tick}");
        }
        assert!(Vec2::new(b.pos.x, b.pos.z).length() > 5.0, "went nowhere: {:?}", b.pos);
    }
}
