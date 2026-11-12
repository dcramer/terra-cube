//! `cargo world`: the world and what lives in it, judged with no window. It makes the same
//! world the game does and steps the same simulation, then answers in numbers (`survey`,
//! `run`) or in one picture from above (`map`).

use cube_sim::{
    TICKS,
    life::{Act, Driver, Kind, Sim, Temper},
    survey,
    terrain::{self, Biome, Land, SEA},
};
use cube_sim::scale;
use cube_voxel::{Block, CELL, World, cells};
use glam::{IVec3, Vec3};
use std::path::Path;

const HELP: &str = "\
cargo world survey [--seed N] [--radius CHUNKS]   the land in numbers: ok, or FAIL lines
cargo world run [--seconds 120] [--seed N]        the creatures living in it for a while, with a
                                                  person walking about the start: a line a kind,
                                                  then ok or FAIL lines
cargo world years [N] [--seed N]                  the creatures by the day for N (3) years, nobody
                                                  watching: births, deaths and of what, each kind's
                                                  numbers year by year; FAIL if a kind dies out, overruns,
                                                  or (over 16 years or more) has not settled; under a kind
                                                  that fell by half, who hunted it
cargo world who [--seed N]                        where each group of creatures starts: kind, how
                                                  many, metres east and south of the start, biome
                                                  (for `cargo sandbox --wild --scene F --at X,Z`)
cargo world trunks [--seed N]                     how far what stops a body at a trunk is from the trunk
                                                  that is drawn, a metre up, round every trunk near the start
cargo world cost [--seed N]                       what making the world costs: a region's cells by part
                                                  (ground, what stands, cover) and a chunk's mesh, ms
cargo world dig [--holes 120]                     HOLES (120) holes and stones cut to the 5 cm bit all
                                                  over the land: what broken cells cost, and whether
                                                  creatures still live among them
cargo world grade [FILE] [--seed N]                where the ground is easy, steep and cliff, a line
                                                  for each band of height; with FILE, a picture of
                                                  it from above (green easy, yellow steep, red cliff)
cargo world life [--seed N]                       the ecology: how much of the island suits each
                                                  kind of small life and when it is about; where
                                                  each animal's groups live against what it wants
cargo world find deep|BIOME                       the nearest places to the start with water deep
                                                  enough to swim, or of a biome (for `--at X,Z`)
cargo world map FILE [--seconds 60] [--seed N]    a picture from above, with where each creature
                                                  went (dots: white person, yellow timid, red
                                                  hunter and hostile, orange wary)
The seed and radius default to the game's (cube_sim::terrain::SEED, RADIUS).";

struct Args {
    seed: u32,
    radius: i32,
    seconds: f32,
    file: Option<String>,
}

fn fail(message: &str) -> ! {
    eprintln!("world: {message}");
    std::process::exit(2);
}

/// One line of a check: quiet when it passes, unless something else failed.
struct Checks(Vec<(bool, String)>);

impl Checks {
    fn bar(&mut self, name: &str, value: f32, ok: bool, bar: &str) {
        self.0.push((ok, format!("{} {name} {value:.2} ({bar})", if ok { "ok" } else { "FAIL" })));
    }
    fn finish(self) -> ! {
        let failed = self.0.iter().any(|c| !c.0);
        let verbose = std::env::var_os("WORLD_VERBOSE").is_some();
        for (ok, line) in &self.0 {
            if !ok || verbose {
                println!("{line}");
            }
        }
        if failed {
            std::process::exit(1);
        }
        println!("ok");
        std::process::exit(0);
    }
}

fn kinds() -> Vec<Kind> {
    let kinds = Kind::load(&Path::new("assets").join("creatures"));
    if kinds.is_empty() {
        fail("no creatures under assets/creatures; run from the repository root (cargo world does)");
    }
    kinds
}

/// The world with its creatures and a person at the start who wanders, to be fled and chased.
fn living(args: &Args) -> (World, Land, Sim) {
    let (world, land) = made(args);
    let mut sim = Sim::new(kinds(), args.seed as u64);
    sim.populate(&world, &land);
    // As in the game: only what is near the person at the start is lived, the rest sleeps.
    sim.focus = Some(Vec3::new(0.25, land.height(0, 0) as f32 * CELL, 0.25));
    if let Some(person) = sim.kind("player") {
        sim.add(&world, person, Vec3::new(0.25, land.height(0, 0) as f32 * CELL, 0.25), 0.0, Driver::Mind);
    }
    (world, land, sim)
}

/// The lie of the whole island, and the cells of the part round the start that the game would
/// have made standing there: an island a kilometre across is not held whole in cells.
fn made(args: &Args) -> (World, Land) {
    let land = Land::new(args.seed, args.radius);
    let world = terrain::part(&land, (0, 0), terrain::NEAR.min(args.radius));
    (world, land)
}

