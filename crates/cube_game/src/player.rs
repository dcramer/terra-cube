//! Control: walking a creature about with the keys and a camera that follows it. Any creature
//! can be controlled, which is how the player is played and how an animal's gaits are judged
//! at real speeds.
//!
//! The movement itself is `cube_sim::body`, stepped at the simulation's fixed tick; this file
//! only turns keys into an [`Intent`], draws the body between ticks, picks the clip that
//! matches its speed, and flies the camera.

use crate::{
    Args,
    camera::Orbit,
    creatures::{Creature, Registry, Wander},
    world::Terrain,
};
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use cube_sim::body::{Body, Intent, Tuning};

pub struct PlayPlugin;

impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(cube_sim::TICKS as f64))
            .init_resource::<Controls>()
            .add_systems(FixedUpdate, tick)
            .add_systems(Update, (start, keys, script, show, follow, cursor).chain());
    }
}

/// What is asked of the controlled creature, from the keys or from a script.
#[derive(Resource, Default)]
pub struct Controls {
    /// x to the camera's right, y away from the camera.
    pub dir: Vec2,
    pub walk: bool,
    /// Set when asked, cleared by the tick that uses it, so a tap between ticks counts.
    pub jump: bool,
    /// The way the camera faces, which "forward" is measured from.
    pub yaw: f32,
}

/// On the creature being controlled.
#[derive(Component)]
pub struct Controlled {
    pub body: Body,
    /// Where the body was a tick ago, to draw it smoothly between ticks.
    before: Vec3,
    facing: f32,
    /// How far the drawn model still is from the body after a step up or down, eased out.
    lift: f32,
    airborne: f32,
}

/// Puts a creature under control, standing where it is.
pub fn control(commands: &mut Commands, entity: Entity, at: &Transform, def: &crate::creatures::Def, terrain: &Terrain) {
    let half = (def.meta.length.max(def.meta.height * 0.35) * 0.45).clamp(0.2, 0.6);
    let mut pos = at.translation;
    pos.y = terrain.0.ground_height(pos.x, pos.z);
    let body = Body::new(pos, half, def.meta.height.max(0.3));
    let facing = at.rotation.to_euler(EulerRot::YXZ).0;
    commands.entity(entity).remove::<Wander>().insert(Controlled { body, before: pos, facing, lift: 0.0, airborne: 0.0 });
}

/// The speeds a controlled creature moves at. Its clips are measured (`meta.toml`) and mostly
/// slower than play wants, so they are played somewhat fast; `show` keeps feet matched.
fn speeds(def: &crate::creatures::Def) -> (f32, f32) {
    (def.meta.walk_speed.max(0.3) * 1.6, def.meta.run_speed.max(0.6) * 1.3)
}

/// `--play`: control the player (or the first creature there is) from the start. Tab takes
/// and gives up control of the picked creature after that.
fn start(
    mut commands: Commands,
    args: Res<Args>,
    registry: Res<Registry>,
    terrain: Option<Res<Terrain>>,
    keys: Res<ButtonInput<KeyCode>>,
    selected: Option<Res<crate::ui::Selected>>,
    creatures: Query<(Entity, &Creature, &Transform)>,
    controlled: Query<Entity, With<Controlled>>,
    mut cameras: Query<&mut Orbit>,
    mut begun: Local<bool>,
) {
    let Some(terrain) = terrain else { return };
    let toggle = keys.just_pressed(KeyCode::Tab);
    let begin = args.play && !*begun && !creatures.is_empty();
    if toggle && let Some(entity) = controlled.iter().next() {
        commands.entity(entity).remove::<Controlled>();
        return;
    }
    if !(toggle || begin) {
        return;
    }
    *begun = true;
    let picked = selected.and_then(|s| s.0).and_then(|e| creatures.get(e).ok());
    let player = creatures.iter().find(|(_, c, _)| registry.0[c.def].spec.kind == "player");
    let Some((entity, creature, at)) = (if begin { player.or(picked) } else { picked.or(player) }).or_else(|| creatures.iter().next()) else { return };
    let def = &registry.0[creature.def];
    control(&mut commands, entity, at, def, &terrain);
    for mut orbit in &mut cameras {
        orbit.distance = (def.meta.height.max(def.meta.length) * 3.2).clamp(2.5, 9.0);
        orbit.pitch = 0.28;
    }
}

fn keys(keys: Res<ButtonInput<KeyCode>>, args: Res<Args>, mut controls: ResMut<Controls>) {
    if args.scene.is_some() {
        return;
    }
    let held = |k: KeyCode| keys.pressed(k) as i32 as f32;
    controls.dir = Vec2::new(held(KeyCode::KeyD) - held(KeyCode::KeyA), held(KeyCode::KeyW) - held(KeyCode::KeyS));
    controls.walk = keys.pressed(KeyCode::ShiftLeft);
    controls.jump |= keys.pressed(KeyCode::Space);
}

/// With no window there are no keys: a fixed run for a picture to be taken of. Stand, run
/// toward the hills, jump twice on the way. Prints where the body is once a second.
fn script(time: Res<Time>, args: Res<Args>, mut controls: ResMut<Controls>, controlled: Query<(&Controlled, &Creature)>, registry: Res<Registry>, mut clock: Local<(f32, f32)>) {
    if args.scene.is_none() || !args.play {
        return;
    }
    let Ok((c, creature)) = controlled.single() else { return };
    clock.0 += time.delta_secs();
    let t = clock.0;
    controls.dir = if t > 0.5 { Vec2::Y } else { Vec2::ZERO };
    controls.walk = t < 2.0;
    controls.jump |= (3.0..3.1).contains(&t) || (6.0..6.1).contains(&t);
    if t >= clock.1 {
        clock.1 += 1.0;
        let def = &registry.0[creature.def];
        let clip = def.clips.get(creature.clip).map_or("-", |c| c.0.as_str());
        let b = &c.body;
        println!("play t={t:.1} pos={:.1},{:.1},{:.1} speed={:.2} grounded={} clip={clip}x{:.2}", b.pos.x, b.pos.y, b.pos.z, b.ground_speed(), b.grounded, creature.speed);
    }
}

