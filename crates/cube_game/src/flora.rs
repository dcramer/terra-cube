//! What grows, drawn as what it is and not as the cubes that stand for it: trunks that swell
//! into their roots, limbs that wander and taper, and leaf in masses, sprigs of leaflets
//! near to and cards far off, that the wind moves. The simulation still has every tree as
//! cells (a body is stopped by a trunk, a squirrel will not climb a crown);
//! `cube_sim::plants` says, as it grows each thing, what shape it grew (`plants::Form`: its
//! limbs and its clumps of leaf), and that is drawn here in place of the cells.
//!
//! Why: a world whose trees are cubes is Minecraft, whatever its light and paint. The ground
//! is voxels; what grows on it is low poly, as Eco's is.

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
pub struct Grown(HashMap<(i32, i32), (Entity, u8)>);

impl Grown {
    /// Whether a region's trees and rocks have their meshes.
    pub fn has(&self, region: (i32, i32)) -> bool {
        self.0.contains_key(&region)
    }
}

/// How much of a leaf mass's surface its cards cover; how big a card is.
const COVER: f32 = 1.3;
const CARD: f32 = 0.8;
/// Metres from the camera within which leaf is drawn as sprigs.
const FINE: f32 = 36.0;
/// And beyond this, as a few broad cards and the largest masses' bodies: a tenth of the points.
const MID: f32 = 88.0;

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

    /// A limb from `a` to `b`, tapering: not a pipe but wood, which wanders a little along its
    /// length, is ridged along the grain (more the thicker it is, and most where a trunk
    /// meets the ground), and ends in a face, cut wood where nothing grows on from it.
    fn limb(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32, bark: Block, n: u32, cut: [bool; 2], roots: u8) {
        let along = (b - a).normalize_or(Vec3::Y);
        let side = along.any_orthonormal_vector();
        let other = along.cross(side);
        let colour = bark.colors().1;
        let grain = if bark == Block::Birch { Grain::BirchBark } else { Grain::Bark };
        let (ra, rb) = (ra.max(0.03), rb.max(0.02));
        let (thick, length) = (ra.max(rb), a.distance(b));
        let sides = if roots > 0 { 24 } else if thick > 0.28 { 12 } else if thick > 0.1 { 8 } else { 5 };
        // (Never more than a tile of bark between two rings: see where the picture is laid.)
        let rings = ((length / (thick * 2.5).max(0.7)).ceil() as usize).clamp(1, 7).max((length / 1.9).ceil() as usize);
        // Where its rings are, along it. A rooted trunk has more of them low down, where it
        // spreads: it begins half a metre under the ground (`plants::bole`).
        let mut stations: Vec<f32> = (0..=rings).map(|j| j as f32 / rings as f32).collect();
        const UNDER: f32 = 0.5;
        if roots > 0 {
            let low: Vec<f32> = [0.0f32, 0.25, 0.6, 1.1, 1.8, 2.8, 4.2].iter().map(|h| (UNDER - 0.12 + h * ra) / length).filter(|t| *t < 0.7).collect();
            let last = low.last().copied().unwrap_or(0.0);
            stations.retain(|t| *t == 0.0 || *t > last + 0.05);
            stations.splice(1..1, low);
        }
        let rings = stations.len() - 1;
        // The bark's picture once round; each limb begins somewhere of its own in it.
        let from = hash(n, 1) * SPAN as f32;
        // A twig gives a little to the wind; a trunk none.
        let free = |r: f32| ((0.16 - r) / 0.16).clamp(0.0, 1.0) * 0.25;
        // Ridges: three to five along a thick limb, deepest at the foot of an upright one.
        let (lobes, phase) = (if roots > 0 { roots as u32 } else { 3 + (hash(n, 2) * 3.0) as u32 }, hash(n, 3) * std::f32::consts::TAU);
        let upright = along.y > 0.8 && thick > 0.15;
        let start = self.positions.len() as u32;
        let ring = sides as u32 + 1;
        let mut middles = [a, b];
        // The bark's picture at its true size along the limb, whatever its length: each
        // stretch between two rings takes its own piece of the tile, so every ring but the
        // ends is laid twice, once as the top of one stretch and once as the foot of the
        // next. One tile pulled the length of a pine was bark drawn out five times too long.
        let cells = length / CELL;
        let room = SPAN as f32 - 0.04;
        let piece = |s: usize| {
            let (lo, long) = ((stations[s] * cells + from).rem_euclid(room), ((stations[s + 1] - stations[s]) * cells).min(room));
            let lo = if lo + long > room { room - long } else { lo };
            (lo + 0.02, lo + long + 0.02)
        };
        for (j, t) in stations.iter().copied().enumerate() {
          for along in [(j > 0).then(|| piece(j - 1).1), (j < rings).then(|| piece(j).0)].into_iter().flatten() {
            let hold = (t * std::f32::consts::PI).sin();
            let wander = (side * (hash(n, 20 + j as u32) - 0.5) + other * (hash(n, 40 + j as u32) - 0.5)) * thick.min(0.35) * 0.7 * hold;
            let at = a.lerp(b, t) + wander;
            let r = ra + (rb - ra) * t;
            if j == 0 {
                middles[0] = at;
            } else if j == rings {
                middles[1] = at;
            }
            // How far up from its low end, in its own thicknesses: the flare dies out by two.
            let foot = if upright { (1.0 - t * length / (thick * 4.0)).clamp(0.0, 1.0) } else { 0.0 };
            let deep = (0.05 + 0.22 * foot * foot) * (thick / 0.2).min(1.0);
            for k in 0..=sides {
                let turn = (k % sides) as f32 / sides as f32 * std::f32::consts::TAU;
                let out = side * turn.cos() + other * turn.sin();
                let mut ridge = 1.0 + deep * (turn * lobes as f32 + phase).cos() + 0.04 * (hash(n, 60 + (k % sides) as u32 + 16 * j as u32) - 0.5);
                if roots > 0 {
                    // The foot: the whole trunk swells toward the ground, and along each root
                    // it runs out in a sharp-backed buttress, each to its own length, the
                    // bark between them drawn in.
                    let high = ((t * length - UNDER + 0.12) / ra).max(0.0);
                    let wave = (turn * lobes as f32 + phase + 0.5 * (turn * 2.0 + phase).sin()).cos();
                    let back = wave.max(0.0).powf(2.2);
                    let which = ((turn * lobes as f32 + phase) / std::f32::consts::TAU).round() as i32;
                    // (A slender trunk's roots are slight: a birch does not stand on a star.)
                    let reach = (1.5 + 1.6 * hash(n, 300 + which.rem_euclid(lobes as i32) as u32)) * (ra / 0.35).clamp(0.3, 1.0);
                    ridge = 1.0 + 0.45 * (-high / 1.6).exp() + reach * back * (-high / 0.75).exp() - 0.1 * (1.0 - back) * (-high / 2.0).exp();
                }
                // Darker in the furrows and toward the ground, where it is damp.
                let shade = (0.8 + 0.25 * hash(n, 7 + (k % sides) as u32)) * (0.86 + 0.6 * (ridge - 1.0).clamp(-0.2, 0.2)) * (1.0 - 0.25 * foot);
                self.point(at + out * r * ridge, out, colour.map(|c| c * shade), grain.uv(k as f32 / sides as f32 * (SPAN as f32 - 0.02) + 0.01, along), free(r));
            }
          }
        }
        for j in 0..rings as u32 {
            for k in 0..sides as u32 {
                let (lo, hi) = (start + 2 * j * ring + k, start + (2 * j + 1) * ring + k);
                self.indices.extend([lo, hi, hi + 1, lo, hi + 1, lo + 1]);
            }
        }
        // The ends: bark grown over where a limb goes on, pale cut wood where it does not.
        for (end, r) in [(0usize, ra), (1, rb)] {
            if r < 0.07 {
                continue;
            }
            let (face, wood) = (if end == 0 { -along } else { along }, cut[end] && r > 0.12);
            let (paint, tint) = if wood { (Grain::EndGrain, bark.colors().0.map(|c| c * 1.5)) } else { (grain, colour.map(|c| c * 0.8)) };
            // Where another limb goes on from here the end is rounded over, so a bend has no gap.
            let dome = if cut[end] { 0.08 } else { 0.6 };
            let across = (2.0 * r / CELL).min(SPAN as f32 - 0.1);
            let first = start + if end == 0 { 0 } else { (2 * rings as u32 - 1) * ring };
            let hub = self.point(middles[end] + face * r * dome, face, tint, paint.uv(0.05 + across / 2.0, 0.05 + across / 2.0), 0.0);
            let rim = self.positions.len() as u32;
            for k in 0..=sides as u32 {
                let turn = (k % sides as u32) as f32 / sides as f32 * std::f32::consts::TAU;
                let at = Vec3::from(self.positions[(first + k) as usize]);
                self.point(at, face, tint.map(|c| c * 0.9), paint.uv(0.05 + across * (0.5 + 0.5 * turn.cos()), 0.05 + across * (0.5 + 0.5 * turn.sin())), 0.0);
            }
            for k in 0..sides as u32 {
                if end == 0 {
                    self.indices.extend([hub, rim + k, rim + k + 1]);
                } else {
                    self.indices.extend([hub, rim + k + 1, rim + k]);
                }
            }
        }
    }

    /// A mass of leaf: cards over the outside of an ellipsoid and a few within, each a piece
    /// of the leaf picture with its gaps, lit as the mass it is part of and not as a flat card.
    fn clump(&mut self, middle: Vec3, radii: Vec3, leaf: Block, n: u32, ball: &[[Vec3; 3]], level: u8) {
        let (top, under) = leaf.colors();
        // No two masses the one green: each a little yellower or bluer, lighter or darker.
        let warm = hash(n, 90) - 0.5;
        let tint = |c: [f32; 3], w: f32| [c[0] * (1.0 + 0.5 * w), c[1] * (1.0 + 0.16 * w), c[2] * (1.0 - 0.4 * w)];
        let (top, under) = (tint(top, warm), tint(under, warm));
        // A spray of needles lies flat along its bough; a mass of leaf is round.
        let spray = radii.y < radii.x.min(radii.z) * 0.6;
        let grain = if leaf == Block::Needles { Grain::Needles } else { Grain::Leaves };
        // The body of it: a dark faceted mass inside, so a crown has depth and the sky does
        // not show through its middle. The cards are its ragged outside.
        let fine = level == 0;
        // (Far off, only a mass big enough to be seen as one has a body.)
        if radii.min_element() > if level == 2 { 1.1 } else { 0.45 } {
            let lump = |d: Vec3| middle + d * radii * (0.62 + 0.16 * hash(n, 200 + ((d.x * 7.0 + d.y * 13.0 + d.z * 29.0) * 9.0) as i32 as u32));
            for corners in ball {
                let at = corners.map(lump);
                let normal = (at[1] - at[0]).cross(at[2] - at[0]).normalize_or(Vec3::Y);
                let shade = 0.42 + 0.3 * (normal.y * 0.5 + 0.5);
                let start = self.positions.len() as u32;
                for (i, p) in at.into_iter().enumerate() {
                    self.point(p, normal, under.map(|c| c * shade), grain.uv(1.0 + 2.5 * (i & 1) as f32, 1.0 + 2.5 * (i >> 1) as f32), 0.3);
                }
                self.indices.extend([start, start + 1, start + 2]);
            }
        }
        // The surface of an ellipsoid, near enough.
        let area = 4.0 * std::f32::consts::PI * ((radii.x * radii.y).powf(1.6) + (radii.x * radii.z).powf(1.6) + (radii.y * radii.z).powf(1.6)).powf(1.0 / 1.6) / 3.0f32.powf(1.0 / 1.6);
        let size = CARD.min(radii.min_element() * 1.1).max(0.3);
        let cards = ((area * COVER * [0.7, 0.28, 0.09][level as usize] / (size * size)) as u32).clamp([5, 2, 1][level as usize], 300);
        for i in 0..cards {
            let roll = |k: u32| hash(n.wrapping_mul(977).wrapping_add(i), k);
            // A point on the ball, pushed out to the surface or a little inside it.
            let (turn, up) = (roll(1) * std::f32::consts::TAU, roll(2) * 2.0 - 1.0);
            let flat = (1.0 - up * up).sqrt();
            let out = Vec3::new(turn.cos() * flat, up, turn.sin() * flat);
            let deep = 0.62 + 0.38 * roll(3).sqrt();
            let at = middle + out * radii * deep;
            // Facing mostly outward, tipped any way; lit as if it faced outward and a little up.
            let lean = Vec3::new(roll(4) - 0.5, roll(5) - 0.5, roll(6) - 0.5);
            let face = if spray { (Vec3::Y + out * Vec3::new(0.7, 0.0, 0.7) + lean * 0.7).normalize_or(Vec3::Y) } else { (out + lean * 1.2).normalize_or(Vec3::Y) };
            let (u, v) = (face.any_orthonormal_vector(), face.cross(face.any_orthonormal_vector()));
            let spin = roll(7) * std::f32::consts::TAU;
            let (u, v) = (u * spin.cos() + v * spin.sin(), v * spin.cos() - u * spin.sin());
            let lit = (out * (radii.max_element() / radii) + Vec3::Y * 0.6).normalize_or(Vec3::Y);
            // Deeper in and lower down is in the crown's own shade.
            let shade = (0.5 + 0.6 * (deep - 0.62) / 0.38) * (0.82 + 0.22 * (up * 0.5 + 0.5)) * (0.9 + 0.2 * roll(8));
            let colour = tint(if up > -0.2 { top } else { under }, (roll(12) - 0.5) * 0.7).map(|c| c * shade);
            // A piece of the picture as big as the card, anywhere in the tile.
            let span = (size / CELL).min(SPAN as f32 * 0.5);
            let (ou, ov) = (roll(9) * (SPAN as f32 - span - 0.02) + 0.01, roll(10) * (SPAN as f32 - span - 0.02) + 0.01);
            let half = size * (0.75 + 0.5 * roll(11)) * 0.5;
            if !fine {
                // Far off: one pointed card, larger, for the few there are.
                let start = self.positions.len() as u32;
                for (su, sv) in [(-1.3f32, 0.0f32), (0.0, -0.62), (1.3, 0.0), (0.0, 0.62)] {
                    self.point(at + (u * su + v * sv) * half * [1.0, 1.8, 2.8][level as usize], lit, colour, grain.uv(ou + (su / 2.6 + 0.5) * span, ov + (sv / 1.24 + 0.5) * span), 0.45);
                }
                self.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
                continue;
            }
            // Near to: a sprig. Leaflets fan from where it joins its twig, each pointed at
            // both ends and a little cupped; needles in a wider, narrower fan, as a bough's
            // end is. One flat card was a shard of paper however it was painted.
            let (blades, fan, wide) = if leaf == Block::Needles { (5, 1.9f32, 0.13f32) } else { (3, 1.25, 0.3) };
            let root = at - u * half * 1.1;
            for blade in 0..blades {
                let turn = (blade as f32 / (blades - 1) as f32 - 0.5) * fan + (roll(20 + blade) - 0.5) * 0.25;
                let (along, across) = (u * turn.cos() + v * turn.sin(), v * turn.cos() - u * turn.sin());
                let long = half * 2.3 * (1.0 - 0.3 * (turn / fan * 2.0).abs()) * (0.85 + 0.3 * roll(30 + blade));
                let cup = face * long * 0.08;
                let glow = 0.88 + 0.24 * roll(40 + blade);
                let start = self.positions.len() as u32;
                for (k, (sa, sc, up)) in [(0.0f32, 0.0f32, 0.0f32), (0.42, -wide, 1.0), (1.0, 0.0, 0.0), (0.42, wide, 1.0)].into_iter().enumerate() {
                    let _ = k;
                    self.point(root + along * sa * long + across * sc * long + cup * up, lit, colour.map(|c| c * glow), grain.uv(ou + sa * span, ov + (sc / wide * 0.5 + 0.5) * span * 0.5), 0.3 + 0.3 * sa);
                }
                self.indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
            }
        }
    }
}

