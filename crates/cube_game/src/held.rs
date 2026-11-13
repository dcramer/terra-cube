//! What a creature has in hand is seen in its hand: the sword, the torch, the shovel, each a
//! few boxes from `assets/world/held.toml`, hung on the bone of the right hand so the clip
//! that moves the arm moves the thing. Until 2026-11-11 nothing was drawn: a sword was a word
//! on the bar and the torch a light by the shoulder.
//!
//! Only a figure on Tripo's humanoid skeleton has a hand to find, by the bone's name
//! (`…RightHand`): the one place a bone is known by name, and not for animals.

use crate::motion::{Live, Mover};
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;

const FILE: &str = include_str!("../../../assets/world/held.toml");

pub struct HeldPlugin;

impl Plugin for HeldPlugin {
    fn build(&self, app: &mut App) {
        let things: BTreeMap<String, Thing> = toml::from_str(FILE).unwrap_or_else(|e| panic!("assets/world/held.toml: {e}"));
        app.insert_resource(Things(things)).add_systems(Update, (hold, loose)).add_systems(PostUpdate, carry.after(bevy::app::AnimationSystems).after(crate::stride::pose).before(bevy::transform::TransformSystems::Propagate));
    }
}

#[derive(Deserialize)]
struct Piece {
    size: [f32; 3],
    at: [f32; 3],
    colour: [f32; 3],
    #[serde(default)]
    glow: f32,
}

#[derive(Deserialize)]
struct Thing {
    turn: [f32; 3],
    shift: [f32; 3],
    light: Option<[f32; 3]>,
    /// How the arm carries it, where the clip's own arm would not do (a torch is held up, and
    /// every clip hangs the arm): the way the upper arm and then the forearm point, in the
    /// body's own frame (y up, +z ahead). The clip still moves the rest.
    arm: Option<[[f32; 3]; 2]>,
    /// Which way the thing points as it is carried, in the body's frame: the forearm is turned
    /// about itself until it does, as nearly as that allows.
    aim: Option<[f32; 3]>,
    /// The arm is left to the clip, swinging as it swings, but for the forearm, which is kept
    /// hanging: this is the most it may rise (its direction's y: -1 straight down, 0 level).
    /// For what is carried low at the side, where `arm` would hold the whole arm still.
    droop: Option<f32>,
    /// Which way it points while it is carried so, where that is not `aim` (a bow is carried
    /// slanted at the side and stood upright to shoot).
    low: Option<[f32; 3]>,
    /// How much of the clip's own swing of that arm is kept under the carry, 0 (none: the arm
    /// is held as `arm` says) to 1. With any, a blow that is the clip's own is left to the clip.
    #[serde(default)]
    sway: f32,
    /// A blow with it, where the clip's own would not do (every clip strikes with an empty
    /// hand): how the arm that holds it points drawn back, and then as the blow lands.
    blow: Option<[[[f32; 3]; 2]; 2]>,
    /// And what the other arm does through a blow (a bow is drawn with it), the same way.
    draw: Option<[[[f32; 3]; 2]; 2]>,
    /// What it looses as its blow lands (an arrow): boxes, as it is, flying along +y.
    #[serde(default)]
    looses: Vec<Piece>,
    /// Made as big as its bearer is tall beside a person.
    #[serde(default)]
    fits: bool,
    /// Carried in the left hand (a bow), not the right.
    #[serde(default)]
    left: bool,
    pieces: Vec<Piece>,
}

#[derive(Resource)]
struct Things(BTreeMap<String, Thing>);

/// A thing drawn in a hand: whose, and what; the bones of the arm that carries it, from the
/// shoulder down; whether it is drawn before the eye instead (the first person, whose own
/// body is not drawn: the thing is hung on the camera, low and to the right); and the bones
/// of the other arm.
#[derive(Component)]
struct Held(Entity, String, [Entity; 3], bool, [Entity; 3]);

/// The hand's hold on it: where the handle lies in the hand and how it is turned (the hand
/// bone's frame), and each finger bone with its turn closed round it (forge's `[grip]`).
#[derive(Component, Default)]
struct Grip(Vec3, Quat, Vec<(Entity, Quat)>);

/// Something loosed, on its way: how fast, and the seconds it has left.
#[derive(Component)]
struct Loosed(Vec3, f32);

/// Where a held thing that lights gives its light from (`player::torch` puts the light there).
#[derive(Component)]
pub struct Flame;

