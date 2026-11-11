//! What is heard. Every sound is a file in `assets/audio` (CC0; its README says where each is
//! from), found by name: `wolf` is `wolf_1.ogg`, `wolf_2.ogg`, and one of them is picked.
//!
//! Nothing here decides what happens, only what it sounds like. The air of a place is what
//! the ecology says it is (`ecology::ambience`: wind, birdsong, the night's insects, water).
//! A creature is heard when the simulation says it does something: a blow, a wound, a death,
//! and now and then, at the hours it is about, its call. The player is heard walking on what
//! is underfoot. Every sound played is also written to the trace (`sound NAME`), so a
//! scripted run can be checked for what was heard without anyone listening.

use crate::{
    motion::{Live, Mover},
    player::{Controlled, Menus, Torch},
    world::Terrain,
};
use bevy::{audio::{DefaultSpatialScale, SpatialScale, Volume}, prelude::*};
use cube_sim::{
    ecology,
    life::{Act, Temper},
};
use std::collections::HashMap;

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DefaultSpatialScale(SpatialScale::new(1.0 / 5.0)))
            .init_resource::<Bank>()
            .add_systems(Startup, load)
            .add_systems(Update, (ears, air, creatures, footsteps, hands).chain());
    }
}

/// How loud everything is, of 1: `--quiet` makes it 0.
const MASTER: f32 = 0.8;
/// How far off a creature is still heard, metres.
const HEARD: f32 = 45.0;

/// Every sound, by name, and what has been asked to play this frame.
#[derive(Resource, Default)]
pub struct Bank {
    sets: HashMap<String, Vec<Handle<AudioSource>>>,
    turn: u32,
    loud: f32,
    said: bool,
}

impl Bank {
    fn roll(&mut self) -> f32 {
        self.turn ^= self.turn << 13;
        self.turn ^= self.turn >> 17;
        self.turn ^= self.turn << 5;
        (self.turn & 0xFFFF) as f32 / 65535.0
    }

    /// One of a set, at a place in the world (heard from where it is) or in the head (None).
    pub fn play(&mut self, commands: &mut Commands, name: &str, at: Option<Vec3>, volume: f32) {
        let Some(count) = self.sets.get(name).map(Vec::len).filter(|n| *n > 0) else { return };
        let pick = (self.roll() * count as f32) as usize % count;
        let handle = self.sets[name][pick].clone();
        if self.said {
            println!("sound {name}_{} vol={volume:.2}{}", pick + 1, at.map_or(String::new(), |p| format!(" at {:.0},{:.0},{:.0}", p.x, p.y, p.z)));
        }
        // A little off its pitch each time, so the same file is not the same sound.
        let settings = PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume * self.loud)).with_speed(0.93 + self.roll() * 0.14);
        match at {
            Some(at) => commands.spawn((AudioPlayer::new(handle), settings.with_spatial(true), Transform::from_translation(at))),
            None => commands.spawn((AudioPlayer::new(handle), settings)),
        };
    }
}

/// A loop that is always playing, its loudness set each frame.
#[derive(Component)]
struct Loop(&'static str);

fn load(mut commands: Commands, args: Res<crate::Args>, assets: Res<AssetServer>, mut bank: ResMut<Bank>) {
    bank.turn = 0x51ED_270B;
    bank.loud = if args.quiet { 0.0 } else { MASTER };
    bank.said = args.trace || args.scene.is_some() && args.play;
    let dir = std::path::Path::new(&crate::asset_root()).join("audio");
    let mut names: Vec<String> = std::fs::read_dir(&dir).into_iter().flatten().flatten().filter_map(|f| f.file_name().to_str().and_then(|n| n.strip_suffix(".ogg")).map(str::to_owned)).collect();
    names.sort();
    for name in names {
        // `wolf_2` is of the set `wolf`; `amb_wind` is its own.
        let set = name.rsplit_once('_').filter(|(_, n)| n.parse::<u32>().is_ok()).map_or(name.as_str(), |(set, _)| set).to_owned();
        bank.sets.entry(set).or_default().push(assets.load(format!("audio/{name}.ogg")));
    }
    for name in ["amb_wind", "amb_birds", "amb_night", "amb_water", "torch_loop"] {
        if let Some(handle) = bank.sets.get(name).and_then(|s| s.first()) {
            commands.spawn((Loop(name), AudioPlayer::new(handle.clone()), PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0))));
        }
    }
}