fn survey_command(args: &Args) -> ! {
    let (world, land) = made(args);
    let s = survey::survey(&land);
    // (Trees and rocks are drawn as shapes by the game, not by the mesher: counted as it draws.)
    cube_voxel::mesh::MODELLED.store(true, std::sync::atomic::Ordering::Relaxed);
    // What the game draws standing at the start: each chunk meshed at the level its distance
    // gives it, as the game does. (All of the world at full detail is never drawn.)
    let faces_of = |c: IVec3, step: i32| [false, true].map(|liquid| if step == 1 { cube_voxel::mesh::mesh_chunk(&world, c, liquid) } else { cube_voxel::mesh::mesh_chunk_far(&world, c, liquid, step) }.indices.len() / 6).iter().sum::<usize>();
    let (mut drawn, mut near) = (0, 0);
    for &c in world.chunks.keys() {
        let away = ((c.as_vec3() + 0.5) * cube_voxel::CHUNK as f32 * CELL - Vec3::new(0.0, SEA as f32 * CELL, 0.0)).length();
        let faces = faces_of(c, if away < 40.0 { 1 } else if away < 104.0 { 2 } else { 4 });
        drawn += faces;
        near += if away < 40.0 { faces } else { 0 };
    }
    let shares: Vec<String> = Biome::ALL.iter().map(|b| format!("{} {:.0}%", b.name(), s.share[*b as usize] * 100.0)).collect();
    println!("land seed={} {:.0}m across, {:.2}m cells: {} · trees {} · highest {:.0}m · cliffs {:.0}% · reached on foot {:.0}%, swimming {:.0}% · {} chunks, {:.2}M faces drawn from the start ({:.2}M of them within 40 m)", args.seed, (land.span * 2) as f32 * CELL, CELL, shares.join(" "), s.trees, s.highest, s.cliffs * 100.0, s.on_foot * 100.0, s.swimming * 100.0, world.chunks.len(), drawn as f32 / 1e6, near as f32 / 1e6);
    let mut checks = Checks(Vec::new());
    let share = |b: Biome| s.share[b as usize];
    checks.bar("water", share(Biome::Water), (0.2..=0.6).contains(&share(Biome::Water)), "0.20 to 0.60 of the world");
    for biome in [Biome::Meadow, Biome::Forest, Biome::Rock] {
        checks.bar(biome.name(), share(biome), share(biome) >= 0.06, "at least 0.06");
    }
    checks.bar("trees", s.trees as f32, s.trees >= 400, "at least 400");
    // Scale: the bars are in cube_sim::scale, with the sizes they judge.
    println!("scale: a person is {}m · the middle tree {:.1} people tall, the tallest {:.0}m · {} rocks, the largest {:.0} times the smallest · easy ground {:.0}% of lowland meadow and forest", scale::PERSON, s.tree, s.tallest, land.rocks.len(), s.rocks, s.easy * 100.0);
    checks.bar("tree-height", s.tree, s.tree >= scale::BAR_TREE, "the middle tree at least 8 people tall");
    checks.bar("rock-sizes", s.rocks, s.rocks >= scale::BAR_ROCKS, "the largest rock at least 10 times the smallest");
    checks.bar("easy-ground", s.easy, s.easy >= scale::BAR_EASY, "at least 0.65 of lowland meadow and forest within 0.75 m over two");
    checks.bar("highest", s.highest, s.highest >= 40.0, "at least 40 m");
    checks.bar("cliffs", s.cliffs, (0.03..=0.35).contains(&s.cliffs), "0.03 to 0.35 of the land");
    checks.bar("drawn", drawn as f32 / 1e6, drawn <= 2_000_000, "at most 2 million faces from the start, with levels of detail");
    checks.bar("on-foot", s.on_foot, s.on_foot >= 0.5, "at least 0.50 of the land from the start");
    checks.bar("swimming", s.swimming, s.swimming >= 0.9, "at least 0.90 of the land from the start");
    checks.finish()
}

fn run_command(args: &Args) -> ! {
    let (world, land, mut sim) = living(args);
    // WORLD_TRACE=wolf: a line a second for one of that kind, the one that ends up stuck most.
    let traced = std::env::var("WORLD_TRACE").ok().and_then(|id| {
        let mut ahead = Sim::clone(&sim);
        (0..(args.seconds * TICKS as f32) as u32).for_each(|_| ahead.step(&world));
        ahead.agents.iter().filter(|(_, a)| ahead.kinds[a.kind].id == id).max_by(|a, b| a.1.log.stuck.total_cmp(&b.1.log.stuck)).map(|(&id, _)| id)
    });
    let mut last_stuck = 0.0;
    for tick in 0..(args.seconds * TICKS as f32) as u32 {
        sim.step(&world);
        if let Some(a) = traced.filter(|_| tick % TICKS == 0).and_then(|id| sim.agents.get(&id)) {
            println!("t={} pos={:.1},{:.1},{:.1} speed={:.1} {:?} {:?} left={:.1} target={:?} stuck={:.0} asks={:.1},{:.1}x{:.1} hurt={:.1}", tick / TICKS, a.body.pos.x, a.body.pos.y, a.body.pos.z, a.body.ground_speed(), a.mind.state, a.act, a.mind.left, a.mind.target, a.log.stuck, a.intent.dir.x, a.intent.dir.y, a.intent.speed, a.hurt);
            // Caught on something: the ground round it, a row a line, in cells over (+) or
            // under its feet; `^` where there is something solid within its height above that.
            if a.log.stuck > last_stuck {
                let cell = |v: f32| (v / CELL).floor() as i32;
                let (cx, level, cz) = (cell(a.body.pos.x), cell(a.body.pos.y + 0.01), cell(a.body.pos.z));
                let solid = |x: i32, y: i32, z: i32| world.get(IVec3::new(x, y, z)).solid();
                for dz in -3..=3 {
                    let row: Vec<String> = (-3..=3)
                        .map(|dx| {
                            let mut top = level + 3;
                            while top > level - 6 && !solid(cx + dx, top - 1, cz + dz) {
                                top -= 1;
                            }
                            let roofed = (0..(a.body.height / CELL).ceil() as i32 + 2).any(|up| solid(cx + dx, top + up, cz + dz));
                            format!("{:>3}{}", top - level, if roofed { '^' } else { ' ' })
                        })
                        .collect();
                    println!("    {}", row.join(""));
                }
            }
            last_stuck = a.log.stuck;
        }
    }
    let mut checks = Checks(Vec::new());
    println!("life seed={} creatures={} (lived, near the start: {}) seconds={}", args.seed, sim.agents.len(), sim.agents.values().filter(|a| a.awake).count(), args.seconds);
    // Whether the larger animals keep to the trails: the share of them standing on one now,
    // against the share of meadow and forest that is trail at all.
    let big: Vec<_> = sim.agents.values().filter(|a| a.awake && a.dead.is_none() && sim.kinds[a.kind].height > 0.5 && sim.kinds[a.kind].temper != cube_sim::life::Temper::Person).collect();
    let on = big.iter().filter(|a| land.trail((a.body.pos.x / CELL).floor() as i32, (a.body.pos.z / CELL).floor() as i32) > 0.0).count();
    let (mut ground, mut trail) = (0, 0);
    for z in (-land.span..land.span).step_by(7) {
        for x in (-land.span..land.span).step_by(7) {
            if matches!(land.biome(x, z), Biome::Meadow | Biome::Forest) {
                ground += 1;
                trail += (land.trail(x, z) > 0.0) as usize;
            }
        }
    }
    println!("trails: {} of {} larger animals on one ({:.0}%) · trails are {:.1}% of meadow and forest", on, big.len(), on as f32 * 100.0 / big.len().max(1) as f32, trail as f32 * 100.0 / ground.max(1) as f32);
    println!("grazed: {} tufts, flowers and ferns eaten by what grazed in sight", sim.eaten.len());
    // How alive it is, in one line: of the creatures lived near the watcher (not the watcher),
    // how much of their time was standing, and what happened among them a minute.
    {
        let lived: Vec<_> = sim.agents.values().filter(|a| a.driver == cube_sim::life::Driver::Mind && a.log.acts.iter().sum::<f32>() > args.seconds * 0.5).collect();
        let (time, minutes) = (lived.iter().map(|a| a.log.acts.iter().sum::<f32>()).sum::<f32>().max(1.0), args.seconds / 60.0);
        let share = |act: Act| lived.iter().map(|a| a.log.acts[act as usize]).sum::<f32>() / time;
        let count = |f: &dyn Fn(&cube_sim::life::Log) -> u32| lived.iter().map(|a| f(&a.log)).sum::<u32>() as f32;
        let lain = lived.iter().map(|a| a.log.lain).sum::<f32>() / time;
        let (idle, fast) = (share(Act::Idle) - lain, share(Act::Run) + share(Act::Sprint));
        let (drank, hunts, kills, fled, trips) = (count(&|l| l.drank), count(&|l| l.hunts), count(&|l| l.kills), count(&|l| l.fled), count(&|l| l.trips));
        println!("alive: {} lived · standing about {:.0}% of the time, lying up {:.0}%, feeding {:.0}%, running {:.1}% · a minute: {:.1} set out for somewhere, {:.1} drank, {:.1} took fright, {:.1} hunts ({:.0} kills in all)", lived.len(), idle * 100.0, lain * 100.0, share(Act::Eat) * 100.0, fast * 100.0, trips / minutes, drank / minutes, fled / minutes, hunts / minutes, kills);
        if args.seconds >= 240.0 {
            checks.bar("alive standing", idle, idle <= 0.35, "in their hours, creatures stand about at most a third of the time");
            checks.bar("alive errands", trips / minutes, (2.0..=60.0).contains(&(trips / minutes)), "from two to sixty a minute set out for somewhere with a reason (more is the same errand begun over and over)");
        }
    }
    for (k, kind) in sim.kinds.iter().enumerate() {
        // Of those that were lived at all: the rest of the island slept.
        let of: Vec<_> = sim.agents.values().filter(|a| a.kind == k && a.log.acts.iter().sum::<f32>() > args.seconds * 0.5).collect();
        if of.is_empty() {
            continue;
        }
        let n = of.len() as f32;
        let total = n * args.seconds;
        let sum = |f: &dyn Fn(&cube_sim::life::Log) -> f32| of.iter().map(|a| f(&a.log)).sum::<f32>();
        let mut acts: Vec<(Act, f32)> = Act::ALL.iter().map(|&act| (act, sum(&|l| l.acts[act as usize]) / total)).filter(|a| a.1 >= 0.005).collect();
        acts.sort_by(|a, b| b.1.total_cmp(&a.1));
        let acts: Vec<String> = acts.iter().map(|(act, part)| format!("{} {:.0}%", act.clip().0, part * 100.0)).collect();
        let (moved, stuck, swim, boxed) = (sum(&|l| l.moved) / n, sum(&|l| l.stuck) / total, sum(&|l| l.acts[Act::Swim as usize]) / total, sum(&|l| l.boxed) / total);
        let (inside, fell) = (sum(&|l| l.inside as f32), sum(&|l| l.fell as f32));
        println!("{} ×{}: went {:.0}m each · {} · fled {} · hit {} · drank {} · set out {} · hunts {}", kind.id, of.len(), moved, acts.join(" "), sum(&|l| l.fled as f32), sum(&|l| l.struck as f32), sum(&|l| l.drank as f32), sum(&|l| l.trips as f32), sum(&|l| l.hunts as f32));
        checks.bar(&format!("{} inside-ground", kind.id), inside, inside == 0.0, "0 ticks");
        checks.bar(&format!("{} fell-out", kind.id), fell, fell == 0.0, "0 times");
        checks.bar(&format!("{} stuck", kind.id), stuck, stuck <= 0.08, "at most 0.08 of the time");
        checks.bar(&format!("{} boxed-in", kind.id), boxed, boxed <= 0.1, "at most 0.10 of the time");
        checks.bar(&format!("{} swimming", kind.id), swim, swim <= 0.05, "at most 0.05 of the time");
        if args.seconds >= 60.0 {
            checks.bar(&format!("{} went", kind.id), moved, moved >= 3.0, "at least 3 m each");
        }
    }
    checks.finish()
}

