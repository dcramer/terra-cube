//! Creatures: the registry read from `assets/creatures/*/creature.toml`, spawning one from its
//! `model.glb`, and playing its clips. A creature is spawned as an entity with [`Creature`];
//! its scene and animation player are attached when the file has loaded.

use crate::{
    motion::{Gait, Live, Mover},
    world::Terrain,
};
use bevy::{prelude::*, world_serialization::WorldAssetRoot};
use serde::Deserialize;

pub struct CreaturesPlugin;

impl Plugin for CreaturesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_registry).add_systems(Update, (attach_scenes, bind_players, legs, drive, wander).chain());
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
    pub height: f32,
    pub length: f32,
    pub triangles: usize,
    pub joints: usize,
    #[serde(default)]
    pub clips: std::collections::HashMap<String, ClipMeta>,
    /// The body as it stands, for moving it by its feet (`limbs`): forge's `[walk]`.
    #[serde(default)]
    pub walk: Option<Walk>,
    /// Bones by role, as forge read them off the skeleton: `root`, `leg0`… (`feet`).
    #[serde(default)]
    pub bones: std::collections::HashMap<String, String>,
}

/// Each leg in the walker's order: its hip, knee and end by name, where the hip is, how long
/// its two bones are, how high its end stands (metres, in the model's frame).
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Walk {
    pub legs: Vec<String>,
    pub hips: Vec<[f32; 3]>,
    pub bones: Vec<[f32; 2]>,
    pub ankle: Vec<f32>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct ClipMeta {
    pub looping: bool,
    pub seconds: f32,
    pub source: String,
    /// For a gait: how fast the ground passes under it played as it is, m/s (0: not said).
    #[serde(default)]
    pub speed: f32,
}

pub struct Def {
    pub id: String,
    pub spec: Spec,
    pub meta: Meta,
    pub gltf: Handle<Gltf>,
    pub graph: Option<Handle<AnimationGraph>>,
    /// Clip name, graph node, seconds; idle first.
    pub clips: Vec<(String, AnimationNodeIndex, f32)>,
    /// The blow's clip, and (once a model of this kind has been seen: `legs`) the same clip
    /// as a second node that leaves the legs alone, for a blow struck on the move. `Some(None)`
    /// where the kind has no legs known by name to leave alone.
    blow: Option<Handle<AnimationClip>>,
    over: Option<Option<AnimationNodeIndex>>,
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
    /// Strikes a blow over whatever clip it is playing: its arms and trunk do the blow, its
    /// legs go on as they were (`motion::show` sets it while a blow is struck on the move).
    pub blow: bool,
    /// Going over the ground, standing included: how fast it goes, and the speeds its walk,
    /// run and sprint were made for (m/s). Its clips are then mixed by its speed, in step
    /// with each other, whatever `clip` says (`drive`). Nothing for a clip played alone.
    pub pace: Option<[f32; 4]>,
    /// It is being moved by its feet and no clip (`limbs`): whoever chooses its clip stands
    /// it still for standing, going and lying, and leaves the rest to its legs.
    pub walked: bool,
    /// Where in a stride it is, 0 to 1: every gait in the mix is at the same part of its own.
    phase: f32,
    playing: Option<usize>,
    struck: bool,
}

impl Creature {
    /// Start the current clip again from its first frame.
    pub fn replay(&mut self) {
        self.playing = None;
    }
    pub fn new(def: usize) -> Self {
        Self { def, clip: 0, speed: 1.0, hold: None, blend: 0.2, clip_name: None, blow: false, pace: None, walked: false, phase: 0.0, playing: None, struck: false }
    }
}

/// A creature that is only a picture: its model and its clips, moved by whoever spawned it
/// (the small life of the air and the water, `air`). The simulation has no body for it, and
/// it is not something to be chosen, followed or controlled.
#[derive(Component)]
pub struct Picture;

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

const CLIP_ORDER: [&str; 13] = ["idle", "walk", "run", "sprint", "jump", "fall", "land", "swim", "attack", "hurt", "die", "eat", "lie"];

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
            Ok(spec) => defs.push(Def { meta, gltf: assets.load(format!("creatures/{id}/model.glb")), id, spec, graph: None, clips: Vec::new(), blow: None, over: None }),
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
            def.blow = gltf.named_animations.get("attack").cloned();
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
                    commands.entity(player).try_insert(AnimationGraphHandle(graph.clone()));
                    commands.entity(at).try_insert(Rig(player));
                }
                break;
            }
            let Ok(parent) = parents.get(at) else { break };
            at = parent.parent();
        }
    }
}