/// The camera is the ears.
fn ears(mut commands: Commands, cameras: Query<Entity, (With<Camera3d>, Without<SpatialListener>)>) {
    for camera in &cameras {
        commands.entity(camera).insert(SpatialListener::new(0.2));
    }
}

/// The air of the place the camera is in, as the ecology gives it, eased so nothing cuts in.
fn air(time: Res<Time>, bank: Res<Bank>, live: Option<Res<Live>>, terrain: Option<Res<Terrain>>, cameras: Query<&GlobalTransform, With<Camera3d>>, torches: Query<(), With<Torch>>, mut loops: Query<(&Loop, &mut AudioSink)>, mut now: Local<[f32; 5]>) {
    let (Some(live), Some(terrain), Some(eye)) = (live, terrain, cameras.iter().next().map(|c| c.translation())) else { return };
    let a = ecology::ambience(&terrain.2.place(&terrain.1, eye.x, eye.z), live.0.when);
    let want = [a.wind * 0.35, a.birds * 0.5, a.night * 0.45, a.water * 0.5, if torches.is_empty() { 0.0 } else { 0.35 }];
    let ease = 1.0 - (-1.5 * time.delta_secs()).exp();
    for (i, name) in ["amb_wind", "amb_birds", "amb_night", "amb_water", "torch_loop"].into_iter().enumerate() {
        now[i] += (want[i] - now[i]) * ease;
        for (which, mut sink) in &mut loops {
            if which.0 == name {
                sink.set_volume(Volume::Linear(now[i] * bank.loud));
            }
        }
    }
}

/// What a creature was last heard doing, and when it may next call.
#[derive(Default)]
struct Heard {
    act: Option<Act>,
    calls: f32,
}

/// Creatures near the camera are heard when what they are doing changes, and call now and
/// then at the hours they are about.
fn creatures(mut commands: Commands, time: Res<Time>, mut bank: ResMut<Bank>, live: Option<Res<Live>>, cameras: Query<&GlobalTransform, With<Camera3d>>, movers: Query<&Mover>, mut heard: Local<HashMap<u32, Heard>>) {
    let (Some(live), Some(eye)) = (live, cameras.iter().next().map(|c| c.translation())) else { return };
    let t = time.elapsed_secs();
    for mover in &movers {
        let Some(agent) = live.agent(mover) else { continue };
        let kind = &live.0.kinds[agent.kind];
        let at = agent.body.pos + Vec3::Y * kind.height * 0.6;
        if at.distance(eye) > HEARD {
            heard.remove(&mover.id);
            continue;
        }
        let person = kind.temper == Temper::Person;
        let voice = kind.voice.as_ref().map(|v| v.name().to_owned());
        // A big thing is louder.
        let loud = (0.5 + kind.height * 0.35).min(1.6);
        let first = !heard.contains_key(&mover.id);
        let mine = heard.entry(mover.id).or_default();
        if first {
            mine.calls = t + 6.0 + bank.roll() * 80.0;
        }
        if mine.act != Some(agent.act) {
            match agent.act {
                Act::Attack if person => bank.play(&mut commands, "swing", Some(at), 0.7),
                Act::Attack => {
                    if let Some(voice) = &voice {
                        bank.play(&mut commands, voice, Some(at), loud);
                    }
                }
                Act::Hurt => {
                    bank.play(&mut commands, "hit", Some(at), 0.9);
                    if person {
                        bank.play(&mut commands, "hurt", Some(at), 0.8);
                    }
                }
                Act::Die => bank.play(&mut commands, if person { "hurt" } else if kind.height > 1.0 { "monster_die" } else { voice.as_deref().unwrap_or("hit") }, Some(at), loud),
                _ => {}
            }
            mine.act = Some(agent.act);
        }
        // Its call: seldom, and only at the hours it is about.
        if t >= mine.calls && agent.dead.is_none() {
            mine.calls = t + 35.0 + bank.roll() * 80.0;
            let about = ecology::Niche { name: "", biomes: &[], water: kind.water, cover: kind.cover, hours: kind.hours }.about(live.0.when);
            if let Some(voice) = voice.filter(|_| !person && about > 0.4 && matches!(agent.act, Act::Idle | Act::Walk | Act::Eat)) {
                bank.play(&mut commands, &voice, Some(at), loud * 0.7);
            }
        }
    }
}