/// The top block of a column and how high it is, looking down from the sky.
fn top(world: &World, x: i32, z: i32) -> (Block, i32) {
    let mut y = SEA + cells(55.0);
    while y > 0 && world.get(IVec3::new(x, y, z)) == Block::Air {
        y -= 1;
    }
    (world.get(IVec3::new(x, y, z)), y)
}

fn map_command(args: &Args) -> ! {
    let Some(file) = &args.file else { fail("map wants a file: cargo world map shots/map.png") };
    let (world, land, mut sim) = living(args);
    let side = (land.span * 2) as u32;
    // A pixel is a quarter of a metre.
    let scale = (CELL / 0.25).round().max(1.0) as u32;
    let mut picture = image::RgbImage::new(side * scale, side * scale);
    let tone = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
    for z in 0..side as i32 {
        for x in 0..side as i32 {
            let (cx, cz) = (x - land.span, z - land.span);
            let (block, y) = top(&world, cx, cz);
            let (_, y_lit) = top(&world, cx - 1, cz - 1);
            // Lit from the north-west: a rise toward the light is bright, away from it dark.
            let shade = (1.0 + (y - y_lit) as f32 * 0.12).clamp(0.55, 1.5) * (0.8 + (y - SEA) as f32 * CELL * 0.012);
            let mut colour = block.colors().0;
            if block == Block::Water {
                let depth = (SEA - land.height(cx, cz)) as f32 * CELL * 2.0;
                colour = [0.10 - depth * 0.006, 0.34 - depth * 0.018, 0.50 - depth * 0.02];
            }
            let shade = if block == Block::Water { 1.0 } else { shade };
            let pixel = image::Rgb(colour.map(|c| tone(c * shade * 1.6)));
            for (dx, dz) in (0..scale).flat_map(|dx| (0..scale).map(move |dz| (dx, dz))) {
                picture.put_pixel(x as u32 * scale + dx, z as u32 * scale + dz, pixel);
            }
        }
    }
    let mut dot = |pos: Vec3, colour: [u8; 3], size: i32| {
        let (px, pz) = (((pos.x / CELL) + land.span as f32) * scale as f32, ((pos.z / CELL) + land.span as f32) * scale as f32);
        for dz in -size..=size {
            for dx in -size..=size {
                let (x, z) = (px as i32 + dx, pz as i32 + dz);
                if x >= 0 && z >= 0 && (x as u32) < side * scale && (z as u32) < side * scale {
                    picture.put_pixel(x as u32, z as u32, image::Rgb(colour));
                }
            }
        }
    };
    let colour = |temper: Temper| match temper {
        Temper::Person => [255, 255, 255],
        Temper::Timid => [255, 230, 60],
        Temper::Wary => [255, 140, 30],
        Temper::Hunter | Temper::Hostile => [230, 30, 40],
    };
    // Where each went, a fine dot every half second, then where it ended, a fat one.
    for tick in 0..(args.seconds * TICKS as f32) as u32 {
        sim.step(&world);
        if tick % (TICKS / 2) == 0 {
            for agent in sim.agents.values() {
                dot(agent.body.pos, colour(sim.kinds[agent.kind].temper).map(|c| c / 5 * 4), 0);
            }
        }
    }
    for agent in sim.agents.values() {
        dot(agent.body.pos, [0, 0, 0], 3);
        dot(agent.body.pos, colour(sim.kinds[agent.kind].temper), 2);
    }
    if let Some(dir) = Path::new(file).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = picture.save(file) {
        fail(&format!("saving {file}: {e}"));
    }
    println!("map {file} seed={} {}px a side, {:.0}m across, creatures={} after {}s", args.seed, side * scale, side as f32 * CELL, sim.agents.len(), args.seconds);
    std::process::exit(0);
}

