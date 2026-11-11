//! What the player sees over the world: health, the bar of things to hand, the time, the pack
//! (I), and an admin panel (F1) for whoever is testing. It shows what the simulation holds
//! and asks it for changes; no rule lives here.

use crate::{
    motion::{Live, Mover},
    player::{Controlled, Menus},
    world::Terrain,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use cube_sim::{
    items::{BAR, Item, SLOTS},
    life::Driver,
    sky,
};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Admin>().add_systems(EguiPrimaryContextPass, (hud, pack, admin)).add_systems(Update, (god, asked));
    }
}

/// What the admin panel holds between frames.
#[derive(Resource)]
struct Admin {
    /// Nothing hurts the player.
    god: bool,
    /// How fast time runs, of 1.
    speed: f32,
    /// A slot picked up in the pack window, to be put down on another.
    picked: Option<usize>,
    spawn: usize,
}

impl Default for Admin {
    fn default() -> Self {
        Admin { god: false, speed: 1.0, picked: None, spawn: 0 }
    }
}

fn tint(item: Item) -> egui::Color32 {
    match item {
        Item::Torch => egui::Color32::from_rgb(230, 150, 60),
        Item::Sword => egui::Color32::from_rgb(170, 180, 195),
        Item::Shovel => egui::Color32::from_rgb(150, 110, 70),
        Item::Berries => egui::Color32::from_rgb(170, 40, 70),
        Item::Meat => egui::Color32::from_rgb(200, 110, 100),
        Item::Bow => egui::Color32::from_rgb(120, 150, 90),
        Item::Jerkin => egui::Color32::from_rgb(110, 80, 60),
    }
}

/// One slot, as a button: what is in it, how many, and whether it is the one in hand.
fn slot(ui: &mut egui::Ui, held: Option<(Item, u32)>, key: Option<usize>, lit: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(54.0, 54.0), egui::Sense::click());
    let paint = ui.painter();
    paint.rect_filled(rect, 5.0, egui::Color32::from_black_alpha(150));
    paint.rect_stroke(rect, 5.0, egui::Stroke::new(if lit { 3.0 } else { 1.0 }, if lit { egui::Color32::from_rgb(240, 220, 150) } else { egui::Color32::from_gray(110) }), egui::StrokeKind::Inside);
    if let Some((item, count)) = held {
        paint.rect_filled(rect.shrink(12.0), 4.0, tint(item));
        paint.text(rect.center_bottom() - egui::vec2(0.0, 3.0), egui::Align2::CENTER_BOTTOM, item.name(), egui::FontId::proportional(9.5), egui::Color32::WHITE);
        if count > 1 {
            paint.text(rect.right_top() + egui::vec2(-4.0, 3.0), egui::Align2::RIGHT_TOP, count.to_string(), egui::FontId::proportional(13.0), egui::Color32::WHITE);
        }
    }
    if let Some(key) = key {
        paint.text(rect.left_top() + egui::vec2(4.0, 2.0), egui::Align2::LEFT_TOP, (key + 1).to_string(), egui::FontId::proportional(11.0), egui::Color32::from_gray(190));
    }
    response
}

/// Health, the bar, and the time, always on.
fn hud(mut contexts: EguiContexts, live: Option<Res<Live>>, controlled: Query<&Mover, With<Controlled>>) -> Result {
    let (Some(live), Ok(mover)) = (live, controlled.single()) else { return Ok(()) };
    let Some(agent) = live.agent(mover) else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let full = live.0.kinds[agent.kind].health();
    egui::Area::new("bar".into()).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -14.0]).show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            let label = if agent.dead.is_some() { "You died. Waking at home…".to_owned() } else { format!("{:.0} / {:.0}", agent.health.ceil(), full) };
            ui.add(egui::ProgressBar::new(agent.health / full).desired_width(BAR as f32 * 58.0).fill(egui::Color32::from_rgb(160, 40, 40)).text(label));
            if let Some(pack) = &agent.pack {
                ui.horizontal(|ui| {
                    for i in 0..BAR {
                        slot(ui, pack.slots[i], Some(i), i == pack.hand);
                    }
                });
            }
        });
    });
    // The time: the hour, the day of the season, the moon.
    let when = live.0.when;
    let hour = sky::hour(when);
    let moon = ["new moon", "waxing crescent", "first quarter", "waxing gibbous", "full moon", "waning gibbous", "last quarter", "waning crescent"][((sky::month(when) * 8.0 + 0.5) as usize) % 8];
    egui::Area::new("clock".into()).anchor(egui::Align2::RIGHT_TOP, [-12.0, 10.0]).show(ctx, |ui| {
        egui::Frame::new().fill(egui::Color32::from_black_alpha(120)).corner_radius(5.0).inner_margin(6.0).show(ui, |ui| {
            ui.label(egui::RichText::new(format!("{:02}:{:02}  ·  {:?}, day {}  ·  {moon}", hour as u32, (hour.fract() * 60.0) as u32, sky::season(when), (when.rem_euclid(sky::SEASON)) as u32 + 1)).color(egui::Color32::WHITE));
        });
    });
    Ok(())
}

