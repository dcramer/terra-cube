//! The sandbox: a small voxel world to spawn creatures in and watch their clips.
//!
//!   cargo sandbox                     interactive, with the spawn panel
//!   cargo sandbox --spawn wolf,player start with these spawned
//!   cargo sandbox --shot wolf         write shots/wolf.png, a contact sheet of every clip, and exit
//!
//! Map: `world` (terrain from cube_voxel) · `creatures` (registry, spawning, clips, wander) ·
//! `ui` (the egui panel, skeleton overlay) · `camera` (orbit) · `shot` (contact sheets).

mod camera;
mod creatures;
mod shot;
mod ui;
mod world;

use bevy::prelude::*;

/// What the command line asked for.
#[derive(Resource, Default, Clone)]
pub struct Args {
    pub spawn: Vec<String>,
    pub shot: Option<String>,
    pub view: String,
    pub cols: usize,
    pub clips: Vec<String>,
}

fn parse_args() -> Args {
    let mut args = Args { view: "quarter".into(), cols: 6, ..default() };
    let mut it = std::env::args().skip(1);
    let list = |v: Option<String>| v.map(|s| s.split(',').map(str::to_owned).collect::<Vec<_>>()).unwrap_or_default();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--spawn" => args.spawn = list(it.next()),
            "--shot" => args.shot = it.next(),
            "--view" => args.view = it.next().unwrap_or_default(),
            "--cols" => args.cols = it.next().and_then(|v| v.parse().ok()).unwrap_or(6),
            "--clips" => args.clips = list(it.next()),
            other => {
                eprintln!("unknown argument {other}; see crates/cube_game/src/main.rs");
                std::process::exit(2);
            }
        }
    }
    args
}

fn main() {
    let args = parse_args();
    let shot = args.shot.is_some();
    let mut app = App::new();
    // A shot draws to an image, with no window: it works on a machine with no screen (winit
    // still wants a display to exist, so a server runs Xvfb; docs/development.md).
    app.add_plugins(DefaultPlugins.set(bevy::render::RenderPlugin { synchronous_pipeline_compilation: shot, ..default() }).set(if shot {
        WindowPlugin { primary_window: None, exit_condition: bevy::window::ExitCondition::DontExit, ..default() }
    } else {
        WindowPlugin { primary_window: Some(Window { title: "terra-cube sandbox".into(), resolution: (1600, 900).into(), ..default() }), ..default() }
    }))
    .insert_resource(args)
    .add_plugins(creatures::CreaturesPlugin);
    if shot {
        app.add_plugins(shot::ShotPlugin);
    } else {
        app.add_plugins((world::WorldPlugin, camera::CameraPlugin, ui::UiPlugin));
    }
    app.run();
}
