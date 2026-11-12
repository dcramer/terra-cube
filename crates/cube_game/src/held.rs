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
        app.insert_resource(Things(things)).add_systems(Update, hold).add_systems(PostUpdate, carry.after(bevy::app::AnimationSystems).before(bevy::transform::TransformSystems::Propagate));
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
    /// body's own frame (x to its right, y up, -z ahead). The clip still moves the rest.
    arm: Option<[[f32; 3]; 2]>,
    /// Made as big as its bearer is tall beside a person.
    #[serde(default)]
    fits: bool,
    pieces: Vec<Piece>,
}

#[derive(Resource)]
struct Things(BTreeMap<String, Thing>);

/// A thing drawn in a hand: whose, and what; the bones of the arm that carries it, from the
/// shoulder down; and whether it is drawn before the eye instead (the first person, whose
/// own body is not drawn: the thing is hung on the camera, low and to the right).
#[derive(Component)]
struct Held(Entity, String, [Entity; 3], bool);

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
    mut seats: Query<&mut Transform, With<Held>>,
    firsts: Query<&crate::player::Controlled>,
    cameras: Query<Entity, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cube: Local<Option<Handle<Mesh>>>,
) {
    let Some(live) = live else { return };
    // What is drawn and no longer held, or whose holder is gone, goes.
    // What it has taken from its pack, or what its kind is seen to carry (a goblin's club).
    let wants = |owner: Entity| -> Option<String> {
        let (_, m) = movers.get(owner).ok()?;
        let agent = live.agent(m).filter(|a| a.dead.is_none())?;
        agent.pack.as_ref().and_then(|p| p.held()).map(|i| i.name().to_owned()).or_else(|| live.kind(m).and_then(|k| k.holds.clone()))
    };
    let first = |owner: Entity| firsts.get(owner).is_ok_and(|c| c.first_person);
    for (entity, Held(owner, item, _, before_eye)) in &held {
        if wants(*owner).as_ref() != Some(item) || first(*owner) != *before_eye {
            commands.entity(entity).despawn();
        }
    }
    // A thing in a hand is kept to its size in metres whatever the bone's own scale comes to
    // (on the frame a model arrives its bones are not yet placed, and a goblin's club made
    // then was made to the wrong size and never seen).
    for (entity, Held(owner, item, [.., hand], before_eye)) in &held {
        let (Some(thing), Some(scale), Ok(mut seat)) = (things.0.get(item), globals.get(*hand).ok().map(|g| g.scale().x).filter(|s| *s > 1e-4), seats.get_mut(entity)) else { continue };
        if !*before_eye {
            let big = if thing.fits { movers.get(*owner).ok().and_then(|(_, m)| live.kind(m)).map_or(1.0, |k| k.height / 1.75) } else { 1.0 };
            (seat.translation, seat.scale) = (Vec3::from_array(thing.shift) * big / scale, Vec3::splat(big / scale));
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
        let (Some(upper), Some(fore), Some(hand)) = (bone("RightArm"), bone("RightForeArm"), bone("RightHand")) else { continue };
        let Some(scale) = globals.get(hand).ok().map(|g| g.scale().x).filter(|s| *s > 1e-4) else { continue };
        // The bone is in the model's own units: the thing is sized in metres whatever they are.
        let turn = Quat::from_euler(EulerRot::XYZ, thing.turn[0].to_radians(), thing.turn[1].to_radians(), thing.turn[2].to_radians());
        let before_eye = first(owner);
        // Sized to who carries it, where it says so: a troll's club is a troll's.
        let big = if thing.fits { movers.get(owner).ok().and_then(|(_, m)| live.kind(m)).map_or(1.0, |k| k.height / 1.75) } else { 1.0 };
        let (on, seat) = match cameras.iter().next().filter(|_| before_eye) {
            Some(camera) => (camera, Transform { translation: Vec3::new(0.3, -0.32, -0.6), rotation: Quat::from_euler(EulerRot::XYZ, if thing.arm.is_some() { -0.15 } else { -0.9 }, 0.0, -0.12), ..default() }),
            None => (hand, Transform { translation: Vec3::from_array(thing.shift) * big / scale, rotation: turn, scale: Vec3::splat(big / scale) }),
        };
        let cube = cube.get_or_insert_with(|| meshes.add(Cuboid::new(1.0, 1.0, 1.0))).clone();
        let root = commands.spawn((Held(owner, item, [upper, fore, hand], before_eye), ChildOf(on), seat, Visibility::default())).id();
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

/// After the clip has posed the arm and before the pose is used: the arm that carries a thing
/// with a carry of its own is turned to it, each bone by the least turn that points it the
/// way it should go, so the clip's own twist and the rest of the body are left as they were.
fn carry(things: Res<Things>, held: Query<&Held>, owners: Query<&Transform, With<Mover>>, parents: Query<&ChildOf>, globals: Query<&GlobalTransform>, mut bones: Query<&mut Transform, Without<Mover>>) {
    for Held(owner, item, [upper, fore, hand], _) in &held {
        let (Some(ways), Ok(body)) = (things.0.get(item).and_then(|t| t.arm), owners.get(*owner)) else { continue };
        // The shoulder's turn in the world is last frame's: it moves little, and this frame's is not worked out yet.
        let Some(mut parent) = parents.get(*upper).ok().and_then(|p| globals.get(p.parent()).ok()).map(|g| g.rotation()) else { continue };
        for (bone, child, way) in [(*upper, *fore, ways[0]), (*fore, *hand, ways[1])] {
            let Some(along) = bones.get(child).ok().and_then(|c| c.translation.try_normalize()) else { break };
            let Ok(mut pose) = bones.get_mut(bone) else { break };
            let now = parent * pose.rotation;
            let want = (body.rotation * Vec3::from_array(way)).normalize();
            let turned = Quat::from_rotation_arc(now * along, want) * now;
            pose.rotation = parent.inverse() * turned;
            parent = turned;
        }
    }
}