/// The pack: every slot. Click one, then another, to change them over.
fn pack(mut contexts: EguiContexts, menus: Res<Menus>, mut admin: ResMut<Admin>, mut live: Option<ResMut<Live>>, controlled: Query<&Mover, With<Controlled>>) -> Result {
    if !menus.pack {
        admin.picked = None;
        return Ok(());
    }
    let (Some(live), Ok(mover)) = (live.as_mut(), controlled.single()) else { return Ok(()) };
    let Some(pack) = live.agent_mut(mover).and_then(|a| a.pack.as_mut()) else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    egui::Window::new("Pack").anchor(egui::Align2::CENTER_CENTER, [0.0, -40.0]).collapsible(false).resizable(false).show(ctx, |ui| {
        ui.label("Click a slot, then another, to move what is in it. The top row is the bar.");
        egui::Grid::new("slots").spacing([4.0, 4.0]).show(ui, |ui| {
            for i in 0..SLOTS {
                if slot(ui, pack.slots[i], (i < BAR).then_some(i), admin.picked == Some(i) || (i == pack.hand && admin.picked.is_none())).clicked() {
                    match admin.picked.take() {
                        Some(from) => pack.swap(from, i),
                        None if pack.slots[i].is_some() => admin.picked = Some(i),
                        None => {}
                    }
                }
                if i % BAR == BAR - 1 {
                    ui.end_row();
                }
            }
        });
    });
    Ok(())
}

