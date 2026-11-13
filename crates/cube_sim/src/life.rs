//! The creatures in the world: what kinds there are, where each one is and what it is doing,
//! and one tick of all of them. The game draws what is here and feeds the player's keys in;
//! a test or `cargo world run` steps the same thing with no window.
//!
//! A tick: minds decide ([`crate::mind`]), bodies are pushed out of each other, each body
//! moves ([`crate::body`]), and what each is doing is named ([`Act`]) for whoever draws it.

use crate::{
    DT,
    body::{self, Body, Intent, Pillar, Tuning},
    mind::{self, Mind},
    rng::Rng,
    terrain::{Biome, Land},
};
use cube_voxel::{CELL, World};
use glam::{Vec2, Vec3};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Gait {
    #[default]
    Walk,
    Run,
    Sprint,
}

/// How a creature takes the others, which is all its mind is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Temper {
    /// The player's kind: nothing to decide.
    #[default]
    Person,
    /// Grazes, and runs from people, hunters and monsters.
    Timid,
    /// Runs timid creatures down.
    Hunter,
    /// Minds its own business until something comes close, then stands it off and hits it.
    Wary,
    /// Goes for people on sight.
    Hostile,
}

/// The `[life]` table of a `creature.toml`: how the creature behaves and where it lives.
#[derive(Deserialize, Clone, Debug)]
pub struct Life {
    pub temper: Temper,
    /// The biome its groups are placed in: meadow, forest, rock, shore or snow.
    #[serde(default)]
    pub habitat: Option<Habitat>,
    /// How many keep together, and how many such groups a world starts with.
    #[serde(default = "one")]
    pub group: usize,
    #[serde(default)]
    pub groups: usize,
    /// How near water it keeps, how much tree cover it wants, and when it is about
    /// (`ecology`): `water = "edge"`, `cover = "deep"`, `hours = "night"`.
    #[serde(default)]
    pub water: crate::ecology::Water,
    #[serde(default)]
    pub cover: crate::ecology::Cover,
    #[serde(default)]
    pub hours: crate::ecology::Hours,
    /// How much it is about in the depth of winter, of 1: `winter = 0.1` for what sleeps it
    /// through.
    #[serde(default = "whole")]
    pub winter: f32,
    /// The sound it makes: the name of a set of files in `assets/audio` (`wolf` for
    /// `wolf_1.ogg`, `wolf_2.ogg`). None, and it is silent.
    #[serde(default)]
    pub voice: Option<Voice>,
    /// What it eats: `plants`, `meat` or `all`. Unsaid, the timid and the wary eat plants and
    /// what hunts or is hostile eats meat.
    #[serde(default)]
    pub diet: Option<Diet>,
    /// Young to a pair in a year; years it lives; and how many of its kind the ground it
    /// ranges over will feed. Unsaid, each follows from its size ([`Kind`]).
    #[serde(default)]
    pub litter: Option<f32>,
    #[serde(default)]
    pub lifespan: Option<f32>,
    #[serde(default)]
    pub capacity: Option<usize>,
    /// How far it ranges from where it lives, metres, where its size does not say: groups of
    /// a kind keep that far apart, so a long range is a kind that stays few.
    #[serde(default)]
    pub range: Option<f32>,
    /// Days between the meals of a meat-eater, where six is not right: a troll does not eat
    /// as often as a wolf.
    #[serde(default)]
    pub meal: Option<f32>,
    /// What it is seen to carry in its hand, by its name in `assets/world/held.toml`: a
    /// goblin's club. Only to be seen: what a blow does is the kind's own.
    #[serde(default)]
    pub holds: Option<String>,
    /// How common it is beside the usual for its `groups`: `share = 0.1` for a hunter, of
    /// which a land carries one to every twenty or fifty of what it hunts.
    #[serde(default = "whole")]
    pub share: f32,
}

/// What a creature eats.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Diet {
    Plants,
    Meat,
    /// Feeds on what grows and hunts besides: it kills less often and does not starve for want of prey.
    All,
    /// Does not eat: a thing of stone, or one that is not alive.
    None,
}

/// A voice's name, short enough to copy about with the rest of a `Life`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Voice([u8; 16]);

impl Voice {
    pub fn name(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("").trim_end_matches('\0')
    }
}

impl<'de> Deserialize<'de> for Voice {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let name = String::deserialize(d)?;
        let mut bytes = [0u8; 16];
        if name.len() > 16 {
            return Err(serde::de::Error::custom("a voice's name is at most 16 letters"));
        }
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Ok(Voice(bytes))
    }
}

fn whole() -> f32 {
    1.0
}

fn one() -> usize {
    1
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(try_from = "String")]
pub struct Habitat(pub Biome);

impl TryFrom<String> for Habitat {
    type Error = String;
    fn try_from(name: String) -> Result<Self, String> {
        Biome::named(&name).map(Habitat).ok_or(format!("no biome `{name}`"))
    }
}

/// A kind of creature, as the simulation knows it: its size, its speeds and its temper.
#[derive(Clone, Debug)]
pub struct Kind {
    pub id: String,
    pub height: f32,
    pub length: f32,
    pub width: f32,
    /// Metres a second: the speeds its clips were made for (`meta.toml`).
    pub walk: f32,
    pub run: f32,
    /// 0 for a creature with no sprint.
    pub sprint: f32,
    pub temper: Temper,
    pub habitat: Biome,
    pub group: usize,
    pub groups: usize,
    pub water: crate::ecology::Water,
    pub cover: crate::ecology::Cover,
    pub hours: crate::ecology::Hours,
    pub voice: Option<Voice>,
    pub winter: f32,
    pub diet: Diet,
    /// Young to a pair in a year.
    pub litter: f32,
    /// Years it lives (a year is `sky::YEAR` days).
    pub lifespan: f32,
    /// How many of its kind the ground within its range will feed, and that range, metres.
    pub capacity: usize,
    pub range: f32,
    /// Days between a meat-eater's meals.
    pub meal: f32,
    /// What it is seen to carry (`held.toml`).
    pub holds: Option<String>,
    pub share: f32,
}

impl Kind {
    /// What it wants of a place and an hour (`ecology`), whatever biome it is in.
    pub fn niche(&self) -> crate::ecology::Niche {
        crate::ecology::Niche { name: "", biomes: &[], water: self.water, cover: self.cover, hours: self.hours, cold: self.winter }
    }

    /// The shape a creature is bumped as, from its measured standing size: its radius, and
    /// for a body longer than it is wide how far its capsule runs fore and aft of its middle.
    pub fn shape(&self) -> (f32, f32) {
        let (length, width) = (self.length, if self.width > 0.0 { self.width } else { self.length });
        if length > width * 1.3 {
            let radius = (width * 0.425).clamp(0.1, 1.2);
            (radius, (length / 2.0 - radius).max(0.0))
        } else {
            (((length + width) / 4.0).clamp(0.1, 1.2), 0.0)
        }
    }

    /// The highest ledge it walks up without jumping, metres: half a metre for anything up to
    /// a person's size, more for what is bigger. A golem twice a person's height does not hop
    /// up a kerb.
    /// What it can take: by its size, a rabbit a blow and a golem a long fight. A person, 100.
    pub fn health(&self) -> f32 {
        if self.temper == Temper::Person { 100.0 } else { 12.0 + 55.0 * self.height.powf(1.4) }
    }

    /// What its blow takes: by its size. A person's is their fists', until they hold a weapon.
    pub fn harm(&self) -> f32 {
        if self.temper == Temper::Person { FISTS } else { 4.0 + 9.0 * self.height }
    }

    pub fn step(&self) -> f32 {
        (self.height * 0.28).clamp(0.55, 1.25)
    }

    /// The speed a gait moves at: the speed its clip was built or measured for, so the clip
    /// plays at its own rate and the feet hold. A faster creature wants a faster clip, made
    /// in forge, not a number changed here.
    pub fn speed(&self, gait: Gait) -> f32 {
        match gait {
            Gait::Walk => self.walk.max(0.1),
            Gait::Sprint if self.sprint > 0.0 => self.sprint,
            _ => self.run.max(0.2),
        }
    }

    /// Every creature under `assets/creatures` that has a model, by id.
    pub fn load(creatures: &Path) -> Vec<Kind> {
        #[derive(Deserialize)]
        struct Spec {
            kind: String,
            life: Option<Life>,
        }
        #[derive(Deserialize)]
        struct Meta {
            height: f32,
            length: f32,
            #[serde(default)]
            width: f32,
            walk_speed: f32,
            run_speed: f32,
            #[serde(default)]
            sprint_speed: f32,
        }
        let mut dirs: Vec<_> = std::fs::read_dir(creatures).into_iter().flatten().flatten().map(|e| e.path()).collect();
        dirs.sort();
        let mut kinds = Vec::new();
        for dir in dirs {
            let read = |file: &str| std::fs::read_to_string(dir.join(file)).ok();
            let (Some(spec), Some(meta)) = (read("creature.toml"), read("meta.toml")) else { continue };
            if !dir.join("model.glb").exists() {
                continue;
            }
            let id = dir.file_name().unwrap().to_string_lossy().into_owned();
            let (spec, meta) = match (toml::from_str::<Spec>(&spec), toml::from_str::<Meta>(&meta)) {
                (Ok(spec), Ok(meta)) => (spec, meta),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("assets/creatures/{id}: {e}");
                    continue;
                }
            };
            // With no [life], a player is a person and anything else just stands its ground.
            let life = spec.life.unwrap_or(Life { temper: if spec.kind == "player" { Temper::Person } else { Temper::Wary }, habitat: None, group: 1, groups: 0, water: Default::default(), cover: Default::default(), hours: Default::default(), winter: 1.0, voice: None, diet: None, litter: None, lifespan: None, capacity: None, range: None, meal: None, holds: None, share: 1.0 });
            let diet = life.diet.unwrap_or(match life.temper {
                Temper::Hunter | Temper::Hostile => Diet::Meat,
                Temper::Person => Diet::All,
                _ => Diet::Plants,
            });
            // How far it ranges from where it lives: a hunter two and a half times as far as
            // what it hunts, as in life, so a land holds few of them.
            let range = life.range.unwrap_or((60.0 + meta.height * 40.0) * if diet == Diet::Meat { 2.5 } else { 1.0 });
            kinds.push(Kind {
                id,
                height: meta.height,
                length: meta.length,
                width: meta.width,
                walk: meta.walk_speed,
                run: meta.run_speed,
                sprint: meta.sprint_speed,
                temper: life.temper,
                habitat: life.habitat.map_or(Biome::Meadow, |h| h.0),
                group: life.group,
                groups: life.groups,
                water: life.water,
                cover: life.cover,
                hours: life.hours,
                voice: life.voice,
                winter: life.winter,
                // What is not said follows from its size, as it does in life: the small breed
                // fast and die young, the great are few, slow to breed and long-lived.
                diet,
                // (A thing that does not eat is not born and does not grow old.)
                litter: life.litter.unwrap_or(if diet == Diet::None { 0.0 } else if life.temper == Temper::Hostile { 0.4 } else if meta.height < 0.4 { 4.0 } else if meta.height < 1.5 { 2.0 } else { 1.0 }),
                lifespan: life.lifespan.unwrap_or(if diet == Diet::None { 10_000.0 } else if life.temper == Temper::Hostile { 25.0 } else if meta.height < 0.4 { 3.0 } else if meta.height < 1.5 { 8.0 } else { 15.0 }),
                // What eats plants: three times a group to every 80 m of the ground it ranges
                // over, which may hold several groups. What hunts keeps a territory to itself:
                // one group, grown to twice its size at most before half must leave.
                capacity: life.capacity.unwrap_or(if diet == Diet::Meat { (life.group * 2).max(4) } else { ((life.group * 3) as f32 * (range / 80.0f32).powi(2)).round().max(4.0) as usize }),
                range,
                meal: life.meal.unwrap_or(6.0),
                holds: life.holds.clone(),
                share: life.share,
            });
        }
        kinds
    }
}

