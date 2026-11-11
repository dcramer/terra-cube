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
#[derive(Deserialize, Clone, Copy, Debug)]
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
}

impl Kind {
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
            let life = spec.life.unwrap_or(Life { temper: if spec.kind == "player" { Temper::Person } else { Temper::Wary }, habitat: None, group: 1, groups: 0, water: Default::default(), cover: Default::default(), hours: Default::default() });
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
}

impl Act {
    pub const ALL: [Act; 12] = [Act::Idle, Act::Walk, Act::Run, Act::Sprint, Act::Jump, Act::Fall, Act::Land, Act::Swim, Act::Eat, Act::Attack, Act::Hurt, Act::Die];
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
        }
    }
}

/// What a creature has done so far, for judging a run by numbers (`cargo world run`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Log {
    /// Metres over the ground.
    pub moved: f32,
    /// Seconds in each [`Act`].
    pub acts: [f32; 12],
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
    /// Seconds since it was last hurt: left in peace, it mends.
    pub calm: f32,
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
const RETURNS: f32 = 150.0;
const WAKES: f32 = 5.0;
/// A blow lands this long into a swing, reaches this far past the body, and to this far
/// either side of straight ahead (radians).
const SWING: f32 = 0.35;
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
    /// The dead not yet replaced: kind, where it lived, and the tick it died.
    graves: Vec<(usize, Vec2, u64)>,
    /// The time: days since the world began, the hour its fraction (`sky`).
    pub when: f32,
    pub rng: Rng,
    next: u32,
}

/// The fastest anything turns, radians a second: a full turn about in a third of a second.
const TURN_RATE: f32 = 10.0;