impl Shape {
    /// A piece of rock as the shape it is: a ball of points, each drawn in to the nearest of
    /// the faces struck off the stone, and every facet lit flat, as broken rock is.
    fn stone(&mut self, stone: &cube_sim::plants::Stone, ball: &[[Vec3; 3]], n: u32) {
        let (sin, cos) = stone.turn.sin_cos();
        let (top, side) = stone.kind.colors();
        // Where the stone's surface is along a direction (in its own unit space), in the world.
        let surface = |d: Vec3| {
            let most = stone.faces.iter().filter(|(normal, _)| d.dot(*normal) > 1e-4).map(|(normal, far)| far / d.dot(*normal)).fold(1.0f32, f32::min);
            let q = d * most * stone.radii;
            stone.middle + Vec3::new(q.x * cos - q.z * sin, q.y, q.x * sin + q.z * cos)
        };
        for (f, corners) in ball.iter().enumerate() {
            let at = corners.map(surface);
            let normal = (at[1] - at[0]).cross(at[2] - at[0]).normalize_or(Vec3::Y);
            // Each facet a little of its own shade; tops paler, as weathered stone is.
            let shade = 0.85 + 0.3 * hash(n.wrapping_mul(131).wrapping_add(f as u32 / 4), 3);
            let colour = if normal.y > 0.55 { top } else { side }.map(|c| c * shade);
            let grain = if normal.y > 0.55 { Grain::Rock } else { Grain::Strata };
            let start = self.positions.len() as u32;
            for p in at {
                // The stone's picture by where the point is, kept inside its tile.
                let (u, v) = if normal.y.abs() > 0.55 { (p.x, p.z) } else if normal.x.abs() > normal.z.abs() { (p.z, -p.y) } else { (p.x, -p.y) };
                let within = |m: f32| ((m - (if normal.y.abs() > 0.55 { at[0].x } else { at[0].y }).floor()) / CELL).rem_euclid(SPAN as f32 - 0.1) + 0.05;
                self.point(p, normal, colour, grain.uv(within(u), within(v)), 0.0);
            }
            self.indices.extend([start, start + 1, start + 2]);
        }
    }
}

