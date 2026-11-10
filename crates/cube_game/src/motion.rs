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

impl Plugin for MotionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(cube_sim::TICKS as f64)).add_systems(FixedUpdate, tick).add_systems(Update, (enrol, show).chain());
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
}

/// Half the width of a creature's body box, from its measured size.
pub fn half_width(def: &Def) -> f32 {
    (def.meta.length.max(def.meta.height * 0.35) * 0.45).clamp(0.2, 0.6)
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
        let body = Body::new(pos, half_width(def), def.meta.height.max(0.3));
        let facing = at.rotation.to_euler(EulerRot::YXZ).0;
        commands.entity(entity).insert(Mover { body, intent: Intent::default(), gait: Gait::Walk, facing, turn_to: None, before: pos, lift: 0.0, airborne: 0.0, jumped: false });
    }
    for entity in &idle {
        commands.entity(entity).remove::<Mover>();
    }
}

fn tick(terrain: Option<Res<Terrain>>, registry: Res<Registry>, mut movers: Query<&mut Mover>, standing: Query<(&Creature, &Transform), Without<Mover>>) {
    let Some(terrain) = terrain else { return };
    let world = &terrain.0;
    // Creatures standing about are pillars to walk round; movers push each other.
    let pillars: Vec<Pillar> = standing.iter().map(|(c, at)| Pillar { pos: at.translation, half: half_width(&registry.0[c.def]), height: registry.0[c.def].meta.height }).collect();
    let mut bodies: Vec<Body> = movers.iter().map(|m| m.body).collect();
    body::separate(world, &mut bodies, &pillars);
    for (mut m, pushed) in movers.iter_mut().zip(bodies) {
        let (was, was_grounded) = (m.body.pos, m.body.grounded);
        m.body = pushed;
        let intent = m.intent;
        m.body.step(world, intent, &Tuning::default());
        m.intent.jump = false;
        m.before = was;
        if was_grounded && !m.body.grounded {
            m.jumped = m.body.vel.y > 0.5;
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
        transform.rotation = Quat::from_rotation_y(m.facing);

        let def = &registry.0[creature.def];
        let meta = &def.meta;
        // The clip wanted, its rate, and what to play where a creature has no such clip.
        let (name, rate, or) = if m.body.swimming {
            ("swim", if speed > 0.3 { 1.1 } else { 0.55 }, "walk")
        } else if m.airborne > 0.1 {
            (if m.jumped && m.body.vel.y > 0.0 { "jump" } else { "fall" }, 1.0, "run")
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
