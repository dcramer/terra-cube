//! Creatures: the registry read from `assets/creatures/*/creature.toml`, spawning one from its
//! `model.glb`, and playing its clips. A creature is spawned as an entity with [`Creature`];
//! its scene and animation player are attached when the file has loaded.

use crate::world::Terrain;
use bevy::{prelude::*, world_serialization::WorldAssetRoot};
use serde::Deserialize;
use std::time::Duration;

pub struct CreaturesPlugin;

impl Plugin for CreaturesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_registry).add_systems(Update, (attach_scenes, bind_players, drive, wander).chain());
    }
}

/// The part of `creature.toml` the game reads. The pipeline's fields are forge's.
#[derive(Deserialize, Clone, Debug)]
pub struct Spec {
    pub name: String,
    /// player, monster or wildlife
    pub kind: String,
    /// The body plan: biped, quadruped…
    pub plan: String,
}

/// The part of `meta.toml`, which forge measures, that the game reads.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Meta {
    /// Metres a second the ground passes under the walk and run clips at speed 1.
    pub walk_speed: f32,
    pub run_speed: f32,
    /// 0 for a creature with no sprint clip.
    #[serde(default)]
    pub sprint_speed: f32,
    pub height: f32,
    pub length: f32,
    pub triangles: usize,
    pub joints: usize,
    #[serde(default)]
    pub clips: std::collections::HashMap<String, ClipMeta>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct ClipMeta {
    pub looping: bool,
    pub seconds: f32,
    pub source: String,
}

pub struct Def {
    pub id: String,
    pub spec: Spec,
    pub meta: Meta,
    pub gltf: Handle<Gltf>,
    pub graph: Option<Handle<AnimationGraph>>,
    /// Clip name, graph node, seconds; idle first.
    pub clips: Vec<(String, AnimationNodeIndex, f32)>,
}

#[derive(Resource, Default)]
pub struct Registry(pub Vec<Def>);

impl Registry {
    pub fn index(&self, id: &str) -> Option<usize> {
        self.0.iter().position(|d| d.id == id)
    }
}

#[derive(Component)]
pub struct Creature {
    pub def: usize,
    pub clip: usize,
    pub speed: f32,
    /// Hold the clip at this time instead of playing it.
    pub hold: Option<f32>,
    /// Seconds to blend into a newly chosen clip.
    pub blend: f32,
    /// Chosen by name before the clips are known; resolved to `clip` once they are.
    pub clip_name: Option<String>,
    playing: Option<usize>,
}

impl Creature {
    /// Start the current clip again from its first frame.
    pub fn replay(&mut self) {
        self.playing = None;
    }
    pub fn new(def: usize) -> Self {
        Self { def, clip: 0, speed: 1.0, hold: None, blend: 0.2, clip_name: None, playing: None }
    }
}

/// The entity under a creature that holds its `AnimationPlayer`.
#[derive(Component)]
pub struct Rig(pub Entity);

#[derive(Component)]
struct SceneAttached;

/// Walks about, turning now and then, at the creature's walk speed.
#[derive(Component, Default)]
pub struct Wander {
    pub heading: f32,
    pub until_turn: f32,
    pub turn: f32,
    pub run: bool,
}

const CLIP_ORDER: [&str; 11] = ["idle", "walk", "run", "sprint", "jump", "fall", "swim", "attack", "hurt", "die", "eat"];

/// Sorts clips into the order they are listed everywhere: the common ones first.
pub fn clip_rank(name: &str) -> (usize, String) {
    (CLIP_ORDER.iter().position(|c| *c == name).unwrap_or(CLIP_ORDER.len()), name.to_owned())
}

fn load_registry(mut commands: Commands, assets: Res<AssetServer>) {
    let mut defs = Vec::new();
    let mut dirs: Vec<_> = std::fs::read_dir("assets/creatures").into_iter().flatten().flatten().map(|e| e.path()).collect();
    dirs.sort();
    for dir in dirs {
        let id = dir.file_name().unwrap().to_string_lossy().into_owned();
        let Ok(text) = std::fs::read_to_string(dir.join("creature.toml")) else { continue };
        if !dir.join("model.glb").exists() {
            continue;
        }
        let meta = std::fs::read_to_string(dir.join("meta.toml")).ok().and_then(|t| toml::from_str::<Meta>(&t).ok()).unwrap_or_default();
        match toml::from_str::<Spec>(&text) {
            Ok(spec) => defs.push(Def { meta, gltf: assets.load(format!("creatures/{id}/model.glb")), id, spec, graph: None, clips: Vec::new() }),
            Err(e) => error!("assets/creatures/{id}/creature.toml: {e}"),
        }
    }
    commands.insert_resource(Registry(defs));
}

pub fn spawn(commands: &mut Commands, registry: &Registry, id: &str, transform: Transform) -> Option<Entity> {
    let def = registry.index(id)?;
    Some(commands.spawn((Creature::new(def), transform, Visibility::default())).id())
}

