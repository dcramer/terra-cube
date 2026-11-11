//! The screen the window opens on (`--play`): a cover over the world while what is near the
//! player is made, lifted when it is. Behind it the world is held still (`Loading::up`: the
//! keys do nothing, the simulation's clock is stopped, nothing is heard), so nobody stands
//! in half a world or is bitten before they can see.
//!
//! What it waits for is what `shot` waits for before a picture, kept to what is near: the
//! regions' cells, the chunks' meshes, the regions' trees and rocks, and the models of the
//! player and whatever is awake. The bar is those counts. It cannot hang: twenty seconds
//! with nothing new made and it lifts anyway, saying why.

use crate::{
    audio::{Bank, Mix},
    creatures::Rig,
    flora::Grown,
    motion::Mover,
    player::Controlled,
    world::{Drawn, Regions, Terrain},
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use cube_sim::terrain::REGION;
use cube_voxel::CELL;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Loading { up: true, shown: 1.0, label: "Raising the land", ..default() }).add_systems(Update, watch).add_systems(EguiPrimaryContextPass, cover);
    }
}

/// The world is ready when everything within this of the camera is made, metres.
const NEAR: f32 = 60.0;
/// With nothing new made for this long, seconds, the screen lifts anyway.
const STALLED: f32 = 20.0;
/// While it is up, how long of each frame goes to making regions, and again to meshing
/// chunks, seconds: nearly all of it. Nothing behind the cover needs a smooth frame, and the
/// bar is told from a few frames a second; every frame drawn meanwhile is time not spent
/// making the world.
pub const SLICE: f32 = 0.12;
/// How long the screen takes to fade once it lifts, seconds.
const FADE: f32 = 0.5;

/// Always there, and down unless the plugin put it up: what holds the world still asks it.
#[derive(Resource, Default)]
pub struct Loading {
    /// The world near the player is still being made, and is held still meanwhile.
    pub up: bool,
    /// Keys pressed while it was up, and turned away.
    pub barred: u32,
    /// How much of the cover is left, of 1: it fades when it lifts.
    shown: f32,
    /// How far along, of 1, and what is being made.
    part: f32,
    label: &'static str,
    /// Things made so far, how long since that last grew, and frames in a row all was ready.
    made: usize,
    still: f32,
    ready: u32,
    /// How loud the game was set before it was hushed.
    mix: Option<Mix>,
}

