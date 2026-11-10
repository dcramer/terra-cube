//! Control: driving a creature with the keys and a camera that follows it. Any creature can be
//! controlled, which is how the player is played and how an animal's gaits are judged at real
//! speeds. The movement itself is `motion` (and under it `cube_sim::body`); this file turns
//! keys into what is asked of the body, and flies the camera, behind it or from its eyes.

use crate::{
    Args,
    camera::Orbit,
    creatures::{Creature, Registry, Wander},
    motion::{self, Gait, Mover},
    world::Terrain,
};
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

pub struct PlayPlugin;

impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Controls>().add_systems(Update, (start, keys, script, drive, follow, cursor, trace).chain());
    }
}

/// What is asked of the controlled creature, from the keys or from a script.
#[derive(Resource, Default)]
pub struct Controls {
    /// x to the camera's right, y away from the camera.
    pub dir: Vec2,
    pub gait: Gait,
    pub jump: bool,
}

/// On the creature being controlled.
#[derive(Component, Default)]
pub struct Controlled {
    /// Seen from its own eyes, the model hidden.
    pub first_person: bool,
}

/// `--play`: control the player (`--as ID`: that creature) from the start. Tab takes and gives
/// up control of the picked creature after that.
fn start(
    mut commands: Commands,
    args: Res<Args>,
    registry: Res<Registry>,
    keys: Res<ButtonInput<KeyCode>>,
    selected: Option<Res<crate::ui::Selected>>,
    creatures: Query<(Entity, &Creature, &Transform)>,
    controlled: Query<Entity, With<Controlled>>,
    mut cameras: Query<&mut Orbit>,
    mut begun: Local<bool>,
) {
    let toggle = keys.just_pressed(KeyCode::Tab);
    let begin = args.play && !*begun && !creatures.is_empty();
    if toggle && let Some(entity) = controlled.iter().next() {
        commands.entity(entity).remove::<Controlled>().insert(Visibility::Inherited);
        return;
    }
    if !(toggle || begin) {
        return;
    }
    *begun = true;
    let picked = selected.and_then(|s| s.0).and_then(|e| creatures.get(e).ok());
    let wanted = args.play_as.as_deref().unwrap_or("player");
    let named = creatures.iter().find(|(_, c, _)| registry.0[c.def].id == wanted);
    let Some((entity, creature, at)) = (if begin { named.or(picked) } else { picked.or(named) }).or_else(|| creatures.iter().next()) else { return };
    let def = &registry.0[creature.def];
    commands.entity(entity).remove::<Wander>().insert(Controlled { first_person: args.view == "first" });
    for mut orbit in &mut cameras {
        orbit.distance = (def.meta.height.max(def.meta.length) * 3.2).clamp(2.5, 9.0);
        orbit.pitch = 0.28;
        // Behind it, looking the way it faces. A scene's camera is aimed by `--yaw` instead.
        if args.scene.is_none() {
            orbit.yaw = at.rotation.to_euler(EulerRot::YXZ).0 + std::f32::consts::PI;
        }
    }
}

fn keys(keys: Res<ButtonInput<KeyCode>>, args: Res<Args>, mut controls: ResMut<Controls>, mut controlled: Query<&mut Controlled>) {
    if args.scene.is_some() {
        return;
    }
    let held = |k: KeyCode| keys.pressed(k) as i32 as f32;
    controls.dir = Vec2::new(held(KeyCode::KeyD) - held(KeyCode::KeyA), held(KeyCode::KeyW) - held(KeyCode::KeyS));
    controls.gait = if keys.pressed(KeyCode::ControlLeft) {
        Gait::Walk
    } else if keys.pressed(KeyCode::ShiftLeft) {
        Gait::Sprint
    } else {
        Gait::Run
    };
    controls.jump = keys.pressed(KeyCode::Space);
    if keys.just_pressed(KeyCode::KeyV) {
        for mut c in &mut controlled {
            c.first_person = !c.first_person;
        }
    }
}

