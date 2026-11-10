//! Moving creatures: anything walking about, whether the keys or a wander drives it, has a
//! [`Mover`], which holds its `cube_sim` body. Each simulation tick the bodies are kept out of
//! each other and stepped; each frame they are drawn between ticks with the clip that fits
//! what the body is doing.

use crate::{
    creatures::{Creature, Def, Registry, Wander},
    player::Controlled,
    world::Terrain,
};
use bevy::prelude::*;
use cube_sim::body::{self, Body, Intent, Pillar, Tuning};

pub struct MotionPlugin;

/// Draw the shape each creature is bumped as (`--bodies`, or the panel).
#[derive(Resource, Default)]
pub struct ShowBodies(pub bool);

impl Plugin for MotionPlugin {
    fn build(&self, app: &mut App) {
        let drawn = app.world().resource::<crate::Args>().bodies;
        app.insert_resource(Time::<Fixed>::from_hz(cube_sim::TICKS as f64)).insert_resource(ShowBodies(drawn)).add_systems(FixedUpdate, tick).add_systems(Update, ((enrol, show).chain(), bodies));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Gait {
    #[default]
    Walk,
    Run,
    Sprint,
}

#[derive(Component)]
pub struct Mover {
    pub body: Body,
    /// What its driver asks of it; `gait` says which clip the speed belongs to.
    pub intent: Intent,
    pub gait: Gait,
    /// The way the model faces, radians about Y from +Z. `turn_to` overrides following the
    /// direction of travel (first person).
    pub facing: f32,
    pub turn_to: Option<f32>,
    /// Where the body was a tick ago, to draw it smoothly between ticks.
    before: Vec3,
    /// How far the drawn model still is from the body after a step up or down, eased out.
    lift: f32,
    airborne: f32,
    /// Left the ground by jumping, not by walking off something.
    jumped: bool,
    /// Seconds a jump is still wanted for: pressed a moment before landing, it still jumps.
    pub want_jump: f32,
    /// The jump key is down: the jump rises to its full height.
    pub hold_jump: bool,
    /// Seconds of the landing still to play, after a fall worth one.
    landing: f32,
    /// The model tipped into its turns and its starts and stops: roll and pitch, radians.
    tilt: Vec2,
    was_speed: f32,
}

/// The shape a creature is bumped as, from its measured standing size: its radius, and for a
/// body longer than it is wide how far its capsule runs fore and aft of its middle.
pub fn shape(def: &Def) -> (f32, f32) {
    let (length, width) = (def.meta.length, if def.meta.width > 0.0 { def.meta.width } else { def.meta.length });
    if length > width * 1.3 {
        let radius = (width * 0.425).clamp(0.1, 0.6);
        (radius, (length / 2.0 - radius).max(0.0))
    } else {
        (((length + width) / 4.0).clamp(0.1, 0.6), 0.0)
    }
}

/// The capsule's half-line over the ground for a body facing `facing`.
fn along(facing: f32, reach: f32) -> Vec2 {
    Vec2::new(facing.sin(), facing.cos()) * reach
}

/// The speed a gait moves at: the speed its clip was built or measured for (`meta.toml`), so
/// the clip plays at its own rate and the feet hold. A faster creature wants a faster clip,
/// made in forge, not a number changed here.
pub fn speed(def: &Def, gait: Gait) -> f32 {
    let m = &def.meta;
    match gait {
        Gait::Walk => m.walk_speed.max(0.1),
        Gait::Sprint if m.sprint_speed > 0.0 => m.sprint_speed,
        _ => m.run_speed.max(0.2),
    }
}

/// Anything driven gets a body where it stands; anything no longer driven gives it up, so the
/// panel's clip buttons are in charge again.
fn enrol(
    mut commands: Commands,
    registry: Res<Registry>,
    terrain: Option<Res<Terrain>>,
    driven: Query<(Entity, &Creature, &Transform), (Or<(With<Controlled>, With<Wander>)>, Without<Mover>)>,
    idle: Query<Entity, (With<Mover>, Without<Controlled>, Without<Wander>)>,
) {
    let Some(terrain) = terrain else { return };
    for (entity, creature, at) in &driven {
        let def = &registry.0[creature.def];
        let mut pos = at.translation;
        pos.y = pos.y.max(terrain.0.ground_height(pos.x, pos.z));
        let body = Body::new(pos, shape(def).0, def.meta.height.max(0.3));
        let facing = at.rotation.to_euler(EulerRot::YXZ).0;
        commands.entity(entity).insert(Mover { body, intent: Intent::default(), gait: Gait::Walk, facing, turn_to: None, before: pos, lift: 0.0, airborne: 0.0, jumped: false, want_jump: 0.0, hold_jump: false, landing: 0.0, tilt: Vec2::ZERO, was_speed: 0.0 });
    }
    for entity in &idle {
        commands.entity(entity).remove::<Mover>();
    }
}

fn tick(terrain: Option<Res<Terrain>>, registry: Res<Registry>, mut movers: Query<(&mut Mover, &Creature)>, standing: Query<(&Creature, &Transform), Without<Mover>>) {
    let Some(terrain) = terrain else { return };
    let world = &terrain.0;
    // Creatures standing about are pillars to walk round; movers push each other.
    let pillars: Vec<Pillar> = standing
        .iter()
        .map(|(c, at)| {
            let def = &registry.0[c.def];
            let (half, reach) = shape(def);
            Pillar { pos: at.translation, half, height: def.meta.height, long: along(at.rotation.to_euler(EulerRot::YXZ).0, reach) }
        })
        .collect();
    for (mut m, c) in &mut movers {
        m.body.long = along(m.facing, shape(&registry.0[c.def]).1);
    }
    let mut bodies: Vec<Body> = movers.iter().map(|(m, _)| m.body).collect();
    body::separate(world, &mut bodies, &pillars);
    for ((mut m, _), pushed) in movers.iter_mut().zip(bodies) {
        let (was, was_grounded) = (m.body.pos, m.body.grounded);
        m.body = pushed;
        let mut intent = m.intent;
        intent.jump = m.want_jump > 0.0;
        intent.hold = m.hold_jump;
        let fall = m.body.vel.y;
        m.body.step(world, intent, &Tuning::default());
        m.want_jump = if m.body.jumping { 0.0 } else { (m.want_jump - cube_sim::DT).max(0.0) };
        m.before = was;
        if was_grounded && !m.body.grounded {
            m.jumped = m.body.vel.y > 0.5;
        }
        // Down from more than about a cell: a landing.
        if !was_grounded && m.body.grounded && fall < -4.5 {
            m.landing = 0.3;
        }
        // A step up or down moves the body half a metre in one tick; the model eases after.
        let rise = m.body.pos.y - was.y;
        if was_grounded && m.body.grounded && rise.abs() > 0.2 {
            m.lift -= rise;
            m.before.y = m.body.pos.y;
        }
        // Off the edge of the test world: back to the middle.
        if m.body.pos.y < -20.0 {
            m.body.pos = Vec3::new(0.25, world.ground_height(0.25, 0.25), 0.25);
            m.body.vel = Vec3::ZERO;
            m.before = m.body.pos;
        }
    }
}

/// Draws each body between ticks, turns the model the way it is going, and plays the clip that
/// fits: swim, jump or fall, or its gait at the rate that keeps its feet from sliding.
fn show(time: Res<Time>, fixed: Res<Time<Fixed>>, registry: Res<Registry>, mut movers: Query<(&mut Mover, &mut Creature, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut m, mut creature, mut transform) in &mut movers {
        m.lift *= (-14.0 * dt).exp();
        m.airborne = if m.body.grounded || m.body.swimming { 0.0 } else { m.airborne + dt };
        transform.translation = m.before.lerp(m.body.pos, fixed.overstep_fraction()) + Vec3::Y * m.lift;

        let speed = m.body.ground_speed();
        let want = m.turn_to.or((speed > 0.3).then(|| m.body.vel.x.atan2(m.body.vel.z)));
        if let Some(want) = want {
            let turn = (want - m.facing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            m.facing += turn * (1.0 - (-14.0 * dt).exp());
        }
        // Tip into turns and into speeding up, out of slowing down: a few degrees, eased.
        let turning = want.map_or(0.0, |w| (w - m.facing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI);
        let pushing = if dt > 0.0 && m.body.grounded { (speed - m.was_speed) / dt } else { 0.0 };
        m.was_speed = speed;
        let lean = Vec2::new((-turning * speed * 0.12).clamp(-0.16, 0.16), (pushing * 0.006).clamp(-0.07, 0.09));
        let ease = 1.0 - (-8.0 * dt).exp();
        m.tilt = m.tilt + (lean - m.tilt) * ease;
        transform.rotation = Quat::from_rotation_y(m.facing) * Quat::from_rotation_x(m.tilt.y) * Quat::from_rotation_z(m.tilt.x);
        m.landing = (m.landing - dt).max(0.0);

        let def = &registry.0[creature.def];
        let meta = &def.meta;
        // The clip wanted, its rate, and what to play where a creature has no such clip.
        let (name, rate, or) = if m.body.swimming {
            ("swim", if speed > 0.3 { 1.1 } else { 0.55 }, "walk")
        } else if m.airborne > 0.1 {
            (if m.jumped && m.body.vel.y > 0.0 { "jump" } else { "fall" }, 1.0, "run")
        } else if m.landing > 0.0 && speed < 1.5 {
            ("land", 1.0, "idle")
        } else if speed < 0.2 {
            ("idle", 1.0, "idle")
        } else {
            match m.gait {
                Gait::Walk => ("walk", speed / meta.walk_speed.max(0.1), "walk"),
                Gait::Sprint if meta.sprint_speed > 0.0 => ("sprint", speed / meta.sprint_speed, "run"),
                _ => ("run", speed / meta.run_speed.max(0.1), "walk"),
            }
        };
        let find = |name: &str| def.clips.iter().position(|clip| clip.0 == name);
        if let Some(index) = find(name).or_else(|| find(or)) {
            creature.clip = index;
            creature.speed = rate.clamp(0.3, 2.5);
            creature.hold = None;
        }
    }
}

/// The shape each creature is bumped as: a ring at each end of its capsule, at its feet and
/// its top, joined along the sides.
fn bodies(show: Res<ShowBodies>, registry: Res<Registry>, creatures: Query<(&Creature, &Transform)>, mut gizmos: Gizmos) {
    if !show.0 {
        return;
    }
    let colour = Color::srgb(0.2, 0.9, 1.0);
    for (creature, at) in &creatures {
        let def = &registry.0[creature.def];
        let (radius, reach) = shape(def);
        let facing = Quat::from_rotation_y(at.rotation.to_euler(EulerRot::YXZ).0);
        let (ahead, side) = (facing * Vec3::Z * reach, facing * Vec3::X * radius);
        for up in [0.02, def.meta.height] {
            let middle = at.translation + Vec3::Y * up;
            for end in [ahead, -ahead] {
                gizmos.circle(Isometry3d::new(middle + end, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), radius, colour);
            }
            for edge in [side, -side] {
                gizmos.line(middle + ahead + edge, middle - ahead + edge, colour);
            }
        }
        gizmos.line(at.translation + ahead, at.translation + ahead + Vec3::Y * def.meta.height, colour);
        gizmos.line(at.translation - ahead, at.translation - ahead + Vec3::Y * def.meta.height, colour);
    }
}
