//! Contact sheets: `--shot ID` lays the creature out as a grid, a row a clip and a column a
//! moment in it, each held still, and writes one picture of it to `shots/ID.png`. One frame
//! shows every clip, so checking a creature by eye costs one image.
//!
//! `--view quarter|side|front` turns the creatures (the camera is level, so feet read against
//! the slab under each row); `--cols N` moments a clip; `--clips a,b` only those rows.

use crate::{
    Args,
    creatures::{self, Creature, Meta, Registry, Rig},
    world,
};
use bevy::{
    camera::{RenderTarget, ScalingMode},
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, save_to_disk},
    },
};

pub struct ShotPlugin;

impl Plugin for ShotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.62, 0.66, 0.72))).init_resource::<Stage>().add_systems(Startup, setup).add_systems(Update, (layout, capture).chain());
    }
}

#[derive(Resource, Default)]
struct Stage {
    laid_out: bool,
    expected: usize,
    settled: u32,
    asked: bool,
    path: String,
    target: Handle<Image>,
    legend: String,
    taken: usize,
    frames: Vec<image::RgbImage>,
}

fn rows(id: &str, args: &Args) -> Vec<String> {
    let meta: Meta = std::fs::read_to_string(format!("assets/creatures/{id}/meta.toml")).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    let mut names: Vec<String> = meta.clips.keys().cloned().collect();
    names.sort_by_key(|n| creatures::clip_rank(n));
    if !args.clips.is_empty() {
        names.retain(|n| args.clips.contains(n));
    }
    names
}

/// The picture that fits the sheet: square cells, at most about 1000 pixels tall.
fn sheet_size(id: &str, args: &Args) -> (u32, u32) {
    let rows = rows(id, args).len().max(1) as u32;
    let cell = (1000 / rows).min(240);
    (args.cols as u32 * cell, rows * cell)
}

fn setup(mut commands: Commands) {
    // No shadows: on a sheet one row's would fall across the next.
    world::spawn_lights(&mut commands, false);
}

fn layout(
    mut commands: Commands,
    args: Res<Args>,
    registry: Res<Registry>,
    mut stage: ResMut<Stage>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if stage.laid_out {
        return;
    }
    stage.laid_out = true;
    let id = args.shot.clone().unwrap();
    let Some(def) = registry.index(&id).map(|i| &registry.0[i]) else {
        eprintln!("no creature `{id}` with a model.glb under assets/creatures");
        std::process::exit(2);
    };
    let names = rows(&id, &args);
    let (cols, n) = (args.cols, names.len());
    let cell = def.meta.height.max(def.meta.length) * 1.3;
    let turn = match args.view.as_str() {
        "front" => 0.0,
        "side" => std::f32::consts::FRAC_PI_2,
        _ => 0.65,
    };
    let slab = meshes.add(Cuboid::new(cols as f32 * cell, cell * 0.02, cell * 0.7));
    let slab_material = materials.add(StandardMaterial { base_color: Color::srgb(0.25, 0.27, 0.3), perceptual_roughness: 1.0, ..default() });
    let middle = Vec3::new(0.0, -((n - 1) as f32) * cell / 2.0 + cell * 0.42, 0.0);
    let (width, height) = sheet_size(&id, &args);
    stage.target = images.add(Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None));
    let camera = commands
        .spawn((
            Camera3d::default(),
            RenderTarget::Image(stage.target.clone().into()),
            Projection::Orthographic(OrthographicProjection { scaling_mode: ScalingMode::AutoMin { min_width: cols as f32 * cell, min_height: n as f32 * cell }, ..OrthographicProjection::default_3d() }),
            Transform::from_translation(middle + Vec3::Z * 50.0).looking_at(middle, Vec3::Y),
        ))
        .id();
    for (row, name) in names.iter().enumerate() {
        let y = -(row as f32) * cell;
        commands.spawn((Mesh3d(slab.clone()), MeshMaterial3d(slab_material.clone()), Transform::from_xyz(0.0, y - cell * 0.01, 0.0)));
        let info = &def.meta.clips[name];
        for col in 0..cols {
            let part = if info.looping { col as f32 / cols as f32 } else { col as f32 / (cols - 1).max(1) as f32 };
            let x = (col as f32 - (cols - 1) as f32 / 2.0) * cell;
            let mut creature = Creature::new(registry.index(&id).unwrap());
            creature.hold = Some(info.seconds * part * 0.999);
            creature.clip_name = Some(name.clone());
            commands.spawn((creature, Transform::from_xyz(x, y, 0.0).with_rotation(Quat::from_rotation_y(turn)), Visibility::default()));
        }
        commands.spawn((
            Text::new(format!("{name} {:.1}s {}", info.seconds, info.source)),
            TextFont { font_size: FontSize::Px(15.0), ..default() },
            TextColor(Color::BLACK),
            Node { position_type: PositionType::Absolute, left: px(6), top: percent(100.0 * row as f32 / n as f32 + 0.5), ..default() },
            UiTargetCamera(camera),
        ));
    }
    stage.expected = n * cols;
    stage.path = if args.view == "quarter" { format!("shots/{id}.png") } else { format!("shots/{id}-{}.png", args.view) };
    stage.legend = format!("rows={} cols={cols} view={}", names.join(","), args.view);
}

