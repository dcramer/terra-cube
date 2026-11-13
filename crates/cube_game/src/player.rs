//! Control: driving a creature with the keys and a camera that follows it. Any creature can be
//! controlled, which is how the player is played and how an animal's gaits are judged at real
//! speeds. The movement itself is `motion` (and under it `cube_sim::body`); this file turns
//! keys into what is asked of the body, and flies the camera, behind it or from its eyes.

use crate::{
    Args,
    camera::Orbit,
    creatures::{Creature, Registry, Wander},
    motion::{Gait, Live, Mover},
    world::Terrain,
};
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

pub struct PlayPlugin;

impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Controls>().init_resource::<Menus>().add_systems(Update, (start, keys, script, drive, follow, edit, cursor, trace).chain());
        app.add_systems(Update, (torch, arrows, saves, mark));
    }
}

/// What is asked of the controlled creature, from the keys or from a script.
#[derive(Resource, Default)]
pub struct Controls {
    /// x to the camera's right, y away from the camera.
    pub dir: Vec2,
    pub gait: Gait,
    pub jump: bool,
    /// Swim down.
    pub dive: bool,
    /// Dig a hole where it looks, or build a lump there: once, then cleared.
    pub dig: bool,
    pub put: bool,
    /// Something to do on the spot, once: a blow, a bite of grass, a flinch.
    pub act: Option<cube_sim::life::Act>,
    /// What is in hand is to be used, once: a blow, a bite, a spadeful.
    pub use_held: bool,
    /// A slot of the bar to take in hand, or a step along it.
    pub hold: Option<usize>,
    pub cycle: i32,
    /// The guard is up (the right button held, with anything but the shovel in hand).
    pub guard: bool,
    /// Aiming (the right button held): the camera comes in over the shoulder, a mark shows
    /// where it looks, and the body turns to face that way, so a blow, a shot or a spadeful
    /// goes where the mark is.
    pub aim: bool,
    /// A kind of creature to be put down just ahead (a script's `spawn=ID`).
    pub spawn: Option<String>,
    /// An arrow loosed: from where, and which way.
    pub shot: Option<(Vec3, Vec3)>,
    /// The player's state is to be written to the save file, or read from it.
    pub save: bool,
    pub load: bool,
}

/// The windows that take the mouse from the camera while they are open: the pack, the admin
/// panel.
#[derive(Resource, Default)]
pub struct Menus {
    pub pack: bool,
    pub admin: bool,
}

impl Menus {
    pub fn open(&self) -> bool {
        self.pack || self.admin
    }
}

/// On the creature being controlled.
#[derive(Component, Default)]
pub struct Controlled {
    /// Seen from its own eyes, the model hidden.
    pub first_person: bool,
}

