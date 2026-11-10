//! `cargo world`: the world and what lives in it, judged with no window. It makes the same
//! world the game does and steps the same simulation, then answers in numbers (`survey`,
//! `run`) or in one picture from above (`map`).

use cube_sim::{
    TICKS,
    life::{Act, Driver, Kind, Sim, Temper},
    survey,
    terrain::{self, Biome, Land, SEA},
};
use cube_voxel::{Block, CELL, World, cells};
use glam::{IVec3, Vec3};
use std::path::Path;

const HELP: &str = "\
cargo world survey [--seed N] [--radius CHUNKS]   the land in numbers: ok, or FAIL lines
cargo world run [--seconds 120] [--seed N]        the creatures living in it for a while, with a
                                                  person walking about the start: a line a kind,
                                                  then ok or FAIL lines
cargo world who [--seed N]                        where each group of creatures starts: kind, how
                                                  many, metres east and south of the start, biome
                                                  (for `cargo sandbox --wild --scene F --at X,Z`)
cargo world dig [--holes 120]                     HOLES (120) holes and stones cut to the 5 cm bit all
                                                  over the land: what broken cells cost, and whether
                                                  creatures still live among them
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
    let (world, land) = terrain::generate(args.seed, args.radius);
    let mut sim = Sim::new(kinds(), args.seed as u64);
    sim.populate(&world, &land);
    if let Some(person) = sim.kind("player") {
        sim.add(&world, person, Vec3::new(0.25, land.height(0, 0) as f32 * CELL, 0.25), 0.0, Driver::Mind);
    }
    (world, land, sim)
}

fn survey_command(args: &Args) -> ! {
    let (world, land) = terrain::generate(args.seed, args.radius);
    let s = survey::survey(&land);
    // What the game will have to draw: every chunk meshed as it meshes them.
    let faces: usize = world.chunks.keys().map(|&c| [false, true].map(|liquid| cube_voxel::mesh::mesh_chunk(&world, c, liquid).indices.len() / 6).iter().sum::<usize>()).sum();
    let shares: Vec<String> = Biome::ALL.iter().map(|b| format!("{} {:.0}%", b.name(), s.share[*b as usize] * 100.0)).collect();
    println!("land seed={} {:.0}m across, {:.2}m cells: {} · trees {} · highest {:.0}m · cliffs {:.0}% · reached on foot {:.0}%, swimming {:.0}% · {} chunks, {:.1}M faces to draw", args.seed, (land.span * 2) as f32 * CELL, CELL, shares.join(" "), s.trees, s.highest, s.cliffs * 100.0, s.on_foot * 100.0, s.swimming * 100.0, world.chunks.len(), faces as f32 / 1e6);
    let mut checks = Checks(Vec::new());
    let share = |b: Biome| s.share[b as usize];
    checks.bar("water", share(Biome::Water), (0.2..=0.6).contains(&share(Biome::Water)), "0.20 to 0.60 of the world");
    for biome in [Biome::Meadow, Biome::Forest, Biome::Rock] {
        checks.bar(biome.name(), share(biome), share(biome) >= 0.06, "at least 0.06");
    }
    checks.bar("trees", s.trees as f32, s.trees >= 150, "at least 150");
    checks.bar("highest", s.highest, s.highest >= 15.0, "at least 15 m");
    checks.bar("cliffs", s.cliffs, (0.03..=0.35).contains(&s.cliffs), "0.03 to 0.35 of the land");
    checks.bar("faces", faces as f32 / 1e6, faces <= 4_000_000, "at most 4 million");
    checks.bar("on-foot", s.on_foot, s.on_foot >= 0.5, "at least 0.50 of the land from the start");
    checks.bar("swimming", s.swimming, s.swimming >= 0.9, "at least 0.90 of the land from the start");
    checks.finish()
}

fn run_command(args: &Args) -> ! {
    let (world, _, mut sim) = living(args);
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
    println!("life seed={} creatures={} seconds={}", args.seed, sim.agents.len(), args.seconds);
    for (k, kind) in sim.kinds.iter().enumerate() {
        let of: Vec<_> = sim.agents.values().filter(|a| a.kind == k).collect();
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
        println!("{} ×{}: went {:.0}m each · {} · fled {} · hit {}", kind.id, of.len(), moved, acts.join(" "), sum(&|l| l.fled as f32), sum(&|l| l.struck as f32));
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
        world.bits.len(),
        world.bits.len() as f32 / holes as f32,
        world.bits.len() as f32 * 141.0 / 1e6,
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
