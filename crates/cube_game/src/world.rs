//! The world's terrain (`cube_sim::terrain`), meshed a chunk at a time, with sun, sky, fog and
//! a sea that runs on to the horizon.

use bevy::{asset::RenderAssetUsages, light::CascadeShadowConfigBuilder, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::terrain::{self, Land, SEA};
use bevy::camera::visibility::VisibilityRange;
use cube_voxel::{
    Block, CELL, CHUNK, World as Voxels,
    mesh::{mesh_chunk, mesh_chunk_far},
};
use std::collections::HashSet;

pub struct WorldPlugin;

/// The voxel world, and the lie of the land it was made from (heights, biomes, trees).
#[derive(Resource)]
pub struct Terrain(pub Voxels, pub Land);

/// The chunks whose cells have changed since they were last meshed.
#[derive(Resource, Default)]
pub struct Dirty(pub HashSet<IVec3>);

/// The two materials every chunk is drawn with.
#[derive(Resource)]
struct Look {
    solid: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
}

impl Terrain {
    /// Digs (with air) or builds a ball in the world, to the bit, and marks what to redraw.
    pub fn ball(&mut self, dirty: &mut Dirty, middle: Vec3, radius: f32, block: Block) {
        self.0.ball(middle, radius, block);
        // Every chunk the ball reaches, and a cell beyond: a neighbour's faces change too.
        let reach = Vec3::splat(radius + CELL);
        let (lo, hi) = (((middle - reach) / CELL).floor().as_ivec3().div_euclid(IVec3::splat(CHUNK)), ((middle + reach) / CELL).floor().as_ivec3().div_euclid(IVec3::splat(CHUNK)));
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    dirty.0.insert(IVec3::new(x, y, z));
                }
            }
        }
    }

    /// The ground's height at a point, metres: the land itself, not a tree over it.
    pub fn ground(&self, x: f32, z: f32) -> f32 {
        self.1.height((x / CELL).floor() as i32, (z / CELL).floor() as i32) as f32 * CELL
    }
}

/// Far things fade into the sky: the island's far shore is hazy, the sea has no edge.
pub fn fog() -> DistanceFog {
    DistanceFog { color: SKY, falloff: FogFalloff::Linear { start: 180.0, end: 700.0 }, ..default() }
}

pub const SKY: Color = Color::srgb(0.53, 0.72, 0.90);

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(SKY)).init_resource::<Dirty>().init_resource::<Drawn>().add_systems(Startup, setup).add_systems(Update, stream);
    }
}

pub fn spawn_lights(commands: &mut Commands, shadows: bool) {
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.85, 1.0), brightness: 480.0, ..default() });
    commands.spawn((
        DirectionalLight { illuminance: 3_800.0, shadow_maps_enabled: shadows, color: Color::srgb(1.0, 0.96, 0.88), ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, -0.9, 0.0)),
        CascadeShadowConfigBuilder { first_cascade_far_bound: 12.0, maximum_distance: 160.0, ..default() }.build(),
    ));
}

fn setup(mut commands: Commands, args: Res<crate::Args>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let (voxels, land) = terrain::generate(args.seed, terrain::RADIUS);
    let material = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, reflectance: 0.2, ..default() });
    // Water is its own mesh and material: it is blended over what is under it.
    let water = materials.add(StandardMaterial { base_color: Color::WHITE, alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.25, reflectance: 0.5, ..default() });
    let mut voxels = voxels;
    for &(x, z, across, build) in &args.dug {
        let ground = land.height((x / CELL).floor() as i32, (z / CELL).floor() as i32) as f32 * CELL;
        // A hole is a bowl with its rim at the ground; a stone sits two thirds out of it.
        let (lift, block) = if build { (across / 6.0, Block::Stone) } else { (across * 0.1, Block::Air) };
        voxels.ball(Vec3::new(x, ground + lift, z), across / 2.0, block);
    }
    // The chunks are meshed as the camera comes near them (`stream`), not here.
    commands.insert_resource(Look { solid: material, water });
    // The sea beyond the world's edge: four sheets round it at the water's level, out past the fog.
    let (edge, far, level) = (land.span as f32 * CELL, 1500.0, SEA as f32 * CELL - 0.01);
    let [r, g, b] = cube_voxel::Block::Water.colors().0;
    let sea = materials.add(StandardMaterial { base_color: Color::linear_rgb(r, g, b), perceptual_roughness: 0.25, reflectance: 0.5, ..default() });
    for (x, z, w, d) in [(0.0, -(edge + far) / 2.0, far, (far - edge) / 2.0), (0.0, (edge + far) / 2.0, far, (far - edge) / 2.0), (-(edge + far) / 2.0, 0.0, (far - edge) / 2.0, edge), ((edge + far) / 2.0, 0.0, (far - edge) / 2.0, edge)] {
        commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(w * 2.0, d * 2.0))), MeshMaterial3d(sea.clone()), Transform::from_xyz(x, level, z)));
    }
    commands.insert_resource(Terrain(voxels, land));
    spawn_lights(&mut commands, true);
}

