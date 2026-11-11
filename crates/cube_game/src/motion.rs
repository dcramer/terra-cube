//! Creatures in motion. The moving is the simulation's (`cube_sim::life`): every creature in
//! the game has an agent there, stepped at the fixed tick. This file keeps the two in step:
//! it enrols creatures as agents, says who drives each (the keys, a wander, its own mind, or
//! nobody), and each frame draws the agent between ticks with the clip for what it is doing.

use crate::{
    creatures::{Creature, Registry, Wander},
    player::Controlled,
    world::Terrain,
};
use bevy::prelude::*;
use cube_sim::life::{Act, Agent, Driver, Kind, Sim};
pub use cube_sim::life::Gait;

pub struct MotionPlugin;

/// Draw the shape each creature is bumped as (`--bodies`, or the panel).
#[derive(Resource, Default)]
pub struct ShowBodies(pub bool);

/// The simulation of everything alive.
#[derive(Resource)]
pub struct Live(pub Sim);

impl Live {
    pub fn agent(&self, mover: &Mover) -> Option<&Agent> {
        self.0.agents.get(&mover.id)
    }
    pub fn agent_mut(&mut self, mover: &Mover) -> Option<&mut Agent> {
        self.0.agents.get_mut(&mover.id)
    }
    pub fn kind(&self, mover: &Mover) -> Option<&Kind> {
        self.agent(mover).map(|a| &self.0.kinds[a.kind])
    }
}

/// On every creature: its agent in the simulation, and what only the drawing needs.
#[derive(Component)]
pub struct Mover {
    pub id: u32,
    /// How far the drawn model still is from the body after a step up or down, eased out.
    lift: f32,
    /// The model tipped into its turns and its starts and stops: roll and pitch, radians.
    tilt: Vec2,
    was_speed: f32,
}

impl Mover {
    pub fn new(id: u32) -> Self {
        Self { id, lift: 0.0, tilt: Vec2::ZERO, was_speed: 0.0 }
    }
}

/// On a creature that lives by its own mind.
#[derive(Component)]
pub struct Wild;

impl Plugin for MotionPlugin {
    fn build(&self, app: &mut App) {
        let args = app.world().resource::<crate::Args>().clone();
        let kinds = Kind::load(&std::path::Path::new("assets").join("creatures"));
        app.insert_resource(Time::<Fixed>::from_hz(cube_sim::TICKS as f64))
            .insert_resource(ShowBodies(args.bodies))
            .insert_resource(Live({
                let mut sim = Sim::new(kinds, args.seed as u64);
                sim.hour = args.hour.unwrap_or(sim.hour);
                sim
            }))
            .add_systems(FixedUpdate, tick)
            .add_systems(Update, (populate, enrol, drivers, show).chain())
            .add_systems(Update, bodies);
    }
}

/// `--wild`: once the world exists, its creatures are put in it, each with its own mind.
fn populate(mut commands: Commands, args: Res<crate::Args>, registry: Res<Registry>, terrain: Option<Res<Terrain>>, mut live: ResMut<Live>, mut done: Local<bool>) {
    let Some(terrain) = terrain.filter(|_| args.wild && !*done) else { return };
    *done = true;
    live.0.populate(&terrain.0, &terrain.1);
    for (&id, agent) in &live.0.agents {
        let Some(def) = registry.index(&live.0.kinds[agent.kind].id) else { continue };
        let at = Transform::from_translation(agent.body.pos).with_rotation(Quat::from_rotation_y(agent.facing));
        commands.spawn((Creature::new(def), at, Visibility::default(), Mover::new(id), Wild));
    }
}

/// A creature with no agent gets one where it stands; an agent whose creature is gone goes.
fn enrol(mut commands: Commands, registry: Res<Registry>, terrain: Option<Res<Terrain>>, mut live: ResMut<Live>, mut new: Query<(Entity, &Creature, &mut Transform), Without<Mover>>, movers: Query<&Mover>) {
    let Some(terrain) = terrain else { return };
    for (entity, creature, mut at) in &mut new {
        let Some(kind) = live.0.kind(&registry.0[creature.def].id) else { continue };
        let id = live.0.add(&terrain.0, kind, at.translation, at.rotation.to_euler(EulerRot::YXZ).0, Driver::Stand);
        // Put down astride a step it is stood on the higher part: drawn there too, not sunk in it.
        at.translation = live.0.agents[&id].body.pos;
        commands.entity(entity).insert(Mover::new(id));
    }
    if live.0.agents.len() != movers.iter().count() + new.iter().count() {
        let kept: std::collections::HashSet<u32> = movers.iter().map(|m| m.id).collect();
        // Ones just enrolled have no `Mover` to be found by until the commands are applied.
        let fresh = live.0.agents.len() - new.iter().count();
        let mut n = 0;
        live.0.agents.retain(|id, _| {
            n += 1;
            kept.contains(id) || n > fresh
        });
    }
}