/// Who decides what a creature does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Driver {
    /// Nobody: it stands where it was put, for others to walk round (a sandbox model).
    Stand,
    /// Its `intent` is set from outside each tick: the player's keys, a script.
    Outside,
    /// Its own mind.
    Mind,
}

/// What a creature is doing, as whoever draws it needs to know: the name of a clip.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash, PartialOrd, Ord)]
pub enum Act {
    #[default]
    Idle,
    Walk,
    Run,
    Sprint,
    Jump,
    Fall,
    Land,
    Swim,
    Eat,
    Attack,
    Hurt,
    /// Down, and staying down.
    Die,
    /// Lying up: down on its belly at rest, out of its hours or fed and easy.
    Lie,
}

impl Act {
    pub const ALL: [Act; 13] = [Act::Idle, Act::Walk, Act::Run, Act::Sprint, Act::Jump, Act::Fall, Act::Land, Act::Swim, Act::Eat, Act::Attack, Act::Hurt, Act::Die, Act::Lie];
    /// The clip, and the one to play where a creature has no such clip.
    pub fn clip(self) -> (&'static str, &'static str) {
        match self {
            Act::Idle => ("idle", "idle"),
            Act::Walk => ("walk", "walk"),
            Act::Run => ("run", "walk"),
            Act::Sprint => ("sprint", "run"),
            Act::Jump => ("jump", "run"),
            Act::Fall => ("fall", "run"),
            Act::Land => ("land", "idle"),
            Act::Swim => ("swim", "walk"),
            Act::Eat => ("eat", "idle"),
            Act::Attack => ("attack", "idle"),
            Act::Hurt => ("hurt", "idle"),
            Act::Die => ("die", "hurt"),
            Act::Lie => ("lie", "idle"),
        }
    }
}

/// What a creature has done so far, for judging a run by numbers (`cargo world run`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Log {
    /// Times it went to water and drank.
    pub drank: u32,
    /// Metres over the ground.
    pub moved: f32,
    /// Seconds in each [`Act`].
    pub acts: [f32; 13],
    /// Times it was killed, and what it has killed.
    pub died: u32,
    pub kills: u32,
    /// Seconds spent walking and getting nowhere: caught on something.
    pub stuck: f32,
    /// Seconds spent wanting to go somewhere with every way shut: a ledge, deep water.
    pub boxed: f32,
    /// Ticks ended inside a solid cell, and times it fell out of the world. Both must be 0.
    pub inside: u32,
    pub fell: u32,
    pub struck: u32,
    pub fled: u32,
    /// Chases begun by what hunts, and times it set out for somewhere with a reason (to
    /// water, to fresh grazing, home to lie up, after prey).
    pub hunts: u32,
    pub trips: u32,
    /// Seconds at rest out of its hours: lying up, not standing about.
    pub lain: f32,
}

#[derive(Clone, Debug)]
pub struct Agent {
    pub kind: usize,
    pub body: Body,
    /// Where the body and its facing were a tick ago, to draw between ticks.
    pub before: Vec3,
    pub facing_before: f32,
    /// The way it faces, radians about Y from +Z. It turns to the way it is going, or to
    /// `turn_to` when that is set (first person; a creature facing what it watches).
    pub facing: f32,
    pub turn_to: Option<f32>,
    pub intent: Intent,
    /// Which gait `intent.speed` belongs to.
    pub gait: Gait,
    /// Seconds a jump is still wanted for: asked a moment before landing, it still jumps.
    pub want_jump: f32,
    pub hold_jump: bool,
    pub driver: Driver,
    pub mind: Mind,
    pub act: Act,
    /// Something done on the spot (eating, a blow), and the seconds of it left.
    pub doing: Option<(Act, f32)>,
    /// Metres stepped up or down since whoever draws it last took this: the model eases after.
    pub stepped: f32,
    /// Seconds of flinching left after being hit.
    pub hurt: f32,
    /// What it can take before it dies, of [`Kind::health`]; and once dead, for how long.
    pub health: f32,
    pub dead: Option<f32>,
    /// Seconds until a blow it has begun lands (a driven creature's: a mind times its own).
    pub swing: Option<f32>,
    /// What its blow takes from what it hits: its kind's, or its weapon's.
    pub power: f32,
    /// Whether it has its guard up: a blow from in front then takes a third of what it would.
    pub guard: bool,
    /// Seconds since it was last hurt: left in peace, it mends.
    pub calm: f32,
    /// Whether it is being lived this tick: near enough to whoever is watching, and with
    /// ground made under it. Asleep, it is as it was left.
    pub awake: bool,
    /// When it was born (`Sim::when`, days): its age is the time since.
    pub born: f32,
    /// How long since it fed, in meals: at 1 it must eat, at 4 it starves (`Sim::day`).
    pub hunger: f32,
    /// How long since it drank, in days: past seven tenths it goes to water (`mind`).
    pub thirst: f32,
    /// What it carries, if it is a person (`items`).
    pub pack: Option<crate::items::Pack>,
    landing: f32,
    airborne: f32,
    jumped: bool,
    pub log: Log,
}

/// A blow landed this tick: who, and the push it gives.
pub struct Hit {
    pub on: u32,
    /// Who struck, and how much health the blow takes.
    pub by: u32,
    pub harm: f32,
    pub push: Vec3,
}

/// Seconds something lies where it fell before it is gone; seconds after that until another
/// of its kind comes to live where it did; and seconds a person lies before waking at home.
const LIES: f32 = 25.0;
/// How far a hunter out of sight hunts, in the reaches of its own ground; and how many of
/// the hunted within that make a hunt as likely to succeed as it can be.
const HUNT_REACH: f32 = 2.5;
const PLENTY: f32 = 8.0;
/// How hungry a meat-eater can get before it dies of it: four meals missed (`Sim::day` holds
/// the ones out of sight to the same).
pub const STARVES: f32 = 4.0;
const WAKES: f32 = 5.0;
/// How far from the watcher a creature is still lived, metres.
pub const AWAKE: f32 = 110.0;
/// A blow lands this long into a swing, reaches this far past the body, and to this far
/// either side of straight ahead (radians).
pub const SWING: f32 = 0.35;
const REACH: f32 = 1.5;
const ARC: f32 = 1.0;
/// What a person does with bare hands.
pub const FISTS: f32 = 6.0;

#[derive(Clone)]
pub struct Sim {
    pub kinds: Vec<Kind>,
    /// By id, in order, so a tick visits them the same way every run.
    pub agents: BTreeMap<u32, Agent>,
    pub tick: u64,
    /// Where whoever is watching is. Only what is within [`AWAKE`] of it is lived; with
    /// none, everything is. A big island has thousands of creatures and one person.
    pub focus: Option<Vec3>,
    /// The time: days since the world began, the hour its fraction (`sky`).
    pub when: f32,
    pub rng: Rng,
    next: u32,
    /// The seed of the land its creatures were put on, by which they know its trails
    /// (`terrain::worn`); none, and they know of none.
    pub trails: Option<u32>,
    /// Hunters that have starved in sight of someone, since the start.
    pub starved: u32,
    /// What there is to graze, of 1, on each piece of ground [`PASTURE`] metres across that
    /// anything has grazed: eaten down by what feeds there and grown back by the season
    /// (`day`). Ground nothing has grazed is not here, and is as full as ground gets.
    pub forage: BTreeMap<(i32, i32), f32>,
    /// What has been grazed in sight since the last day, by piece: taken by the tick, each
    /// too little to take from a number near 1 without being lost, so kept apart until
    /// `day` takes it off.
    nibbled: BTreeMap<(i32, i32), f32>,
    /// The cells of grass, flower and fern that grazers in sight have eaten since whoever
    /// keeps the world last took them: the simulation does not change the world, so it says
    /// what should go and the game takes it (`motion::tick`).
    pub eaten: Vec<glam::IVec3>,
    /// What has been cut loose and is falling (`loose`): stepped by whoever keeps the world,
    /// since it changes it.
    pub loose: crate::loose::Loose,
}

/// The size of a piece of grazing, metres; what a grazer a metre tall eats of one in a day
/// (by its height squared: a moose eats forty rabbits' worth); and how lean the grazing is
/// when what feeds on it begins to starve and to rear fewer young.
pub const PASTURE: f32 = 64.0;
const GRAZED: f32 = 0.0013;
const LEAN: f32 = 0.3;

impl Sim {
    /// What there is to graze at a place, of 1.
    pub fn forage_at(&self, at: Vec2) -> f32 {
        let piece = ((at.x / PASTURE).floor() as i32, (at.y / PASTURE).floor() as i32);
        (self.forage.get(&piece).copied().unwrap_or(1.0) - self.nibbled.get(&piece).copied().unwrap_or(0.0)).max(0.0)
    }
}

/// The fastest anything turns, radians a second: a full turn about in a third of a second.
const TURN_RATE: f32 = 10.0;