/// `--play`: control the player (`--as ID`: that creature) from the start. Tab takes and gives
/// up control of the picked creature after that.
fn start(
    mut commands: Commands,
    args: Res<Args>,
    registry: Res<Registry>,
    keys: Res<ButtonInput<KeyCode>>,
    selected: Option<Res<crate::ui::Selected>>,
    creatures: Query<(Entity, &Creature, &Transform), Without<crate::creatures::Picture>>,
    controlled: Query<Entity, With<Controlled>>,
    mut cameras: Query<&mut Orbit>,
    mut begun: Local<bool>,
    // The creature last let go of, and what was picked in the panel at the time.
    mut last: Local<Option<(Entity, Option<Entity>)>>,
    loading: Res<crate::loading::Loading>,
) {
    let toggle = keys.just_pressed(KeyCode::Tab) && !loading.up;
    let picked_now = selected.as_ref().and_then(|s| s.0);
    let begin = args.play && !*begun && !creatures.is_empty();
    if toggle && let Some(entity) = controlled.iter().next() {
        commands.entity(entity).remove::<Controlled>().insert(Visibility::Inherited);
        *last = Some((entity, picked_now));
        return;
    }
    if !(toggle || begin) {
        return;
    }
    *begun = true;
    let picked = selected.and_then(|s| s.0).and_then(|e| creatures.get(e).ok());
    let wanted = args.play_as.as_deref().unwrap_or("player");
    // (Of its kind, the one nearest where the scene is: with the island's creatures about,
    // `--as deer` took whichever deer came first, somewhere on the island, and a scene of it
    // showed nothing.)
    let here = Vec3::new(args.at.0, 0.0, args.at.1);
    let named = creatures.iter().filter(|(_, c, _)| registry.0[c.def].id == wanted).min_by(|a, b| a.2.translation.with_y(0.0).distance(here).total_cmp(&b.2.translation.with_y(0.0).distance(here)));
    // Tab goes back to the creature it let go of, unless another has been picked since.
    let again = last.and_then(|(entity, picked_then)| (picked_then == picked_now).then(|| creatures.get(entity).ok()).flatten());
    let Some((entity, creature, at)) = (if begin { named.or(picked) } else { again.or(picked).or(named) }).or_else(|| creatures.iter().next()) else { return };
    let def = &registry.0[creature.def];
    commands.entity(entity).remove::<Wander>().insert(Controlled { first_person: args.view == "first" });
    for mut orbit in &mut cameras {
        orbit.distance = (def.meta.height.max(def.meta.length) * 3.2).clamp(2.5, 9.0);
        orbit.pitch = 0.28;
        // Behind it, looking the way it faces. A scene's camera is aimed by `--yaw` instead.
        if args.scene.is_none() {
            orbit.yaw = at.rotation.to_euler(EulerRot::YXZ).0 + std::f32::consts::PI;
        }
    }
}

fn keys(keys: Res<ButtonInput<KeyCode>>, buttons: Res<ButtonInput<MouseButton>>, args: Res<Args>, mut loading: ResMut<crate::loading::Loading>, mut menus: ResMut<Menus>, mut controls: ResMut<Controls>, mut controlled: Query<&mut Controlled>) {
    if args.scene.is_some() {
        return;
    }
    // Behind the loading screen no key does anything; each one pressed is counted, so a test
    // can tell a key that was turned away from one that never arrived.
    if loading.up {
        loading.barred += keys.get_just_pressed().len() as u32;
        *controls = default();
        return;
    }
    // The pack opens on I and the admin panel on F1; Escape shuts both. While either is
    // open the keys and the mouse are the window's.
    if keys.just_pressed(KeyCode::KeyI) {
        menus.pack = !menus.pack;
    }
    if keys.just_pressed(KeyCode::F5) {
        controls.save = true;
    }
    if keys.just_pressed(KeyCode::F9) {
        controls.load = true;
    }
    if keys.just_pressed(KeyCode::F1) {
        menus.admin = !menus.admin;
    }
    if keys.just_pressed(KeyCode::Escape) {
        (menus.pack, menus.admin) = (false, false);
    }
    if menus.open() {
        (controls.dir, controls.jump) = (Vec2::ZERO, false);
        return;
    }
    let held = |k: KeyCode| keys.pressed(k) as i32 as f32;
    controls.dir = Vec2::new(held(KeyCode::KeyD) - held(KeyCode::KeyA), held(KeyCode::KeyW) - held(KeyCode::KeyS));
    controls.gait = if keys.pressed(KeyCode::ControlLeft) {
        Gait::Walk
    } else if keys.pressed(KeyCode::ShiftLeft) {
        Gait::Sprint
    } else {
        Gait::Run
    };
    controls.jump = keys.pressed(KeyCode::Space);
    controls.dive = keys.pressed(KeyCode::ControlLeft);
    // The left button uses what is in hand; 1 to 8 take a slot of the bar, Q and E step
    // along it, T takes the torch.
    controls.use_held |= buttons.just_pressed(MouseButton::Left) && !controlled.is_empty();
    for (slot, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8].into_iter().enumerate() {
        if keys.just_pressed(key) {
            controls.hold = Some(slot);
        }
    }
    controls.cycle += keys.just_pressed(KeyCode::KeyE) as i32 - keys.just_pressed(KeyCode::KeyQ) as i32;
    if keys.just_pressed(KeyCode::KeyT) {
        controls.hold = Some(usize::MAX);
    }
    // (Building was the right button's, a click; held, the right button now aims, and a
    // lump is laid with the middle button or B.)
    controls.put |= (buttons.just_pressed(MouseButton::Middle) || keys.just_pressed(KeyCode::KeyB)) && !controlled.is_empty();
    controls.guard = buttons.pressed(MouseButton::Right);
    controls.aim = buttons.pressed(MouseButton::Right) && !controlled.is_empty();
    if keys.just_pressed(KeyCode::KeyV) {
        for mut c in &mut controlled {
            c.first_person = !c.first_person;
        }
    }
}