fn tick(mut controls: ResMut<Controls>, terrain: Option<Res<Terrain>>, registry: Res<Registry>, mut controlled: Query<(&mut Controlled, &Creature)>) {
    let Some(terrain) = terrain else { return };
    // The camera looks along -Z turned by its yaw; right is +X turned the same.
    let (sin, cos) = controls.yaw.sin_cos();
    let dir = Vec2::new(cos, -sin) * controls.dir.x + Vec2::new(-sin, -cos) * controls.dir.y;
    for (mut c, creature) in &mut controlled {
        let (walk, run) = speeds(&registry.0[creature.def]);
        let intent = Intent { dir, speed: if controls.walk { walk } else { run }, jump: controls.jump };
        let (was, was_grounded) = (c.body.pos, c.body.grounded);
        c.body.step(&terrain.0, intent, &Tuning::default());
        c.before = was;
        // A step up or down moves the body half a metre in one tick; the model eases after.
        let rise = c.body.pos.y - was.y;
        if was_grounded && c.body.grounded && rise.abs() > 0.2 {
            c.lift -= rise;
            c.before.y = c.body.pos.y;
        }
        // Off the edge of the test world: back to the middle.
        if c.body.pos.y < -20.0 {
            c.body.pos = Vec3::new(0.25, terrain.0.ground_height(0.25, 0.25), 0.25);
            c.body.vel = Vec3::ZERO;
            c.before = c.body.pos;
        }
    }
    controls.jump = false;
}

/// Draws the body between ticks, turns the model the way it is going, and plays the clip that
/// matches its speed at the rate that keeps its feet from sliding.
fn show(time: Res<Time>, fixed: Res<Time<Fixed>>, registry: Res<Registry>, mut controlled: Query<(&mut Controlled, &mut Creature, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut c, mut creature, mut transform) in &mut controlled {
        c.lift *= (-14.0 * dt).exp();
        c.airborne = if c.body.grounded || c.body.swimming { 0.0 } else { c.airborne + dt };
        transform.translation = c.before.lerp(c.body.pos, fixed.overstep_fraction()) + Vec3::Y * c.lift;

        let speed = c.body.ground_speed();
        if speed > 0.3 {
            let want = c.body.vel.x.atan2(c.body.vel.z);
            let turn = (want - c.facing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            c.facing += turn * (1.0 - (-14.0 * dt).exp());
        }
        transform.rotation = Quat::from_rotation_y(c.facing);

        let def = &registry.0[creature.def];
        let (walk, run) = speeds(def);
        let (name, rate) = if c.airborne > 0.12 {
            // No falling clip yet: the run, slowed, reads as legs in the air.
            ("run", 0.45)
        } else if speed < 0.2 {
            ("idle", 1.0)
        } else if speed < (walk + run) / 2.0 {
            ("walk", speed / def.meta.walk_speed.max(0.1))
        } else {
            ("run", speed / def.meta.run_speed.max(0.1))
        };
        if let Some(index) = def.clips.iter().position(|clip| clip.0 == name) {
            creature.clip = index;
            creature.speed = rate.clamp(0.3, 2.5);
            creature.hold = None;
        }
    }
}

/// The camera behind the controlled creature: the mouse turns it, the wheel moves it in and
/// out, and ground between it and the creature pulls it closer.
fn follow(
    mut controls: ResMut<Controls>,
    args: Res<Args>,
    registry: Res<Registry>,
    terrain: Option<Res<Terrain>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    controlled: Query<(&Creature, &Transform), (With<Controlled>, Without<Orbit>)>,
    mut cameras: Query<(&mut Orbit, &mut Transform)>,
) {
    let (Ok((creature, at)), Some(terrain)) = (controlled.single(), terrain) else { return };
    let def = &registry.0[creature.def];
    for (mut orbit, mut transform) in &mut cameras {
        if args.scene.is_none() {
            orbit.yaw -= motion.delta.x * 0.003;
            orbit.pitch = (orbit.pitch + motion.delta.y * 0.003).clamp(-1.2, 1.45);
            orbit.distance = (orbit.distance * (1.0 - scroll.delta.y * 0.1)).clamp(1.2, 30.0);
        }
        controls.yaw = orbit.yaw;
        orbit.focus = at.translation + Vec3::Y * def.meta.height * 0.85;
        let back = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0) * Vec3::Z;
        let clear = terrain.0.raycast(orbit.focus, back, orbit.distance + 0.3).map_or(orbit.distance, |hit| (hit - 0.3).max(0.4));
        transform.translation = orbit.focus + back * clear.min(orbit.distance);
        transform.look_at(orbit.focus, Vec3::Y);
    }
}

/// The mouse belongs to the camera while a creature is controlled, and to the panel otherwise.
fn cursor(controlled: Query<(), With<Controlled>>, mut windows: Query<&mut CursorOptions>) {
    let locked = !controlled.is_empty();
    for mut options in &mut windows {
        let mode = if locked { CursorGrabMode::Locked } else { CursorGrabMode::None };
        if options.grab_mode != mode {
            options.grab_mode = mode;
            options.visible = !locked;
        }
    }
}