fn turn_between(from: f32, to: f32) -> f32 {
    (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl Sim {
    pub fn new(kinds: Vec<Kind>, seed: u64) -> Self {
        Self { kinds, agents: BTreeMap::new(), tick: 0, focus: None, when: crate::sky::START, rng: Rng::new(seed), next: 1, trails: None, starved: 0, forage: BTreeMap::new(), nibbled: BTreeMap::new(), eaten: Vec::new(), loose: Default::default() }
    }

    /// A person's state as text, a thing a line: when it is, where they are, their health,
    /// what is in each slot of their pack and which is in hand. What `load` reads.
    pub fn save(&self, id: u32) -> String {
        let Some(a) = self.agents.get(&id) else { return String::new() };
        let mut out = format!("when={}\npos={},{},{}\nfacing={}\nhealth={}\n", self.when, a.body.pos.x, a.body.pos.y, a.body.pos.z, a.facing, a.health);
        if let Some(pack) = &a.pack {
            out += &format!("hand={}\n", pack.hand);
            for (i, slot) in pack.slots.iter().enumerate() {
                if let Some((item, n)) = slot {
                    out += &format!("slot={i},{},{n}\n", item.name());
                }
            }
        }
        out
    }

    /// Puts a person back as `save` left them. False, and nothing changed, if the text is
    /// not a save.
    pub fn load(&mut self, id: u32, text: &str) -> bool {
        let mut pack = crate::items::Pack::default();
        let (mut when, mut pos, mut facing, mut health) = (None, None, None, None);
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { return false };
            let numbers: Vec<f32> = value.split(',').filter_map(|v| v.parse().ok()).collect();
            match key {
                "when" => when = numbers.first().copied(),
                "pos" if numbers.len() == 3 => pos = Some(Vec3::new(numbers[0], numbers[1], numbers[2])),
                "facing" => facing = numbers.first().copied(),
                "health" => health = numbers.first().copied(),
                "hand" => pack.hand = (numbers.first().copied().unwrap_or(0.0) as usize).min(crate::items::BAR - 1),
                "slot" => {
                    let parts: Vec<&str> = value.split(',').collect();
                    let (Some(i), Some(item), Some(n)) = (parts.first().and_then(|v| v.parse::<usize>().ok()), parts.get(1).and_then(|v| crate::items::Item::named(v)), parts.get(2).and_then(|v| v.parse().ok())) else { return false };
                    if i >= crate::items::SLOTS {
                        return false;
                    }
                    pack.slots[i] = Some((item, n));
                }
                _ => return false,
            }
        }
        let (Some(when), Some(pos), Some(facing), Some(health), Some(agent)) = (when, pos, facing, health, self.agents.get_mut(&id)) else { return false };
        self.when = when;
        (agent.body.pos, agent.before, agent.body.vel, agent.facing, agent.health, agent.dead) = (pos, pos, Vec3::ZERO, facing, health, None);
        agent.pack = Some(pack);
        true
    }

    /// A person uses what is in their hand: a blade swings, berries are eaten, a shovel, an
    /// axe or a pick asks for what it cuts to be cut (which is the caller's to do: the world
    /// is not ours here).
    ///
    /// And what a cut took is theirs: [`Sim::took`].
    /// One of what is in hand has been put down in the world.
    pub fn placed(&mut self, id: u32) {
        if let Some(pack) = self.agents.get_mut(&id).and_then(|a| a.pack.as_mut()) {
            pack.take(pack.hand);
        }
    }

    pub fn took(&mut self, id: u32, taken: [u32; 3]) {
        use crate::items::Stuff;
        let Some(pack) = self.agents.get_mut(&id).and_then(|a| a.pack.as_mut()) else { return };
        for (stuff, bits) in [Stuff::Earth, Stuff::Wood, Stuff::Stone].into_iter().zip(taken) {
            // (One carried for each cell's worth cut, and one for the odd half.)
            pack.add(stuff.yields(), (bits + 62) / 125);
        }
    }

    pub fn use_held(&mut self, id: u32) -> crate::items::Used {
        use crate::items::Used;
        let Some(agent) = self.agents.get_mut(&id).filter(|a| a.dead.is_none()) else { return Used::Nothing };
        let held = agent.pack.as_ref().and_then(|p| p.held());
        match held {
            Some(item) if item.cuts().is_some() => {
                // A blow with it, as with anything swung: what it cuts is cut as the blow
                // lands (the caller's to time: `SWING` after this). One blow at a time.
                if agent.swing.is_some() || agent.doing.is_some() {
                    return Used::Nothing;
                }
                let (stuff, across) = item.cuts().unwrap();
                self.swing(id);
                Used::Cut(stuff, across)
            }
            Some(item) if item.places().is_some() => Used::Place(item.places().unwrap()),
            Some(item) if item.heals().is_some() => {
                let full = self.kinds[agent.kind].health();
                if agent.health >= full || agent.doing.is_some() {
                    return Used::Nothing;
                }
                let pack = agent.pack.as_mut().unwrap();
                pack.take(pack.hand);
                agent.health = (agent.health + item.heals().unwrap()).min(full);
                agent.doing = Some((Act::Eat, 1.2));
                Used::Ate
            }
            _ => {
                if agent.swing.is_some() || agent.doing.is_some() {
                    return Used::Nothing;
                }
                self.swing(id);
                Used::Swing
            }
        }
    }

    /// A driven creature begins a blow: it lands a third of a second on.
    pub fn swing(&mut self, id: u32) {
        if let Some(agent) = self.agents.get_mut(&id).filter(|a| a.dead.is_none() && a.swing.is_none()) {
            agent.swing = Some(SWING);
            agent.doing = Some((Act::Attack, 1.0));
        }
    }

    pub fn kind(&self, id: &str) -> Option<usize> {
        self.kinds.iter().position(|k| k.id == id)
    }

    /// Puts a creature in the world with its feet at `pos`.
    pub fn add(&mut self, world: &World, kind: usize, pos: Vec3, facing: f32, driver: Driver) -> u32 {
        let k = self.kinds[kind].clone();
        // Put down grown: somewhere in the prime of its life. (What is born says so after.)
        let born = self.when - self.rng.between(0.15, 0.6) * k.lifespan * crate::sky::YEAR;
        // And as it would be found: fed some while ago, drunk some while ago.
        let (hunger, thirst) = (self.rng.between(0.1, 0.9), self.rng.between(0.0, 0.6));
        let mut body = Body::new(pos, k.shape().0, k.height.max(0.3));
        // Put down astride a step, it stands on the higher part, not in it.
        for _ in 0..20 {
            if !body.blocked(world, body.pos) {
                break;
            }
            body.pos.y += cube_voxel::BIT;
        }
        let pos = body.pos;
        let id = self.next;
        self.next += 1;
        let mind = Mind::new(Vec2::new(pos.x, pos.z), &mut self.rng);
        self.agents.insert(
            id,
            Agent { kind, body, before: pos, facing_before: facing, facing, turn_to: None, intent: Intent::default(), gait: Gait::Walk, want_jump: 0.0, hold_jump: false, driver, mind, act: Act::Idle, doing: None, health: k.health(), dead: None, swing: None, power: k.harm(), guard: false, calm: 0.0, awake: true, born, hunger, thirst, pack: (k.temper == Temper::Person).then(crate::items::Pack::starting), stepped: 0.0, hurt: 0.0, landing: 0.0, airborne: 0.0, jumped: false, log: Log::default() },
        );
        id
    }

    /// Who is lived: what is near the watcher and has ground made under it. One just woken
    /// is stood clear of whatever its place turned out to hold. Every tick begins with it; on
    /// its own it is a world held still (no time passes, nothing moves) that still knows who
    /// is near: what the game does while it is first being made.
    pub fn wake(&mut self, world: &World) {
        let focus = self.focus;
        for agent in self.agents.values_mut() {
            let made = world.chunks.contains_key(&(agent.body.pos / (CELL * cube_voxel::CHUNK as f32)).floor().as_ivec3().with_y(0)) || focus.is_none();
            let awake = made && focus.is_none_or(|f| f.distance(agent.body.pos) < AWAKE);
            if awake && !agent.awake {
                for _ in 0..200 {
                    if !agent.body.blocked(world, agent.body.pos) {
                        break;
                    }
                    agent.body.pos.y += cube_voxel::BIT;
                }
                agent.before = agent.body.pos;
            }
            agent.awake = awake;
        }
    }

    /// One tick of [`DT`] for every creature.
    pub fn step(&mut self, world: &World) {
        self.tick += 1;
        self.when = crate::sky::later(self.when, DT);
        self.wake(world);
        // Minds see the world as it was at the start of the tick.
        let seen: Vec<mind::Seen> = self.agents.iter().filter(|(_, a)| a.dead.is_none() && a.awake).map(|(&id, a)| mind::Seen { id, pos: a.body.pos, vel: a.body.vel, temper: self.kinds[a.kind].temper, kind: a.kind, errand: a.mind.errand(), reach: a.body.half + a.body.long.length() }).collect();
        let mut hits = Vec::new();
        let when = self.when;
        // What is awake grows hungry and thirsty by the second, at the day's rates (`day`).
        for agent in self.agents.values_mut().filter(|a| a.driver == Driver::Mind && a.dead.is_none() && a.awake) {
            let kind = &self.kinds[agent.kind];
            let days = DT / crate::sky::DAY;
            agent.hunger += days * kind.appetite();
            agent.thirst += days * if kind.diet == Diet::None { 0.0 } else { 1.0 };
            // What grazes in sight eats the ground down as it grazes: a day's need in the two
            // hours a day's grazing takes (`day` takes it for the ones out of sight).
            if agent.mind.state == mind::State::Graze && matches!(kind.diet, Diet::Plants | Diet::All) {
                let piece = ((agent.body.pos.x / PASTURE).floor() as i32, (agent.body.pos.z / PASTURE).floor() as i32);
                *self.nibbled.entry(piece).or_insert(0.0) += kind.height * kind.height * GRAZED * 12.0 * days * if kind.diet == Diet::All { 0.3 } else { 1.0 };
                // And what stands at its feet is eaten, to be seen: a tuft, a flower or a fern
                // every second or so, the nearest to its head first. (It grows again when the
                // ground is next made: what is eaten is not kept.)
                if self.rng.unit() < DT / 1.2 {
                    let head = agent.body.pos + Vec3::new(agent.facing.sin(), 0.0, agent.facing.cos()) * kind.length * 0.5;
                    let at = (head / cube_voxel::CELL).floor().as_ivec3();
                    let near = (-1..=1).flat_map(|dz| (-1..=1).flat_map(move |dx| (-1..=1).map(move |dy| glam::IVec3::new(dx, dy, dz)))).map(|d| at + d).filter(|c| world.get(*c).sprig() && !self.eaten.contains(c)).min_by_key(|c| (c.as_vec3() - head / cube_voxel::CELL).length_squared() as i32);
                    self.eaten.extend(near);
                }
            }
            // A hunter that has found nothing for four meals weakens, and then lies down and
            // dies where it is, in sight: what `day` does to the ones nobody is near.
            if kind.diet == Diet::Meat && agent.hunger >= STARVES - 1.0 {
                agent.health -= kind.health() * (agent.hunger - (STARVES - 1.0)).min(1.0) * DT / 30.0;
                agent.calm = 0.0;
                if agent.hunger >= STARVES || agent.health <= 0.0 {
                    (agent.health, agent.dead, agent.swing, agent.intent) = (0.0, Some(0.0), None, Intent::default());
                    agent.log.died += 1;
                    self.starved += 1;
                }
            }
        }
        for (&id, agent) in self.agents.iter_mut().filter(|(_, a)| a.driver == Driver::Mind && a.dead.is_none() && a.awake) {
            let k = &self.kinds[agent.kind];
            agent.mind.about = k.niche().about(when);
            // A beast of the dusk is about a good deal by day as well; a firefly is not.
            if k.hours == crate::ecology::Hours::Dusk {
                agent.mind.about = agent.mind.about.max(0.35);
            }
            hits.extend(mind::think(id, agent, &self.kinds[agent.kind], &seen, world, self.trails, &mut self.rng));
        }
        // A driven creature's blow: it lands part way through the swing on the nearest thing
        // in reach in front of it.
        for (&id, agent) in self.agents.iter_mut().filter(|(_, a)| a.dead.is_none() && a.awake) {
            // What a person's blow takes is what is in their hand.
            if let Some(pack) = &agent.pack {
                agent.power = pack.held().and_then(|i| i.power()).unwrap_or(FISTS);
            }
            let Some(left) = &mut agent.swing else { continue };
            *left -= DT;
            if *left > 0.0 {
                continue;
            }
            agent.swing = None;
            let (me, ahead) = (Vec2::new(agent.body.pos.x, agent.body.pos.z), Vec2::new(agent.facing.sin(), agent.facing.cos()));
            // By what is in hand: a blade's step and sweep, or a bow's long narrow line.
            let (far, arc) = agent.pack.as_ref().and_then(|p| p.held()).map_or((REACH, ARC), |i| i.reach());
            let reach = agent.body.half + agent.body.long.length() + far;
            let struck = seen
                .iter()
                .filter(|s| s.id != id && (s.pos.y - agent.body.pos.y).abs() < 2.5)
                .map(|s| (s, Vec2::new(s.pos.x, s.pos.z) - me))
                .filter(|(s, to)| to.length() - s.reach < reach && (to.length() < 0.3 || to.normalize().dot(ahead) > arc.cos()))
                .min_by(|a, b| a.1.length().total_cmp(&b.1.length()));
            if let Some((s, to)) = struck {
                let away = to.try_normalize().unwrap_or(ahead);
                hits.push(Hit { on: s.id, by: id, harm: agent.power, push: Vec3::new(away.x * 4.0, 2.5, away.y * 4.0) });
            }
        }
        // What has come down (`loose`) strikes whoever was under it: by how fast it came and
        // how big it was, a falling tree a killing blow and a clod a bruise.
        for fallen in std::mem::take(&mut self.loose.landed) {
            if fallen.settled {
                continue;
            }
            let size = (fallen.cells as f32).cbrt() * cube_voxel::CELL;
            for (id, agent) in self.agents.iter().filter(|(_, a)| a.dead.is_none() && a.body.pos.distance(fallen.at) < size * 0.9 + a.body.half + 0.4) {
                let away = (agent.body.pos - fallen.at).with_y(0.0).normalize_or(Vec3::X);
                hits.push(Hit { on: *id, by: 0, harm: (fallen.speed * size * 6.0).min(400.0), push: away * 3.0 + Vec3::Y * 1.5 });
            }
            self.loose.heard.push(fallen);
        }
        for hit in hits {
            let Some(agent) = self.agents.get_mut(&hit.on).filter(|a| a.dead.is_none()) else { continue };
            agent.hurt = 0.5;
            agent.doing = None;
            agent.body.vel += hit.push;
            agent.log.struck += 1;
            agent.calm = 0.0;
            // Guarded, and the blow comes from in front: most of it is turned.
            let from_front = Vec2::new(agent.facing.sin(), agent.facing.cos()).dot(-Vec2::new(hit.push.x, hit.push.z).normalize_or_zero()) > 0.3;
            let turned = agent.guard && from_front;
            // What is worn takes its share too.
            let worn = agent.pack.as_ref().map_or(1.0, |p| p.slots.iter().flatten().map(|s| s.0.armour()).fold(1.0, f32::min));
            agent.health -= hit.harm * worn * if turned { 0.3 } else { 1.0 };
            if turned {
                agent.body.vel -= hit.push * 0.7;
            }
            if agent.health <= 0.0 {
                (agent.health, agent.dead, agent.swing, agent.intent) = (0.0, Some(0.0), None, Intent::default());
                agent.log.died += 1;
                // Whoever killed it and carries a pack takes meat from it, by its size.
                let meat = 1 + (self.kinds[agent.kind].height * 1.5) as u32;
                if let Some(killer) = self.agents.get_mut(&hit.by) {
                    killer.log.kills += 1;
                    // What it killed, it eats: a hunter is fed for days.
                    killer.hunger = 0.0;
                    if let Some(pack) = killer.pack.as_mut() {
                        pack.add(crate::items::Item::Meat, meat);
                    }
                }
            }
        }
        // The dead: a person wakes at home after a little; anything else lies a while, is
        // gone, and in time another of its kind comes to live where it did.
        let mut gone = Vec::new();
        for (&id, agent) in self.agents.iter_mut() {
            let Some(lain) = &mut agent.dead else {
                agent.calm += DT;
                if agent.calm > 8.0 {
                    agent.health = (agent.health + 2.0 * DT).min(self.kinds[agent.kind].health());
                }
                continue;
            };
            *lain += DT;
            agent.intent = Intent::default();
            agent.act = Act::Die;
            if self.kinds[agent.kind].temper == Temper::Person {
                if *lain >= WAKES {
                    let home = agent.mind.home;
                    agent.body.pos = Vec3::new(home.x, world.ground_height(home.x, home.y), home.y);
                    // Clear of whatever has grown or been built where it lives.
                    for _ in 0..200 {
                        if !agent.body.blocked(world, agent.body.pos) {
                            break;
                        }
                        agent.body.pos.y += cube_voxel::BIT;
                    }
                    (agent.body.vel, agent.before, agent.dead, agent.health, agent.hurt) = (Vec3::ZERO, agent.body.pos, None, self.kinds[agent.kind].health(), 0.0);
                }
            } else if *lain >= LIES {
                gone.push(id);
            }
        }
        // What has died is gone. Its kind comes back by being born (`day`), not by being put
        // back: until 2026-11-11 the dead returned where they lived after two and a half
        // minutes, and nothing that happened to a herd made any difference to it.
        for id in gone {
            self.agents.remove(&id);
        }

        // Bodies out of each other: creatures standing about are pillars, movers share the push.
        let along = |a: &Agent, kinds: &[Kind]| Vec2::new(a.facing.sin(), a.facing.cos()) * kinds[a.kind].shape().1;
        let mut pillars = Vec::new();
        for agent in self.agents.values_mut() {
            agent.body.long = along(agent, &self.kinds);
            if agent.driver == Driver::Stand && agent.awake {
                pillars.push(Pillar { pos: agent.body.pos, half: agent.body.half, height: agent.body.height, long: agent.body.long });
            }
        }
        let mut bodies: Vec<Body> = self.agents.values().filter(|a| a.driver != Driver::Stand && a.awake).map(|a| a.body).collect();
        body::separate(world, &mut bodies, &pillars);

        for (agent, pushed) in self.agents.values_mut().filter(|a| a.driver != Driver::Stand && a.awake).zip(bodies) {
            let (was, was_grounded) = (agent.body.pos, agent.body.grounded || agent.body.swimming);
            agent.body = pushed;
            let mut intent = if agent.dead.is_some() { Intent::default() } else { agent.intent };
            intent.jump = agent.want_jump > 0.0 && agent.dead.is_none();
            intent.hold = agent.hold_jump;
            let fall = agent.body.vel.y;
            let tuning = Tuning { step: self.kinds[agent.kind].step(), ..Tuning::default() };
            agent.body.step(world, intent, &tuning);
            agent.want_jump = if agent.body.jumping { 0.0 } else { (agent.want_jump - DT).max(0.0) };
            agent.before = was;
            if was_grounded && !agent.body.grounded && !agent.body.swimming {
                agent.jumped = agent.body.vel.y > 0.5;
            }
            // Down from more than about half a metre: a landing.
            if !was_grounded && agent.body.grounded && fall < -4.5 {
                agent.landing = 0.3;
            }
            // A step up or down moves the body all at once; the model eases after.
            let rise = agent.body.pos.y - was.y;
            if was_grounded && (agent.body.grounded || agent.body.swimming) && rise.abs() > 0.2 {
                agent.stepped += rise;
                agent.before.y = agent.body.pos.y;
            }
            // Off the edge of the world: back to where it lives.
            if agent.body.pos.y < -20.0 {
                let home = agent.mind.home;
                agent.body.pos = Vec3::new(home.x, world.ground_height(home.x, home.y), home.y);
                agent.body.vel = Vec3::ZERO;
                agent.before = agent.body.pos;
                agent.log.fell += 1;
            }

            // Facing follows the way it goes, eased.
            let speed = agent.body.ground_speed();
            agent.facing_before = agent.facing;
            // It turns to where it is steered, not to where it happens to be sliding: facing
            // that followed velocity lagged every change of direction and swung wide. Eased
            // close to, and no faster than a turn rate, so a reversal is a quick turn about
            // and not a flip. Left alone it keeps the way it faced.
            let steered = (intent.dir.length() > 0.1).then(|| intent.dir.x.atan2(intent.dir.y));
            if let Some(want) = agent.turn_to.or(steered) {
                let most = TURN_RATE * DT;
                agent.facing += (turn_between(agent.facing, want) * (1.0 - (-16.0 * DT).exp())).clamp(-most, most);
            }

            agent.airborne = if agent.body.grounded || agent.body.swimming { 0.0 } else { agent.airborne + DT };
            agent.landing = (agent.landing - DT).max(0.0);
            agent.hurt = (agent.hurt - DT).max(0.0);
            if let Some((_, left)) = &mut agent.doing {
                *left -= DT;
                if *left <= 0.0 {
                    agent.doing = None;
                }
            }
            agent.act = if agent.dead.is_some() {
                Act::Die
            } else if agent.hurt > 0.0 {
                Act::Hurt
            } else if agent.body.swimming {
                Act::Swim
            } else if agent.airborne > 0.1 {
                if agent.jumped && agent.body.vel.y > 0.0 { Act::Jump } else { Act::Fall }
            } else if agent.landing > 0.0 && speed < 1.5 {
                Act::Land
            } else if speed < 0.2 {
                agent.doing.map_or(Act::Idle, |d| d.0)
            } else {
                match agent.gait {
                    Gait::Walk => Act::Walk,
                    Gait::Run => Act::Run,
                    Gait::Sprint => Act::Sprint,
                }
            };

            agent.log.moved += Vec2::new(agent.body.pos.x - was.x, agent.body.pos.z - was.z).length();
            agent.log.acts[agent.act as usize] += DT;
            agent.log.inside += agent.body.blocked(world, agent.body.pos) as u32;
        }
    }

    /// Fills a new world with its creatures: each kind's groups, in the biome it lives in.
    /// Anything that might hurt the player starts well away from the start. Returns how many.
    pub fn populate(&mut self, world: &World, land: &Land) -> usize {
        let mut placed = 0;
        self.trails = Some(land.seed);
        // Where each kind's groups make their home is the ecology's to say: the biome it
        // lives in, how near water, under how much cover.
        let ecology = crate::ecology::Ecology::new(land);
        // The rare first: one golem needs its rock before two dozen goats have taken every
        // ledge, since what hunts and what is hunted do not make their homes side by side.
        let mut order: Vec<usize> = (0..self.kinds.len()).collect();
        order.sort_by_key(|&i| (self.kinds[i].groups * self.kinds[i].group, i));
        for kind in order {
            let k = self.kinds[kind].clone();
            // How near the start a group may live: what does no harm, near; what hunts, far.
            // (At 60 m for everything but the timid, a player at the start saw nothing but a
            // rabbit in the grass, and said so.)
            let keep_off = match k.temper {
                Temper::Timid => 12.0,
                Temper::Hunter => 45.0,
                Temper::Hostile => 70.0,
                _ => 22.0,
            };
            // By the island's area: twice as wide is four times the ground.
            let wanted = ((k.groups as f32 * (land.span as f32 * CELL / 96.0).powi(2) * k.share).round() as usize).max((k.groups > 0) as usize);
            let mut groups = 0;
            for _ in 0..80000 {
                if groups == wanted {
                    break;
                }
                let (x, z) = (self.rng.below(land.span as usize * 2) as i32 - land.span, self.rng.below(land.span as usize * 2) as i32 - land.span);
                let middle = Vec2::new(x as f32 + 0.5, z as f32 + 0.5) * CELL;
                if land.biome(x, z) != k.habitat || land.slope(x, z) > 1 || middle.length() < keep_off {
                    continue;
                }
                // The better a place suits it, the likelier it settles there.
                let niche = k.niche();
                let place = ecology.place(land, middle.x, middle.y);
                let suits = crate::ecology::Niche { biomes: &[(Biome::Water, 1.0), (Biome::Shore, 1.0), (Biome::Meadow, 1.0), (Biome::Forest, 1.0), (Biome::Rock, 1.0), (Biome::Snow, 1.0)], ..niche }.suits(&place);
                if self.rng.between(0.0, 1.0) > suits.max(0.02) {
                    continue;
                }
                // The timid do not make their home beside what hunts them, nor the other way
                // round: a beaver put down on a strip of shore between a giant toad and the
                // water spent its whole life fleeing into a corner.
                let fierce = |t: Temper| matches!(t, Temper::Hunter | Temper::Hostile);
                let uneasy = self.agents.values().any(|a| {
                    let theirs = self.kinds[a.kind].temper;
                    ((k.temper == Temper::Timid && fierce(theirs)) || (fierce(k.temper) && theirs == Temper::Timid)) && a.mind.home.distance(middle) < 35.0
                });
                if uneasy {
                    continue;
                }
                // A kind keeps its own ground: herds were put down wherever suited them, three
                // of ponies within a stone's throw, and the developer met one great cluster.
                let apart = if k.group > 1 { 80.0 } else { 30.0 };
                if self.agents.values().any(|a| a.kind == kind && a.mind.home.distance(middle) < apart) {
                    continue;
                }
                let mut members = 0;
                for _ in 0..k.group * 12 {
                    if members == k.group {
                        break;
                    }
                    let at = middle + Vec2::new(self.rng.between(-1.0, 1.0), self.rng.between(-1.0, 1.0)) * (1.0 + k.group as f32 * k.length.max(k.width));
                    let (cx, cz) = ((at.x / CELL).floor() as i32, (at.y / CELL).floor() as i32);
                    let pos = Vec3::new(at.x, land.height(cx, cz) as f32 * CELL, at.y);
                    // The same body `add` gives it: a squirrel is not put where only a shorter one fits.
                    let probe = Body::new(pos, k.shape().0, k.height.max(0.3));
                    let crowded = self.agents.values().any(|a| a.body.pos.distance(pos) < a.body.half + k.length.max(k.width));
                    // Room to leave: a body's length off, on the ground there, it fits in at least
                    // three of four directions. A deer put down in a thicket stood there for good.
                    let reach = k.length.max(k.width).max(0.5);
                    let ways = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)].into_iter().filter(|(dx, dz)| {
                        let (x, z) = (at.x + dx * reach, at.y + dz * reach);
                        let ground = land.height((x / CELL).floor() as i32, (z / CELL).floor() as i32) as f32 * CELL;
                        !probe.blocked(world, Vec3::new(x, ground, z))
                    });
                    let hemmed = ways.count() < 3;
                    if matches!(land.biome(cx, cz), Biome::Water) || probe.blocked(world, pos) || crowded || hemmed {
                        continue;
                    }
                    let facing = self.rng.between(0.0, std::f32::consts::TAU);
                    let id = self.add(world, kind, pos, facing, Driver::Mind);
                    // A group shares the place it keeps to.
                    self.agents.get_mut(&id).unwrap().mind.home = middle;
                    members += 1;
                    placed += 1;
                }
                groups += (members > 0) as usize;
            }
        }
        placed
    }
}