fn turn_between(from: f32, to: f32) -> f32 {
    (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl Sim {
    pub fn new(kinds: Vec<Kind>, seed: u64) -> Self {
        Self { kinds, agents: BTreeMap::new(), tick: 0, graves: Vec::new(), when: crate::sky::START, rng: Rng::new(seed), next: 1 }
    }

    /// A person uses what is in their hand: a blade swings, berries are eaten, a shovel asks
    /// for the ground to be dug (which is the caller's to do: the world is not ours here).
    pub fn use_held(&mut self, id: u32) -> crate::items::Used {
        use crate::items::{Item, Used};
        let Some(agent) = self.agents.get_mut(&id).filter(|a| a.dead.is_none()) else { return Used::Nothing };
        let held = agent.pack.as_ref().and_then(|p| p.held());
        match held {
            Some(Item::Shovel) => Used::Dig,
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
            Agent { kind, body, before: pos, facing_before: facing, facing, turn_to: None, intent: Intent::default(), gait: Gait::Walk, want_jump: 0.0, hold_jump: false, driver, mind, act: Act::Idle, doing: None, health: k.health(), dead: None, swing: None, power: k.harm(), calm: 0.0, pack: (k.temper == Temper::Person).then(crate::items::Pack::starting), stepped: 0.0, hurt: 0.0, landing: 0.0, airborne: 0.0, jumped: false, log: Log::default() },
        );
        id
    }

    /// One tick of [`DT`] for every creature.
    pub fn step(&mut self, world: &World) {
        self.tick += 1;
        self.when = crate::sky::later(self.when, DT);
        // Minds see the world as it was at the start of the tick.
        let seen: Vec<mind::Seen> = self.agents.iter().filter(|(_, a)| a.dead.is_none()).map(|(&id, a)| mind::Seen { id, pos: a.body.pos, vel: a.body.vel, temper: self.kinds[a.kind].temper, reach: a.body.half + a.body.long.length() }).collect();
        let mut hits = Vec::new();
        for (&id, agent) in self.agents.iter_mut().filter(|(_, a)| a.driver == Driver::Mind && a.dead.is_none()) {
            hits.extend(mind::think(id, agent, &self.kinds[agent.kind], &seen, world, &mut self.rng));
        }
        // A driven creature's blow: it lands part way through the swing on the nearest thing
        // in reach in front of it.
        for (&id, agent) in self.agents.iter_mut().filter(|(_, a)| a.dead.is_none()) {
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
            let reach = agent.body.half + agent.body.long.length() + REACH;
            let struck = seen
                .iter()
                .filter(|s| s.id != id && (s.pos.y - agent.body.pos.y).abs() < 2.5)
                .map(|s| (s, Vec2::new(s.pos.x, s.pos.z) - me))
                .filter(|(s, to)| to.length() - s.reach < reach && (to.length() < 0.3 || to.normalize().dot(ahead) > ARC.cos()))
                .min_by(|a, b| a.1.length().total_cmp(&b.1.length()));
            if let Some((s, to)) = struck {
                let away = to.try_normalize().unwrap_or(ahead);
                hits.push(Hit { on: s.id, by: id, harm: agent.power, push: Vec3::new(away.x * 4.0, 2.5, away.y * 4.0) });
            }
        }
        for hit in hits {
            let Some(agent) = self.agents.get_mut(&hit.on).filter(|a| a.dead.is_none()) else { continue };
            agent.hurt = 0.5;
            agent.doing = None;
            agent.body.vel += hit.push;
            agent.log.struck += 1;
            agent.calm = 0.0;
            agent.health -= hit.harm;
            if agent.health <= 0.0 {
                (agent.health, agent.dead, agent.swing, agent.intent) = (0.0, Some(0.0), None, Intent::default());
                agent.log.died += 1;
                if let Some(killer) = self.agents.get_mut(&hit.by) {
                    killer.log.kills += 1;
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
                    (agent.body.vel, agent.before, agent.dead, agent.health, agent.hurt) = (Vec3::ZERO, agent.body.pos, None, self.kinds[agent.kind].health(), 0.0);
                }
            } else if *lain >= LIES {
                gone.push(id);
            }
        }
        for id in gone {
            if let Some(agent) = self.agents.remove(&id) {
                self.graves.push((agent.kind, agent.mind.home, self.tick));
            }
        }
        let due = (RETURNS / DT) as u64;
        let back: Vec<(usize, Vec2)> = self.graves.iter().filter(|g| self.tick - g.2 >= due).map(|g| (g.0, g.1)).collect();
        self.graves.retain(|g| self.tick - g.2 < due);
        for (kind, home) in back {
            let facing = self.rng.between(0.0, std::f32::consts::TAU);
            let id = self.add(world, kind, Vec3::new(home.x, world.ground_height(home.x, home.y), home.y), facing, Driver::Mind);
            self.agents.get_mut(&id).unwrap().mind.home = home;
        }

        // Bodies out of each other: creatures standing about are pillars, movers share the push.
        let along = |a: &Agent, kinds: &[Kind]| Vec2::new(a.facing.sin(), a.facing.cos()) * kinds[a.kind].shape().1;
        let mut pillars = Vec::new();
        for agent in self.agents.values_mut() {
            agent.body.long = along(agent, &self.kinds);
            if agent.driver == Driver::Stand {
                pillars.push(Pillar { pos: agent.body.pos, half: agent.body.half, height: agent.body.height, long: agent.body.long });
            }
        }
        let mut bodies: Vec<Body> = self.agents.values().filter(|a| a.driver != Driver::Stand).map(|a| a.body).collect();
        body::separate(world, &mut bodies, &pillars);

        for (agent, pushed) in self.agents.values_mut().filter(|a| a.driver != Driver::Stand).zip(bodies) {
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
            let wanted = k.groups * ((land.span as f32 * CELL / 96.0).powi(2).round() as usize).max(1);
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
                let niche = crate::ecology::Niche { name: "", biomes: &[], water: k.water, cover: k.cover, hours: k.hours };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TICKS, mind::State, terrain};
    use cube_voxel::Block;

    fn kind(id: &str, temper: Temper, height: f32, speeds: [f32; 3]) -> Kind {
        Kind { id: id.into(), height, length: height * 0.8, width: height * 0.4, walk: speeds[0], run: speeds[1], sprint: speeds[2], temper, habitat: Biome::Meadow, group: 2, groups: 3, water: Default::default(), cover: Default::default(), hours: Default::default() }
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
    fn a_rabbit_runs_from_a_wolf_and_the_wolf_catches_it() {
        let (world, mut sim) = stage();
        let rabbit = sim.add(&world, 0, at(0.0, 0.0), 0.0, Driver::Mind);
        let wolf = sim.add(&world, 1, at(7.0, 0.0), 0.0, Driver::Mind);
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
        // With a blade, one more is enough; it lies, then is gone, then another lives there.
        sim.agents.get_mut(&person).unwrap().pack.as_mut().unwrap().hold(0);
        sim.agents.get_mut(&person).unwrap().doing = None;
        sim.swing(person);
        run(&mut sim, &world, 1.0);
        assert!(sim.agents[&rabbit].dead.is_some() && sim.agents[&rabbit].act == Act::Die);
        assert_eq!(sim.agents[&person].log.kills, 1);
        run(&mut sim, &world, LIES + 1.0);
        assert!(!sim.agents.contains_key(&rabbit));
        run(&mut sim, &world, RETURNS);
        assert_eq!(sim.agents.values().filter(|a| a.kind == 0).count(), 1);
        // A blow at nothing, or at what is behind, lands on nothing.
        let behind = sim.add(&world, 0, at(0.0, -1.0), 0.0, Driver::Stand);
        sim.agents.get_mut(&person).unwrap().doing = None;
        sim.swing(person);
        run(&mut sim, &world, 1.0);
        assert_eq!(sim.agents[&behind].health, sim.kinds[0].health());
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
        assert_eq!(sim.use_held(person), Used::Dig);
        hand(&mut sim, Item::Berries);
        assert_eq!(sim.use_held(person), Used::Nothing);
        sim.agents.get_mut(&person).unwrap().health = 40.0;
        assert_eq!(sim.use_held(person), Used::Ate);
        let p = &sim.agents[&person];
        assert_eq!((p.health, p.pack.as_ref().unwrap().count(Item::Berries)), (55.0, 5));
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
        let (world, land) = terrain::generate(6, terrain::RADIUS / 2);
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
}