fn capture(mut commands: Commands, mut stage: ResMut<Stage>, args: Res<Args>, time: Res<Time>, rigs: Query<&Rig>, creatures: Query<(), With<Creature>>, mut exit: MessageWriter<AppExit>, mut waited: Local<f32>) {
    // Every creature there is going to be has its model: the ones asked for, and any the world came with.
    if !stage.laid_out || rigs.iter().count() < stage.expected.max(creatures.iter().count()) {
        return;
    }
    stage.settled += 1;
    *waited += time.delta_secs();
    if args.strip > 0 {
        // A strip: a picture every `every` seconds; `gather` saves them as one when all are in.
        if stage.settled >= 40 && *waited >= args.after + stage.taken as f32 * args.every && stage.taken < args.strip {
            stage.taken += 1;
            commands.spawn(Screenshot::image(stage.target.clone())).observe(gather);
        }
        if stage.settled > 4000 {
            eprintln!("the strip never finished");
            exit.write(AppExit::error());
        }
        return;
    }
    if stage.settled >= 40 && *waited >= args.after && !stage.asked {
        stage.settled = 40;
        stage.asked = true;
        if let Some(dir) = std::path::Path::new(&stage.path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::remove_file(&stage.path);
        commands.spawn(Screenshot::image(stage.target.clone())).observe(save_to_disk(stage.path.clone()));
    }
    if stage.asked && std::path::Path::new(&stage.path).exists() && stage.settled > 50 {
        println!("shot {} {}", stage.path, stage.legend);
        exit.write(AppExit::Success);
    }
    if stage.asked && stage.settled > 600 {
        eprintln!("the screenshot never arrived");
        exit.write(AppExit::error());
    }
}

/// One picture of a strip arriving. When the last is in they are set side by side and saved.
fn gather(shot: On<bevy::render::view::screenshot::ScreenshotCaptured>, args: Res<Args>, mut stage: ResMut<Stage>, mut exit: MessageWriter<AppExit>) {
    let Ok(picture) = shot.image.clone().try_into_dynamic() else { return };
    stage.frames.push(picture.to_rgb8());
    if stage.frames.len() < args.strip {
        return;
    }
    let (w, h) = stage.frames[0].dimensions();
    let mut strip = image::RgbImage::new(w * args.strip as u32, h);
    for (i, frame) in stage.frames.iter().enumerate() {
        image::imageops::replace(&mut strip, frame, (i as u32 * w) as i64, 0);
    }
    if let Some(dir) = std::path::Path::new(&stage.path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match strip.save(&stage.path) {
        Ok(()) => println!("shot {} {} strip={} every={}s from={}s", stage.path, stage.legend, args.strip, args.every, args.after),
        Err(e) => eprintln!("saving {}: {e}", stage.path),
    }
    exit.write(AppExit::Success);
}

/// `--scene FILE`: the test world with the `--spawn` creatures (`all` for one of each) standing
/// in it, every other one walking, drawn offscreen to one picture. How an agent sees the world.
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stage>().add_systems(Update, (scene, capture).chain());
    }
}

fn scene(mut commands: Commands, args: Res<Args>, registry: Res<Registry>, terrain: Option<Res<world::Terrain>>, mut stage: ResMut<Stage>, mut images: ResMut<Assets<Image>>) {
    let Some(terrain) = terrain else { return };
    if stage.laid_out {
        return;
    }
    stage.laid_out = true;
    let ids = &args.spawn;
    for (n, id) in ids.iter().enumerate() {
        let Some(def) = registry.index(id) else {
            eprintln!("no creature `{id}`");
            continue;
        };
        let mut creature = Creature::new(def);
        if n % 2 == 1 && !args.play {
            creature.clip_name = Some("walk".into());
        }
        commands.spawn((creature, creatures::spot(n, &terrain, args.at), Visibility::default()));
        stage.expected += 1;
    }
    let (w, h) = if args.strip > 0 { (1920 / args.strip.clamp(4, 12) as u32, 300) } else { (1600, 900) };
    stage.target = images.add(Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None));
    let focus = Vec3::new(args.at.0, terrain.ground(args.at.0, args.at.1) + 0.8, args.at.1);
    // Without `--play` the camera stands off the start: `--dist` metres away, `--pitch` up,
    // round to `--yaw`. `--view high` is the lie of the land from the air.
    let high = args.view == "high";
    let dist = args.dist.unwrap_or(if high { 150.0 } else { 8.7 * (1.0 + ids.len() as f32 * 0.07) });
    let pitch = args.pitch.unwrap_or(if high { 0.6 } else { 0.3 });
    let yaw = if args.play || args.yaw_given { args.yaw } else { 0.57 };
    let from = focus + Quat::from_euler(EulerRot::YXZ, yaw, -pitch, 0.0) * Vec3::Z * dist;
    commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(stage.target.clone().into()),
        // Only `--play` uses the orbit: behind the player, facing the way the route runs.
        crate::camera::Orbit { focus, yaw: args.yaw, pitch: 0.28, distance: 5.5 },
        Transform::from_translation(from).looking_at(focus, Vec3::Y),
        world::fog(),
    ));
    stage.path = args.scene.clone().unwrap();
    stage.legend = format!("creatures={}", ids.join(","));
}