/// The levels of detail a chunk is drawn at: how many cells a side are taken as one, and the
/// metres from the camera between which that level shows. Each fades into the next across
/// eight metres, so nothing pops. Up close every cell is drawn, with ground cover and the
/// fine shape of broken cells; past 40 m the cells are half a metre, a quarter of the faces;
/// past 104 m a metre, a sixteenth.
pub const LEVELS: [(i32, f32, f32); 3] = [(1, 0.0, 40.0), (2, 40.0, 104.0), (4, 104.0, f32::MAX)];
const FADE: f32 = 8.0;

/// How far outside a level's range a chunk's mesh for it is made (so it is there before it is
/// wanted) and, at twice this, thrown away.
const KEEP: f32 = 12.0;

/// The chunk meshes there are: for each chunk and level, what was spawned for it (nothing, for
/// a chunk with no faces at that level). And how many are still wanted and not yet made.
#[derive(Resource, Default)]
pub struct Drawn {
    made: std::collections::HashMap<(IVec3, usize), Vec<Entity>>,
    pub waiting: usize,
}

fn middle_of(chunk: IVec3) -> Vec3 {
    ((chunk * CHUNK).as_vec3() + CHUNK as f32 / 2.0) * CELL
}

/// Meshes one chunk at one level and spawns what it draws: its solid cells and its water.
fn draw_level(commands: &mut Commands, meshes: &mut Assets<Mesh>, look: &Look, voxels: &Voxels, chunk: IVec3, level: usize) -> Vec<Entity> {
    // Every level of a chunk is drawn from the chunk's middle, so the camera's distance to it
    // is the same whichever level is asked: the fade between two levels needs that.
    let middle = middle_of(chunk);
    let (step, from, to) = LEVELS[level];
    let range = VisibilityRange { start_margin: (from - FADE).max(0.0)..from, end_margin: if to == f32::MAX { 1.0e9..1.1e9 } else { to - FADE..to }, use_aabb: false };
    let mut spawned = Vec::new();
    for (liquid, material) in [(false, &look.solid), (true, &look.water)] {
        let mut data = if step == 1 { mesh_chunk(voxels, chunk, liquid) } else { mesh_chunk_far(voxels, chunk, liquid, step) };
        if data.indices.is_empty() {
            continue;
        }
        data.positions.iter_mut().for_each(|p| *p = (Vec3::from_array(*p) - middle).to_array());
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, data.colors)
            .with_inserted_indices(Indices::U32(data.indices));
        spawned.push(commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone()), Transform::from_translation(middle), range.clone())).id());
    }
    spawned
}

/// Keeps the meshes there are in step with where the camera is: a chunk is meshed at a level
/// when the camera comes within that level's range (nearest first, a few milliseconds' worth a
/// frame), and its mesh is dropped when the camera is well outside it. A chunk that changed
/// loses its meshes and is made again. So what is meshed and held is what can be seen, and
/// the world can be bigger than that.
fn stream(mut commands: Commands, time: Res<Time>, args: Res<crate::Args>, terrain: Option<Res<Terrain>>, look: Option<Res<Look>>, mut meshes: ResMut<Assets<Mesh>>, mut drawn: ResMut<Drawn>, mut dirty: ResMut<Dirty>, cameras: Query<&GlobalTransform, With<Camera3d>>) {
    let (Some(terrain), Some(look), Some(eye)) = (terrain, look, cameras.iter().next().map(|t| t.translation())) else { return };
    for chunk in dirty.0.drain() {
        for level in 0..LEVELS.len() {
            for entity in drawn.made.remove(&(chunk, level)).into_iter().flatten() {
                commands.entity(entity).despawn();
            }
        }
    }
    let mut wanted: Vec<(f32, IVec3, usize)> = Vec::new();
    for &chunk in terrain.0.chunks.keys() {
        let away = eye.distance(middle_of(chunk));
        for (level, &(_, from, to)) in LEVELS.iter().enumerate() {
            let has = drawn.made.contains_key(&(chunk, level));
            if !has && away > from - FADE - KEEP && away < to + KEEP {
                wanted.push((away, chunk, level));
            } else if has && (away < from - FADE - KEEP * 2.0 || away > to + KEEP * 2.0) {
                for entity in drawn.made.remove(&(chunk, level)).into_iter().flatten() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
    wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
    drawn.waiting = wanted.len();
    // A picture waits for the world, so it may take as long as it likes; a window gets a long
    // slice while it first fills in, then a few milliseconds a frame.
    let slice = if args.scene.is_some() || time.elapsed_secs() < 3.0 { 0.05 } else { 0.004 };
    let started = std::time::Instant::now();
    for (_, chunk, level) in wanted {
        let spawned = draw_level(&mut commands, &mut meshes, &look, &terrain.0, chunk, level);
        drawn.made.insert((chunk, level), spawned);
        drawn.waiting -= 1;
        if started.elapsed().as_secs_f32() > slice {
            break;
        }
    }
}
