//! The world's terrain (`cube_sim::terrain`), meshed a chunk at a time, with sun, sky, fog and
//! a sea that runs on to the horizon.

use bevy::{asset::RenderAssetUsages, light::CascadeShadowConfigBuilder, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::terrain::{self, Land, SEA};
use bevy::camera::{Exposure, visibility::VisibilityRange};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::{Atmosphere, AtmosphereEnvironmentMapLight, SunDisk, atmosphere::ScatteringMedium, light_consts::lux};
use bevy::pbr::{AtmosphereSettings, ExtendedMaterial, MaterialExtension, ScreenSpaceAmbientOcclusion};
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};
use bevy::post_process::bloom::Bloom;
use bevy::image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use cube_voxel::{
    Block, CELL, CHUNK, World as Voxels,
    mesh::{mesh_chunk, mesh_chunk_far},
};
use std::collections::HashSet;

pub struct WorldPlugin;

/// The voxel world, and the lie of the land it was made from (heights, biomes, trees).
#[derive(Resource)]
pub struct Terrain(pub Voxels, pub Land);

/// The land's material: Bevy's own, with wind in what grows. The wind is in the two vertex
/// shaders (`assets/shaders/wind.wesl` for the picture, `wind_prepass.wesl` for depth and
/// shadows), which move each corner by how free the mesher said it is.
pub type Windy = ExtendedMaterial<StandardMaterial, Wind>;

#[derive(Asset, AsBindGroup, TypePath, Clone, Default)]
pub struct Wind {}

impl MaterialExtension for Wind {
    fn vertex_shader() -> ShaderRef {
        "shaders/wind.wesl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/wind_prepass.wesl".into()
    }
}

/// How many faces a chunk's mesh has: what a scene's `pace` adds up for the ones in view.
#[derive(Component)]
pub struct Faces(pub u32);

/// The chunks whose cells have changed since they were last meshed.
#[derive(Resource, Default)]
pub struct Dirty(pub HashSet<IVec3>);

/// The two materials every chunk is drawn with.
#[derive(Resource)]
struct Look {
    solid: Handle<Windy>,
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

/// What every camera that looks at the world carries: the sky as real air scattering the
/// sun (so the light's colour, the sky's and the haze's all come from where the sun is), that
/// sky lighting what the sun does not reach, an exposure for full sunlight, a film-like roll
/// off of the brightest light, a glow round it, and haze that warms toward the sun.
pub fn look() -> impl Bundle {
    (
        DistanceFog {
            color: Color::srgba(0.62, 0.74, 0.88, 1.0),
            directional_light_color: Color::srgba(1.0, 0.86, 0.62, 0.6),
            directional_light_exponent: 14.0,
            falloff: FogFalloff::from_visibility_colors(900.0, Color::srgb(0.42, 0.50, 0.58), Color::srgb(0.78, 0.82, 0.86)),
        },
        AtmosphereSettings::default(),
        // The sky fills shadows, but not so far that they stop being shadows.
        AtmosphereEnvironmentMapLight { intensity: 0.9, ..default() },
        Exposure { ev100: 13.4 },
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        // Richer than the tonemapper leaves it, a little warm, with more between light and dark.
        ColorGrading {
            global: ColorGradingGlobal { post_saturation: 1.12, ..default() },
            shadows: ColorGradingSection { contrast: 1.04, ..default() },
            midtones: ColorGradingSection { contrast: 1.04, ..default() },
            highlights: ColorGradingSection::default(),
        },
        // Corners, the feet of trunks and the undersides of crowns are darker: depth that the
        // cubes' own baked shade cannot give between one thing and another.
        Msaa::Off,
        ScreenSpaceAmbientOcclusion::default(),
    )
}

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<Windy>::default()).insert_resource(ClearColor(Color::BLACK)).init_resource::<Dirty>().init_resource::<Drawn>().add_systems(Startup, setup).add_systems(Update, stream);
    }
}