/// Holes dug all over the land, to the bit, and what they cost: the cells that broke, the
/// memory for them, the faces added, how long a chunk takes to draw again, and whether the
/// creatures still live without ending up in the ground.
/// How far what stops a body is from what is drawn, at the trunks near the start. A metre
/// above each trunk's foot, from sixteen sides: how far out the first solid point is (what a
/// body meets) and how far out the first cube of the trunk as the game draws it near (cubes
/// of 10 cm, or 15 for a great trunk, of the same shape, `Limb::fill`). The gap is the one
/// less the other: more than nothing, a body is stopped by air; less, it walks into wood.
fn trunks_command(args: &Args) -> ! {
    let land = Land::new(args.seed, args.radius);
    let mut world = World::default();
    let r = cube_sim::terrain::REGION;
    let ((), form) = cube_sim::plants::recorded(|| {
        for rz in -2..2 {
            for rx in -2..2 {
                cube_sim::terrain::fill(&mut world, &land, (rx * r, rz * r), ((rx + 1) * r, (rz + 1) * r));
            }
        }
    });
    let mut gaps: Vec<f32> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // (Not the trunks at the edge of what was made: the part of one that stands in ground
    // not made is not there.)
    let inside = (2 * r) as f32 * CELL - 4.0;
    for limb in form.limbs.iter().filter(|l| l.roots > 0 && l.a.x.abs() < inside && l.a.z.abs() < inside && seen.insert((l.a.x.to_bits(), l.a.z.to_bits()))) {
        // What is drawn of the whole tree there: every limb of it that comes near, each at
        // the size the game draws a limb of its thickness near the eye.
        let up = limb.a.y + cube_sim::plants::Limb::UNDER + 1.0;
        let thick = limb.ra.max(limb.rb);
        let mut drawn = [std::collections::HashSet::new(), std::collections::HashSet::new()];
        for other in form.limbs.iter().filter(|o| o.ra.max(o.rb) >= 0.16 && (o.a - limb.a).with_y(0.0).length() < thick + 0.3 && o.a.y.min(o.b.y) < up + 0.3 && o.a.y.max(o.b.y) > up - 0.3) {
            let wide = other.ra.max(other.rb) > 0.5;
            other.fill_span(if wide { 0.15 } else { 0.1 }, None, (up - 0.3, up + 0.3), |c| {
                drawn[wide as usize].insert(c);
            });
        }
        let axis = limb.a.lerp(limb.b, ((up - limb.a.y) / (limb.b.y - limb.a.y).max(0.01)).clamp(0.0, 1.0));
        for k in 0..16 {
            let way = Vec3::new((k as f32 * std::f32::consts::TAU / 16.0).cos(), 0.0, (k as f32 * std::f32::consts::TAU / 16.0).sin());
            // (From a little way out, and only this trunk's own wood: a rock or the next
            // tree in the way is not this trunk's collision.)
            let out = thick * 5.0 + 0.5;
            let first = |hit: &dyn Fn(Vec3) -> bool| (0..(out * 100.0) as i32).map(|i| out - i as f32 * 0.01).find(|d| hit(Vec3::new(axis.x, up, axis.z) + way * *d));
            let (stopped, shown) = (first(&|p| world.bit((p / cube_voxel::BIT).floor().as_ivec3()) == limb.bark), first(&|p| drawn[0].contains(&(p / 0.1).floor().as_ivec3()) || drawn[1].contains(&(p / 0.15).floor().as_ivec3())));
            if let (Some(stopped), Some(shown)) = (stopped, shown) {
                gaps.push(stopped - shown);
            }
        }
    }
    // And what lies on the ground, from above: how far the top a body stands on is from the
    // top that is drawn, along every fallen trunk. (A body was seen to sink into one.)
    let mut tops: Vec<f32> = Vec::new();
    for limb in form.limbs.iter().filter(|l| l.roots == 0 && l.ra.max(l.rb) >= 0.2 && (l.b.y - l.a.y).abs() < 0.3 * l.a.distance(l.b) && l.a.x.abs() < inside && l.a.z.abs() < inside && l.a.y.min(l.b.y) < land.height((l.a.x / CELL).floor() as i32, (l.a.z / CELL).floor() as i32) as f32 * CELL + 1.5) {
        let g = limb.near();
        let mut drawn = std::collections::HashSet::new();
        limb.fill_span(g, None, (f32::MIN, f32::MAX), |c| {
            drawn.insert(c);
        });
        for part in [0.25, 0.5, 0.75] {
            let p = limb.a.lerp(limb.b, part);
            let from = p.y + limb.ra + 0.5;
            let first = |hit: &dyn Fn(Vec3) -> bool| (0..((limb.ra * 2.0 + 0.6) * 100.0) as i32).map(|i| from - i as f32 * 0.01).find(|y| hit(Vec3::new(p.x, *y, p.z)));
            if let (Some(stood), Some(shown)) = (first(&|q| world.bit((q / cube_voxel::BIT).floor().as_ivec3()) == limb.bark), first(&|q| drawn.contains(&(q / g).floor().as_ivec3()))) {
                tops.push(shown - stood);
            }
        }
    }
    tops.sort_by(f32::total_cmp);
    gaps.sort_by(f32::total_cmp);
    let mut checks = Checks(Vec::new());
    if !tops.is_empty() {
        // (All but the place in ten where other wood stands over it: its own bough, the stump beside it.)
        let (sunk, over) = (tops[tops.len() - 1] * 100.0, tops[tops.len() / 10] * 100.0);
        println!("logs: {} places on {} fallen trunks · a body stands from {:.0} cm above the wood that is drawn to {:.0} cm down in it", tops.len(), tops.len() / 3, -over, sunk);
        checks.bar("logs sunk", sunk, sunk < 8.0, "a body sinks no more than 8 cm into a fallen trunk");
        checks.bar("logs over", -over, -over < 8.0, "nor stands more than 8 cm over one");
    }
    if gaps.is_empty() {
        checks.bar("trunks", 0.0, false, "some trunks near the start");
        checks.finish()
    }
    let (n, at) = (gaps.len(), |part: f32| gaps[((gaps.len() - 1) as f32 * part) as usize] * 100.0);
    let mut off: Vec<f32> = gaps.iter().map(|g| g.abs()).collect();
    off.sort_by(f32::total_cmp);
    let mean = off[off.len() / 2] * 100.0;
    println!("trunks: {} sides of {} trunks · what stops a body is {mean:.1} cm from what is drawn, at the middle · from {:.0} cm inside it to {:.0} cm outside (the 5th and 95th of a hundred)", n, n / 16, at(0.05), at(0.95));
    checks.bar("trunks off", mean, mean < 4.0, "under 4 cm at the middle");
    checks.bar("trunks inside", at(0.05), at(0.05) > -8.0, "a body goes no more than 8 cm into wood that is drawn");
    checks.bar("trunks outside", at(0.95), at(0.95) < 8.0, "a body is stopped no more than 8 cm short of it");
    checks.finish()
}

