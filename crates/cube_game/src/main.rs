//! The sandbox: a small voxel world to spawn creatures in and watch their clips.
//!
//! `cargo sandbox --help` lists the ways to start it (`HELP` below).
//!
//! Map: `world` (terrain from cube_voxel) · `creatures` (registry, spawning, clips, wander) ·
//! `motion` (bodies that walk, jump and bump, and the clip that fits) · `player` (controlling
//! one with the keys, the follow and first-person cameras) · `ui` (the egui panel, skeleton
//! overlay) · `camera` (free orbit) · `shot` (contact sheets and scene pictures).

mod camera;
mod creatures;
mod motion;
mod player;
mod shot;
mod ui;
mod world;

use bevy::prelude::*;

/// What the command line asked for.
#[derive(Resource, Default, Clone)]
pub struct Args {
    pub spawn: Vec<String>,
    pub shot: Option<String>,
    pub scene: Option<String>,
    pub view: String,
    pub cols: usize,
    pub clips: Vec<String>,
    /// Control the player from the start.
    pub play: bool,
    /// The way a scene's play camera faces, radians: the way the set route runs.
    pub yaw: f32,
    /// The creature to control instead of the player.
    pub play_as: Option<String>,
    /// Seconds into a scene at which its picture is taken.
    pub after: f32,
}

const HELP: &str = "\
cargo sandbox                           the window, with the spawn panel
cargo sandbox --play [--as wolf]        start as the player, or as any creature: WASD runs, Shift
                                        sprints, Ctrl walks, Space jumps, the mouse looks, the
                                        wheel zooms, V is first person, Tab gives the mouse back
cargo sandbox --spawn wolf,player       start with these in the world (`all` for one of each)
cargo sandbox --list                    the creatures that can be spawned
cargo sandbox --shot ID                 no window: shots/ID.png, every clip at six moments
        [--view quarter|side|front] [--cols N] [--clips walk,run]
cargo sandbox --scene FILE --spawn all  no window: a picture of the world with creatures in it
cargo sandbox --scene FILE --play [--as ID] [--after SECONDS] [--view first] [--yaw 2.16]
                                        no window: a set route (walk, run, jump, sprint, jump),
                                        pictured from the camera at SECONDS. It runs toward the
                                        hills; --yaw 2.16 runs it into the pond instead
In the window: Tab controls the picked creature, and lets go of it. Otherwise right-drag
orbits, wheel zooms, WASD and QE move, F looks at the picked creature.
Setup trouble: cargo forge doctor";

fn fail(message: &str) -> ! {
    eprintln!("sandbox: {message}");
    std::process::exit(2);
}

/// Creatures on disk that have a model, as the registry will find them.
fn creature_ids() -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir("assets/creatures")
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("creature.toml").exists() && e.path().join("model.glb").exists())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    ids
}

fn parse_args() -> Args {
    let mut args = Args { view: "quarter".into(), cols: 6, yaw: -2.2, ..default() };
    let mut it = std::env::args().skip(1);
    let list = |v: Option<String>| v.map(|s| s.split(',').map(str::to_owned).collect::<Vec<_>>()).unwrap_or_default();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" | "help" => {
                println!("{HELP}");
                std::process::exit(0);
            }
            "--list" => {
                println!("{}", creature_ids().join(" "));
                std::process::exit(0);
            }
            "--spawn" => args.spawn = list(it.next()),
            "--shot" => args.shot = it.next(),
            "--scene" => args.scene = it.next(),
            "--view" => args.view = it.next().unwrap_or_default(),
            "--cols" => args.cols = it.next().and_then(|v| v.parse().ok()).unwrap_or(6),
            "--clips" => args.clips = list(it.next()),
            "--play" => args.play = true,
            "--yaw" => args.yaw = it.next().and_then(|v| v.parse().ok()).unwrap_or(-2.2),
            "--as" => {
                args.play = true;
                args.play_as = it.next();
            }
            "--after" => args.after = it.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
            other => fail(&format!("unknown argument {other}\n{HELP}")),
        }
    }
    // Names are settled here, so a typo stops before the engine starts instead of in its log.
    let known = creature_ids();
    if known.is_empty() {
        fail("no creatures under assets/creatures; run from the repository root (cargo sandbox does)");
    }
    if args.spawn.iter().any(|s| s == "all") {
        args.spawn = known.clone();
    }
    let me = args.play_as.clone().unwrap_or("player".into());
    if args.play && !args.spawn.contains(&me) {
        args.spawn.insert(0, me);
    }
    if let Some(id) = args.spawn.iter().chain(&args.shot).chain(&args.play_as).find(|id| !known.contains(id)) {
        fail(&format!("no creature `{id}`; there are: {}", known.join(" ")));
    }
    let headless = args.shot.is_some() || args.scene.is_some();
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        fail(if headless { "no display: start `Xvfb :1 -screen 0 1920x1080x24` and set DISPLAY=:1 (docs/development.md)" } else { "no display: the window needs a real screen. For a picture instead: cargo sandbox --scene shots/scene.png --spawn all" });
    }
    args
}

/// Makes the folder that holds `assets/` the working directory, and returns `assets/` in
/// full for Bevy. Started by cargo that is already so; a built program started on its own
/// (a Windows build launched from WSL, a double click) finds it above where it sits.
fn asset_root() -> String {
    let here = std::env::current_dir().unwrap_or_default();
    let exe = std::env::current_exe().unwrap_or_default();
    let found = here.ancestors().chain(exe.ancestors()).find(|dir| dir.join("assets/creatures/style.toml").exists());
    let Some(root) = found else { fail("no assets/ folder here or above the program; run it from the repository") };
    if std::env::set_current_dir(root).is_err() {
        fail(&format!("cannot enter {}", root.display()));
    }
    root.join("assets").to_string_lossy().into_owned()
}

fn main() {
    let assets = asset_root();
    let args = parse_args();
    let scene = args.scene.is_some();
    let shot = args.shot.is_some() || scene;
    let mut app = App::new();
    // A shot draws to an image, with no window: it works on a machine with no screen (winit
    // still wants a display to exist, so a server runs Xvfb; docs/development.md).
    app.add_plugins(DefaultPlugins.set(bevy::render::RenderPlugin { synchronous_pipeline_compilation: shot, ..default() }).set(if shot {
        WindowPlugin { primary_window: None, exit_condition: bevy::window::ExitCondition::DontExit, ..default() }
    } else {
        WindowPlugin { primary_window: Some(Window { title: "terra-cube sandbox".into(), resolution: (1600, 900).into(), ..default() }), ..default() }
    }).set(bevy::asset::AssetPlugin { file_path: assets, ..default() }))
    .insert_resource(args)
    .add_plugins(creatures::CreaturesPlugin);
    if scene {
        app.add_plugins((world::WorldPlugin, motion::MotionPlugin, player::PlayPlugin, shot::ScenePlugin));
    } else if shot {
        app.add_plugins(shot::ShotPlugin);
    } else {
        app.add_plugins((world::WorldPlugin, camera::CameraPlugin, motion::MotionPlugin, player::PlayPlugin, ui::UiPlugin));
    }
    app.run();
}