/// Once, for each kind with a person's legs: its legs and hips are named to its clips'
/// graph as a group a clip may leave alone, and the blow is added a second time leaving them
/// alone. So a blow can be struck running: until 2026-11-12 the blow landed and nothing was
/// seen of it, since the legs' clip was the only one played.
fn legs(mut registry: ResMut<Registry>, mut graphs: ResMut<Assets<AnimationGraph>>, creatures: Query<(Entity, &Creature), With<Rig>>, children: Query<&Children>, bones: Query<(&Name, &bevy::animation::AnimationTargetId)>) {
    for (entity, creature) in &creatures {
        let def = &mut registry.0[creature.def];
        if def.over.is_some() {
            continue;
        }
        // (Mixamo's names, as Tripo rigs a person: the one skeleton whose bones are known by name.)
        let lower: Vec<_> = children.iter_descendants(entity).filter_map(|e| bones.get(e).ok()).filter(|(n, _)| ["Hips", "UpLeg", "Leg", "Foot", "ToeBase", "Toe_End"].iter().any(|part| n.as_str().ends_with(part))).map(|(_, id)| *id).collect();
        let (Some(clip), Some(mut graph), false) = (def.blow.clone(), def.graph.as_ref().and_then(|g| graphs.get_mut(g)), lower.is_empty()) else {
            // (No bones yet on the frame the model arrives: asked again next frame. None by name at all: never.)
            if children.iter_descendants(entity).any(|e| bones.contains(e)) {
                def.over = Some(None);
            }
            continue;
        };
        for id in lower {
            graph.add_target_to_mask_group(id, 0);
        }
        let root = graph.root;
        def.over = Some(Some(graph.add_clip_with_mask(clip, 1, 1.0, root)));
    }
}

