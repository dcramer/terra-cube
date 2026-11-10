//! The free orbit camera: right-drag orbits, wheel zooms, WASD/QE moves the focus, F focuses
//! the selected creature. While a creature is controlled the camera is `player::follow`'s.

use crate::{creatures::Creature, ui::Selected, world};
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
};

pub struct CameraPlugin;

#[derive(Component)]
pub struct Orbit {
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, orbit);
    }
}

fn setup(mut commands: Commands, args: Res<crate::Args>) {
    commands.spawn((Camera3d::default(), Orbit { focus: Vec3::new(args.at.0, 11.0, args.at.1), yaw: 0.6, pitch: 0.35, distance: 12.0 }, world::fog()));
}

fn orbit(
    mut cameras: Query<(&mut Orbit, &mut Transform)>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    selected: Res<Selected>,
    creatures: Query<&GlobalTransform, With<Creature>>,
    mut egui: bevy_egui::EguiContexts,
    controlled: Query<(), With<crate::player::Controlled>>,
) {
    if !controlled.is_empty() {
        return;
    }
    let over_ui = egui.ctx_mut().map(|c| c.egui_wants_pointer_input() || c.is_pointer_over_egui()).unwrap_or(false);
    for (mut o, mut t) in &mut cameras {
        if buttons.pressed(MouseButton::Right) && !over_ui {
            o.yaw -= motion.delta.x * 0.005;
            o.pitch = (o.pitch + motion.delta.y * 0.005).clamp(-0.2, 1.5);
        }
        if !over_ui {
            o.distance = (o.distance * (1.0 - scroll.delta.y * 0.1)).clamp(1.0, 150.0);
        }
        let rot = Quat::from_euler(EulerRot::YXZ, o.yaw, -o.pitch, 0.0);
        let flat = Quat::from_rotation_y(o.yaw);
        let mut step = Vec3::ZERO;
        for (key, dir) in [(KeyCode::KeyW, Vec3::NEG_Z), (KeyCode::KeyS, Vec3::Z), (KeyCode::KeyA, Vec3::NEG_X), (KeyCode::KeyD, Vec3::X)] {
            if keys.pressed(key) {
                step += flat * dir;
            }
        }
        if keys.pressed(KeyCode::KeyE) {
            step += Vec3::Y;
        }
        if keys.pressed(KeyCode::KeyQ) {
            step -= Vec3::Y;
        }
        let d = o.distance;
        o.focus += step * time.delta_secs() * d.max(4.0);
        if keys.pressed(KeyCode::KeyF)
            && let Some(at) = selected.0.and_then(|e| creatures.get(e).ok())
        {
            o.focus = at.translation() + Vec3::Y * 0.8;
        }
        t.translation = o.focus + rot * Vec3::Z * o.distance;
        t.look_at(o.focus, Vec3::Y);
    }
}