/// The route a scripted run takes when `--script` gives none: toward where the camera faces.
/// What `--gif` runs when no script is given: every gait and every change between them, a
/// turn about, a jump, a stop, a blow and a flinch, then off again. Out and back, so it stays
/// in the clearing at the start where nothing stands between it and the camera.
pub const SHOW: &str = "0.5 fwd walk; 2.5 run; 4 back sprint; 5 jump; 5.6 stop; 6.4 attack; 8 hurt; 8.8 left run; 9.6 jump; 10.4 right walk; 11.5 stop";

pub const ROUTE: &str = "0.5 fwd walk; 2 run; 3 jump; 5 sprint; 6 jump";

/// With no window there are no keys: `--script` stands in for them. It is steps split by `;`,
/// each a time in seconds and what changes then: `fwd back left right stop` (direction),
/// `walk run sprint` (gait), `jump` (held, a full jump), `hop` (tapped), `dive surface` (in water), `dig put` (a hole or a lump where it looks), `first third` (view).
fn script(time: Res<Time>, args: Res<Args>, stage: Option<Res<crate::shot::Stage>>, mut controls: ResMut<Controls>, mut controlled: Query<&mut Controlled>, mut clock: Local<(f32, f32)>) {
    if args.scene.is_none() || !args.play || controlled.is_empty() {
        return;
    }
    // A script's times are on the picture's clock: seconds since the scene was ready.
    let (before, now) = (clock.0, stage.map_or(clock.0 + time.delta_secs(), |s| s.ready_for));
    clock.0 = now;
    controls.jump = now < clock.1;
    for step in args.script.split(';') {
        let mut words = step.split_whitespace();
        let Some(at) = words.next().and_then(|w| w.parse::<f32>().ok()) else { continue };
        if !(before < at && at <= now) {
            continue;
        }
        for word in words {
            match word {
                "fwd" => controls.dir = Vec2::Y,
                "back" => controls.dir = Vec2::NEG_Y,
                "left" => controls.dir = Vec2::NEG_X,
                "right" => controls.dir = Vec2::X,
                "stop" => controls.dir = Vec2::ZERO,
                "walk" => controls.gait = Gait::Walk,
                "run" => controls.gait = Gait::Run,
                "sprint" => controls.gait = Gait::Sprint,
                "jump" => clock.1 = now + 0.4,
                "dive" => controls.dive = true,
                "attack" | "use" => controls.use_held = true,
                "torch" => controls.hold = Some(usize::MAX),
                "save" => controls.save = true,
                "load" => controls.load = true,
                "guard" => controls.guard = true,
                "unguard" => controls.guard = false,
                "hold1" | "hold2" | "hold3" | "hold4" | "hold5" | "hold6" | "hold7" | "hold8" => controls.hold = word[4..].parse::<usize>().ok().map(|n| n - 1),
                "eat" => controls.act = Some(cube_sim::life::Act::Eat),
                "hurt" => controls.act = Some(cube_sim::life::Act::Hurt),
                "dig" => controls.dig = true,
                "put" => controls.put = true,
                "surface" => controls.dive = false,
                "hop" => clock.1 = now + 0.04,
                "aim" | "noaim" => controls.aim = word == "aim",
                "first" | "third" => controlled.iter_mut().for_each(|mut c| c.first_person = word == "first"),
                // `spawn=goblin`: one of that kind is put down two metres ahead, for a fight.
                other if other.starts_with("spawn=") => controls.spawn = Some(other[6..].to_owned()),
                other => warn!("--script: no such step `{other}`"),
            }
        }
    }
}