/// The faces of a ball of 320 triangles, each its three corners as directions.
fn ball() -> Vec<[Vec3; 3]> {
    ball_of(1)
}

/// The faces of a ball divided so many times.
fn ball_of(times: u32) -> Vec<[Vec3; 3]> {
    let mesh = Sphere::new(1.0).mesh().ico(times).unwrap();
    let Some(bevy::mesh::VertexAttributeValues::Float32x3(at)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { return Vec::new() };
    let Some(Indices::U32(index)) = mesh.indices() else { return Vec::new() };
    index.chunks(3).map(|t| [Vec3::from(at[t[0] as usize]).normalize(), Vec3::from(at[t[1] as usize]).normalize(), Vec3::from(at[t[2] as usize]).normalize()]).collect()
}

/// Keeps the meshes in step with the regions that are made.
fn grow(mut commands: Commands, terrain: Option<Res<Terrain>>, look: Option<Res<Look>>, forms: Res<Forms>, mut grown: ResMut<Grown>, mut meshes: ResMut<Assets<Mesh>>, cameras: Query<&GlobalTransform, With<Camera3d>>) {
    let (Some(_), Some(look)) = (terrain, look) else { return };
    grown.0.retain(|region, (entity, _)| {
        let keep = forms.0.contains_key(region);
        if !keep {
            commands.entity(*entity).despawn();
        }
        keep
    });
    let size = terrain::REGION as f32 * CELL;
    // Near the camera every leaf mass is sprigs of leaflets; farther off, plain cards, a
    // fifth of the points; far off, a few broad ones. Three levels, as the ground has.
    let eye = cameras.iter().next().map(|t| t.translation()).unwrap_or_default();
    let away = |r: &(i32, i32)| Vec2::new((r.0 as f32 + 0.5) * size - eye.x, (r.1 as f32 + 0.5) * size - eye.z).length();
    let level = |r: &(i32, i32)| {
        let far = away(r);
        let now = if far < FINE { 0u8 } else if far < MID { 1 } else { 2 };
        match grown.0.get(r) {
            // Next to the line it is on, it stays as it is.
            Some((_, was)) if was.abs_diff(now) == 1 && (far - if (*was).min(now) == 0 { FINE } else { MID }).abs() < 8.0 => *was,
            _ => now,
        }
    };
    // A few a frame, nearest first: a region's trees are some tens of thousands of points.
    let mut wanted: Vec<((i32, i32), u8)> = forms.0.keys().map(|r| (*r, level(r))).filter(|(r, level)| grown.0.get(r).is_none_or(|(_, was)| was != level)).collect();
    wanted.sort_by(|a, b| away(&a.0).total_cmp(&away(&b.0)));
    for (region, level) in wanted.into_iter().take(3) {
        if let Some((old, _)) = grown.0.remove(&region) {
            commands.entity(old).despawn();
        }
        let form = &forms.0[&region];
        let origin = Vec3::new(region.0 as f32 * size, 0.0, region.1 as f32 * size);
        let mut shape = Shape::default();
        // An end is cut wood if no other limb starts or stops there.
        let key = |p: Vec3| p.to_array().map(f32::to_bits);
        let mut ends: HashMap<[u32; 3], u32> = HashMap::new();
        for limb in &form.limbs {
            *ends.entry(key(limb.a)).or_default() += 1;
            *ends.entry(key(limb.b)).or_default() += 1;
        }
        for (n, limb) in form.limbs.iter().enumerate() {
            shape.limb(limb.a - origin, limb.b - origin, limb.ra, limb.rb, limb.bark, n as u32, [ends[&key(limb.a)] == 1, ends[&key(limb.b)] == 1], limb.roots);
        }
        let facets = ball();
        let coarse = ball_of(0);
        for (n, stone) in form.stones.iter().enumerate() {
            let mut stone = stone.clone();
            stone.middle -= origin;
            shape.stone(&stone, &facets, n as u32);
        }
        for (n, clump) in form.clumps.iter().enumerate() {
            shape.clump(clump.middle - origin, clump.radii, clump.leaf, n as u32 + 100_000, &coarse, level);
        }
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, shape.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, shape.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, shape.colors)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, shape.uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, shape.tangents)
            .with_inserted_indices(Indices::U32(shape.indices));
        let entity = commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(look.solid.clone()), Transform::from_translation(origin))).id();
        grown.0.insert(region, (entity, level));
    }
}