/// Who drives each creature: the keys or a wander (set from outside each frame), its own
/// mind, or nobody, in which case it stands where the panel put it.
fn drivers(mut live: ResMut<Live>, movers: Query<(&Mover, &Transform, Has<Controlled>, Has<Wander>, Has<Wild>)>) {
    for (mover, at, controlled, wander, wild) in &movers {
        let Some(agent) = live.agent_mut(mover) else { continue };
        let driver = if controlled || wander {
            Driver::Outside
        } else if wild {
            Driver::Mind
        } else {
            Driver::Stand
        };
        if agent.driver != driver {
            agent.driver = driver;
            agent.intent = default();
            agent.turn_to = None;
            agent.doing = None;
            agent.hold_jump = false;
            if driver == Driver::Stand {
                agent.body.vel = Vec3::ZERO;
            }
        }
        if driver == Driver::Stand {
            agent.body.pos = at.translation;
            agent.before = at.translation;
        }
    }
}

fn tick(terrain: Option<Res<Terrain>>, mut live: ResMut<Live>) {
    if let Some(terrain) = terrain {
        live.0.step(&terrain.0);
    }
}

/// Draws each body between ticks, tips the model into its turns, and plays the clip that
/// fits what it is doing, at the rate that keeps its feet from sliding.
fn show(time: Res<Time>, fixed: Res<Time<Fixed>>, registry: Res<Registry>, terrain: Option<Res<crate::world::Terrain>>, mut live: ResMut<Live>, mut movers: Query<(&mut Mover, &mut Creature, &mut Transform)>) {
    let dt = time.delta_secs();
    let between = fixed.overstep_fraction();
    let Sim { agents, kinds, .. } = &mut live.0;
    for (mut m, mut creature, mut transform) in &mut movers {
        let Some(agent) = agents.get_mut(&m.id) else { continue };
        if agent.driver == Driver::Stand {
            continue;
        }
        // The model eases after a step so it does not pop, but never trails the body by more
        // than a few centimetres: on a climb the steps come sixteen a second, and an uncapped
        // lag put the feet a third of a metre inside the hill.
        m.lift = ((m.lift - std::mem::take(&mut agent.stepped)) * (-22.0 * dt).exp()).clamp(-0.08, 0.08);
        transform.translation = agent.before.lerp(agent.body.pos, between) + Vec3::Y * m.lift;
        // On the ground its feet are where the ground is drawn: the body stands on cubes, and
        // the cubes are drawn as a slope.
        if let (true, Some(terrain)) = (agent.body.grounded, &terrain) {
            let at = Vec3::new(transform.translation.x, agent.body.pos.y, transform.translation.z);
            if let Some(y) = cube_voxel::mesh::eased_under(&terrain.0, at) {
                transform.translation.y = y;
                m.lift = 0.0;
            }
        }

        let speed = agent.body.ground_speed();
        let turn = (agent.facing - agent.facing_before + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let facing = agent.facing_before + turn * between;
        // Tip into turns and into speeding up, out of slowing down: a few degrees, eased.
        let pushing = if dt > 0.0 && agent.body.grounded { (speed - m.was_speed) / dt } else { 0.0 };
        m.was_speed = speed;
        let lean = Vec2::new((-turn * cube_sim::TICKS as f32 * speed * 0.009).clamp(-0.16, 0.16), (pushing * 0.006).clamp(-0.07, 0.09));
        let ease = 1.0 - (-8.0 * dt).exp();
        m.tilt = m.tilt + (lean - m.tilt) * ease;
        transform.rotation = Quat::from_rotation_y(facing) * Quat::from_rotation_x(m.tilt.y) * Quat::from_rotation_z(m.tilt.x);

        let kind = &kinds[agent.kind];
        let def = &registry.0[creature.def];
        let rate = match agent.act {
            Act::Swim => {
                if speed > 0.3 {
                    1.1
                } else {
                    0.55
                }
            }
            Act::Walk => speed / kind.walk.max(0.1),
            Act::Run => speed / kind.run.max(0.1),
            Act::Sprint if kind.sprint > 0.0 => speed / kind.sprint,
            Act::Sprint => speed / kind.run.max(0.1),
            _ => 1.0,
        };
        let (name, or) = agent.act.clip();
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
fn bodies(show: Res<ShowBodies>, live: Res<Live>, creatures: Query<(&Mover, &Transform)>, mut gizmos: Gizmos) {
    if !show.0 {
        return;
    }
    let colour = Color::srgb(0.2, 0.9, 1.0);
    for (mover, at) in &creatures {
        let Some(kind) = live.kind(mover) else { continue };
        let (radius, reach) = kind.shape();
        let facing = Quat::from_rotation_y(at.rotation.to_euler(EulerRot::YXZ).0);
        let (ahead, side) = (facing * Vec3::Z * reach, facing * Vec3::X * radius);
        for up in [0.02, kind.height] {
            let middle = at.translation + Vec3::Y * up;
            for end in [ahead, -ahead] {
                gizmos.circle(Isometry3d::new(middle + end, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), radius, colour);
            }
            for edge in [side, -side] {
                gizmos.line(middle + ahead + edge, middle - ahead + edge, colour);
            }
        }
        gizmos.line(at.translation + ahead, at.translation + ahead + Vec3::Y * kind.height, colour);
        gizmos.line(at.translation - ahead, at.translation - ahead + Vec3::Y * kind.height, colour);
    }
}
