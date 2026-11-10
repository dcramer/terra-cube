//! The sandbox panel: spawn any creature, pick one, play its clips, scrub, send it wandering,
//! and draw skeletons over the models.

use crate::{
    Args,
    camera::Orbit,
    creatures::{self, Creature, Registry, Wander},
    world::Terrain,
};
use bevy::{mesh::skinning::SkinnedMesh, prelude::*};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

pub struct UiPlugin;

/// The creature the panel is controlling.
#[derive(Resource, Default)]
pub struct Selected(pub Option<Entity>);

#[derive(Resource, Default)]
struct Debug {
    skeletons: bool,
    spawned: usize,
}

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .init_resource::<Selected>()
            .init_resource::<Debug>()
            .add_systems(Startup, gizmos_on_top)
            .add_systems(Update, spawn_from_args)
            .add_systems(EguiPrimaryContextPass, panel)
            .add_systems(Update, skeletons);
    }
}

fn gizmos_on_top(mut store: ResMut<GizmoConfigStore>) {
    store.config_mut::<DefaultGizmoConfigGroup>().0.depth_bias = -1.0;
}

/// Once, when the world exists: it is made by a startup system, so not yet at startup.
fn spawn_from_args(mut commands: Commands, args: Res<Args>, registry: Res<Registry>, terrain: Option<Res<Terrain>>, mut debug: ResMut<Debug>, mut selected: ResMut<Selected>, mut done: Local<bool>) {
    let Some(terrain) = terrain.filter(|_| !*done) else { return };
    *done = true;
    for id in &args.spawn {
        match creatures::spawn(&mut commands, &registry, id, creatures::spot(debug.spawned, &terrain)) {
            Some(e) => {
                selected.0 = Some(e);
                debug.spawned += 1;
            }
            None => warn!("no creature `{id}`"),
        }
    }
}