/// What a day did to the island's creatures.
#[derive(Default, Clone, Debug)]
pub struct Tally {
    pub born: u32,
    /// Died of age, were killed and eaten, starved (for want of prey, or of ground to feed
    /// on), and died of the winter.
    pub old: u32,
    pub hunted: u32,
    pub starved: u32,
    pub winter: u32,
    /// Groups that outgrew their ground and sent half their number to find more.
    pub moved: u32,
    /// What there is to graze over the ground that is grazed, of 1: on the whole, and on
    /// the barest piece.
    pub forage: (f32, f32),
    /// Each kill: the kind that hunted, the kind taken.
    pub taken: Vec<(usize, usize)>,
}

impl Sim {
    /// How grown a creature is, from 0.45 (new-born) to 1: young are small.
    pub fn grown(&self, agent: &Agent) -> f32 {
        let kind = &self.kinds[agent.kind];
        let age = (self.when - agent.born) / crate::sky::YEAR;
        (0.45 + 0.55 * age / kind.maturity()).clamp(0.45, 1.0)
    }

    /// A day of the island's life, for every creature on it: called once as each day turns.
    ///
    /// The creatures near whoever is watching live second by second (`step`); the two
    /// thousand elsewhere cannot, and need not: what matters of a day far off is who was
    /// born, who died and of what. So once a day:
    ///
    /// - the old die: seldom past four fifths of their kind's years, soon past all of them;
    /// - what eats meat grows hungry, a meal every three days. Hungry, it hunts what eats
    ///   plants within its range and no more than half as big again as itself, and takes
    ///   one six times in ten. Twelve days without, it starves;
    /// - what eats plants dies for want of food where there are more of its kind than the
    ///   ground feeds, the faster the more there are; and a few die of the winter;
    /// - on the first day of spring, pairs have young (and the small, a second litter in
    ///   summer): fewer the nearer the ground is to full, none when it is;
    /// - a group grown to twice its size sends half its number to find ground of their
    ///   own, in their own kind of country.
    ///
    /// So numbers are made by the island and not set: prey rises to what its ground feeds,
    /// hunters to what the prey will bear, and a herd that is hunted out is gone until its
    /// kind spreads back. (What is awake is not hunted here nor hunts here: it does that
    /// for real.) `cargo world years` runs this by itself for years and prints the numbers.
    pub fn day(&mut self, world: &World, land: Option<&Land>) -> Tally {
        let mut tally = Tally::default();
        let year = crate::sky::YEAR;
        let day = self.when.floor() as i64;
        let lives = |a: &Agent, kinds: &[Kind]| a.dead.is_none() && a.driver == Driver::Mind && kinds[a.kind].temper != Temper::Person;
        let ids: Vec<u32> = self.agents.iter().filter(|(_, a)| lives(a, &self.kinds)).map(|(&id, _)| id).collect();

        // The old.
        for &id in &ids {
            let agent = &self.agents[&id];
            let (age, span) = ((self.when - agent.born) / year, self.kinds[agent.kind].lifespan);
            let dies = if age > span { 0.25 } else if age > span * 0.8 { 0.012 } else { 0.0 };
            if self.rng.unit() < dies {
                self.agents.remove(&id);
                tally.old += 1;
            }
        }

        // The hunters and the hunted.
        let hunters: Vec<u32> = ids.iter().copied().filter(|id| self.agents.get(id).is_some_and(|a| matches!(self.kinds[a.kind].diet, Diet::Meat | Diet::All) && self.kinds[a.kind].temper != Temper::Wary && !a.awake)).collect();
        for id in hunters {
            let Some(hunter) = self.agents.get_mut(&id) else { continue };
            // A meal every six days; one that also feeds on what grows hunts a third as often.
            let kind = self.kinds[hunter.kind].clone();
            // (One that is awake hungers by the second, in `step`.)
            if !hunter.awake {
                hunter.hunger += kind.appetite();
            }
            if hunter.hunger < 1.0 {
                continue;
            }
            let (home, hunger, own) = (hunter.mind.home, hunter.hunger, hunter.kind);
            // What it can take: a tenth its size to half as big again, and a pack takes larger.
            let most = kind.height * 1.5 * (kind.group as f32).sqrt();
            // It hunts the ground it keeps and a way beyond, not the island: so where the
            // hunters are many the hunted grow few, the hunt fails, and the hunters go hungry
            // there. With the whole island to take from, wolves grew sixfold in twenty years
            // while the deer beside them fell to a third.
            let reach = kind.range * HUNT_REACH;
            let prey: Vec<u32> = self
                .agents
                .iter()
                .filter(|(_, a)| lives(a, &self.kinds) && !a.awake && a.mind.home.distance(home) < reach && matches!(self.kinds[a.kind].diet, Diet::Plants) && self.kinds[a.kind].height < most && self.kinds[a.kind].height > kind.height * 0.1 && a.mind.home.distance(home) < kind.range)
                .map(|(&p, _)| p)
                .collect();
            // (By the square: the last few of anything are hard to find, which is what keeps
            // a hunted kind from being hunted out.)
            if self.rng.unit() < 0.5 * (prey.len() as f32 / PLENTY).min(1.0).powi(2) {
                let taken = prey[self.rng.below(prey.len())];
                tally.taken.push((own, self.agents[&taken].kind));
                let size = self.kinds[self.agents[&taken].kind].height / kind.height;
                let big = size >= 0.6;
                self.agents.remove(&taken);
                // The kill feeds the one that made it, and if it is big, the pack; and a big
                // kill feeds for longer than a small one, by its bulk: a wolf on a deer does
                // not hunt again for weeks, on a rabbit it hunts again in days. (Every kill
                // one meal, the trolls and ogres ate the deer, ponies and moose to nothing.)
                let full = -(2.0 * size.powi(3)).min(3.0);
                for a in self.agents.values_mut().filter(|a| a.kind == own && a.mind.home.distance(home) < 1.0) {
                    if big {
                        a.hunger = full / kind.group.max(1) as f32;
                    }
                }
                self.agents.get_mut(&id).unwrap().hunger = if big { full / kind.group.max(1) as f32 } else { full };
                tally.hunted += 1;
            } else if hunger >= STARVES && kind.diet == Diet::Meat {
                self.agents.remove(&id);
                tally.starved += 1;
            }
        }

        // What grows is eaten and grows back. Each grazer takes from the ground it lives on,
        // by its size; what is left grows by the season, fastest in spring and not at all in
        // winter, and slowest where it is nearly gone or nearly whole (it grows from what is
        // there, and has only so far to grow). Until 2026-11-11 the ground was never eaten:
        // what fed on it was held only by a count of its own kind.
        let season = crate::sky::season(self.when);
        for (piece, taken) in std::mem::take(&mut self.nibbled) {
            let left = self.forage.entry(piece).or_insert(1.0);
            *left = (*left - taken).max(0.0);
        }
        for &id in &ids {
            let Some(agent) = self.agents.get(&id) else { continue };
            let kind = &self.kinds[agent.kind];
            let eats = match kind.diet {
                Diet::Plants => 1.0,
                Diet::All => 0.3,
                _ => continue,
            };
            // (What is awake takes its own as it grazes, in `step`.)
            if agent.awake {
                continue;
            }
            let piece = ((agent.mind.home.x / PASTURE).floor() as i32, (agent.mind.home.y / PASTURE).floor() as i32);
            let left = self.forage.entry(piece).or_insert(1.0);
            *left = (*left - eats * kind.height * kind.height * GRAZED).max(0.0);
        }
        let grows = match season {
            crate::sky::Season::Spring => 0.2,
            crate::sky::Season::Summer => 0.15,
            crate::sky::Season::Autumn => 0.06,
            crate::sky::Season::Winter => 0.0,
        };
        for left in self.forage.values_mut() {
            *left = (*left + grows * left.max(0.3) * (1.0 - *left)).min(1.0);
        }
        tally.forage = if self.forage.is_empty() { (1.0, 1.0) } else { (self.forage.values().sum::<f32>() / self.forage.len() as f32, self.forage.values().copied().fold(1.0, f32::min)) };

        // What the ground will feed, and the winter.
        let winter = season == crate::sky::Season::Winter;
        let near = |agents: &BTreeMap<u32, Agent>, kind: usize, home: Vec2, range: f32| agents.values().filter(|a| a.kind == kind && a.dead.is_none() && a.mind.home.distance(home) < range).count();
        for &id in &ids {
            let Some(agent) = self.agents.get(&id) else { continue };
            let kind = &self.kinds[agent.kind];
            if matches!(kind.diet, Diet::Meat | Diet::None) {
                continue;
            }
            let crowd = near(&self.agents, agent.kind, agent.mind.home, kind.range) as f32 / kind.capacity as f32;
            // (Crowded by its own kind, or on ground grazed lean by anything.)
            let lean = (LEAN - self.forage_at(agent.mind.home)).max(0.0) * 0.4;
            let (hungry, cold) = ((((crowd - 1.0) * 0.03).max(0.0) + lean).clamp(0.0, 0.25), if winter { 0.004 } else { 0.0 });
            let roll = self.rng.unit();
            if roll < hungry {
                self.agents.remove(&id);
                tally.starved += 1;
            } else if roll < hungry + cold {
                self.agents.remove(&id);
                tally.winter += 1;
            }
        }

        // The young: on the first day of spring, and for the small again at midsummer.
        let of_year = day.rem_euclid(year as i64);
        let mut groups: BTreeMap<(usize, i32, i32), Vec<u32>> = BTreeMap::new();
        for (&id, a) in self.agents.iter().filter(|(_, a)| lives(a, &self.kinds)) {
            groups.entry((a.kind, (a.mind.home.x * 4.0) as i32, (a.mind.home.y * 4.0) as i32)).or_default().push(id);
        }
        for ((kind, _, _), members) in &groups {
            let k = self.kinds[*kind].clone();
            if !(of_year == 0 || (of_year == crate::sky::SEASON as i64 + 3 && k.lifespan <= 4.0)) {
                continue;
            }
            let home = self.agents[&members[0]].mind.home;
            let adults = members.iter().filter(|id| (self.when - self.agents[*id].born) / year >= k.maturity()).count();
            // One that lives alone finds a mate if another of its kind lives within reach.
            let kin = near(&self.agents, *kind, home, k.range * 6.0);
            let pairs = if k.group > 1 { adults as f32 / 2.0 } else if kin > adults { adults as f32 * 0.5 } else { 0.0 };
            // (Fewer young on lean ground, for what grazes.)
            let fat = if matches!(k.diet, Diet::Plants | Diet::All) { (self.forage_at(home) / (LEAN + 0.1)).min(1.0) } else { 1.0 };
            let room = (1.0 - near(&self.agents, *kind, home, k.range) as f32 / k.capacity as f32).max(0.0) * fat;
            // What hunts rears as many as it has fed: a pack that has gone hungry rears none.
            let fed = if matches!(k.diet, Diet::Meat) { (1.0 - (members.iter().map(|id| self.agents[id].hunger).sum::<f32>() / members.len() as f32 - 1.0) / 2.0).clamp(0.0, 1.0) } else { 1.0 };
            let wanted = pairs * k.litter * room * fed;
            let young = wanted.floor() as usize + (self.rng.unit() < wanted.fract()) as usize;
            let (at, facing) = (self.agents[&members[0]].body.pos, self.agents[&members[0]].facing);
            for _ in 0..young {
                let beside = Vec3::new(self.rng.between(-1.5, 1.5), 0.0, self.rng.between(-1.5, 1.5));
                let id = self.add(world, *kind, at + beside, facing, Driver::Mind);
                let (when, awake) = (self.when, self.agents[&members[0]].awake);
                let born = self.agents.get_mut(&id).unwrap();
                (born.mind.home, born.born, born.awake) = (home, when, awake);
                tally.born += 1;
            }
        }

        // A group grown too large for its ground sends half its number to find their own.
        if let Some(land) = land {
            for ((kind, _, _), members) in &groups {
                let k = self.kinds[*kind].clone();
                let here: Vec<u32> = members.iter().copied().filter(|id| self.agents.get(id).is_some_and(|a| !a.awake)).collect();
                if k.group < 2 || here.len() < k.group * 2 {
                    continue;
                }
                let home = self.agents[&here[0]].mind.home;
                for _ in 0..12 {
                    let (way, far) = (self.rng.between(0.0, std::f32::consts::TAU), k.range * self.rng.between(1.5, 3.0));
                    let to = home + Vec2::new(way.cos(), way.sin()) * far;
                    let (x, z) = ((to.x / CELL).floor() as i32, (to.y / CELL).floor() as i32);
                    if land.biome(x, z) != k.habitat || land.slope(x, z) > 1 || near(&self.agents, *kind, to, k.range) > 0 {
                        continue;
                    }
                    let ground = land.height(x, z) as f32 * CELL;
                    for id in here.iter().skip(here.len() / 2) {
                        let a = self.agents.get_mut(id).unwrap();
                        a.mind.home = to;
                        a.mind.goal = to;
                        a.body.pos = Vec3::new(to.x, ground, to.y);
                        a.before = a.body.pos;
                    }
                    tally.moved += 1;
                    break;
                }
            }
        }
        tally
    }
}