/// Plays each creature's clips. One clip at a time comes in over the one before it; going
/// over the ground, its idle, walk, run and sprint are mixed by how fast it goes, each at the
/// same part of its stride, so it speeds up and slows down through its gaits and is never
/// switched from one to the next. (Until 2026-11-12 it was: a clip chosen by the gait asked
/// for and played faster or slower to fit the speed, so a body getting under way took a
/// walk's steps at a third of their rate, and walk became run in a fifth of a second
/// whatever the legs were doing. The developer: "animations are still not totally natural".)
/// The weights are kept here, not by Bevy's `AnimationTransitions`, which knows only one
/// clip coming in and the ones going out.
fn drive(time: Res<Time>, registry: Res<Registry>, mut creatures: Query<(Entity, &mut Creature, &Rig, Has<crate::player::Controlled>)>, mut players: Query<&mut AnimationPlayer>) {
    let dt = time.delta_secs();
    for (entity, mut creature, rig, controlled) in &mut creatures {
        let Ok(mut player) = players.get_mut(rig.0) else { continue };
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
        let find = |name: &str| clips.iter().position(|c| c.0 == name);
        let long = |k: usize| clips[k].2.max(0.05);
        // What is wanted: each clip's share, how fast it is played, and whether it keeps step.
        let mut wanted: Vec<(usize, f32, f32, bool)> = Vec::new();
        let gaits = creature.pace.filter(|_| creature.hold.is_none()).and_then(|pace| Some((pace, find("idle")?, find("walk")?, find("run")?))).filter(|(pace, ..)| pace[1] > 0.0 && pace[2] > pace[1]);
        if let Some(([speed, walk, run, sprint], idle, walks, runs)) = gaits {
            // Two gaits side by side, each at its own rate: the share of the faster is the
            // one at which their mix covers the ground at the speed it is going, so neither
            // is played fast or slow to fit and the stride is what shortens. (With the share
            // by how far the speed was between the two and the mix then played to fit, a
            // run between a walk and a 5 m/s run was that run at three quarters of its
            // rate: slow motion.) A stride takes as long as the mix says.
            let mut mix = |a: usize, b: usize, from: f32, to: f32| {
                let covers = |part: f32| {
                    let stride = long(a) + (long(b) - long(a)) * part;
                    from * long(a) / stride * (1.0 - part) + to * long(b) / stride * part
                };
                let (mut low, mut high) = (0.0f32, 1.0f32);
                for _ in 0..12 {
                    let part = (low + high) / 2.0;
                    if covers(part) < speed { low = part } else { high = part }
                }
                let part = (low + high) / 2.0;
                let stride = long(a) + (long(b) - long(a)) * part;
                let rate = (speed / covers(part).max(0.01)).clamp(0.8, 1.25);
                wanted.extend([(a, 1.0 - part, long(a) / stride * rate, true), (b, part, long(b) / stride * rate, true)]);
                rate / stride
            };
            let sprints = find("sprint").filter(|_| sprint > run);
            let strides = if speed < walk {
                // Getting under way and coming to a stand: the walk comes in over standing.
                let part = (speed / walk).clamp(0.0, 1.0);
                let rate = 0.75 + 0.25 * part;
                wanted.extend([(idle, 1.0 - part, 1.0, false), (walks, part, rate, true)]);
                rate / long(walks)
            } else if speed < run {
                mix(walks, runs, walk, run)
            } else if let Some(sprints) = sprints.filter(|_| speed < sprint) {
                mix(runs, sprints, run, sprint)
            } else {
                let (last, made_for) = sprints.map_or((runs, run), |s| (s, sprint));
                let rate = (speed / made_for).clamp(0.6, 1.6);
                wanted.push((last, 1.0, rate, true));
                rate / long(last)
            };
            creature.phase = (creature.phase + strides * dt).fract();
            // (Leaving the ground's gaits for a clip of its own, that clip begins.)
            creature.playing = None;
        } else {
            if creature.playing != Some(index) {
                let looping = def.meta.clips.get(&clips[index].0).is_none_or(|c| c.looping) || creature.hold.is_some();
                let began = player.start(node).set_weight(0.0);
                if looping {
                    began.repeat();
                }
                creature.playing = Some(index);
            }
            wanted.push((index, 1.0, if creature.hold.is_some() { 0.0 } else { creature.speed }, false));
        }
        wanted.retain(|w| w.1 > 0.0);
        for &(k, _, _, steps) in &wanted {
            if !player.is_playing_animation(clips[k].1) {
                let began = player.start(clips[k].1).set_weight(0.0).repeat();
                if steps {
                    began.seek_to(creature.phase * long(k));
                }
            }
        }
        // Each clip's share goes toward what is wanted of it, over the creature's blend
        // time; what is wanted no longer goes out and is stopped. (With nothing yet seen of
        // any of them, what is wanted is there at once: nothing to come in over.)
        let over = def.over.flatten();
        let seen: f32 = player.playing_animations().filter(|(n, _)| Some(**n) != over).map(|(_, a)| a.weight()).sum();
        let step = if seen <= 0.0 || creature.hold.is_some() { 1.0 } else { dt / creature.blend.max(0.01) };
        let mut gone = Vec::new();
        for (n, active) in player.playing_animations_mut() {
            if Some(*n) == over {
                continue;
            }
            let want = wanted.iter().find(|w| clips[w.0].1 == *n);
            let (share, to) = (active.weight(), want.map_or(0.0, |w| w.1));
            let share = share + (to - share).clamp(-step, step);
            active.set_weight(share);
            match (want, creature.hold) {
                (Some(_), Some(t)) => {
                    active.set_speed(0.0).seek_to(t);
                }
                (Some(w), None) => {
                    active.set_speed(w.2);
                }
                (None, _) if share <= 0.0 => gone.push(*n),
                _ => {}
            }
        }
        for n in gone {
            player.stop(n);
        }
        // `ANIM_TRACE=ID`: what that creature's clips are doing, a line every half second or
        // so (every frame with `ANIM_ALL=1`): which are wanted, each one's share and speed.
        if std::env::var("ANIM_TRACE").is_ok_and(|id| id == def.id) && (std::env::var_os("ANIM_ALL").is_some() || (time.elapsed_secs() * 2.0).fract() < dt * 2.0) {
            let all: Vec<String> = wanted.iter().map(|w| format!("{}={:.2}x{:.2}", clips[w.0].0, player.animation(clips[w.0].1).map_or(0.0, |a| a.weight()), w.2)).collect();
            println!("anim {} {entity} rig={} controlled={controlled} clip={} pace={:?} {}", def.id, rig.0, clips[index].0, creature.pace.map(|p| p[0]), all.join(" "));
        }
        // A blow over the clip: begun when asked, far heavier than the clip under it (the two
        // are mixed by weight where both move a bone, and it moves all but the legs), let go
        // when no longer asked.
        if let Some(Some(over)) = def.over {
            if creature.blow && !creature.struck {
                player.start(over).set_weight(12.0);
            } else if !creature.blow && creature.struck {
                player.stop(over);
            }
            creature.struck = creature.blow;
        }
    }
}