#[allow(clippy::too_many_arguments)]
fn hold(
    mut commands: Commands,
    things: Res<Things>,
    live: Option<Res<Live>>,
    movers: Query<(Entity, &Mover)>,
    children: Query<&Children>,
    names: Query<&Name>,
    globals: Query<&GlobalTransform>,
    held: Query<(Entity, &Held)>,
    mut seats: Query<(&mut Transform, &Grip), With<Held>>,
    firsts: Query<&crate::player::Controlled>,
    cameras: Query<Entity, With<Camera3d>>,
    (mut meshes, mut materials, registry, creatures): (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>, Res<crate::creatures::Registry>, Query<&crate::creatures::Creature>),
    mut cube: Local<Option<Handle<Mesh>>>,
    time: Res<Time>,
    mut blow: Local<f32>,
) {
    let Some(live) = live else { return };
    // What is drawn and no longer held, or whose holder is gone, goes.
    // What it has taken from its pack, or what its kind is seen to carry (a goblin's club).
    let wants = |owner: Entity| -> Option<String> {
        let (_, m) = movers.get(owner).ok()?;
        // (Nothing while it swims: both hands are for the water, a sword was waved over his
        // head at every stroke, and a torch does not burn there.)
        let agent = live.agent(m).filter(|a| a.dead.is_none() && a.act != cube_sim::life::Act::Swim)?;
        agent.pack.as_ref().and_then(|p| p.held()).map(|i| i.name().to_owned()).or_else(|| live.kind(m).and_then(|k| k.holds.clone()))
    };
    let first = |owner: Entity| firsts.get(owner).is_ok_and(|c| c.first_person);
    for (entity, Held(owner, item, _, before_eye, _)) in &held {
        if wants(*owner).as_ref() != Some(item) || first(*owner) != *before_eye {
            commands.entity(entity).despawn();
        }
    }
    // A thing in a hand is kept to its size in metres whatever the bone's own scale comes to
    // (on the frame a model arrives its bones are not yet placed, and a goblin's club made
    // then was made to the wrong size and never seen).
    for (entity, Held(owner, item, [.., hand], before_eye, _)) in &held {
        let (Some(thing), Some(scale), Ok((mut seat, grip))) = (things.0.get(item), globals.get(*hand).ok().map(|g| g.scale().x).filter(|s| *s > 1e-4), seats.get_mut(entity)) else { continue };
        if !*before_eye {
            let big = if thing.fits { movers.get(*owner).ok().and_then(|(_, m)| live.kind(m)).map_or(1.0, |k| k.height / 1.75) } else { 1.0 };
            (seat.translation, seat.scale) = (grip.0 + grip.1 * Vec3::from_array(thing.shift) * big / scale, Vec3::splat(big / scale));
        } else if let Some(agent) = movers.get(*owner).ok().and_then(|(_, m)| live.agent(m)) {
            // Before the eye it goes with him: it rides up and down and a little across as he
            // walks, faster as he goes faster, and with a blow it is thrown forward and down
            // and comes back. (Fixed in its place it was a picture stuck on the glass.)
            let (t, pace) = (time.elapsed_secs(), (agent.body.ground_speed() / 3.5).min(1.4) * agent.body.grounded as u8 as f32);
            *blow += ((agent.swing.is_some() as u8 as f32) - *blow) * (1.0 - (-16.0 * time.delta_secs()).exp());
            let step = Vec3::new((t * 5.5).cos() * 0.012, (t * 11.0).sin() * 0.016, 0.0) * pace;
            // And the blow is the thing's own: a bow stands upright at the left and is drawn
            // back toward him, then let go; a torch is thrust straight out; anything else is
            // cut across the view. (Until 2026-11-11 a bow was swung like a sword.)
            (seat.translation, seat.rotation) = if thing.draw.is_some() {
                let drawn = agent.swing.map_or(0.0, |left| (1.0 - left / 0.35).clamp(0.0, 1.0));
                (Vec3::new(-0.34, -0.08, -1.0) + step + Vec3::new(0.06, 0.03, 0.12) * drawn - Vec3::Z * 0.05 * (*blow - drawn).max(0.0), Quat::from_euler(EulerRot::XYZ, -0.1, 1.45, 0.08 - 0.1 * drawn))
            } else if thing.arm.is_some() {
                (Vec3::new(0.3, -0.32, -0.6) + step + Vec3::new(-0.12, 0.1, -0.4) * *blow, Quat::from_euler(EulerRot::XYZ, -0.15 - 0.75 * *blow, 0.0, -0.12 + 0.1 * *blow))
            } else {
                (Vec3::new(0.3, -0.32, -0.6) + step + Vec3::new(-0.2, 0.1, -0.2) * *blow, Quat::from_euler(EulerRot::XYZ, -0.9 - 0.45 * *blow, 0.6 * *blow, -0.12 + 0.5 * *blow))
            };
        }
    }
    for (owner, _) in &movers {
        let Some(item) = wants(owner) else { continue };
        if held.iter().any(|(_, h)| h.0 == owner) {
            continue;
        }
        let Some(thing) = things.0.get(&item) else { continue };
        // Its hand, once its model is there and placed.
        let bone = |name: &str| children.iter_descendants(owner).find(|e| names.get(*e).is_ok_and(|n| n.as_str().ends_with(name)));
        let side = if thing.left { "Left" } else { "Right" };
        let arm = |side: &str| Some([bone(&format!("{side}Arm"))?, bone(&format!("{side}ForeArm"))?, bone(&format!("{side}Hand"))?]);
        let (Some([upper, fore, hand]), Some(other)) = (arm(side), arm(if thing.left { "Right" } else { "Left" })) else { continue };
        let Some(scale) = globals.get(hand).ok().map(|g| g.scale().x).filter(|s| *s > 1e-4) else { continue };
        // The bone is in the model's own units: the thing is sized in metres whatever they are.
        let turn = Quat::from_euler(EulerRot::XYZ, thing.turn[0].to_radians(), thing.turn[1].to_radians(), thing.turn[2].to_radians());
        let before_eye = first(owner);
        // How this hand holds a handle, where its model says: the thing is seated in the palm
        // and the fingers are closed round it. (Without it, it is seated on the hand's bone.)
        let grip = creatures.get(owner).ok().and_then(|c| registry.0.get(c.def)).and_then(|d| d.meta.grip.get(side)).filter(|_| !before_eye);
        let grip = grip.map_or_else(Grip::default, |g| Grip(Vec3::from_array(g.at), Quat::from_array(g.turn), g.bones.iter().zip(&g.turns).filter_map(|(n, q)| Some((bone(n)?, Quat::from_array(*q)))).collect()));
        // Sized to who carries it, where it says so: a troll's club is a troll's.
        let big = if thing.fits { movers.get(owner).ok().and_then(|(_, m)| live.kind(m)).map_or(1.0, |k| k.height / 1.75) } else { 1.0 };
        let (on, seat) = match cameras.iter().next().filter(|_| before_eye) {
            Some(camera) => (camera, Transform { translation: Vec3::new(0.3, -0.32, -0.6), rotation: Quat::from_euler(EulerRot::XYZ, if thing.arm.is_some() { -0.15 } else { -0.9 }, 0.0, -0.12), ..default() }),
            None => (hand, Transform { translation: grip.0 + grip.1 * Vec3::from_array(thing.shift) * big / scale, rotation: grip.1 * turn, scale: Vec3::splat(big / scale) }),
        };
        let cube = cube.get_or_insert_with(|| meshes.add(Cuboid::new(1.0, 1.0, 1.0))).clone();
        let root = commands.spawn((Held(owner, item, [upper, fore, hand], before_eye, other), grip, ChildOf(on), seat, Visibility::default())).id();
        for piece in &thing.pieces {
            let colour = Color::srgb(piece.colour[0], piece.colour[1], piece.colour[2]);
            let material = materials.add(StandardMaterial { base_color: colour, emissive: colour.to_linear() * piece.glow, perceptual_roughness: 0.7, ..default() });
            commands.spawn((ChildOf(root), Mesh3d(cube.clone()), MeshMaterial3d(material), Transform { translation: Vec3::from_array(piece.at), scale: Vec3::from_array(piece.size), ..default() }));
        }
        if let Some(light) = thing.light {
            commands.spawn((ChildOf(root), Flame, Transform::from_translation(Vec3::from_array(light)), Visibility::default()));
        }
    }
}