#[allow(clippy::too_many_arguments)]
fn watch(
    time: Res<Time<Real>>,
    mut loading: ResMut<Loading>,
    mut bank: ResMut<Bank>,
    terrain: Option<Res<Terrain>>,
    regions: Res<Regions>,
    drawn: Res<Drawn>,
    grown: Res<Grown>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    players: Query<Has<Rig>, With<Controlled>>,
    movers: Query<Has<Rig>, With<Mover>>,
) {
    if !loading.up {
        // Lifted: the cover fades and the sound comes up with it.
        if loading.shown > 0.0 {
            loading.shown = (loading.shown - time.delta_secs() / FADE).max(0.0);
            if let Some(mix) = loading.mix {
                let up = 1.0 - loading.shown;
                bank.mix = Mix { air: mix.air * up, voices: mix.voices * up, steps: mix.steps * up };
            }
        }
        return;
    }
    if loading.mix.is_none() {
        loading.mix = Some(bank.mix);
    }
    bank.mix = Mix { air: 0.0, voices: 0.0, steps: 0.0 };
    let (Some(terrain), Some(eye)) = (terrain, cameras.iter().next().map(|c| c.translation())) else { return };
    // The regions any of whose ground is within reach, on the island.
    let size = REGION as f32 * CELL;
    let edge = terrain.1.span.div_euclid(REGION);
    let (here, reach) = (((eye.x / size).floor() as i32, (eye.z / size).floor() as i32), (NEAR / size).ceil() as i32);
    let away = |r: &(i32, i32)| Vec2::new((eye.x - (r.0 as f32 + 0.5) * size).abs() - size / 2.0, (eye.z - (r.1 as f32 + 0.5) * size).abs() - size / 2.0).max(Vec2::ZERO).length();
    let near: Vec<(i32, i32)> = (-reach..=reach).flat_map(|z| (-reach..=reach).map(move |x| (here.0 + x, here.1 + z))).filter(|r| r.0 >= -edge && r.0 < edge && r.1 >= -edge && r.1 < edge && away(r) < NEAR).collect();
    let land = (near.iter().filter(|r| regions.is_made(**r)).count(), near.len());
    let ground = drawn.near(&terrain.0, eye, NEAR);
    let woods = (near.iter().filter(|r| regions.is_made(**r) && grown.has(**r)).count(), near.len());
    let models = (movers.iter().filter(|rig| *rig).count(), movers.iter().count());
    // The player is there, and is the one the camera is on.
    let player = players.iter().next().unwrap_or(false);
    let of = |(made, wanted): (usize, usize)| if wanted == 0 { 1.0 } else { made as f32 / wanted as f32 };
    // The ground and the models to be made are only all known once the land is, so each is
    // counted for as much of the land as there is; and the bar never goes back (a region made
    // has its neighbours' edges drawn again).
    let part = of(land) * (0.3 + 0.4 * of(ground) + 0.1 * of(models)) + 0.2 * of(woods);
    loading.part = loading.part.max(part.min(1.0));
    loading.label = if land.0 < land.1 {
        "Raising the land"
    } else if ground.0 < ground.1 {
        "Laying the ground"
    } else if woods.0 < woods.1 {
        "Growing the woods"
    } else {
        "Waking what lives here"
    };
    let all = player && land.0 == land.1 && ground.0 == ground.1 && woods.0 == woods.1 && models.0 == models.1;
    // For a few frames together: a region just made is drawn a frame later.
    loading.ready = if all { loading.ready + 1 } else { 0 };
    let made = land.0 + ground.0 + woods.0 + models.0 + player as usize;
    loading.still = if made == loading.made { loading.still + time.delta_secs() } else { 0.0 };
    loading.made = made;
    let stalled = loading.still > STALLED;
    if stalled {
        warn!("loading: nothing new made for {STALLED:.0} s (land {}/{}, ground {}/{}, woods {}/{}, models {}/{}, player {player}): lifting anyway", land.0, land.1, ground.0, ground.1, woods.0, woods.1, models.0, models.1);
    }
    if loading.ready >= 3 || stalled {
        loading.up = false;
        println!("loading done t={:.1} regions={} meshes={} barred={}{}", time.elapsed_secs(), land.0, ground.0, loading.barred, if stalled { " stalled" } else { "" });
    }
}

/// The cover: dark earth over the whole window, the name, a bar, and what is being made.
fn cover(mut contexts: EguiContexts, loading: Res<Loading>) -> Result {
    if loading.shown <= 0.0 {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let screen = ctx.content_rect();
    let paint = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("loading")));
    let fade = |r: u8, g: u8, b: u8| egui::Color32::from_rgba_unmultiplied(r, g, b, (loading.shown * 255.0) as u8);
    paint.rect_filled(screen, 0.0, fade(17, 15, 12));
    let middle = screen.center();
    paint.text(middle - egui::vec2(0.0, 58.0), egui::Align2::CENTER_CENTER, "terra-cube", egui::FontId::proportional(46.0), fade(226, 210, 172));
    // A plain bar, as a line ruled in ink and filled in.
    let bar = egui::Rect::from_center_size(middle + egui::vec2(0.0, 8.0), egui::vec2((screen.width() * 0.34).clamp(220.0, 520.0), 7.0));
    paint.rect_filled(egui::Rect::from_min_size(bar.min, egui::vec2(bar.width() * loading.part, bar.height())), 0.0, fade(196, 164, 104));
    paint.rect_stroke(bar, 0.0, egui::Stroke::new(1.0, fade(120, 104, 78)), egui::StrokeKind::Outside);
    paint.text(middle + egui::vec2(0.0, 36.0), egui::Align2::CENTER_CENTER, format!("{}…", loading.label), egui::FontId::proportional(16.0), fade(160, 146, 118));
    Ok(())
}