/// `--trace`, and every scripted run: the controlled body in a line, four times a second.
fn trace(time: Res<Time>, args: Res<Args>, live: Res<Live>, controlled: Query<(&Mover, &Creature, &Controlled)>, registry: Res<Registry>, cameras: Query<&Orbit>, mut clock: Local<(f32, f32)>) {
    if !(args.trace || args.scene.is_some() && args.play) {
        return;
    }
    let Ok((m, creature, c)) = controlled.single() else { return };
    let Some(agent) = live.agent(m) else { return };
    clock.0 += time.delta_secs();
    if clock.0 >= clock.1 {
        clock.1 += if args.trace { 0.25 } else { 1.0 };
        let def = &registry.0[creature.def];
        let clip = def.clips.get(creature.clip).map_or("-", |c| c.0.as_str());
        let b = &agent.body;
        let yaw = cameras.iter().next().map_or(0.0, |o| o.yaw);
        println!(
            "play t={:.1} pos={:.1},{:.1},{:.1} speed={:.2} grounded={} swimming={} clip={clip}x{:.2} yaw={yaw:.2} first={} health={:.0} held={} kills={}",
            clock.0, b.pos.x, b.pos.y, b.pos.z, b.ground_speed(), b.grounded, b.swimming, creature.speed, c.first_person, agent.health, agent.pack.as_ref().and_then(|p| p.held()).map_or("nothing", |i| i.name()), agent.log.kills
        );
    }
}