/// After the clip has posed the arms and before the pose is used: the arm that carries a
/// thing with a carry of its own is turned to it, and through a blow the arms do what the
/// thing says a blow with it is (a sword is raised and cut down, a torch thrust, a bow drawn:
/// until 2026-11-11 every blow was the clip's, an empty hand's, whatever was held). Each bone
/// by the least turn that points it the way it should go, so the clip's own twist and the
/// rest of the body are left as they were.
fn carry(things: Res<Things>, live: Option<Res<Live>>, held: Query<(Entity, &Held, &Grip)>, owners: Query<(&Transform, &Mover)>, parents: Query<&ChildOf>, globals: Query<&GlobalTransform>, mut bones: Query<&mut Transform, Without<Mover>>) {
    let Some(live) = live else { return };
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| Vec3::from_array(a).lerp(Vec3::from_array(b), t).to_array();
    let ease = |t: f32| t.clamp(0.0, 1.0) * t.clamp(0.0, 1.0) * (3.0 - 2.0 * t.clamp(0.0, 1.0));
    for (seat, Held(owner, item, arm, _, other), grip) in &held {
        // The hand is closed round it, whatever the clip has the fingers doing.
        for (finger, closed) in &grip.2 {
            if let Ok(mut pose) = bones.get_mut(*finger) {
                pose.rotation = *closed;
            }
        }
        let (Some(thing), Ok((body, mover))) = (things.0.get(item), owners.get(*owner)) else { continue };
        // How far into a blow it is, seconds (a blow is a second long and lands at 0.35): how
        // much of the blow's pose is on (quickly on, off as it ends), and how far it has gone
        // from drawn back to landed.
        let into = live.agent(mover).and_then(|a| a.doing).filter(|d| d.0 == cube_sim::life::Act::Attack).map(|d| 1.0 - d.1);
        let (on, landed) = into.map_or((0.0, 0.0), |t| (ease(t / 0.12).min(ease((0.95 - t) / 0.3)), ease((t - 0.22) / 0.14)));
        let blow = |b: Option<[[[f32; 3]; 2]; 2]>| b.filter(|_| on > 0.0).map(|[back, hit]| [mix(back[0], hit[0], landed), mix(back[1], hit[1], landed)]);
        // Carried low: the clip's own arm, with the forearm brought down where it has risen
        // and turned about itself until the thing points as it is carried. A blow is the clip's.
        if let (Some(droop), None, [upper, fore, hand]) = (thing.droop, thing.arm, *arm)
            && let Some(shoulder) = parents.get(upper).ok().and_then(|p| globals.get(p.parent()).ok()).map(|g| g.rotation())
            && let (Ok(over), Ok(elbow), Ok(wrist), Ok(seated)) = (bones.get(upper).map(|b| b.rotation), bones.get(fore).map(|b| b.rotation), bones.get(hand).map(|b| (b.rotation, b.translation)), bones.get(seat).map(|b| b.rotation))
            && let Some(along) = wrist.1.try_normalize()
        {
            let (above, keep) = (shoulder * over, 1.0 - on);
            let mut turned = above * elbow;
            let way = turned * along;
            if way.y > droop {
                // Straightened toward the line of the upper arm until it hangs low enough, and
                // never past that line: an elbow does not bend backwards. (Brought straight
                // down whatever the upper arm was doing, it did, each time the arm swung forward.)
                let straight = Quat::from_rotation_arc(way, (above * along).normalize());
                let much = (1..=10).map(|n| n as f32 / 10.0).find(|t| (Quat::IDENTITY.slerp(straight, *t) * way).y <= droop).unwrap_or(1.0);
                turned = Quat::IDENTITY.slerp(straight, much * keep) * turned;
            }
            let axis = turned * along;
            let level = |v: Vec3| (v - axis * v.dot(axis)).try_normalize();
            if let (Some(aim), Some(from)) = (thing.low.or(thing.aim), level(turned * wrist.0 * seated * Vec3::Y))
                && let Some(to) = level(body.rotation * Vec3::from_array(aim))
            {
                turned = Quat::from_axis_angle(axis, from.cross(to).dot(axis).atan2(from.dot(to)) * keep) * turned;
            }
            if let Ok(mut pose) = bones.get_mut(fore) {
                pose.rotation = above.inverse() * turned;
            }
        }
        let holding = match (thing.arm, blow(thing.blow)) {
            (Some(rest), Some(b)) => Some(([mix(rest[0], b[0], on), mix(rest[1], b[1], on)], 1.0)),
            (Some(rest), None) => Some((rest, if thing.sway > 0.0 { (1.0 - thing.sway) * (1.0 - on) } else { 1.0 })),
            (None, Some(b)) => Some((b, on)),
            (None, None) => None,
        };
        for ([upper, fore, hand], ways, much) in [(arm, holding), (other, blow(thing.draw).map(|b| (b, on)))].into_iter().filter_map(|(a, w)| w.map(|(ways, much)| (*a, ways, much))) {
            // The shoulder's turn in the world is last frame's: it moves little, and this frame's is not worked out yet.
            let Some(mut parent) = parents.get(upper).ok().and_then(|p| globals.get(p.parent()).ok()).map(|g| g.rotation()) else { continue };
            let mut above = parent;
            for (bone, child, way) in [(upper, fore, ways[0]), (fore, hand, ways[1])] {
                let Some(along) = bones.get(child).ok().and_then(|c| c.translation.try_normalize()) else { break };
                let Ok(mut pose) = bones.get_mut(bone) else { break };
                let now = parent * pose.rotation;
                let want = (body.rotation * Vec3::from_array(way)).normalize();
                let turned = now.slerp(Quat::from_rotation_arc(now * along, want) * now, much);
                pose.rotation = parent.inverse() * turned;
                (above, parent) = (parent, turned);
            }
            // And the forearm turned about itself until the thing points the way it is
            // carried: a bone pointed by the least turn keeps whatever roll the clip gave it,
            // and the blade went wherever the wrist happened to be turned.
            let (Some(aim), true, Ok(seated), Ok(wrist)) = (thing.aim, hand == arm[2], bones.get(seat).map(|s| s.rotation), bones.get(hand).map(|h| h.rotation)) else { continue };
            let axis = (body.rotation * Vec3::from_array(ways[1])).normalize();
            let flat = |v: Vec3| (v - axis * v.dot(axis)).try_normalize();
            let (Some(from), Some(to)) = (flat(parent * wrist * seated * Vec3::Y), flat(body.rotation * Vec3::from_array(aim))) else { continue };
            let roll = Quat::from_axis_angle(axis, from.cross(to).dot(axis).atan2(from.dot(to)) * much);
            if let Ok(mut pose) = bones.get_mut(fore) {
                pose.rotation = above.inverse() * roll * parent;
            }
        }
    }
}