/// The sun of the world, in mid afternoon: low enough that everything throws a shadow longer
/// than itself and a slope facing it is plainly lit. Its light is raw sunlight; the air
/// between (`look`) is what colours it.
fn spawn_sun(commands: &mut Commands, mediums: &mut Assets<ScatteringMedium>) {
    commands.insert_resource(GlobalAmbientLight::NONE);
    commands.spawn(Atmosphere::earth(mediums.add(ScatteringMedium::earth(256, 256))));
    commands.spawn((
        DirectionalLight { illuminance: lux::RAW_SUNLIGHT, shadow_maps_enabled: true, ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, -0.62, 0.0)),
        CascadeShadowConfigBuilder { first_cascade_far_bound: 12.0, maximum_distance: 220.0, ..default() }.build(),
        SunDisk::EARTH,
    ));
}

/// Plain studio light, for contact sheets.
pub fn spawn_lights(commands: &mut Commands, shadows: bool) {
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.85, 1.0), brightness: 480.0, ..default() });
    commands.spawn((
        DirectionalLight { illuminance: 3_800.0, shadow_maps_enabled: shadows, color: Color::srgb(1.0, 0.96, 0.88), ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, -0.9, 0.0)),
        CascadeShadowConfigBuilder { first_cascade_far_bound: 12.0, maximum_distance: 160.0, ..default() }.build(),
    ));
}

fn setup(mut commands: Commands, args: Res<crate::Args>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>, mut mediums: ResMut<Assets<ScatteringMedium>>, mut windy: ResMut<Assets<Windy>>) {
    let (voxels, land) = terrain::generate(args.seed, terrain::RADIUS);
    // The paint: grain for every surface, made in code (`cube_voxel::texture`). Drawn smooth,
    // so it reads as paint and not as pixels; far off the smaller copies take over, so distant
    // ground does not sparkle.
    let atlas = cube_voxel::texture::atlas();
    let side = atlas.size as u32;
    let mut picture = |bytes: Vec<u8>, format: TextureFormat| {
        let mut image = Image::new_uninit(Extent3d { width: side, height: side, depth_or_array_layers: 1 }, TextureDimension::D2, format, RenderAssetUsages::RENDER_WORLD);
        image.texture_descriptor.mip_level_count = atlas.levels as u32;
        image.data = Some(bytes);
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { mag_filter: ImageFilterMode::Linear, min_filter: ImageFilterMode::Linear, mipmap_filter: ImageFilterMode::Linear, lod_max_clamp: (atlas.levels - 1) as f32, anisotropy_clamp: 4, ..default() });
        images.add(image)
    };
    // The paint, and the relief that goes with it: which way each point of a surface faces.
    let (paint, relief) = (picture(atlas.rgba, TextureFormat::Rgba8UnormSrgb), picture(atlas.relief, TextureFormat::Rgba8Unorm));
    let material = windy.add(ExtendedMaterial { base: StandardMaterial { base_color: Color::WHITE, base_color_texture: Some(paint), normal_map_texture: Some(relief), perceptual_roughness: 0.95, reflectance: 0.2, ..default() }, extension: Wind {} });
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
    spawn_sun(&mut commands, &mut mediums);
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
    for liquid in [false, true] {
        let mut data = if step == 1 { mesh_chunk(voxels, chunk, liquid) } else { mesh_chunk_far(voxels, chunk, liquid, step) };
        if data.indices.is_empty() {
            continue;
        }
        data.positions.iter_mut().for_each(|p| *p = (Vec3::from_array(*p) - middle).to_array());
        let faces = Faces(data.indices.len() as u32 / 6);
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, data.colors)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, data.tangents)
            .with_inserted_indices(Indices::U32(data.indices));
        let mut drawn = commands.spawn((Mesh3d(meshes.add(mesh)), Transform::from_translation(middle), range.clone(), faces));
        if liquid {
            drawn.insert(MeshMaterial3d(look.water.clone()));
        } else {
            drawn.insert(MeshMaterial3d(look.solid.clone()));
        }
        spawned.push(drawn.id());
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
