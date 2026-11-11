//! What grows, drawn as what it is and not as the cubes that stand for it: round trunks and
//! limbs that taper, and leaf in masses of small cards that the light comes through and the
//! wind moves. The simulation still has every tree as cells (a body is stopped by a trunk, a
//! squirrel will not climb a crown); `cube_sim::plants` says, as it grows each thing, what
//! shape it grew (`plants::Form`: its limbs and its clumps of leaf), and that is drawn here
//! in place of the cells.
//!
//! Why: a world whose trees are cubes is Minecraft, whatever its light and paint. The land
//! and what grows on it are drawn as shapes; cubes are for what a person builds.

use crate::world::{Look, Terrain};
use bevy::{asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology};
use cube_sim::{plants::Form, terrain};
use cube_voxel::{
    Block, CELL,
    texture::{Grain, MEAN, SPAN},
};
use std::collections::HashMap;

pub struct FloraPlugin;

impl Plugin for FloraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Forms>().init_resource::<Grown>().add_systems(Update, grow);
    }
}

/// The shapes of what grows in each region whose cells are made: put here when the region is
/// made and taken away when it is dropped (`world::regions`).
#[derive(Resource, Default)]
pub struct Forms(pub HashMap<(i32, i32), Form>);

/// The meshes made from them.
#[derive(Resource, Default)]
struct Grown(HashMap<(i32, i32), Entity>);

/// Sides to a trunk; how much of a leaf mass's surface its cards cover; how big a card is.
const SIDES: usize = 7;
const COVER: f32 = 0.75;
const CARD: f32 = 0.8;