/// Steers a wandering creature: its body (`motion`) does the moving, at the speed its clip
/// was measured at, so feet that slide show the measurement is wrong.
fn wander(time: Res<Time>, args: Res<crate::Args>, live: Option<ResMut<Live>>, mut movers: Query<(&mut Wander, &Mover)>) {
    // A contact sheet has creatures and no simulation.
    let Some(mut live) = live else { return };
    let dt = time.delta_secs();
    let middle = Vec2::new(args.at.0, args.at.1);
    for (mut w, mover) in &mut movers {
        let gait = if w.run { Gait::Run } else { Gait::Walk };
        let Some(speed) = live.kind(mover).map(|k| k.speed(gait)) else { continue };
        let Some(agent) = live.agent_mut(mover) else { continue };
        let at = agent.body.pos;
        w.until_turn -= dt;
        if w.until_turn <= 0.0 {
            // A cheap deterministic wobble: no randomness needed for a debug walk.
            let phase = time.elapsed_secs() * 0.37 + at.x;
            w.turn = phase.sin() * 0.8;
            w.until_turn = 2.0 + (phase * 1.7).cos().abs() * 2.0;
        }
        // Stay near where creatures are put.
        let out = at.xz() - middle;
        if out.length() > 9.0 {
            let home = (-out.x).atan2(-out.y);
            let diff = (home - w.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            w.turn = diff.clamp(-1.0, 1.0);
        }
        w.heading += w.turn * dt;
        agent.intent.dir = Vec2::new(w.heading.sin(), w.heading.cos());
        agent.intent.speed = speed;
        agent.gait = gait;
    }
}

/// Where the n-th spawned creature stands: a spiral out from the start (`--at`).
pub fn spot(n: usize, terrain: &Terrain, middle: (f32, f32)) -> Transform {
    let angle = n as f32 * 2.4;
    let radius = 1.5 + 1.1 * (n as f32).sqrt() * 1.6;
    let (x, z) = (middle.0 + radius * angle.cos(), middle.1 + radius * angle.sin());
    Transform::from_xyz(x, terrain.ground(x, z), z).with_rotation(Quat::from_rotation_y(0.6))
}

