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
    world::spawn_lights(&mut commands);
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

fn capture(mut commands: Commands, mut stage: ResMut<Stage>, rigs: Query<&Rig>, mut exit: MessageWriter<AppExit>) {
    if !stage.laid_out || rigs.iter().count() < stage.expected {
        return;
    }
    stage.settled += 1;
    if stage.settled == 20 && !stage.asked {
        stage.asked = true;
        let _ = std::fs::create_dir_all("shots");
        let _ = std::fs::remove_file(&stage.path);
        commands.spawn(Screenshot::image(stage.target.clone())).observe(save_to_disk(stage.path.clone()));
    }
    if stage.asked && std::path::Path::new(&stage.path).exists() && stage.settled > 30 {
        println!("shot {} {}", stage.path, stage.legend);
        exit.write(AppExit::Success);
    }
    if stage.settled > 600 {
        eprintln!("the screenshot never arrived");
        exit.write(AppExit::error());
    }
}