/// With no window there are no keys: a set route for a picture to be taken of. Stand, walk,
/// run and jump, then sprint and jump, the way the camera faces.
fn script(time: Res<Time>, args: Res<Args>, mut controls: ResMut<Controls>, controlled: Query<(), With<Controlled>>, mut clock: Local<f32>) {
    if args.scene.is_none() || !args.play || controlled.is_empty() {
        return;
    }
    *clock += time.delta_secs();
    let t = *clock;
    controls.dir = if t > 0.5 { Vec2::Y } else { Vec2::ZERO };
    controls.gait = if t < 2.0 {
        Gait::Walk
    } else if t < 5.0 {
        Gait::Run
    } else {
        Gait::Sprint
    };
    controls.jump = (3.0..3.1).contains(&t) || (6.0..6.1).contains(&t);
}

/// `--trace`, and every scripted run: the controlled body in a line, four times a second.
fn trace(time: Res<Time>, args: Res<Args>, controlled: Query<(&Mover, &Creature, &Controlled)>, registry: Res<Registry>, cameras: Query<&Orbit>, mut clock: Local<(f32, f32)>) {
    if !(args.trace || args.scene.is_some() && args.play) {
        return;
    }
    let Ok((m, creature, c)) = controlled.single() else { return };
    clock.0 += time.delta_secs();
    if clock.0 >= clock.1 {
        clock.1 += if args.trace { 0.25 } else { 1.0 };
        let def = &registry.0[creature.def];
        let clip = def.clips.get(creature.clip).map_or("-", |c| c.0.as_str());
        let b = &m.body;
        let yaw = cameras.iter().next().map_or(0.0, |o| o.yaw);
        println!(
            "play t={:.1} pos={:.1},{:.1},{:.1} speed={:.2} grounded={} swimming={} clip={clip}x{:.2} yaw={yaw:.2} first={}",
            clock.0, b.pos.x, b.pos.y, b.pos.z, b.ground_speed(), b.grounded, b.swimming, creature.speed, c.first_person
        );
    }
}

/// Hands the controls to the controlled creature's body, turned to the way the camera faces.
fn drive(controls: Res<Controls>, registry: Res<Registry>, cameras: Query<&Orbit>, mut controlled: Query<(&Controlled, &Creature, &mut Mover, &mut Visibility)>) {
    let yaw = cameras.iter().next().map_or(0.0, |o| o.yaw);
    // The camera looks along -Z turned by its yaw; right is +X turned the same.
    let (sin, cos) = yaw.sin_cos();
    for (c, creature, mut m, mut visibility) in &mut controlled {
        m.intent.dir = Vec2::new(cos, -sin) * controls.dir.x + Vec2::new(-sin, -cos) * controls.dir.y;
        m.intent.speed = motion::speed(&registry.0[creature.def], controls.gait);
        // Held until a tick uses it, so a tap between ticks still counts.
        m.intent.jump |= controls.jump;
        m.gait = controls.gait;
        m.turn_to = c.first_person.then_some(yaw + std::f32::consts::PI);
        let want = if c.first_person { Visibility::Hidden } else { Visibility::Inherited };
        if *visibility != want {
            *visibility = want;
        }
    }
}

/// The camera on the controlled creature: the mouse turns it. Behind the creature the wheel
/// moves it in and out and ground in the way pulls it closer; in first person it is the eyes.
fn follow(
    args: Res<Args>,
    registry: Res<Registry>,
    terrain: Option<Res<Terrain>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    controlled: Query<(&Controlled, &Creature, &Transform), Without<Orbit>>,
    mut cameras: Query<(&mut Orbit, &mut Transform)>,
) {
    let (Ok((c, creature, at)), Some(terrain)) = (controlled.single(), terrain) else { return };
    let def = &registry.0[creature.def];
    for (mut orbit, mut transform) in &mut cameras {
        if args.scene.is_none() {
            orbit.yaw -= motion.delta.x * 0.003;
            orbit.pitch = (orbit.pitch + motion.delta.y * 0.003).clamp(-1.3, 1.45);
            if !c.first_person {
                orbit.distance = (orbit.distance * (1.0 - scroll.delta.y * 0.1)).clamp(1.2, 30.0);
            }
        }
        let back = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0) * Vec3::Z;
        if c.first_person {
            let eyes = at.translation + Vec3::Y * def.meta.height * 0.92;
            transform.translation = eyes;
            transform.look_to(-back, Vec3::Y);
            continue;
        }
        orbit.focus = at.translation + Vec3::Y * def.meta.height * 0.85;
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