/// What making the world costs, on one thread: the sixteen regions round the start made one
/// at a time and every chunk of them meshed, as the game does it.
fn cost_command(args: &Args) -> ! {
    let land = Land::new(args.seed, args.radius);
    let mut world = World::default();
    let r = cube_sim::terrain::REGION;
    cube_sim::terrain::fill_cost();
    let started = std::time::Instant::now();
    for rz in -2..2 {
        for rx in -2..2 {
            cube_sim::terrain::fill(&mut world, &land, (rx * r, rz * r), ((rx + 1) * r, (rz + 1) * r));
        }
    }
    let (all, parts) = (started.elapsed().as_secs_f64() * 1000.0 / 16.0, cube_sim::terrain::fill_cost().map(|s| s * 1000.0 / 16.0));
    let chunks: Vec<IVec3> = world.chunks.keys().copied().collect();
    let started = std::time::Instant::now();
    let faces: usize = chunks.iter().map(|&c| [false, true].map(|liquid| cube_voxel::mesh::mesh_chunk(&world, c, liquid).indices.len() / 6).iter().sum::<usize>()).sum();
    let mesh = started.elapsed().as_secs_f64() * 1000.0 / chunks.len().max(1) as f64;
    println!("cost: a region's cells {all:.0} ms (ground {:.0}, what stands {:.0} of which wood {:.0} and leaf {:.0}, cover {:.0}) · a chunk's mesh {mesh:.2} ms ({} chunks, {:.2}M faces) · {} cells broken to bits, {:.1} MB", parts[0], parts[1], parts[3], parts[4], parts[2], chunks.len(), faces as f64 / 1e6, world.broken(), world.broken() as f64 * 141.0 / 1e6);
    std::process::exit(0)
}

fn dig_command(args: &Args) -> ! {
    let (mut world, land, mut sim) = living(args);
    let faces = |world: &World, chunk: IVec3| [false, true].map(|liquid| cube_voxel::mesh::mesh_chunk(world, chunk, liquid).indices.len() / 6).iter().sum::<usize>();
    let before: usize = world.chunks.keys().map(|&c| faces(&world, c)).sum();
    let holes = args.seconds as usize;
    let mut rng = cube_sim::rng::Rng::new(args.seed as u64 + 99);
    let (mut dug, mut touched) = (0, std::collections::HashSet::new());
    let started = std::time::Instant::now();
    while dug < holes {
        let (x, z) = (rng.below(land.span as usize * 2) as i32 - land.span, rng.below(land.span as usize * 2) as i32 - land.span);
        if land.biome(x, z) == Biome::Water {
            continue;
        }
        let across = rng.between(0.3, 1.6);
        let middle = Vec3::new(x as f32, land.height(x, z) as f32, z as f32) * CELL;
        // Every fourth is a stone built up instead, unless that would bury a creature.
        let clear = sim.agents.values().all(|a| a.body.pos.distance(middle) > across + 1.0);
        world.ball(middle, across / 2.0, if dug % 4 == 3 && clear { Block::Stone } else { Block::Air });
        touched.insert((middle / CELL).floor().as_ivec3().div_euclid(IVec3::splat(cube_voxel::CHUNK)));
        dug += 1;
    }
    let digging = started.elapsed().as_secs_f32() * 1000.0 / holes as f32;
    let started = std::time::Instant::now();
    let redrawn: usize = touched.iter().map(|&c| faces(&world, c)).sum();
    let remesh = started.elapsed().as_secs_f32() * 1000.0 / touched.len().max(1) as f32;
    let after: usize = world.chunks.keys().map(|&c| faces(&world, c)).sum();
    for _ in 0..120 * TICKS {
        sim.step(&world);
    }
    let (inside, fell): (u32, u32) = sim.agents.values().fold((0, 0), |(i, f), a| (i + a.log.inside, f + a.log.fell));
    let stuck = sim.agents.values().map(|a| a.log.stuck).sum::<f32>() / (sim.agents.len() as f32 * 120.0);
    println!(
        "dug {holes} holes and stones 0.3 to 1.6 m across: {} cells broke ({:.1} a hole, {:.1} MB) · faces {:.2}M to {:.2}M · {digging:.2} ms a hole, {remesh:.1} ms to draw a changed chunk again ({} faces)",
        world.broken(),
        world.broken() as f32 / holes as f32,
        world.broken() as f32 * 141.0 / 1e6,
        before as f32 / 1e6,
        after as f32 / 1e6,
        redrawn / touched.len().max(1)
    );
    let mut checks = Checks(Vec::new());
    checks.bar("inside-ground", inside as f32, inside == 0, "0 ticks, over two minutes of the creatures living among them");
    checks.bar("fell-out", fell as f32, fell == 0, "0 times");
    checks.bar("stuck", stuck, stuck <= 0.08, "at most 0.08 of the time");
    checks.bar("remesh", remesh, remesh <= 8.0, "at most 8 ms a chunk, half a frame");
    checks.finish()
}