fn panel(
    mut contexts: EguiContexts,
    mut commands: Commands,
    registry: Res<Registry>,
    terrain: Res<Terrain>,
    mut selected: ResMut<Selected>,
    mut debug: ResMut<Debug>,
    mut creatures: Query<(Entity, &mut Creature, Option<&mut Wander>, &Transform)>,
    mut orbit: Query<&mut Orbit>,
    mut show_bodies: ResMut<crate::motion::ShowBodies>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    egui::Window::new("Sandbox").default_pos([8.0, 8.0]).default_width(260.0).show(ctx, |ui| {
        ui.heading("Spawn");
        for kind in ["player", "monster", "wildlife"] {
            let defs: Vec<_> = registry.0.iter().filter(|d| d.spec.kind == kind).collect();
            if defs.is_empty() {
                continue;
            }
            ui.label(egui::RichText::new(kind).strong());
            ui.horizontal_wrapped(|ui| {
                for def in defs {
                    if ui.button(&def.spec.name).on_hover_text(format!("{} · {} tris · {} joints", def.spec.plan, def.meta.triangles, def.meta.joints)).clicked() {
                        selected.0 = creatures::spawn(&mut commands, &registry, &def.id, creatures::spot(debug.spawned, &terrain));
                        debug.spawned += 1;
                    }
                }
            });
        }
        ui.horizontal(|ui| {
            if ui.button("One of each").clicked() {
                for def in &registry.0 {
                    selected.0 = creatures::spawn(&mut commands, &registry, &def.id, creatures::spot(debug.spawned, &terrain));
                    debug.spawned += 1;
                }
            }
            if ui.button("Clear").clicked() {
                for (e, ..) in &creatures {
                    commands.entity(e).despawn();
                }
                debug.spawned = 0;
                selected.0 = None;
            }
        });
        ui.checkbox(&mut debug.skeletons, "Show skeletons");
        ui.checkbox(&mut show_bodies.0, "Show bodies").on_hover_text("The shape each creature is bumped as");
        ui.separator();

        ui.heading("In the world");
        egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
            for (e, c, ..) in &creatures {
                let name = &registry.0[c.def].spec.name;
                if ui.selectable_label(selected.0 == Some(e), format!("{name} #{}", e.index())).clicked() {
                    selected.0 = Some(e);
                }
            }
        });
        ui.separator();

        let Some((entity, mut creature, wander, transform)) = selected.0.and_then(|e| creatures.get_mut(e).ok()) else {
            ui.label("Spawn a creature, or pick one above.");
            return;
        };
        let def = &registry.0[creature.def];
        ui.heading(&def.spec.name);
        ui.label(format!("{} · {:.2} m tall · walk {:.2} m/s · run {:.2} m/s", def.spec.plan, def.meta.height, def.meta.walk_speed, def.meta.run_speed));
        ui.label("Clips");
        ui.horizontal_wrapped(|ui| {
            for (i, (name, _, seconds)) in def.clips.iter().enumerate() {
                if ui.selectable_label(creature.clip == i, name).on_hover_text(format!("{seconds:.2} s")).clicked() {
                    creature.clip = i;
                    creature.replay();
                    if wander.is_some() {
                        commands.entity(entity).remove::<Wander>();
                    }
                }
            }
        });
        ui.add(egui::Slider::new(&mut creature.speed, 0.0..=3.0).text("speed"));
        let seconds = def.clips.get(creature.clip).map_or(1.0, |c| c.2);
        let mut held = creature.hold.is_some();
        if ui.checkbox(&mut held, "Hold a frame").changed() {
            creature.hold = held.then_some(0.0);
            creature.replay();
        }
        if let Some(t) = &mut creature.hold {
            ui.add(egui::Slider::new(t, 0.0..=seconds).text("time"));
        }
        ui.horizontal(|ui| {
            let (mut walking, mut running) = (wander.as_ref().is_some_and(|w| !w.run), wander.as_ref().is_some_and(|w| w.run));
            let was = (walking, running);
            ui.toggle_value(&mut walking, "Wander");
            ui.toggle_value(&mut running, "Run about");
            if (walking, running) != was {
                if walking != was.0 && walking || running != was.1 && running {
                    let heading = transform.rotation.to_euler(EulerRot::YXZ).0;
                    commands.entity(entity).insert(Wander { heading, run: running && running != was.1, ..default() });
                    creature.hold = None;
                } else {
                    commands.entity(entity).remove::<Wander>();
                    creature.clip = 0;
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Look at (F)").clicked() {
                for mut o in &mut orbit {
                    o.focus = transform.translation + Vec3::Y * def.meta.height * 0.6;
                    o.distance = def.meta.height.max(def.meta.length) * 3.0;
                }
            }
            ui.label("Tab: control");
            if ui.button("Remove").clicked() {
                commands.entity(entity).despawn();
                selected.0 = None;
            }
        });
        ui.separator();
        ui.small("Right-drag orbits · wheel zooms · WASD QE moves");
        ui.small("Tab controls the picked creature: WASD, Shift sprints,");
        ui.small("Ctrl walks, Space jumps, V first person");
    });
    Ok(())
}

/// A line from every joint to its parent joint.
fn skeletons(debug: Res<Debug>, skins: Query<&SkinnedMesh>, transforms: Query<&GlobalTransform>, parents: Query<&ChildOf>, mut gizmos: Gizmos) {
    if !debug.skeletons {
        return;
    }
    for skin in &skins {
        for &joint in &skin.joints {
            let (Ok(at), Ok(parent)) = (transforms.get(joint), parents.get(joint)) else { continue };
            if !skin.joints.contains(&parent.parent()) {
                continue;
            }
            if let Ok(from) = transforms.get(parent.parent()) {
                gizmos.line(from.translation(), at.translation(), Color::srgb(1.0, 0.85, 0.1));
                gizmos.sphere(at.translation(), 0.012, Color::srgb(1.0, 0.3, 0.1));
            }
        }
    }
}