/// What a thing looses is seen to go: as the blow of whoever holds it lands, from the hand
/// that holds it, straight the way they face, for as far as it reaches. (It is only seen:
/// what it strikes was settled by the simulation as it left.)
#[allow(clippy::too_many_arguments)]
fn loose(
    mut commands: Commands,
    things: Res<Things>,
    live: Option<Res<Live>>,
    held: Query<&Held>,
    owners: Query<(&Transform, &Mover)>,
    globals: Query<&GlobalTransform>,
    mut flying: Query<(Entity, &mut Transform, &mut Loosed), Without<Mover>>,
    mut swinging: Local<std::collections::HashSet<Entity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for (entity, mut at, mut loosed) in &mut flying {
        at.translation += loosed.0 * time.delta_secs();
        loosed.1 -= time.delta_secs();
        if loosed.1 < 0.0 {
            commands.entity(entity).despawn();
        }
    }
    let Some(live) = live else { return };
    for Held(owner, item, [.., hand], _, _) in &held {
        let (Some(thing), Ok((body, mover))) = (things.0.get(item).filter(|t| !t.looses.is_empty()), owners.get(*owner)) else { continue };
        let now = live.agent(mover).is_some_and(|a| a.swing.is_some());
        if now {
            swinging.insert(*owner);
        } else if swinging.remove(owner) {
            let Ok(from) = globals.get(*hand) else { continue };
            let ahead = body.rotation * Vec3::Z;
            let root = commands.spawn((Loosed(ahead * 60.0, 0.5), Transform { translation: from.translation(), rotation: Quat::from_rotation_arc(Vec3::Y, ahead), ..default() }, Visibility::default())).id();
            for piece in &thing.looses {
                let colour = Color::srgb(piece.colour[0], piece.colour[1], piece.colour[2]);
                commands.spawn((ChildOf(root), Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))), MeshMaterial3d(materials.add(StandardMaterial { base_color: colour, ..default() })), Transform { translation: Vec3::from_array(piece.at), scale: Vec3::from_array(piece.size), ..default() }));
            }
        }
    }
}