/// Hands the controls to the controlled creature's body, turned to the way the camera faces.
fn drive(mut controls: ResMut<Controls>, args: Res<Args>, terrain: Option<Res<Terrain>>, mut live: ResMut<Live>, cameras: Query<&Orbit>, mut controlled: Query<(&Controlled, &Mover, &mut Visibility)>) {
    // A scripted route runs the way `--yaw` says, whatever the camera is doing.
    let yaw = if args.scene.is_some() { args.yaw } else { cameras.iter().next().map_or(0.0, |o| o.yaw) };
    // The camera looks along -Z turned by its yaw; right is +X turned the same.
    let (sin, cos) = yaw.sin_cos();
    for (c, mover, mut visibility) in &mut controlled {
        // What is in hand, and using it: the simulation says what that does.
        if let Some(pack) = live.agent_mut(mover).and_then(|a| a.pack.as_mut()) {
            match controls.hold.take() {
                Some(usize::MAX) => {
                    if let Some(slot) = pack.slots[..cube_sim::items::BAR].iter().position(|s| s.is_some_and(|s| s.0.lights())) {
                        pack.hold(slot);
                    }
                }
                Some(slot) => pack.hold(slot),
                None => {}
            }
            pack.cycle(std::mem::take(&mut controls.cycle));
        }
        if let (Some(id), Some(terrain)) = (controls.spawn.take(), &terrain)
            && let (Some(kind), Some(me)) = (live.0.kind(&id), live.agent(mover).map(|a| (a.body.pos, a.facing)))
        {
            let at = me.0 + Vec3::new(me.1.sin(), 0.0, me.1.cos()) * 2.0;
            live.0.add(&terrain.0, kind, Vec3::new(at.x, terrain.ground(at.x, at.z), at.z), me.1 + std::f32::consts::PI, cube_sim::life::Driver::Mind);
        }
        if std::mem::take(&mut controls.use_held) {
            let used = live.0.use_held(mover.id);
            if used == cube_sim::items::Used::Dig {
                controls.dig = true;
            }
            // A shot: an arrow is seen to fly the way the archer faces.
            if let Some(a) = live.agent(mover).filter(|a| used == cube_sim::items::Used::Swing && a.pack.as_ref().and_then(|p| p.held()) == Some(cube_sim::items::Item::Bow)) {
                controls.shot = Some((a.body.pos + Vec3::Y * 1.4, Vec3::new(a.facing.sin(), 0.0, a.facing.cos())));
            }
        }
        let Some(speed) = live.kind(mover).map(|k| k.speed(controls.gait)) else { continue };
        let Some(m) = live.agent_mut(mover) else { continue };
        // The right button guards, with anything but the shovel in hand.
        let shovel = m.pack.as_ref().and_then(|p| p.held()) == Some(cube_sim::items::Item::Shovel);
        m.guard = controls.guard && !shovel;
        if !shovel {
            controls.put = false;
        }
        m.intent.dir = Vec2::new(cos, -sin) * controls.dir.x + Vec2::new(-sin, -cos) * controls.dir.y;
        m.intent.speed = speed;
        // Wanted for a moment after the press, so one just before landing still jumps.
        if controls.jump {
            m.want_jump = 0.12;
        }
        m.hold_jump = controls.jump;
        match controls.act.take() {
            Some(cube_sim::life::Act::Hurt) => m.hurt = 0.5,
            Some(act) => m.doing = Some((act, if act == cube_sim::life::Act::Eat { 3.0 } else { 1.3 })),
            None => {}
        }
        m.intent.dive = controls.dive;
        m.gait = controls.gait;
        m.turn_to = (c.first_person || controls.aim).then_some(yaw + std::f32::consts::PI);
        let want = if c.first_person { Visibility::Hidden } else { Visibility::Inherited };
        if *visibility != want {
            *visibility = want;
        }
    }
}