/// The admin panel: the time and how fast it runs, the player's health, things to be given,
/// creatures to be put down, places to go. For testing by hand what the tools test by number.
#[allow(clippy::too_many_arguments)]
fn admin(
    mut contexts: EguiContexts,
    menus: Res<Menus>,
    mut admin: ResMut<Admin>,
    mut time: ResMut<Time<Virtual>>,
    mut live: Option<ResMut<Live>>,
    terrain: Option<Res<Terrain>>,
    mut bodies: ResMut<crate::motion::ShowBodies>,
    mut bank: ResMut<crate::audio::Bank>,
    controlled: Query<&Mover, With<Controlled>>,
) -> Result {
    if !menus.admin {
        return Ok(());
    }
    let (Some(live), Some(terrain), Ok(mover)) = (live.as_mut(), terrain, controlled.single()) else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    egui::Window::new("Admin (F1)").default_pos([12.0, 60.0]).default_width(300.0).show(ctx, |ui| {
        ui.heading("Time");
        let (mut day, mut hour) = (live.0.when.floor(), sky::hour(live.0.when));
        let was = (day, hour);
        ui.add(egui::Slider::new(&mut hour, 0.0..=23.99).text("hour"));
        ui.add(egui::Slider::new(&mut day, 0.0..=sky::YEAR * 2.0 - 1.0).step_by(1.0).text("day"));
        ui.horizontal(|ui| {
            for (name, to) in [("spring", 3.0), ("summer", 10.0), ("autumn", 17.0), ("winter", 24.0)] {
                if ui.button(name).clicked() {
                    day = to;
                }
            }
        });
        ui.horizontal(|ui| {
            for (name, to) in [("dawn", 6.0), ("noon", 12.0), ("dusk", 18.0), ("midnight", 0.0)] {
                if ui.button(name).clicked() {
                    hour = to;
                }
            }
        });
        if (day, hour) != was {
            live.0.when = day + hour / 24.0;
        }
        if ui.add(egui::Slider::new(&mut admin.speed, 0.0..=60.0).logarithmic(true).text("speed ×")).changed() {
            time.set_relative_speed(admin.speed.max(0.01));
        }
        ui.heading("Sound");
        ui.add(egui::Slider::new(&mut bank.mix.air, 0.0..=1.5).text("air: wind, birdsong, water"));
        ui.add(egui::Slider::new(&mut bank.mix.voices, 0.0..=1.5).text("creatures' voices"));
        ui.add(egui::Slider::new(&mut bank.mix.steps, 0.0..=1.5).text("footfalls"));
        ui.label(format!("{:?} · moon {:.0}% lit · daylight {:.0}%", sky::season(live.0.when), sky::moon_lit(live.0.when) * 100.0, sky::daylight(live.0.when) * 100.0));

        ui.separator();
        ui.heading("Player");
        ui.checkbox(&mut admin.god, "nothing hurts me");
        ui.checkbox(&mut bodies.0, "draw collision bodies");
        let kinds: Vec<String> = live.0.kinds.iter().map(|k| k.id.clone()).collect();
        let Some(agent) = live.agent_mut(mover) else { return };
        let at = agent.body.pos;
        ui.label(format!("at {:.0}, {:.0}, {:.0} · {:?}", at.x, at.y, at.z, terrain.1.biome((at.x / cube_voxel::CELL) as i32, (at.z / cube_voxel::CELL) as i32)));
        ui.horizontal(|ui| {
            if ui.button("heal").clicked() {
                agent.health = 1.0e6;
            }
            if ui.button("hurt 25").clicked() {
                agent.health = (agent.health - 25.0).max(1.0);
            }
            if ui.button("go home").clicked() {
                let home = agent.mind.home;
                agent.body.pos = Vec3::new(home.x, terrain.ground(home.x, home.y) + 0.1, home.y);
            }
        });
        ui.horizontal(|ui| {
            ui.label("give");
            for item in Item::ALL {
                if ui.button(item.name()).clicked()
                    && let Some(pack) = agent.pack.as_mut()
                {
                    pack.add(item, if item.stack() > 1 { 5 } else { 1 });
                }
            }
        });
        // Go to the nearest place of a kind: the ecology knows where they are.
        ui.horizontal(|ui| {
            ui.label("go to");
            for (name, niche) in [("water", &cube_sim::ecology::DRAGONFLY), ("deep wood", &cube_sim::ecology::FALLING_LEAF), ("open meadow", &cube_sim::ecology::BUTTERFLY), ("rock", &cube_sim::ecology::HAWK)] {
                if ui.button(name).clicked() {
                    let found = (1..120).flat_map(|ring| (0..16).map(move |turn| (ring as f32 * 4.0, turn as f32 * std::f32::consts::FRAC_PI_8))).map(|(out, turn)| (at.x + turn.cos() * out, at.z + turn.sin() * out)).find(|(x, z)| {
                        let place = terrain.2.place(&terrain.1, *x, *z);
                        niche.suits(&place) > 0.8 && place.depth == 0.0
                    });
                    if let Some((x, z)) = found {
                        agent.body.pos = Vec3::new(x, terrain.ground(x, z) + 0.1, z);
                    }
                }
            }
        });

        ui.separator();
        ui.heading("Creatures");
        egui::ComboBox::from_label("kind").selected_text(kinds[admin.spawn.min(kinds.len() - 1)].clone()).show_ui(ui, |ui| {
            for (i, id) in kinds.iter().enumerate() {
                ui.selectable_value(&mut admin.spawn, i, id);
            }
        });
        let facing = agent.facing;
        let ahead = at + Vec3::new(facing.sin(), 0.0, facing.cos()) * 6.0;
        if ui.button("put one down six metres ahead").clicked() {
            let pos = Vec3::new(ahead.x, terrain.ground(ahead.x, ahead.z), ahead.z);
            live.0.add(&terrain.0, admin.spawn, pos, facing + std::f32::consts::PI, Driver::Mind);
        }
        let (alive, dead) = (live.0.agents.values().filter(|a| a.dead.is_none()).count(), live.0.agents.values().filter(|a| a.dead.is_some()).count());
        ui.label(format!("{alive} alive, {dead} lying dead"));
    });
    Ok(())
}

/// With `nothing hurts me` ticked, the player is always whole.
fn god(admin: Res<Admin>, mut live: Option<ResMut<Live>>, controlled: Query<&Mover, With<Controlled>>) {
    if !admin.god {
        return;
    }
    if let (Some(live), Ok(mover)) = (live.as_mut(), controlled.single())
        && let Some(agent) = live.agent_mut(mover)
    {
        agent.health = 1.0e6;
        agent.dead = None;
    }
}

/// `--hud pack` or `--hud admin`: a picture is wanted of that window.
fn asked(args: Res<crate::Args>, mut menus: ResMut<Menus>) {
    match args.hud.as_deref() {
        Some("pack") => menus.pack = true,
        Some("admin") => menus.admin = true,
        _ => {}
    }
}