fn attach_scenes(
    mut commands: Commands,
    mut registry: ResMut<Registry>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    waiting: Query<(Entity, &Creature), Without<SceneAttached>>,
) {
    for (entity, creature) in &waiting {
        let def = &mut registry.0[creature.def];
        if !assets.is_loaded_with_dependencies(&def.gltf) {
            continue;
        }
        let Some(gltf) = gltfs.get(&def.gltf) else { continue };
        if def.graph.is_none() {
            let mut names: Vec<&str> = gltf.named_animations.keys().map(|k| &**k).collect();
            names.sort_by_key(|n| clip_rank(n));
            let (graph, nodes) = AnimationGraph::from_clips(names.iter().map(|n| gltf.named_animations[*n].clone()));
            def.clips = names
                .iter()
                .zip(nodes)
                .map(|(n, node)| (n.to_string(), node, clips.get(&gltf.named_animations[*n]).map_or(1.0, |c| c.duration())))
                .collect();
            def.graph = Some(graphs.add(graph));
        }
        let Some(scene) = gltf.default_scene.clone().or_else(|| gltf.scenes.first().cloned()) else { continue };
        commands.entity(entity).insert((WorldAssetRoot(scene), SceneAttached));
    }
}

fn bind_players(
    mut commands: Commands,
    registry: Res<Registry>,
    players: Query<Entity, Added<AnimationPlayer>>,
    parents: Query<&ChildOf>,
    creatures: Query<&Creature>,
) {
    for player in &players {
        let mut at = player;
        loop {
            if let Ok(creature) = creatures.get(at) {
                if let Some(graph) = &registry.0[creature.def].graph {
                    commands.entity(player).insert((AnimationGraphHandle(graph.clone()), AnimationTransitions::new()));
                    commands.entity(at).insert(Rig(player));
                }
                break;
            }
            let Ok(parent) = parents.get(at) else { break };
            at = parent.parent();
        }
    }
}

fn drive(registry: Res<Registry>, mut creatures: Query<(&mut Creature, &Rig)>, mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>) {
    for (mut creature, rig) in &mut creatures {
        let Ok((mut player, mut transitions)) = players.get_mut(rig.0) else { continue };
        let def = &registry.0[creature.def];
        let clips = &def.clips;
        if clips.is_empty() {
            continue;
        }
        if let Some(name) = creature.clip_name.take() {
            creature.clip = clips.iter().position(|c| c.0 == name).unwrap_or(0);
        }
        let index = creature.clip.min(clips.len() - 1);
        let node = clips[index].1;
        if creature.playing != Some(index) {
            let blend = if creature.playing.is_some() && creature.hold.is_none() { creature.blend } else { 0.0 };
            let active = transitions.play(&mut player, node, Duration::from_secs_f32(blend));
            if def.meta.clips.get(&clips[index].0).is_none_or(|c| c.looping) || creature.hold.is_some() {
                active.repeat();
            }
            creature.playing = Some(index);
        }
        if let Some(active) = player.animation_mut(node) {
            match creature.hold {
                Some(t) => {
                    active.set_speed(0.0).seek_to(t);
                }
                None => {
                    active.set_speed(creature.speed);
                }
            }
        }
    }
}

/// Steers a wandering creature: its body (`motion`) does the moving, at the speed its clip
/// was measured at, so feet that slide show the measurement is wrong.
fn wander(time: Res<Time>, registry: Res<Registry>, mut movers: Query<(&mut Wander, &Creature, &mut crate::motion::Mover)>) {
    let dt = time.delta_secs();
    for (mut w, creature, mut mover) in &mut movers {
        let def = &registry.0[creature.def];
        let at = mover.body.pos;
        w.until_turn -= dt;
        if w.until_turn <= 0.0 {
            // A cheap deterministic wobble: no randomness needed for a debug walk.
            let phase = time.elapsed_secs() * 0.37 + at.x;
            w.turn = phase.sin() * 0.8;
            w.until_turn = 2.0 + (phase * 1.7).cos().abs() * 2.0;
        }
        // Stay near the flat middle of the test world.
        if at.xz().length() > 14.0 {
            let home = (-at.x).atan2(-at.z);
            let diff = (home - w.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            w.turn = diff.clamp(-1.0, 1.0);
        }
        w.heading += w.turn * dt;
        mover.intent.dir = Vec2::new(w.heading.sin(), w.heading.cos());
        mover.intent.speed = crate::motion::speed(def, if w.run { crate::motion::Gait::Run } else { crate::motion::Gait::Walk });
        mover.gait = if w.run { crate::motion::Gait::Run } else { crate::motion::Gait::Walk };
    }
}

/// Where the n-th spawned creature stands: a spiral out from the middle of the flat ground.
pub fn spot(n: usize, terrain: &Terrain) -> Transform {
    let angle = n as f32 * 2.4;
    let radius = 1.5 + 1.1 * (n as f32).sqrt() * 1.6;
    let (x, z) = (radius * angle.cos(), radius * angle.sin());
    Transform::from_xyz(x, terrain.0.ground_height(x, z), z).with_rotation(Quat::from_rotation_y(0.6))
}