/// The camera on the controlled creature: the mouse turns it. Behind the creature the wheel
/// moves it in and out and ground in the way pulls it closer; in first person it is the eyes.
fn follow(
    args: Res<Args>,
    registry: Res<Registry>,
    terrain: Option<Res<Terrain>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    menus: Res<Menus>,
    loading: Res<crate::loading::Loading>,
    controlled: Query<(&Controlled, &Creature, &Transform), Without<Orbit>>,
    mut cameras: Query<(&mut Orbit, &mut Transform)>,
    time: Res<Time>,
    mut steady: Local<Option<(f32, f32)>>,
    controls: Res<Controls>,
    mut aim: Local<f32>,
) {
    let (Ok((c, creature, at)), Some(terrain)) = (controlled.single(), terrain) else { return };
    let def = &registry.0[creature.def];
    // The camera rides a height of its own that comes after the body's: the ground is blocks,
    // each a step of a hand or more, and a camera fixed to the body took every one of them as
    // a jolt (the developer: "the camera shakes a lot when moving up and down terrain"). It
    // is never more than half a metre behind, and a long drop or a waking elsewhere is taken
    // at once.
    let dt = time.delta_secs();
    let (mut height, mut out) = steady.unwrap_or((at.translation.y, f32::MAX));
    let behind = at.translation.y - height;
    height = if behind.abs() > 3.0 || std::env::var_os("CAMERA_RAW").is_some() { at.translation.y } else { at.translation.y - (behind * (-(if c.first_person { 12.0 } else { 7.0 }) * dt).exp()).clamp(-0.5, 0.5) };
    let at = &Transform { translation: at.translation.with_y(height), ..*at };
    for (mut orbit, mut transform) in &mut cameras {
        if args.scene.is_none() && !menus.open() && !loading.up {
            orbit.yaw -= motion.delta.x * 0.003;
            orbit.pitch = (orbit.pitch + motion.delta.y * 0.003).clamp(-1.3, 1.45);
            if !c.first_person {
                orbit.distance = (orbit.distance * (1.0 - scroll.delta.y * 0.1)).clamp(1.2, 30.0);
            }
        }
        if args.scene.is_some() && (args.view == "hand" || args.view == "lefthand") {
            let side = if args.view == "hand" { 1.0 } else { -1.0 };
            // Close on what the right hand holds (`lefthand`: the left), from that side and a little ahead: whether
            // a handle is in a closed hand cannot be read from further off.
            let focus = at.translation + Vec3::Y * def.meta.height * 0.56 + at.rotation * Vec3::new(0.2 * side, 0.0, 0.2) * def.meta.height / 1.75;
            transform.translation = focus + Quat::from_rotation_y(args.yaw + 2.2 * side) * Vec3::Z * def.meta.height * 0.8 + Vec3::Y * def.meta.height * 0.05;
            transform.look_at(focus, Vec3::Y);
            continue;
        }
        if args.scene.is_some() && (args.view == "side" || args.view == "three") {
            // For filmstrips: level, square on to the route, close enough to read the feet. `three`
            // is from ahead and to one side, for a moving picture: the face and the gait both show.
            let size = def.meta.height.max(def.meta.length);
            let focus = at.translation + Vec3::Y * def.meta.height * 0.5;
            let round = if args.view == "three" { std::f32::consts::PI - 0.95 } else { std::f32::consts::FRAC_PI_2 };
            transform.translation = focus + Quat::from_rotation_y(args.yaw + round) * Vec3::Z * size * if args.gif { 2.3 } else { 3.0 } + Vec3::Y * size * 0.25;
            transform.look_at(focus, Vec3::Y);
            continue;
        }
        let back = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0) * Vec3::Z;
        if c.first_person {
            let eyes = at.translation + Vec3::Y * def.meta.height * 0.92;
            transform.translation = eyes;
            transform.look_to(-back, Vec3::Y);
            continue;
        }
        // Aiming, it comes in over the right shoulder: close behind, and looking past him
        // from his right, so what he aims at is seen and not his back.
        *aim += ((controls.aim as u8 as f32) - *aim) * (1.0 - (-12.0 * dt).exp());
        let (far, aside) = (orbit.distance + (orbit.distance.min(2.1) - orbit.distance) * *aim, Quat::from_rotation_y(orbit.yaw) * Vec3::X * 0.55 * *aim);
        orbit.focus = at.translation + Vec3::Y * def.meta.height * (0.85 + 0.1 * *aim) + aside;
        // Ground in the way pulls it in quickly and lets it out slowly: on a slope the ground
        // behind comes and goes with every step, and the camera went in and out with it.
        let clear = terrain.0.raycast(orbit.focus, back, far + 0.3).map_or(far, |hit| (hit - 0.3).max(0.4)).min(far);
        out = if out == f32::MAX { clear } else { (out + (clear - out) * (1.0 - (-(if clear < out { 22.0 } else { 2.5 }) * dt).exp())).min(far.max(clear)) };
        transform.translation = orbit.focus + back * out;
        transform.look_to(-back, Vec3::Y);
    }
    *steady = Some((height, out));
}

/// The mark in the middle of the picture, shown while aiming or looking through his eyes.
#[derive(Component)]
struct Mark;

fn mark(mut commands: Commands, controls: Res<Controls>, menus: Res<Menus>, controlled: Query<&Controlled>, mut marks: Query<&mut Visibility, With<Mark>>) {
    let show = !menus.open() && controlled.iter().next().is_some_and(|c| c.first_person || controls.aim);
    if marks.is_empty() {
        commands.spawn((Mark, Node { position_type: PositionType::Absolute, left: percent(50.0), top: percent(50.0), width: px(4), height: px(4), margin: UiRect::all(px(-2)), ..default() }, BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.8)), Visibility::Hidden));
    }
    for mut seen in &mut marks {
        *seen = if show { Visibility::Visible } else { Visibility::Hidden };
    }
}