/// Where the ground is easy, steep and cliff: a line for each band of height, and with a
/// file, a picture from above (green easy, yellow steep, red cliff).
fn grade_command(args: &Args) -> ! {
    let land = Land::new(args.seed, args.radius);
    let bands = [4.0, 8.0, 12.0, scale::CONIFER_LINE, scale::ROCK_LINE, scale::SNOW_LINE, f32::MAX];
    let mut from = 0.0;
    for (top, (n, [easy, steep, cliff])) in bands.iter().zip(survey::grades(&land, &bands)) {
        let name = if *top == f32::MAX { format!("over {from:.0}m") } else { format!("{from:.0} to {top:.0}m") };
        println!("{name}: {:.0}% of dry land · easy {:.0}% steep {:.0}% cliff {:.0}%", n as f32 / ((land.span * land.span * 4) as f32 * (1.0 - survey::survey(&land).share[0])) * 100.0, easy * 100.0, steep * 100.0, cliff * 100.0);
        from = *top;
    }
    if let Some(file) = &args.file {
        let side = (land.span * 2) as u32;
        let mut picture = image::RgbImage::new(side, side);
        for (x, z, pixel) in picture.enumerate_pixels_mut() {
            let (cx, cz) = (x as i32 - land.span, z as i32 - land.span);
            let lift = (((land.height(cx, cz) - SEA) as f32 * CELL) * 1.6).clamp(0.0, 110.0) as u8;
            *pixel = image::Rgb(match (land.biome(cx, cz), survey::grade(&land, cx, cz)) {
                (Biome::Water, _) => [20, 40, 90],
                (_, 0) => [lift / 2, 110 + lift, lift / 2],
                (_, 1) => [225, 200, 60],
                _ => [220, 50, 40],
            });
        }
        if let Err(e) = picture.save(file) {
            fail(&format!("could not write {file}: {e}"));
        }
        println!("{file}");
    }
    std::process::exit(0)
}

/// The ecology in numbers: for each kind of small life, how much of the island suits it and
/// how much it is about at four hours; then for each animal, where its groups ended up
/// against what it asks for. A kind that fits nowhere, or an animal far from what it wants,
/// is a line that says so.
fn life_command(args: &Args) -> ! {
    use cube_sim::ecology::{Ecology, SMALL_LIFE};
    let (_, land, sim) = living(args);
    let eco = Ecology::new(&land);
    let span = land.span as f32 * CELL;
    let places: Vec<_> = (0..(span as i32)).flat_map(|z| (0..(span as i32)).map(move |x| (x, z))).map(|(x, z)| eco.place(&land, x as f32 * 2.0 - span, z as f32 * 2.0 - span)).collect();
    let mut checks = Checks(Vec::new());
    for niche in SMALL_LIFE {
        let home = places.iter().filter(|p| niche.suits(p) > 0.5).count() as f32 / places.len() as f32;
        let hours: Vec<String> = [6.0, 12.0, 18.0, 23.0].iter().map(|h| format!("{:.0}h {:.0}%", h, niche.about(cube_sim::sky::at(3, *h)) * 100.0)).collect();
        println!("{:<13} at home on {:>4.1}% of the island · about: {}", niche.name, home * 100.0, hours.join(" "));
        checks.bar(niche.name, home, home >= 0.005, "somewhere to live: at least 0.5% of the island");
    }
    for (i, kind) in sim.kinds.iter().enumerate() {
        let homes: Vec<_> = sim.agents.values().filter(|a| a.kind == i).map(|a| eco.place(&land, a.mind.home.x, a.mind.home.y)).collect();
        if homes.is_empty() || kind.id == "player" {
            continue;
        }
        let mean = |f: fn(&cube_sim::ecology::Place) -> f32| homes.iter().map(f).sum::<f32>() / homes.len() as f32;
        println!("{:<13} ×{:<3} {:?} · water {:?}: {:.0}m off · cover {:?}: {:.0}% · hours {:?}", kind.id, homes.len(), kind.habitat, kind.water, mean(|p| p.water), kind.cover, mean(|p| p.cover) * 100.0, kind.hours);
    }
    checks.finish()
}