impl Kind {
    /// How fast it grows hungry, in meals a day: what hunts wants a kill every six days,
    /// what does both a third as often, what eats plants must graze part of every day, and
    /// what does not eat never hungers.
    pub fn appetite(&self) -> f32 {
        match self.diet {
            Diet::Meat => 1.0 / self.meal,
            Diet::All => 1.0 / 18.0,
            Diet::Plants => 2.0,
            Diet::None => 0.0,
        }
    }

    /// Years before it is grown and breeds: a tenth of its life, a season at the least.
    pub fn maturity(&self) -> f32 {
        (self.lifespan * 0.1).clamp(0.25, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TICKS, mind::State, terrain};
    use cube_voxel::Block;

    fn kind(id: &str, temper: Temper, height: f32, speeds: [f32; 3]) -> Kind {
        Kind { id: id.into(), height, length: height * 0.8, width: height * 0.4, walk: speeds[0], run: speeds[1], sprint: speeds[2], temper, habitat: Biome::Meadow, group: 2, groups: 3, water: Default::default(), cover: Default::default(), hours: Default::default(), voice: None, winter: 1.0, diet: if matches!(temper, Temper::Hunter | Temper::Hostile) { Diet::Meat } else { Diet::Plants }, litter: 2.0, lifespan: 8.0, capacity: 8, range: 80.0, meal: 6.0, holds: None, share: 1.0 }
    }

    /// Rabbit, wolf, bear, goblin and a person, on a floor of stone 60 m square.
    fn stage() -> (World, Sim) {
        let mut world = World::default();
        world.fill(Vec3::new(-30.0, 0.5, -30.0), Vec3::new(30.0, 1.0, 30.0), Block::Stone);
        let kinds = vec![
            kind("rabbit", Temper::Timid, 0.3, [0.5, 0.9, 1.6]),
            kind("wolf", Temper::Hunter, 0.85, [1.0, 2.0, 3.6]),
            kind("bear", Temper::Wary, 1.3, [1.1, 2.4, 4.2]),
            kind("goblin", Temper::Hostile, 1.2, [0.8, 2.5, 3.5]),
            kind("person", Temper::Person, 1.75, [1.2, 4.0, 5.6]),
        ];
        (world, Sim::new(kinds, 1))
    }

    fn at(x: f32, z: f32) -> Vec3 {
        Vec3::new(x, 1.0, z)
    }

    fn run(sim: &mut Sim, world: &World, seconds: f32) {
        for _ in 0..(seconds * TICKS as f32) as u32 {
            sim.step(world);
        }
    }

    #[test]
    fn a_world_only_woken_is_held_still() {
        // What the game does behind its loading screen: a wolf beside a rabbit, and nothing
        // happens to either, nor does the clock move.
        let (world, mut sim) = stage();
        let rabbit = sim.add(&world, 0, at(0.0, 0.0), 0.0, Driver::Mind);
        let wolf = sim.add(&world, 1, at(1.0, 0.0), 0.0, Driver::Mind);
        let (when, tick) = (sim.when, sim.tick);
        for _ in 0..TICKS * 5 {
            sim.wake(&world);
        }
        assert_eq!((sim.when, sim.tick), (when, tick));
        assert_eq!(sim.agents[&rabbit].body.pos, at(0.0, 0.0));
        assert_eq!(sim.agents[&wolf].body.pos, at(1.0, 0.0));
        assert_eq!(sim.agents[&rabbit].health, sim.kinds[0].health());
        assert!(sim.agents[&wolf].awake);
    }

    #[test]
    fn a_herd_bolts_when_one_of_it_does_and_a_hungry_hunter_goes_looking() {
        // Two rabbits fifteen metres apart, and a fed wolf near the first only: too far from
        // the second for it to take notice (twelve metres), and it runs all the same.
        let (world, mut sim) = stage();
        let (near, far) = (sim.add(&world, 0, at(0.0, 0.0), 0.0, Driver::Mind), sim.add(&world, 0, at(-15.0, 0.0), 0.0, Driver::Mind));
        let wolf = sim.add(&world, 1, at(9.0, 0.0), 0.0, Driver::Mind);
        sim.agents.get_mut(&wolf).unwrap().hunger = 0.0;
        run(&mut sim, &world, 1.5);
        assert!(sim.agents[&near].mind.state == State::Flee, "the nearer did not run: {:?}", sim.agents[&near].mind.state);
        assert!(sim.agents[&far].log.fled >= 1, "the further did not take fright from the nearer: {:?}", sim.agents[&far].mind.state);
        // And a hungry wolf with rabbits forty metres off, out of its sight, sets out toward them.
        let (world, mut sim) = stage();
        let wolf = sim.add(&world, 1, at(20.0, 0.0), 0.0, Driver::Mind);
        sim.add(&world, 0, at(-20.0, 0.0), 0.0, Driver::Mind);
        (sim.agents.get_mut(&wolf).unwrap().hunger, sim.agents.get_mut(&wolf).unwrap().mind.about) = (1.0, 1.0);
        run(&mut sim, &world, 12.0);
        assert!(sim.agents[&wolf].body.pos.x < 14.0 && sim.agents[&wolf].log.trips >= 1, "the hungry wolf stayed where it was: at {} after {} trips", sim.agents[&wolf].body.pos, sim.agents[&wolf].log.trips);
    }

    #[test]
    fn a_rabbit_runs_from_a_wolf_and_the_wolf_catches_it() {
        let (world, mut sim) = stage();
        let rabbit = sim.add(&world, 0, at(0.0, 0.0), 0.0, Driver::Mind);
        let wolf = sim.add(&world, 1, at(7.0, 0.0), 0.0, Driver::Mind);
        // (A wolf that has not eaten for days.)
        sim.agents.get_mut(&wolf).unwrap().hunger = 1.0;
        run(&mut sim, &world, 2.0);
        let (r, w) = (&sim.agents[&rabbit], &sim.agents[&wolf]);
        assert!(r.mind.state == State::Flee && r.body.pos.x < -0.5, "the rabbit did not run: {:?} at {}", r.mind.state, r.body.pos);
        assert!(w.mind.state == State::Chase && w.body.pos.x < 6.0, "the wolf did not give chase: {:?} at {}", w.mind.state, w.body.pos);
        run(&mut sim, &world, 10.0);
        let (r, w) = (&sim.agents[&rabbit], &sim.agents[&wolf]);
        assert!(r.log.struck >= 1, "never caught: rabbit {} wolf {}", r.body.pos, w.body.pos);
        // Having caught it the wolf is spent, and lets it go.
        assert!(w.mind.tired > 0.0 && w.mind.state != State::Chase, "{:?}", w.mind);
    }

    #[test]
    fn a_goblin_goes_for_a_person_and_knocks_them_back() {
        let (world, mut sim) = stage();
        let person = sim.add(&world, 4, at(0.0, 0.0), 0.0, Driver::Outside);
        let goblin = sim.add(&world, 3, at(0.0, 10.0), 0.0, Driver::Mind);
        run(&mut sim, &world, 8.0);
        let (p, g) = (&sim.agents[&person], &sim.agents[&goblin]);
        assert!(p.log.struck >= 1, "never hit: goblin {:?} at {}", g.mind.state, g.body.pos);
        assert!(p.body.pos.z < -0.3, "not knocked back: {}", p.body.pos);
        assert!(g.log.acts[Act::Attack as usize] > 0.3);
    }

    #[test]
    fn a_blow_takes_health_and_the_dead_lie_then_return() {
        let (world, mut sim) = stage();
        let person = sim.add(&world, 4, at(0.0, 0.0), 0.0, Driver::Outside);
        let rabbit = sim.add(&world, 0, at(0.0, 1.0), 0.0, Driver::Stand);
        let full = sim.agents[&rabbit].health;
        sim.agents.get_mut(&person).unwrap().pack.as_mut().unwrap().hold(7);
        // Bare hands (an empty slot of the bar): a rabbit takes several blows. Each lands a third of a second on.
        sim.swing(person);
        run(&mut sim, &world, 0.3);
        assert_eq!(sim.agents[&rabbit].health, full);
        run(&mut sim, &world, 0.2);
        assert_eq!(sim.agents[&rabbit].health, full - FISTS);
        // With a blade, one more is enough; it lies, then is gone, and nothing is put in its place.
        sim.agents.get_mut(&person).unwrap().pack.as_mut().unwrap().hold(0);
        sim.agents.get_mut(&person).unwrap().doing = None;
        sim.swing(person);
        run(&mut sim, &world, 1.0);
        assert!(sim.agents[&rabbit].dead.is_some() && sim.agents[&rabbit].act == Act::Die);
        assert_eq!(sim.agents[&person].log.kills, 1);
        assert_eq!(sim.agents[&person].pack.as_ref().unwrap().count(crate::items::Item::Meat), 1);
        run(&mut sim, &world, LIES + 1.0);
        assert!(!sim.agents.contains_key(&rabbit));
        run(&mut sim, &world, 200.0);
        assert_eq!(sim.agents.values().filter(|a| a.kind == 0).count(), 0);
        // A blow at nothing, or at what is behind, lands on nothing.
        let behind = sim.add(&world, 0, at(0.0, -1.0), 0.0, Driver::Stand);
        sim.agents.get_mut(&person).unwrap().doing = None;
        sim.swing(person);
        run(&mut sim, &world, 1.0);
        assert_eq!(sim.agents[&behind].health, sim.kinds[0].health());
    }

    #[test]
    fn what_comes_down_strikes_whoever_is_under_it_and_nobody_beside() {
        let (world, mut sim) = stage();
        let under = sim.add(&world, 4, at(0.0, 0.0), 0.0, Driver::Stand);
        let beside = sim.add(&world, 4, at(9.0, 0.0), 0.0, Driver::Stand);
        let full = sim.agents[&under].health;
        // A tree's worth, down fast; and a clod.
        sim.loose.landed.push(crate::loose::Landed { at: at(0.3, 0.0) + Vec3::Y, cells: 6000, put: 6000, speed: 20.0, settled: false });
        sim.step(&world);
        assert!(sim.agents[&under].dead.is_some(), "a tree came down on him and he has {} of {full}", sim.agents[&under].health);
        assert_eq!(sim.agents[&beside].health, full, "it struck someone nine metres off");
        sim.loose.landed.push(crate::loose::Landed { at: at(9.0, 0.2) + Vec3::Y, cells: 8, put: 8, speed: 5.0, settled: false });
        sim.step(&world);
        let left = sim.agents[&beside].health;
        assert!(left < full && left > full * 0.5, "a clod took {} of {full}", full - left);
        assert_eq!(sim.loose.heard.len(), 2, "what landed was not passed on to be heard");
    }

    #[test]
    fn what_is_in_hand_decides_what_using_it_does() {
        use crate::items::{Item, Used};
        let (world, mut sim) = stage();
        let person = sim.add(&world, 4, at(0.0, 0.0), 0.0, Driver::Outside);
        let bear = sim.add(&world, 2, at(0.0, 1.6), 0.0, Driver::Stand);
        let full = sim.agents[&bear].health;
        // The sword is in hand at the start: a blow takes what a sword takes.
        assert_eq!(sim.use_held(person), Used::Swing);
        assert_eq!(sim.use_held(person), Used::Nothing, "one blow at a time");
        run(&mut sim, &world, 1.2);
        assert_eq!(sim.agents[&bear].health, full - Item::Sword.power().unwrap());
        // The shovel asks for a hole; berries mend, one at a time, and only the hurt eat.
        let hand = |sim: &mut Sim, item: Item| {
            let pack = sim.agents.get_mut(&person).unwrap().pack.as_mut().unwrap();
            pack.hold(pack.slots.iter().position(|s| s.is_some_and(|s| s.0 == item)).unwrap());
        };
        hand(&mut sim, Item::Shovel);
        assert_eq!(sim.use_held(person), Used::Cut(crate::items::Stuff::Earth, 0.8));
        assert_eq!(sim.use_held(person), Used::Nothing, "one spadeful at a time");
        (sim.agents.get_mut(&person).unwrap().swing, sim.agents.get_mut(&person).unwrap().doing) = (None, None);
        // What was dug is carried, and is put down again as what it was.
        sim.took(person, [300, 0, 0]);
        hand(&mut sim, Item::Earth);
        assert_eq!(sim.use_held(person), Used::Place(cube_voxel::Block::Dirt));
        sim.placed(person);
        assert_eq!(sim.agents[&person].pack.as_ref().unwrap().count(Item::Earth), 1);
        hand(&mut sim, Item::Berries);
        assert_eq!(sim.use_held(person), Used::Nothing);
        sim.agents.get_mut(&person).unwrap().health = 40.0;
        assert_eq!(sim.use_held(person), Used::Ate);
        let p = &sim.agents[&person];
        assert_eq!((p.health, p.pack.as_ref().unwrap().count(Item::Berries)), (55.0, 5));
    }

    #[test]
    fn a_bow_hits_what_is_far_off_in_line_and_nothing_to_the_side() {
        use crate::items::Item;
        let (world, mut sim) = stage();
        let person = sim.add(&world, 4, at(0.0, 0.0), 0.0, Driver::Outside);
        let (ahead, aside) = (sim.add(&world, 2, at(0.0, 18.0), 0.0, Driver::Stand), sim.add(&world, 2, at(8.0, 10.0), 0.0, Driver::Stand));
        let full = sim.agents[&ahead].health;
        let hold = |sim: &mut Sim, item: Item| {
            let pack = sim.agents.get_mut(&person).unwrap().pack.as_mut().unwrap();
            pack.hold(pack.slots.iter().position(|s| s.is_some_and(|s| s.0 == item)).unwrap());
        };
        // The sword reaches neither.
        sim.use_held(person);
        run(&mut sim, &world, 1.2);
        assert_eq!(sim.agents[&ahead].health, full);
        hold(&mut sim, Item::Bow);
        sim.use_held(person);
        run(&mut sim, &world, 1.2);
        assert_eq!(sim.agents[&ahead].health, full - Item::Bow.power().unwrap());
        assert_eq!(sim.agents[&aside].health, full);
    }

    #[test]
    fn a_person_saved_is_the_person_loaded() {
        let (world, mut sim) = stage();
        let person = sim.add(&world, 4, at(3.0, -2.0), 1.0, Driver::Outside);
        {
            let a = sim.agents.get_mut(&person).unwrap();
            a.health = 61.0;
            let pack = a.pack.as_mut().unwrap();
            pack.add(crate::items::Item::Meat, 3);
            pack.hold(2);
        }
        sim.when = 12.25;
        let (text, was) = (sim.save(person), sim.agents[&person].clone());
        let (_, mut other) = stage();
        let them = other.add(&world, 4, at(0.0, 0.0), 0.0, Driver::Outside);
        assert!(other.load(them, &text));
        let now = &other.agents[&them];
        assert_eq!((other.when, now.body.pos, now.health, &now.pack), (12.25, was.body.pos, 61.0, &was.pack));
        assert!(!other.load(them, "nonsense") && !other.load(them, "slot=99,Torch,1"));
    }

    #[test]
    fn a_guard_turns_most_of_a_blow_from_in_front() {
        let hurt = |guard: bool, facing: f32| {
            let (world, mut sim) = stage();
            let person = sim.add(&world, 4, at(0.0, 0.0), facing, Driver::Outside);
            sim.agents.get_mut(&person).unwrap().guard = guard;
            sim.add(&world, 3, at(0.0, 4.0), 0.0, Driver::Mind);
            let mut least = 100.0f32;
            for _ in 0..(6.0 / DT) as u32 {
                sim.step(&world);
                least = least.min(sim.agents[&person].health);
                if least < 100.0 {
                    break;
                }
            }
            100.0 - least
        };
        let (open, guarded, turned_away) = (hurt(false, 0.0), hurt(true, 0.0), hurt(true, std::f32::consts::PI));
        // And a jerkin carried takes a quarter off.
        let (world, mut sim) = stage();
        let person = sim.add(&world, 4, at(0.0, 0.0), 0.0, Driver::Outside);
        sim.agents.get_mut(&person).unwrap().pack.as_mut().unwrap().add(crate::items::Item::Jerkin, 1);
        sim.add(&world, 3, at(0.0, 4.0), 0.0, Driver::Mind);
        let mut least = 100.0f32;
        for _ in 0..(6.0 / DT) as u32 {
            sim.step(&world);
            least = least.min(sim.agents[&person].health);
            if least < 100.0 {
                break;
            }
        }
        assert!((100.0 - least - open * 0.75).abs() < 0.1, "armoured {} open {open}", 100.0 - least);
        assert!(open > 5.0, "the goblin never struck");
        assert!((guarded - open * 0.3).abs() < 0.1 && (turned_away - open).abs() < 0.1, "open {open} guarded {guarded} turned away {turned_away}");
    }

    #[test]
    fn a_person_killed_wakes_at_home_and_mends() {
        let (world, mut sim) = stage();
        let person = sim.add(&world, 4, at(0.0, 0.0), 0.0, Driver::Outside);
        sim.add(&world, 3, at(0.0, 6.0), 0.0, Driver::Mind);
        sim.agents.get_mut(&person).unwrap().health = 20.0;
        let mut died = false;
        for _ in 0..(40.0 / DT) as u32 {
            sim.step(&world);
            died |= sim.agents[&person].dead.is_some();
        }
        let p = &sim.agents[&person];
        assert!(died && p.log.died >= 1, "the goblin never killed them: health {}", p.health);
        // Hurt but left alone, health comes back.
        let (world, mut sim) = stage();
        let person = sim.add(&world, 4, at(0.0, 0.0), 0.0, Driver::Outside);
        sim.agents.get_mut(&person).unwrap().health = 50.0;
        run(&mut sim, &world, 20.0);
        assert!(sim.agents[&person].health > 70.0);
    }

    #[test]
    fn a_bear_leaves_a_person_alone_until_they_come_close() {
        let (world, mut sim) = stage();
        let person = sim.add(&world, 4, at(0.0, 0.0), 0.0, Driver::Outside);
        let bear = sim.add(&world, 2, at(12.0, 0.0), 0.0, Driver::Mind);
        sim.agents.get_mut(&bear).unwrap().mind.left = 100.0;
        run(&mut sim, &world, 6.0);
        assert_eq!(sim.agents[&person].log.struck, 0);
        // The person walks up to it.
        let p = sim.agents.get_mut(&person).unwrap();
        p.intent = Intent { dir: Vec2::X, speed: 1.2, ..Intent::default() };
        run(&mut sim, &world, 9.0);
        let (p, b) = (&sim.agents[&person], &sim.agents[&bear]);
        assert!(p.log.struck >= 1, "the bear let them walk into it: {:?}, person at {}", b.mind.state, p.body.pos);
        // It faced them to do it.
        assert!(turn_between(b.facing, -std::f32::consts::FRAC_PI_2).abs() < 0.5, "facing {}", b.facing);
    }

    #[test]
    fn the_same_seed_lives_the_same_life_and_nothing_leaves_the_world() {
        let (world, land) = terrain::generate(6, 12);
        let live = || {
            let (_, mut sim) = stage();
            let placed = sim.populate(&world, &land);
            assert!(placed >= 10, "only {placed} placed");
            run(&mut sim, &world, 30.0);
            sim
        };
        let (a, b) = (live(), live());
        for (id, agent) in &a.agents {
            assert_eq!(agent.body.pos, b.agents[id].body.pos);
            assert!(agent.log.inside == 0 && agent.log.fell == 0, "{} #{id}: {:?}", a.kinds[agent.kind].id, agent.log);
        }
        assert!(a.agents.values().map(|agent| agent.log.moved).sum::<f32>() > 20.0);
    }

    #[test]
    fn a_rabbit_grazing_in_sight_eats_the_ground_it_stands_on() {
        let (world, mut sim) = stage();
        let rabbit = sim.add(&world, 0, at(2.0, 2.0), 0.0, Driver::Mind);
        // Hungry, it grazes; and while it does, a little of the grazing there goes.
        sim.agents.get_mut(&rabbit).unwrap().hunger = 0.9;
        run(&mut sim, &world, 120.0);
        let (grazed, left) = (sim.agents[&rabbit].log.acts[Act::Eat as usize], sim.forage_at(Vec2::new(2.0, 2.0)));
        assert!(grazed > 5.0 && left < 1.0 && left > 0.99, "it grazed {grazed:.0} s and left {left}");
        // (No grass stands on the test floor: nothing there to be eaten and seen to go.)
        assert!(sim.eaten.is_empty());
    }

    #[test]
    fn what_grazes_eats_the_ground_down_starves_on_lean_ground_and_it_grows_back() {
        // (The helper's kinds: 0 a rabbit.) A few on one piece of ground, no more than it holds.
        let (world, mut sim) = stage();
        let here = Vec2::new(5.0, 5.0);
        for i in 0..4 {
            let id = sim.add(&world, 0, at(i as f32, 0.0), 0.0, Driver::Mind);
            let rabbit = sim.agents.get_mut(&id).unwrap();
            (rabbit.mind.home, rabbit.awake) = (here, false);
        }
        // A day's grazing takes a little of it.
        sim.when = crate::sky::YEAR * 2.0 + crate::sky::SEASON * 3.5;
        sim.day(&world, None);
        let after = sim.forage_at(here);
        assert!(after < 1.0 && after > 0.99, "four rabbits a day in winter left {after}");
        // On ground grazed nearly bare, in winter when nothing grows, they starve.
        sim.forage.values_mut().for_each(|f| *f = 0.02);
        let mut starved = 0;
        for _ in 0..40 {
            for i in 0..4 {
                let id = sim.add(&world, 0, at(i as f32, 0.0), 0.0, Driver::Mind);
                let rabbit = sim.agents.get_mut(&id).unwrap();
                (rabbit.mind.home, rabbit.awake) = (here, false);
            }
            sim.agents.retain(|_, a| a.mind.home == here);
            let ids: Vec<u32> = sim.agents.keys().copied().collect();
            ids.iter().skip(4).for_each(|id| {
                sim.agents.remove(id);
            });
            sim.when = crate::sky::YEAR * 2.0 + crate::sky::SEASON * 3.5;
            starved += sim.day(&world, None).starved;
        }
        assert!(starved >= 5 && sim.forage_at(here) < 0.05, "on bare ground {starved} of 160 rabbit-days starved, and {:.2} was left", sim.forage_at(here));
        // With them gone most of it grows back within a year.
        sim.agents.clear();
        sim.when = crate::sky::YEAR * 3.0;
        for _ in 0..crate::sky::YEAR as usize {
            sim.day(&world, None);
            sim.when += 1.0;
        }
        assert!(sim.forage_at(here) > 0.6, "a year ungrazed and only {:.2} has grown back", sim.forage_at(here));
    }

    #[test]
    fn a_herd_breeds_in_spring_and_a_hunter_with_no_prey_starves() {
        // (The helper's kinds: 0 a rabbit, 1 a wolf.)
        let (world, mut sim) = stage();
        for i in 0..4 {
            let id = sim.add(&world, 0, at(i as f32, 0.0), 0.0, Driver::Mind);
            let rabbit = sim.agents.get_mut(&id).unwrap();
            // (One herd: they keep to the one place.)
            (rabbit.awake, rabbit.mind.home) = (false, Vec2::ZERO);
        }
        // The first day of a spring: pairs have young, born small.
        sim.when = crate::sky::YEAR * 2.0;
        sim.agents.values_mut().for_each(|a| a.born = sim.when - crate::sky::YEAR * 2.0);
        let spring = sim.day(&world, None);
        assert!(spring.born >= 1, "{spring:?}");
        let young = sim.agents.values().find(|a| a.born == sim.when).unwrap();
        assert!(sim.grown(young) < 0.5);
        // A wolf alone, far from anything it could eat, goes hungry and dies of it.
        let wolf = sim.add(&world, 1, at(900.0, 900.0), 0.0, Driver::Mind);
        let w = sim.agents.get_mut(&wolf).unwrap();
        (w.awake, w.mind.home, w.born) = (false, Vec2::new(900.0, 900.0), sim.when - crate::sky::YEAR);
        let mut starved = 0;
        for _ in 0..30 {
            sim.when += 1.0;
            starved += sim.day(&world, None).starved;
        }
        assert!(!sim.agents.contains_key(&wolf) && starved >= 1);
    }


    #[test]
    fn a_wolf_in_sight_that_finds_nothing_weakens_and_starves() {
        let (world, mut sim) = stage();
        let wolf = sim.add(&world, 1, at(0.0, 0.0), 0.0, Driver::Mind);
        let whole = sim.agents[&wolf].health;
        // Three meals missed: it is hungry and hunts, and is not yet harmed by it.
        sim.agents.get_mut(&wolf).unwrap().hunger = STARVES - 1.5;
        run(&mut sim, &world, 5.0);
        assert!(sim.agents[&wolf].dead.is_none() && sim.agents[&wolf].health >= whole - 0.01);
        // Past that it loses strength by the second, and at four it dies.
        sim.agents.get_mut(&wolf).unwrap().hunger = STARVES - 0.5;
        run(&mut sim, &world, 5.0);
        assert!(sim.agents[&wolf].dead.is_none() && sim.agents[&wolf].health < whole, "a starving wolf lost no strength");
        sim.agents.get_mut(&wolf).unwrap().hunger = STARVES;
        run(&mut sim, &world, 0.1);
        assert!(sim.agents[&wolf].dead.is_some() && sim.starved == 1, "a wolf four meals short still lives");
    }

    #[test]
    fn a_fed_wolf_lets_a_rabbit_be_and_a_thirsty_one_goes_to_water() {
        let (mut world, mut sim) = stage();
        let rabbit = sim.add(&world, 0, at(0.0, 0.0), 0.0, Driver::Stand);
        let wolf = sim.add(&world, 1, at(7.0, 0.0), 0.0, Driver::Mind);
        let w = sim.agents.get_mut(&wolf).unwrap();
        (w.hunger, w.thirst) = (0.0, 0.0);
        run(&mut sim, &world, 6.0);
        assert!(sim.agents[&wolf].mind.state != State::Chase && sim.agents[&rabbit].log.struck == 0, "a fed wolf hunted: {:?}", sim.agents[&wolf].mind.state);
        // Water to the east, at the level water stands at. Thirsty, it walks there and drinks.
        let level = crate::terrain::SEA as f32 * cube_voxel::CELL;
        world.fill(Vec3::new(20.0, level - 0.5, -6.0), Vec3::new(26.0, level, 6.0), Block::Water);
        sim.agents.remove(&rabbit);
        let w = sim.agents.get_mut(&wolf).unwrap();
        (w.thirst, w.mind.about) = (1.0, 1.0);
        run(&mut sim, &world, 40.0);
        let w = &sim.agents[&wolf];
        assert!(w.log.drank >= 1 && w.thirst < 0.2 && w.body.pos.x > 14.0, "did not drink: thirst {} at {} {:?}", w.thirst, w.body.pos, w.mind.state);
    }

}