/// How far the controlled creature reaches to dig or build, and how big a hole and a lump are,
/// metres. Both are cut to the bit: a hole is a rounded bowl, not a missing cell.
const REACH: f32 = 8.0;
const HOLE: f32 = 0.4;
const LUMP: f32 = 0.18;

/// Digs or builds where the controlled creature looks: along the camera's line, from its eyes.
fn edit(mut controls: ResMut<Controls>, live: Res<Live>, terrain: Option<ResMut<Terrain>>, mut dirty: ResMut<crate::world::Dirty>, controlled: Query<(&Mover, &Transform), With<Controlled>>, cameras: Query<&Transform, With<Orbit>>) {
    let (dig, put) = (std::mem::take(&mut controls.dig), std::mem::take(&mut controls.put));
    let (Some(mut terrain), Ok((mover, at)), Some(camera)) = (terrain, controlled.single(), cameras.iter().next()) else { return };
    if !(dig || put) {
        return;
    }
    let Some(kind) = live.kind(mover) else { return };
    // Where the middle of the picture is: along the camera's own line, as far as he reaches
    // from his eyes. (From his eyes along the camera's way, it was beside the mark whenever
    // the camera stood to one side of him.)
    let (eyes, along) = (at.translation + Vec3::Y * kind.height * 0.92, *camera.forward());
    let Some(far) = terrain.0.raycast(camera.translation, along, REACH + camera.translation.distance(eyes)) else { return };
    let hit = camera.translation + along * far;
    if hit.distance(eyes) > REACH {
        return;
    }
    let far = hit.distance(eyes);
    if dig {
        terrain.ball(&mut dirty, hit + along * HOLE * 0.3, HOLE, cube_voxel::Block::Air);
    } else if far > 1.0 {
        // Not on top of itself: a lump it stood inside would hold it fast.
        terrain.ball(&mut dirty, hit - along * LUMP * 0.3, LUMP, cube_voxel::Block::Stone);
    }
}

/// The mouse belongs to the camera while a creature is controlled, and to the panel otherwise.
fn cursor(menus: Res<Menus>, controlled: Query<(), With<Controlled>>, mut windows: Query<&mut CursorOptions>) {
    let locked = !controlled.is_empty() && !menus.open();
    for mut options in &mut windows {
        let mode = if locked { CursorGrabMode::Locked } else { CursorGrabMode::None };
        if options.grab_mode != mode {
            options.grab_mode = mode;
            options.visible = !locked;
        }
    }
}

/// The torch the controlled creature carries: a warm light at its shoulder that throws
/// shadows and wavers as a flame does. T lights it and puts it out; `--torch` starts with it
/// lit. Night is dark on purpose (docs/art-guide.md) and this is how it is played.
#[derive(Component)]
pub struct Torch;