/// The player's feet: a step for each stride on the ground, by what is underfoot; a splash
/// going into water and a stroke for each while swimming.
fn footsteps(mut commands: Commands, mut bank: ResMut<Bank>, live: Option<Res<Live>>, terrain: Option<Res<Terrain>>, controlled: Query<&Mover, With<Controlled>>, mut gone: Local<(f32, Vec3, bool)>) {
    let (Some(live), Some(terrain), Ok(mover)) = (live, terrain, controlled.single()) else { return };
    let Some(agent) = live.agent(mover) else { return };
    let (at, body) = (agent.body.pos, &agent.body);
    let moved = Vec2::new(at.x - gone.1.x, at.z - gone.1.z).length();
    gone.1 = at;
    if moved > 2.0 || agent.dead.is_some() {
        return;
    }
    if body.swimming != gone.2 {
        gone.2 = body.swimming;
        if body.swimming {
            bank.play(&mut commands, "splash", Some(at), 0.8);
        }
    }
    gone.0 += moved;
    let speed = body.ground_speed();
    if body.swimming {
        if gone.0 > 1.6 {
            gone.0 = 0.0;
            bank.play(&mut commands, "swim", Some(at), 0.5);
        }
    } else if body.grounded && gone.0 > (0.75 + speed * 0.14).min(1.6) {
        gone.0 = 0.0;
        let under = terrain.0.get((at / cube_voxel::CELL).floor().as_ivec3() - IVec3::Y);
        use cube_voxel::Block;
        let stone = matches!(under, Block::Stone | Block::Slate | Block::Chalk | Block::Gravel | Block::Broken);
        let wet = terrain.2.place(&terrain.1, at.x, at.z).depth > 0.0;
        bank.play(&mut commands, if wet { "splash" } else if stone { "step_stone" } else { "step_grass" }, Some(at), if wet { 0.35 } else { 0.35 + speed * 0.05 });
    }
}

/// The hands: a click when another thing is taken in hand, the pack opening and shutting,
/// the torch catching.
fn hands(mut commands: Commands, mut bank: ResMut<Bank>, menus: Res<Menus>, live: Option<Res<Live>>, controlled: Query<&Mover, With<Controlled>>, torches: Query<(), With<Torch>>, mut was: Local<(Option<usize>, bool, bool)>) {
    let held = live.and_then(|l| controlled.single().ok().and_then(|m| l.agent(m).and_then(|a| a.pack.as_ref().map(|p| p.hand))));
    if held != was.0 {
        if was.0.is_some() && held.is_some() {
            bank.play(&mut commands, "ui_select", None, 0.5);
        }
        was.0 = held;
    }
    if menus.open() != was.1 {
        was.1 = menus.open();
        bank.play(&mut commands, if was.1 { "ui_open" } else { "ui_close" }, None, 0.5);
    }
    let lit = !torches.is_empty();
    if lit != was.2 {
        was.2 = lit;
        if lit {
            bank.play(&mut commands, "torch_ignite", None, 0.6);
        }
    }
}