fn main() {
    let mut it = std::env::args().skip(1);
    let command = it.next().unwrap_or_default();
    let mut args = Args { seed: terrain::SEED, radius: terrain::RADIUS, seconds: if command == "map" { 60.0 } else { 120.0 }, file: None };
    while let Some(a) = it.next() {
        let mut number = |name: &str| it.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or_else(|| fail(&format!("{name} wants a number")));
        match a.as_str() {
            "--seed" => args.seed = number("--seed") as u32,
            "--radius" => args.radius = number("--radius") as i32,
            "--seconds" | "--holes" => args.seconds = number(&a),
            other if !other.starts_with('-') && args.file.is_none() => args.file = Some(other.into()),
            other => fail(&format!("unknown argument {other}\n{HELP}")),
        }
    }
    match command.as_str() {
        "survey" => survey_command(&args),
        "run" => run_command(&args),
        "map" => map_command(&args),
        "dig" => dig_command(&args),
        "cost" => cost_command(&args),
        "trunks" => trunks_command(&args),
        "grade" => grade_command(&args),
        "life" => life_command(&args),
        "drawn" => {
            // How far the ground that is drawn is from the ground a body stands on: the cells
            // are what stops a body, and they are drawn as blocks of other sizes at heights
            // of their own (`mesh::eased_under`). Over the land round the start, column by
            // column; and from each column to the next, how high the step is that is drawn
            // against the step a body takes.
            let land = Land::new(args.seed, args.radius);
            let mut world = World::default();
            let r = cube_sim::terrain::REGION;
            for rz in -2..2 {
                for rx in -2..2 {
                    cube_sim::terrain::fill(&mut world, &land, (rx * r, rz * r), ((rx + 1) * r, (rz + 1) * r));
                }
            }
            let reach = 2 * r - 16;
            let drawn = |x: i32, z: i32| {
                let top = land.height(x, z);
                cube_voxel::mesh::eased_under(&world, Vec3::new((x as f32 + 0.5) * CELL, top as f32 * CELL, (z as f32 + 0.5) * CELL)).map(|y| (y, top as f32 * CELL))
            };
            let (mut off, mut steps, mut stood) = (Vec::new(), Vec::new(), Vec::new());
            for z in (-reach..reach).step_by(3) {
                for x in (-reach..reach).step_by(3) {
                    let (Some(here), Some(next)) = (drawn(x, z), drawn(x + 1, z)) else { continue };
                    off.push((here.0 - here.1).abs());
                    steps.push((next.0 - here.0).abs());
                    stood.push((next.1 - here.1).abs());
                }
            }
            if off.is_empty() {
                fail("no ground round the start to measure");
            }
            for all in [&mut off, &mut steps, &mut stood] {
                all.sort_by(f32::total_cmp);
            }
            let at = |all: &Vec<f32>, part: f32| all[((all.len() - 1) as f32 * part) as usize] * 100.0;
            println!("drawn: {} columns · the ground is drawn {:.0} cm from where a body stands on it at the middle, {:.0} cm at the 95th of a hundred, {:.0} cm at the worst", off.len(), at(&off, 0.5), at(&off, 0.95), at(&off, 1.0));
            println!("steps from a column to the next: drawn {:.0} cm at the 95th and {:.0} cm at the worst, the cells' {:.0} cm and {:.0} cm", at(&steps, 0.95), at(&steps, 1.0), at(&stood, 0.95), at(&stood, 1.0));
            // And where a body comes to rest, let fall on each: against the highest ground
            // drawn under any part of it, which is what it should be standing on.
            let (mut rest, mut sunk) = (Vec::new(), 0.0f32);
            let tuning = cube_sim::body::Tuning::default();
            for z in (-reach..reach).step_by(7) {
                for x in (-reach..reach).step_by(7) {
                    let Some(here) = drawn(x, z) else { continue };
                    if land.height(x, z) <= SEA {
                        continue;
                    }
                    let mut body = cube_sim::body::Body::new(Vec3::new((x as f32 + 0.5) * CELL, here.0.max(here.1) + 1.0, (z as f32 + 0.5) * CELL), 0.3, 1.75);
                    for _ in 0..90 {
                        body.step(&world, cube_sim::body::Intent::default(), &tuning);
                    }
                    // (Only where nothing but ground is under it: a trunk or a rock is not this measure's.)
                    let under: Vec<Option<(f32, f32)>> = (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (dx, dz))).filter(|(dx, dz)| ((*dx as f32 * CELL).abs() - CELL / 2.0 < 0.3) && ((*dz as f32 * CELL).abs() - CELL / 2.0 < 0.3)).map(|(dx, dz)| drawn(x + dx, z + dz)).collect();
                    if !body.grounded || under.iter().any(|u| u.is_none()) || (-2..=2).any(|dz| (-2..=2).any(|dx| (0..8).any(|dy| matches!(world.get(IVec3::new(x + dx, land.height(x + dx, z + dz) + dy, z + dz)), cube_voxel::Block::Wood | cube_voxel::Block::Birch | cube_voxel::Block::Rock | cube_voxel::Block::Broken)))) {
                        continue;
                    }
                    let highest = under.iter().flatten().map(|u| u.0).fold(f32::MIN, f32::max);
                    rest.push((body.pos.y - highest).abs());
                    sunk = sunk.max(here.0 - body.pos.y);
                }
            }
            rest.sort_by(f32::total_cmp);
            if rest.is_empty() {
                fail("no body came to rest on open ground");
            }
            println!("a body let fall on {} places rests {:.0} cm from the highest ground drawn under it at the 95th, {:.0} cm at the 99th, {:.0} cm at the worst ({} of them over 2 cm); under the ground drawn at its middle by {:.0} cm at the worst", rest.len(), at(&rest, 0.95), at(&rest, 0.99), at(&rest, 1.0), rest.iter().filter(|r| **r > 0.02).count(), sunk * 100.0);
            let mut checks = Checks(Vec::new());
            checks.bar("rests on what is drawn", at(&rest, 0.95), at(&rest, 0.95) < 1.0, "within a centimetre at the 95th place of a hundred");
            checks.bar("in the drawn ground", sunk * 100.0, sunk * 100.0 < 1.0, "never under the ground drawn at its middle");
            checks.finish()
        }
        "find" => {
            // The nearest places to the start of a kind: `deep` water to swim in, or a biome.
            let what = args.file.clone().unwrap_or_default();
            let land = Land::new(args.seed, args.radius);
            let mut found = Vec::new();
            for ring in 1..(land.span as f32 * CELL / 4.0) as i32 {
                for turn in 0..128 {
                    let (out, way) = (ring as f32 * 4.0, turn as f32 * std::f32::consts::TAU / 128.0);
                    let (x, z) = (way.cos() * out, way.sin() * out);
                    let (cx, cz) = ((x / CELL).floor() as i32, (z / CELL).floor() as i32);
                    let depth = (SEA - land.height(cx, cz)) as f32 * CELL;
                    // Deep, and open water for ten metres each way: somewhere to swim, not a pothole.
                    let open = [(10.0, 0.0), (-10.0, 0.0), (0.0, 10.0), (0.0, -10.0)].iter().all(|(dx, dz)| (SEA - land.height(((x + dx) / CELL).floor() as i32, ((z + dz) / CELL).floor() as i32)) as f32 * CELL > 1.2);
                    // A hillside to walk up: open meadow that climbs a fifth to a half of a metre
                    // in every metre for ten, some way; the way is given as the `--yaw` that walks up it.
                    let up = |yaw: f32| (0..10).all(|i| {
                        let h = |d: f32| (land.height(((x - yaw.sin() * d) / CELL).floor() as i32, ((z - yaw.cos() * d) / CELL).floor() as i32), land.biome(((x - yaw.sin() * d) / CELL).floor() as i32, ((z - yaw.cos() * d) / CELL).floor() as i32));
                        let (a, b) = (h(i as f32), h(i as f32 + 1.0));
                        b.1 == Biome::Meadow && (0.2..=0.5).contains(&((b.0 - a.0) as f32 * CELL))
                    });
                    let climb = (what == "slope").then(|| (0..16).map(|k| k as f32 * std::f32::consts::TAU / 16.0).find(|yaw| up(*yaw))).flatten();
                    let depth = climb.map_or(depth, |yaw| -100.0 - yaw);
                    let hit = if what == "slope" { climb.is_some() && land.trees.iter().all(|t| ((t.x - cx) as f32).hypot((t.z - cz) as f32) * CELL > 9.0) } else if what == "deep" { depth > 2.0 && open } else if what == "trail" {
                        // A stretch in the open, to be seen: meadow, and no tree within seven metres.
                        land.trail(cx, cz) > 0.8 && land.biome(cx, cz) == Biome::Meadow && land.trees.iter().all(|t| ((t.x - cx) as f32).hypot((t.z - cz) as f32) * CELL > 7.0)
                    } else { Biome::named(&what) == Some(land.biome(cx, cz)) };
                    if hit && found.len() < 3 && found.iter().all(|f: &(f32, f32, f32)| (f.0 - x).hypot(f.1 - z) > 30.0) {
                        found.push((x, z, depth));
                    }
                }
            }
            if found.is_empty() {
                fail(&format!("no `{what}` on this island (deep, trail, slope, or one of water shore meadow forest rock snow)"));
            }
            for (x, z, depth) in found {
                println!("{what} at {x:.0},{z:.0} · {:.0}m from the start{}", x.hypot(z), if depth <= -100.0 { format!(" · up it is --yaw {:.2}", -depth - 100.0) } else if depth > 0.0 { format!(" · {depth:.1}m of water") } else { String::new() });
            }
        }
        "years" => {
            // The island's creatures by the day and nothing else (`Sim::day`), for years: who
            // is born, who dies and of what, and whether the numbers hold.
            let years: usize = args.file.as_deref().and_then(|v| v.parse().ok()).unwrap_or(3);
            let (world, land, mut sim) = living(&args);
            // Nobody is watching: nothing is lived by the second.
            sim.agents.retain(|_, a| sim.kinds[a.kind].temper != cube_sim::life::Temper::Person);
            sim.agents.values_mut().for_each(|a| a.awake = false);
            let count = |sim: &Sim| {
                let mut n = vec![0usize; sim.kinds.len()];
                sim.agents.values().for_each(|a| n[a.kind] += 1);
                n
            };
            let mut rows = vec![count(&sim)];
            let mut all = cube_sim::life::Tally::default();
            let mut kills: std::collections::BTreeMap<(usize, usize), u32> = Default::default();
            let mut grazing = (0.0f32, 1.0f32, 0.0f32);
            // From the first day of a spring, so a year is a year.
            sim.when = sim.when.floor() - sim.when.floor().rem_euclid(cube_sim::sky::YEAR) + cube_sim::sky::YEAR;
            for _ in 0..years {
                for _ in 0..cube_sim::sky::YEAR as usize {
                    let t = sim.day(&world, Some(&land));
                    (all.born, all.old, all.hunted, all.starved, all.winter, all.moved) = (all.born + t.born, all.old + t.old, all.hunted + t.hunted, all.starved + t.starved, all.winter + t.winter, all.moved + t.moved);
                    grazing = (grazing.0 + t.forage.0, grazing.1.min(t.forage.1), grazing.2 + 1.0);
                    for pair in t.taken {
                        *kills.entry(pair).or_default() += 1;
                    }
                    sim.when += 1.0;
                }
                rows.push(count(&sim));
            }
            let total = |row: &[usize]| row.iter().sum::<usize>();
            println!("years {years} seed={}: {} creatures at the start, {} at the end · born {} · died of age {}, hunted {}, starved {}, of winter {} · groups that moved {}", args.seed, total(&rows[0]), total(&rows[years]), all.born, all.old, all.hunted, all.starved, all.winter, all.moved);
            println!("  grazing: {:.0}% left on the ground that is grazed, on the whole; {:.0}% on the barest piece at its barest", grazing.0 / grazing.2.max(1.0) * 100.0, grazing.1 * 100.0);
            let mut checks = Checks(Vec::new());
            for (k, kind) in sim.kinds.iter().enumerate() {
                if rows[0][k] == 0 {
                    continue;
                }
                let line: Vec<String> = rows.iter().map(|r| r[k].to_string()).collect();
                println!("  {:<12} {:<7} {}", kind.id, format!("{:?}", kind.diet).to_lowercase(), line.join(" > "));
                // What has fallen to under half: who took it.
                if rows[years][k] * 2 < rows[0][k] {
                    let mut by: Vec<(u32, &str)> = kills.iter().filter(|((_, prey), _)| *prey == k).map(|((hunter, _), n)| (*n, sim.kinds[*hunter].id.as_str())).collect();
                    by.sort_by_key(|b| std::cmp::Reverse(b.0));
                    println!("    taken by: {}", by.iter().map(|(n, id)| format!("{id} {n}")).collect::<Vec<_>>().join(", "));
                }
                // A kind that began as more than a handful is still there, and none has overrun the island.
                checks.bar(&format!("{} lasts", kind.id), rows[years][k] as f32, rows[0][k] < 6 || rows[years][k] >= 2, "a kind that began with six or more still has two");
                // Over a few years, none has overrun the island; over many, each has come to
                // a number it keeps (the island is begun with fewer of most kinds than it
                // carries, so growth alone is not a fault: growth that does not stop is).
                if years < 16 {
                    checks.bar(&format!("{} holds", kind.id), rows[years][k] as f32, rows[years][k] <= rows[0][k] * 6 + 10, "no kind grows past six times what it began as");
                } else {
                    let (was, now) = (rows[years - 8][k] as f32, rows[years][k] as f32);
                    checks.bar(&format!("{} settles", kind.id), now, (now - was).abs() <= was * 0.35 + 10.0, "within a third of what it was eight years before");
                }
            }
            let (first, last) = (total(&rows[0]) as f32, total(&rows[years]) as f32);
            checks.bar("creatures", last, last > first * 0.4 && last < first * 3.0, "between two fifths and three times as many as at the start");
            checks.finish()
        }
        "who" => {
            let (_, land, sim) = living(&args);
            let mut groups: std::collections::BTreeMap<(usize, i32, i32), usize> = Default::default();
            for agent in sim.agents.values() {
                *groups.entry((agent.kind, (agent.mind.home.x * 10.0) as i32, (agent.mind.home.y * 10.0) as i32)).or_default() += 1;
            }
            for ((kind, x, z), n) in groups {
                let biome = land.biome((x as f32 / 10.0 / CELL).floor() as i32, (z as f32 / 10.0 / CELL).floor() as i32);
                println!("{} ×{n} at {:.0},{:.0} {}", sim.kinds[kind].id, x as f32 / 10.0, z as f32 / 10.0, biome.name());
            }
        }
        _ => {
            println!("{HELP}");
            std::process::exit(if command.is_empty() || command.contains("help") { 0 } else { 2 });
        }
    }
}