fn hash(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E3779B1) ^ b.wrapping_mul(0x85EBCA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

#[derive(Default)]
struct Shape {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
    tangents: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Shape {
    /// `free`: how far it gives to the wind, 0 held to 1 loose (the mesher's convention, in
    /// the colour's alpha).
    fn point(&mut self, at: Vec3, normal: Vec3, colour: [f32; 3], uv: [f32; 2], free: f32) -> u32 {
        self.positions.push(at.to_array());
        self.normals.push(normal.to_array());
        self.colors.push([colour[0] / MEAN, colour[1] / MEAN, colour[2] / MEAN, 1.0 - free]);
        self.uvs.push(uv);
        self.tangents.push([0.0, 1.0, 0.0, 1.0]);
        self.positions.len() as u32 - 1
    }

    /// A round limb from `a` to `b`, tapering.
    fn limb(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32, bark: Block, n: u32) {
        let along = (b - a).normalize_or(Vec3::Y);
        let side = along.any_orthonormal_vector();
        let other = along.cross(side);
        let colour = bark.colors().1;
        let grain = if bark == Block::Birch { Grain::BirchBark } else { Grain::Bark };
        // The bark's picture once round, and along by its true length (to a tile at most).
        let long = (a.distance(b) / CELL).min(SPAN as f32 - 0.02);
        let from = hash(n, 1) * (SPAN as f32 - 0.02 - long);
        // A twig gives a little to the wind; a trunk none.
        let free = |r: f32| ((0.16 - r) / 0.16).clamp(0.0, 1.0) * 0.25;
        let start = self.positions.len() as u32;
        for (end, (at, r)) in [(a, ra.max(0.03)), (b, rb.max(0.02))].into_iter().enumerate() {
            for k in 0..=SIDES {
                let turn = k as f32 / SIDES as f32 * std::f32::consts::TAU;
                let out = side * turn.cos() + other * turn.sin();
                let shade = 0.82 + 0.3 * hash(n, 7 + k as u32 % SIDES as u32);
                self.point(at + out * r, out, colour.map(|c| c * shade), grain.uv(k as f32 / SIDES as f32 * (SPAN as f32 - 0.02) + 0.01, from + end as f32 * long + 0.01), free(r));
            }
        }
        let ring = SIDES as u32 + 1;
        for k in 0..SIDES as u32 {
            self.indices.extend([start + k, start + ring + k, start + ring + k + 1, start + k, start + ring + k + 1, start + k + 1]);
        }
    }

    /// A mass of leaf: cards over the outside of an ellipsoid and a few within, each a piece
    /// of the leaf picture with its gaps, lit as the mass it is part of and not as a flat card.
    fn clump(&mut self, middle: Vec3, radii: Vec3, leaf: Block, n: u32) {
        let (top, under) = leaf.colors();
        let grain = if leaf == Block::Needles { Grain::Needles } else { Grain::Leaves };
        // The surface of an ellipsoid, near enough.
        let area = 4.0 * std::f32::consts::PI * ((radii.x * radii.y).powf(1.6) + (radii.x * radii.z).powf(1.6) + (radii.y * radii.z).powf(1.6)).powf(1.0 / 1.6) / 3.0f32.powf(1.0 / 1.6);
        let size = CARD.min(radii.min_element() * 1.1).max(0.3);
        let cards = ((area * COVER / (size * size)) as u32).clamp(4, 220);
        for i in 0..cards {
            let roll = |k: u32| hash(n.wrapping_mul(977).wrapping_add(i), k);
            // A point on the ball, pushed out to the surface or a little inside it.
            let (turn, up) = (roll(1) * std::f32::consts::TAU, roll(2) * 2.0 - 1.0);
            let flat = (1.0 - up * up).sqrt();
            let out = Vec3::new(turn.cos() * flat, up, turn.sin() * flat);
            let deep = 0.62 + 0.38 * roll(3).sqrt();
            let at = middle + out * radii * deep;
            // Facing mostly outward, tipped any way; lit as if it faced outward and a little up.
            let face = (out + Vec3::new(roll(4) - 0.5, roll(5) - 0.5, roll(6) - 0.5) * 1.2).normalize_or(Vec3::Y);
            let (u, v) = (face.any_orthonormal_vector(), face.cross(face.any_orthonormal_vector()));
            let spin = roll(7) * std::f32::consts::TAU;
            let (u, v) = (u * spin.cos() + v * spin.sin(), v * spin.cos() - u * spin.sin());
            let lit = (out * (radii.max_element() / radii) + Vec3::Y * 0.6).normalize_or(Vec3::Y);
            // Deeper in and lower down is in the crown's own shade.
            let shade = (0.5 + 0.6 * (deep - 0.62) / 0.38) * (0.82 + 0.22 * (up * 0.5 + 0.5)) * (0.9 + 0.2 * roll(8));
            let colour = if up > -0.2 { top } else { under }.map(|c| c * shade);
            // A piece of the picture as big as the card, anywhere in the tile.
            let span = (size / CELL).min(SPAN as f32 * 0.5);
            let (ou, ov) = (roll(9) * (SPAN as f32 - span - 0.02) + 0.01, roll(10) * (SPAN as f32 - span - 0.02) + 0.01);
            let half = size * (0.75 + 0.5 * roll(11)) * 0.5;
            let start = self.positions.len() as u32;
            for (su, sv) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                self.point(at + (u * su + v * sv) * half, lit, colour, grain.uv(ou + (su * 0.5 + 0.5) * span, ov + (sv * 0.5 + 0.5) * span), 0.45);
            }
            self.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
        }
    }
}

/// Keeps the meshes in step with the regions that are made.
fn grow(mut commands: Commands, terrain: Option<Res<Terrain>>, look: Option<Res<Look>>, forms: Res<Forms>, mut grown: ResMut<Grown>, mut meshes: ResMut<Assets<Mesh>>) {
    let (Some(_), Some(look)) = (terrain, look) else { return };
    grown.0.retain(|region, entity| {
        let keep = forms.0.contains_key(region);
        if !keep {
            commands.entity(*entity).despawn();
        }
        keep
    });
    let size = terrain::REGION as f32 * CELL;
    // A few a frame: a region's trees are some tens of thousands of points.
    let wanted: Vec<(i32, i32)> = forms.0.keys().filter(|r| !grown.0.contains_key(*r)).take(3).copied().collect();
    for region in wanted {
        let form = &forms.0[&region];
        let origin = Vec3::new(region.0 as f32 * size, 0.0, region.1 as f32 * size);
        let mut shape = Shape::default();
        for (n, limb) in form.limbs.iter().enumerate() {
            shape.limb(limb.a - origin, limb.b - origin, limb.ra, limb.rb, limb.bark, n as u32);
        }
        for (n, clump) in form.clumps.iter().enumerate() {
            shape.clump(clump.middle - origin, clump.radii, clump.leaf, n as u32 + 100_000);
        }
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, shape.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, shape.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, shape.colors)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, shape.uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, shape.tangents)
            .with_inserted_indices(Indices::U32(shape.indices));
        let entity = commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(look.solid.clone()), Transform::from_translation(origin))).id();
        grown.0.insert(region, entity);
    }
}