fn torch(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<crate::Args>,
    live: Res<Live>,
    loading: Res<crate::loading::Loading>,
    bearers: Query<(&Transform, &Mover), With<Controlled>>,
    flames: Query<&GlobalTransform, With<crate::held::Flame>>,
    mut torches: Query<(Entity, &mut Transform, &mut PointLight), (With<Torch>, Without<Controlled>)>,
) {
    // Lit while the torch is the thing in hand (T takes it from the bar); `--torch` lights
    // it whatever is held, for pictures.
    let held = |m: &Mover| live.agent(m).is_some_and(|a| a.dead.is_none() && a.act != cube_sim::life::Act::Swim && a.pack.as_ref().and_then(|p| p.held()).is_some_and(|i| i.lights()));
    // (Not behind the loading screen: it would be heard to catch.)
    let Some((bearer, _)) = bearers.iter().next().filter(|(_, m)| !loading.up && (args.torch || held(m))) else {
        for (entity, ..) in &torches {
            commands.entity(entity).despawn();
        }
        return;
    };
    // At the head of the torch in his hand; by the shoulder where no torch is drawn (`--torch`).
    let at = flames.iter().next().map_or(bearer.translation + bearer.rotation * Vec3::new(0.3, 1.55, -0.2), |f| f.translation() + Vec3::Y * 0.08);
    let t = time.elapsed_secs();
    let waver = 1.0 + 0.1 * (t * 11.0).sin() + 0.06 * (t * 23.0 + 1.3).sin() + 0.04 * (t * 5.3).sin();
    if let Ok((_, mut place, mut light)) = torches.single_mut() {
        place.translation = at + Vec3::new((t * 7.0).sin(), (t * 9.0).cos(), (t * 6.0).sin()) * 0.015;
        light.intensity = TORCH * waver;
    } else {
        commands.spawn((Torch, PointLight { color: Color::srgb(1.0, 0.62, 0.28), intensity: TORCH, range: 16.0, radius: 0.05, shadow_maps_enabled: true, ..default() }, Transform::from_translation(at)));
    }
}

/// How bright the torch is, lumens. Set for the camera as it is at night; by day it is lost
/// in the sun, as a real one is.
const TORCH: f32 = 2600.0;

/// An arrow in flight: which way, and how long it has flown. It is only to be seen: what
/// the shot hit was decided by the simulation when the bow was loosed.
#[derive(Component)]
struct Arrow(Vec3, f32);

fn arrows(mut commands: Commands, time: Res<Time>, mut controls: ResMut<Controls>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut flying: Query<(Entity, &mut Arrow, &mut Transform)>, mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>) {
    if let Some((from, way)) = controls.shot.take() {
        let (mesh, material) = look.get_or_insert_with(|| (meshes.add(Cuboid::new(0.025, 0.025, 0.8)), materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.42, 0.25), perceptual_roughness: 0.8, ..default() }))).clone();
        // Loosed a third of a second on, when the simulation lands the shot.
        commands.spawn((Arrow(way, -0.3), Mesh3d(mesh), MeshMaterial3d(material), Transform::from_translation(from + way * 0.6).looking_to(way, Vec3::Y), Visibility::Hidden));
    }
    for (entity, mut arrow, mut at) in &mut flying {
        arrow.1 += time.delta_secs();
        if arrow.1 > 0.0 {
            commands.entity(entity).insert(Visibility::Inherited);
            // Sixty metres a second, dropping a little.
            at.translation += (arrow.0 * 60.0 + Vec3::NEG_Y * arrow.1 * 6.0) * time.delta_secs();
        }
        if arrow.1 > 0.55 {
            commands.entity(entity).despawn();
        }
    }
}

/// Where the player's state is kept between runs: beside where the game is run from.
fn save_file() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_default().join("terra-cube.save")
}

/// F5 writes the player's state (where, when, health, pack) and F9 reads it back; `--continue`
/// reads it at the start. The land is the seed's and is not written: what was dug is lost.
fn saves(args: Res<Args>, mut controls: ResMut<Controls>, mut live: ResMut<Live>, controlled: Query<&Mover, With<Controlled>>, mut began: Local<bool>) {
    let Ok(mover) = controlled.single() else { return };
    let first = !std::mem::replace(&mut *began, true) && args.resume;
    if std::mem::take(&mut controls.save) {
        match std::fs::write(save_file(), live.0.save(mover.id)) {
            Ok(()) => println!("saved {}", save_file().display()),
            Err(e) => warn!("could not save: {e}"),
        }
    }
    if std::mem::take(&mut controls.load) || first {
        match std::fs::read_to_string(save_file()) {
            Ok(text) => println!("loaded {} ({})", save_file().display(), if live.0.load(mover.id, &text) { "ok" } else { "not understood" }),
            Err(e) => warn!("no save to load: {e}"),
        }
    }
}
