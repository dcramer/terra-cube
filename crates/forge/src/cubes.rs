//! A creature rebuilt from cubes: `cargo forge cubes ID` takes a finished model and makes
//! `assets/creatures/ID-cubes/`, the same skeleton and clips with the skin replaced by little
//! cubes, so it is made of the same stuff as the world (a bit is 5 cm).
//!
//! The figure is cut into a dozen or so rigid parts, as games that draw characters in cubes
//! do (docs/research/2026-11-10-cube-characters.md): hips, chest, head, each limb in up to
//! three pieces. The model is stood as in its idle, each part straightened to the grid where
//! it is nearly straight already, its surface sampled into cubes and its inside filled, and
//! the legs moved apart until there is air between them. A cube takes the colour most of the
//! texture there has, moves with its part's bone and never bends, like a jointed toy.
//!
//! A face needs help: a head of true size is four cubes wide, and eyes are smaller than a
//! cube, so they vanish. The head can be made bigger first (`--head 1.4`: a toy's
//! proportions). A person's face is then made flat and one colour, with single cubes for
//! features: two eyes, a nose one cube proud, a mouth. An
//! animal keeps its muzzle and has its eyes darkened.

use crate::glb::Glb;
use crate::rig::{Facing, Leg, Roles, Skeleton};
use crate::{clips, spec};
use anyhow::{Context, Result, bail, ensure};
use glam::{IVec3, Mat4, Quat, Vec2, Vec3};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};

struct Cube {
    colour: Vec3,
    /// The rigid part it is of, and the bone that carries that part.
    part: usize,
    bone: usize,
}

/// What the surface samples that fall in a cube say of it.
#[derive(Default)]
struct Tally {
    colours: Vec<Vec3>,
}

fn light(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

impl Tally {
    /// The colour most of its samples have, not their average: a cube half tunic and half
    /// belt is tunic, where the average is a colour the model has nowhere.
    fn colour(&self) -> Vec3 {
        let bucket = |c: &Vec3| (c.powf(1.0 / 2.2) * 5.0).round().as_ivec3();
        let mut counts: HashMap<IVec3, (Vec3, f32)> = HashMap::new();
        for c in &self.colours {
            let entry = counts.entry(bucket(c)).or_default();
            *entry = (entry.0 + *c, entry.1 + 1.0);
        }
        counts.into_iter().max_by(|a, b| a.1.1.total_cmp(&b.1.1).then(a.0.to_array().cmp(&b.0.to_array()))).map_or(Vec3::splat(0.5), |(_, (sum, n))| sum / n)
    }

    /// How much of it is near black: a pupil is, and hair, fur and shadow are not.
    fn dark(&self) -> f32 {
        self.colours.iter().filter(|c| light(**c) < 0.03).count() as f32 / self.colours.len().max(1) as f32
    }
}

/// A face: its normal, then its two in-plane axes (u, v) such that u × v = normal.
const FACES: [(IVec3, IVec3, IVec3); 6] = [
    (IVec3::X, IVec3::Y, IVec3::Z),
    (IVec3::NEG_X, IVec3::Z, IVec3::Y),
    (IVec3::Y, IVec3::Z, IVec3::X),
    (IVec3::NEG_Y, IVec3::X, IVec3::Z),
    (IVec3::Z, IVec3::X, IVec3::Y),
    (IVec3::NEG_Z, IVec3::Y, IVec3::X),
];

fn linear(srgb: u8) -> f32 {
    let c = srgb as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// The model's colour texture, if it has one.
fn texture(glb: &Glb, primitive: &Value) -> Option<image::RgbImage> {
    let material = &glb.array("materials")[primitive["material"].as_u64()? as usize];
    let texture = &glb.array("textures")[material["pbrMetallicRoughness"]["baseColorTexture"]["index"].as_u64()? as usize];
    let view = &glb.array("bufferViews")[glb.array("images")[texture["source"].as_u64()? as usize]["bufferView"].as_u64()? as usize];
    let from = view["byteOffset"].as_u64().unwrap_or(0) as usize;
    image::load_from_memory(&glb.bin[from..from + view["byteLength"].as_u64()? as usize]).ok().map(|i| i.to_rgb8())
}

/// A rigid piece of the figure: the bone that carries it, and how its share of the skin is
/// moved from where the model stands to where the toy's piece is cut (straightened, and for
/// the head, grown).
struct Part {
    driver: usize,
    /// The joint its direction is read toward, when it has one worth straightening along.
    tip: Option<usize>,
    to_toy: Mat4,
    turn: Quat,
    cells: HashMap<IVec3, Tally>,
}

/// Which part each bone belongs to (by the part's place in the list), and the parts, parents
/// before children. A dozen or so whatever the skeleton: hips, chest, head, each limb in up
/// to three pieces, each tail bone. Fingers, toes, ears and collar bones go with what they
/// hang from.
fn parts(sk: &Skeleton, roles: &Roles) -> (Vec<usize>, Vec<Part>) {
    const NONE: usize = usize::MAX;
    let mut driver = vec![NONE; sk.joints.len()];
    let mut tips: HashMap<usize, usize> = HashMap::new();
    driver[sk.root] = sk.root;
    let chest = roles.spine.get(roles.spine.len() * 2 / 3).copied();
    for &bone in &roles.spine {
        driver[bone] = chest.unwrap_or(sk.root);
    }
    if let Some((&neck, rest)) = roles.head.split_first() {
        driver[neck] = chest.unwrap_or(neck);
        if let Some(c) = chest {
            tips.insert(c, neck);
        }
        let head = rest.first().copied().unwrap_or(neck);
        for &bone in rest {
            driver[bone] = head;
        }
        tips.insert(head, *roles.head.last().unwrap());
    }
    // A limb: up to three pieces, the last taking the rest of the chain.
    let mut limb = |chain: &[usize], driver: &mut Vec<usize>| {
        for (i, &bone) in chain.iter().enumerate() {
            driver[bone] = chain[i.min(2)];
            if i < 2 && i + 1 < chain.len() {
                tips.insert(bone, chain[i + 1]);
            }
        }
        if chain.len() > 3 {
            tips.insert(chain[2], *chain.last().unwrap());
        }
    };
    for leg in &roles.legs {
        limb(&leg.chain, &mut driver);
    }
    for arm in &roles.arms {
        // A collar bone is part of the chest.
        let chain = if arm.chain.len() >= 4 { &arm.chain[1..] } else { &arm.chain[..] };
        if arm.chain.len() >= 4 {
            driver[arm.chain[0]] = chest.unwrap_or(arm.chain[0]);
        }
        limb(chain, &mut driver);
    }
    for pair in roles.tail.windows(2) {
        tips.insert(pair[0], pair[1]);
    }
    for &bone in &roles.tail {
        driver[bone] = bone;
    }
    // Everything else goes with the nearest thing above it that is spoken for.
    for bone in 0..sk.joints.len() {
        let mut at = bone;
        while driver[at] == NONE {
            at = sk.joints[at].parent.unwrap_or(sk.root);
        }
        driver[bone] = driver[at];
    }
    let depth = |mut bone: usize| {
        let mut d = 0;
        while let Some(p) = sk.joints[bone].parent {
            (bone, d) = (p, d + 1);
        }
        d
    };
    let mut drivers: Vec<usize> = driver.clone();
    drivers.sort_by_key(|&d| (depth(d), d));
    drivers.dedup();
    let part_of: Vec<usize> = driver.iter().map(|d| drivers.iter().position(|x| x == d).unwrap()).collect();
    let made = drivers.iter().map(|&d| Part { driver: d, tip: tips.get(&d).copied(), to_toy: Mat4::IDENTITY, turn: Quat::IDENTITY, cells: HashMap::new() }).collect();
    (part_of, made)
}

/// The colour of each cube of one part, its inside filled: whatever the outside cannot reach
/// from a corner of the box round it takes after the nearest surface cube, so a limb that
/// turns shows no hollow.
fn solid(cells: &HashMap<IVec3, Tally>) -> HashMap<IVec3, Vec3> {
    let mut out: HashMap<IVec3, Vec3> = cells.iter().map(|(at, t)| (*at, t.colour())).collect();
    if out.is_empty() {
        return out;
    }
    let (lo, hi) = out.keys().fold((IVec3::MAX, IVec3::MIN), |(lo, hi), k| (lo.min(*k), hi.max(*k)));
    let (lo, hi) = (lo - 1, hi + 1);
    let inside_box = |p: IVec3| p.cmpge(lo).all() && p.cmple(hi).all();
    let mut outside = std::collections::HashSet::from([lo]);
    let mut queue = VecDeque::from([lo]);
    while let Some(at) = queue.pop_front() {
        for (n, ..) in FACES {
            let next = at + n;
            if inside_box(next) && !out.contains_key(&next) && outside.insert(next) {
                queue.push_back(next);
            }
        }
    }
    let mut keys: Vec<IVec3> = out.keys().copied().collect();
    keys.sort_by_key(|k| k.to_array());
    let mut queue: VecDeque<IVec3> = keys.into();
    while let Some(at) = queue.pop_front() {
        let colour = out[&at];
        for (n, ..) in FACES {
            let next = at + n;
            if inside_box(next) && !outside.contains(&next) && !out.contains_key(&next) {
                out.insert(next, colour);
                queue.push_back(next);
            }
        }
    }
    out
}

/// What the checks need to know of how a figure was cut, beside its cubes.
#[derive(Default)]
pub struct Cut {
    /// Each pair of legs, as the parts of one and of the other.
    legs: Vec<(Vec<usize>, Vec<usize>)>,
    /// The upper and middle pieces of every limb: what must not be spindly.
    limbs: Vec<usize>,
    parts: usize,
    /// It has arms, so it is given a face; and the face's width and height in cubes, if one
    /// was made.
    person: bool,
    face: Option<(i32, i32)>,
}

/// Tidies one part's cubes, as a person drawing in cubes would: a cube that hangs on by one
/// face or none (a finger, a wisp of hair, a crumb of hem) is taken off, twice over; then a
/// cube whose colour none of its neighbours shares takes the colour most of them have. A
/// part of a dozen cubes or fewer is left alone: it is all it has.
fn tidy(cells: &mut HashMap<IVec3, Vec3>) {
    if cells.len() <= 12 {
        return;
    }
    for _ in 0..2 {
        let loose: Vec<IVec3> = cells.keys().filter(|at| FACES.iter().filter(|(n, ..)| cells.contains_key(&(**at + *n))).count() <= 1).copied().collect();
        for at in loose {
            cells.remove(&at);
        }
    }
    let bucket = |c: Vec3| (c.powf(1.0 / 2.2) * 5.0).round().as_ivec3();
    let mut keys: Vec<IVec3> = cells.keys().copied().collect();
    keys.sort_by_key(|k| k.to_array());
    let before = cells.clone();
    for at in keys {
        let near: Vec<Vec3> = FACES.iter().filter_map(|(n, ..)| before.get(&(at + *n)).copied()).collect();
        if near.len() < 3 || near.iter().any(|c| bucket(*c) == bucket(before[&at])) {
            continue;
        }
        let mut counts: HashMap<IVec3, (Vec3, f32)> = HashMap::new();
        for c in &near {
            let entry = counts.entry(bucket(*c)).or_default();
            *entry = (entry.0 + *c, entry.1 + 1.0);
        }
        let (sum, n) = counts.into_iter().max_by(|a, b| a.1.1.total_cmp(&b.1.1).then(a.0.to_array().cmp(&b.0.to_array()))).unwrap().1;
        cells.insert(at, sum / n);
    }
}

/// The figure held to the bars in style.toml `[cubes]`: one line a check, `ok` or `FAIL`, each
/// for something that was first caught by eye. docs/pipeline.md says what to do about each.
fn judge(made: &HashMap<IVec3, Cube>, cut: &Cut, bars: &spec::CubeBars, triangles: usize, max_triangles: usize) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut line = |ok: bool, name: &str, value: String, bar: String| out.push((ok, format!("{} {name:<12} {value} ({bar})", if ok { "ok  " } else { "FAIL" })));
    let of = |ids: &[usize]| -> Vec<IVec3> { made.iter().filter(|(_, c)| ids.contains(&c.part)).map(|(at, _)| *at).collect() };
    let (lo, hi) = made.keys().fold((i32::MAX, i32::MIN), |(lo, hi), k| (lo.min(k.y), hi.max(k.y)));
    line(hi - lo + 1 >= bars.min_tall, "tall", format!("{} cubes", hi - lo + 1), format!("≥ {}: smaller wants drawing for the grid, not converting", bars.min_tall));
    line(triangles <= max_triangles, "triangles", triangles.to_string(), format!("≤ {max_triangles}"));
    line((6..=24).contains(&cut.parts), "parts", cut.parts.to_string(), "6 to 24".into());

    // Legs: at every height both have cubes, a column of air between them.
    let mut air = i32::MAX;
    for (a, b) in &cut.legs {
        let (a, b) = (of(a), of(b));
        for y in lo..=hi {
            let row = |cells: &[IVec3]| cells.iter().filter(|c| c.y == y).map(|c| c.x).fold((i32::MAX, i32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
            let (ra, rb) = (row(&a), row(&b));
            if ra.0 != i32::MAX && rb.0 != i32::MAX {
                air = air.min(if ra.0 > rb.1 { ra.0 - rb.1 - 1 } else { rb.0 - ra.1 - 1 });
            }
        }
    }
    if air != i32::MAX {
        line(air >= 1, "legs-apart", format!("{air} cubes of air at the closest"), "≥ 1".into());
    }

    // Limbs: how thick the thinnest is, across its length.
    let mut thinnest = (i32::MAX, 0);
    for &part in &cut.limbs {
        let cells = of(&[part]);
        if cells.is_empty() {
            continue;
        }
        let (lo, hi) = cells.iter().fold((IVec3::MAX, IVec3::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
        let long = (hi - lo).max_position();
        let mut across: Vec<i32> = (lo[long]..=hi[long])
            .filter_map(|at| {
                let slice: Vec<&IVec3> = cells.iter().filter(|c| c[long] == at).collect();
                let (slo, shi) = slice.iter().fold((IVec3::MAX, IVec3::MIN), |(lo, hi), c| (lo.min(**c), hi.max(**c)));
                (!slice.is_empty()).then(|| (0..3).filter(|&axis| axis != long).map(|axis| shi[axis] - slo[axis] + 1).min().unwrap())
            })
            .collect();
        across.sort();
        let thick = across[across.len() / 2];
        if thick < thinnest.0 {
            thinnest = (thick, cells.len());
        }
    }
    if thinnest.0 != i32::MAX {
        line(thinnest.0 >= bars.min_limb, "limbs", format!("thinnest {} cubes across", thinnest.0), format!("≥ {}: thinner reads as noise", bars.min_limb));
    }

    // Loose cubes: holding on by one face or none, within their part.
    let part_at = |at: IVec3| made.get(&at).map(|c| c.part);
    let loose = made.iter().filter(|(at, c)| FACES.iter().filter(|(n, ..)| part_at(**at + *n) == Some(c.part)).count() <= 1).count() as f32 / made.len() as f32;
    line(loose <= bars.max_loose, "loose", format!("{:.1}% of cubes", loose * 100.0), format!("≤ {:.0}%: fingers, wisps, crumbs", bars.max_loose * 100.0));

    // Speckle: cubes on the surface that share their colour with no neighbour.
    let bucket = |c: Vec3| (c.powf(1.0 / 2.2) * 5.0).round().as_ivec3();
    let surface: Vec<(&IVec3, &Cube)> = made.iter().filter(|(at, _)| FACES.iter().any(|(n, ..)| !made.contains_key(&(**at + *n)))).collect();
    let alone = surface.iter().filter(|(at, c)| !FACES.iter().any(|(n, ..)| made.get(&(**at + *n)).is_some_and(|o| bucket(o.colour) == bucket(c.colour)))).count() as f32 / surface.len().max(1) as f32;
    line(alone <= bars.max_speckle, "speckle", format!("{:.1}% of surface cubes", alone * 100.0), format!("≤ {:.0}%: one-cube spots of colour", bars.max_speckle * 100.0));

    if cut.person {
        let (wide, tall) = cut.face.unwrap_or((0, 0));
        line(wide >= 3 && tall >= 3, "face", format!("{wide} wide, {tall} tall"), "≥ 3 by 3: room for eyes, a nose and a mouth".into());
    }
    out
}

/// The cubes of a model stood in a pose, as a toy of rigid parts: each part straightened to
/// the grid where it is nearly straight already, the head grown, each cut into cubes on its
/// own, and the legs moved apart until there is air between them.
fn cubes(glb: &Glb, sk: &Skeleton, roles: &Roles, world: &[Mat4], size: f32, head_scale: f32, cut: &mut Cut) -> Result<HashMap<IVec3, Cube>> {
    let (part_of, mut parts) = parts(sk, roles);
    let at = |joint: usize| world[joint].w_axis.truncate();
    let head = roles.head.get(1).map(|&h| part_of[h]);
    for i in 0..parts.len() {
        let driver = parts[i].driver;
        let Some(parent) = sk.joints[driver].parent.map(|p| part_of[p]).filter(|&p| p != i) else { continue };
        let (above, above_turn) = (parts[parent].to_toy, parts[parent].turn);
        let pivot = at(driver);
        // Along the nearest axis, if it is within 25° of one: a leg that leans a little
        // stands straight; a wolf's neck, which slopes, is left sloping.
        let mut turn = above_turn;
        if let Some(way) = parts[i].tip.and_then(|tip| (at(tip) - pivot).try_normalize()) {
            let now = above_turn * way;
            let axis = now.abs().max_position();
            let straight = Vec3::AXES[axis] * now[axis].signum();
            if now.angle_between(straight) < 25f32.to_radians() {
                turn = Quat::from_rotation_arc(now, straight) * above_turn;
            }
        }
        let grow = if Some(i) == head { head_scale } else { 1.0 };
        parts[i].turn = turn;
        parts[i].to_toy = Mat4::from_translation(above.transform_point3(pivot)) * Mat4::from_quat(turn) * Mat4::from_scale(Vec3::splat(grow)) * Mat4::from_translation(-pivot);
    }

    let skin: Vec<Mat4> = world.iter().zip(&sk.inverse_bind).map(|(w, ib)| *w * *ib).collect();
    for mesh in glb.array("meshes") {
        for p in mesh["primitives"].as_array().into_iter().flatten() {
            let attr = |name: &str| p["attributes"][name].as_u64().map(|a| a as usize);
            let (Some(pos), Some(j), Some(w)) = (attr("POSITION"), attr("JOINTS_0"), attr("WEIGHTS_0")) else { continue };
            let rest: Vec<Vec3> = glb.floats(pos)?.0.chunks(3).map(Vec3::from_slice).collect();
            let joints: Vec<[usize; 4]> = glb.floats(j)?.0.chunks(4).map(|c| [c[0] as usize, c[1] as usize, c[2] as usize, c[3] as usize]).collect();
            let weights: Vec<[f32; 4]> = glb.floats(w)?.0.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
            let uvs: Vec<Vec2> = match attr("TEXCOORD_0") {
                Some(a) => glb.floats(a)?.0.chunks(2).map(Vec2::from_slice).collect(),
                None => vec![Vec2::ZERO; rest.len()],
            };
            let picture = texture(glb, p);
            let stood: Vec<Vec3> = (0..rest.len()).map(|i| (0..4).map(|k| skin[joints[i][k]].transform_point3(rest[i]) * weights[i][k]).sum::<Vec3>() / weights[i].iter().sum::<f32>().max(1e-6)).collect();
            let index: Vec<usize> = match p["indices"].as_u64() {
                Some(a) => glb.floats(a as usize)?.0.iter().map(|&i| i as usize).collect(),
                None => (0..rest.len()).collect(),
            };
            for t in index.chunks_exact(3) {
                let [a, b, c] = [stood[t[0]], stood[t[1]], stood[t[2]]];
                // Points across the triangle, closer together than a third of a cube.
                let n = ((a.distance(b).max(b.distance(c)).max(c.distance(a)) / (size / 3.0)).ceil() as usize).max(1);
                for i in 0..=n {
                    for k in 0..=n - i {
                        let (u, v) = (i as f32 / n as f32, k as f32 / n as f32);
                        let share = [1.0 - u - v, u, v];
                        let point = a * share[0] + b * share[1] + c * share[2];
                        let uv = uvs[t[0]] * share[0] + uvs[t[1]] * share[1] + uvs[t[2]] * share[2];
                        let colour = picture.as_ref().map_or(Vec3::splat(0.5), |img| {
                            let (x, y) = ((uv.x.rem_euclid(1.0) * img.width() as f32) as u32, (uv.y.rem_euclid(1.0) * img.height() as f32) as u32);
                            let px = img.get_pixel(x.min(img.width() - 1), y.min(img.height() - 1));
                            Vec3::new(linear(px[0]), linear(px[1]), linear(px[2]))
                        });
                        // The part that pulls on this point most has it.
                        let mut pull = vec![0.0f32; parts.len()];
                        for (corner, share) in t.iter().zip(share) {
                            for k in 0..4 {
                                pull[part_of[joints[*corner][k]]] += weights[*corner][k] * share;
                            }
                        }
                        let part = (0..parts.len()).max_by(|a, b| pull[*a].total_cmp(&pull[*b]).then(b.cmp(a))).unwrap();
                        let cell = (parts[part].to_toy.transform_point3(point) / size).floor().as_ivec3();
                        parts[part].cells.entry(cell).or_default().colours.push(colour);
                    }
                }
            }
        }
    }
    ensure!(parts.iter().any(|p| !p.cells.is_empty()), "no skinned mesh to make cubes of");

    // Legs apart: each pair is moved out, a cube at a time, until a column of air is between
    // them. Fused legs are what makes a figure of cubes look wrong.
    let mut shift = vec![0i32; parts.len()];
    let pairs: Vec<(&Leg, &Leg)> = match &roles.legs[..] {
        [a, b] => vec![(a, b)],
        legs => legs.iter().filter(|l| l.left).filter_map(|l| legs.iter().find(|r| !r.left && r.front == l.front).map(|r| (l, r))).collect(),
    };
    for (a, b) in pairs {
        let of = |leg: &Leg| -> Vec<usize> { (0..parts.len()).filter(|&i| leg.chain.contains(&parts[i].driver)).collect() };
        let span = |ids: &[usize]| ids.iter().flat_map(|&i| parts[i].cells.keys().map(|c| c.x)).fold((i32::MAX, i32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
        let (mut plus, mut minus) = (of(a), of(b));
        if span(&plus).0 == i32::MAX || span(&minus).0 == i32::MAX {
            continue;
        }
        cut.legs.push((plus.clone(), minus.clone()));
        if span(&plus).0 + span(&plus).1 < span(&minus).0 + span(&minus).1 {
            std::mem::swap(&mut plus, &mut minus);
        }
        let air = span(&plus).0 - span(&minus).1 - 1;
        if air < 1 {
            let need = 1 - air;
            plus.iter().for_each(|&i| shift[i] = (need + 1) / 2);
            minus.iter().for_each(|&i| shift[i] = -(need / 2));
        }
    }

    cut.parts = parts.iter().filter(|p| !p.cells.is_empty()).count();
    cut.person = !roles.arms.is_empty();
    cut.limbs = roles.legs.iter().chain(&roles.arms).flat_map(|l| l.chain.iter().map(|&bone| part_of[bone]).collect::<std::collections::BTreeSet<_>>().into_iter().rev().skip(1).collect::<Vec<_>>()).filter(|&i| Some(i) != roles.spine.first().map(|&s| part_of[s]) && parts[i].driver != sk.root && roles.spine.iter().all(|&s| part_of[s] != i)).collect();
    // One figure: a cell two parts both fill belongs to the one nearer the root.
    let mut out: HashMap<IVec3, Cube> = HashMap::new();
    let mut eyes: HashMap<IVec3, f32> = HashMap::new();
    for (i, part) in parts.iter().enumerate() {
        let mut cells = solid(&part.cells);
        tidy(&mut cells);
        let mut cells: Vec<(IVec3, Vec3)> = cells.into_iter().collect();
        cells.sort_by_key(|c| c.0.to_array());
        for (cell, colour) in cells {
            let moved = cell + IVec3::X * shift[i];
            if let std::collections::hash_map::Entry::Vacant(free) = out.entry(moved) {
                free.insert(Cube { colour, part: i, bone: part.driver });
                if Some(i) == head {
                    eyes.insert(moved, part.cells.get(&cell).map_or(0.0, Tally::dark));
                }
            }
        }
    }

    // Eyes: on the front of the head (a finished model faces +Z), the pair of cubes mirrored
    // across its middle, at least a cube apart, with the most black in them.
    let front: Vec<IVec3> = eyes.keys().filter(|at| !(1..4).any(|ahead| eyes.contains_key(&(**at + IVec3::Z * ahead)))).copied().collect();
    let mirrored = |at: &IVec3| IVec3::new(-1 - at.x, at.y, at.z);
    let found = front
        .iter()
        .filter(|at| at.x >= 1 && front.contains(&mirrored(at)))
        .map(|at| (*at, eyes[at].min(eyes[&mirrored(at)])))
        .filter(|(_, dark)| *dark > 0.04)
        .max_by(|a, b| a.1.total_cmp(&b.1).then(a.0.to_array().cmp(&b.0.to_array())))
        .map(|(at, _)| at);
    let Some(head) = head else { return Ok(out) };
    let driver = parts[head].driver;
    if roles.arms.is_empty() {
        // An animal keeps the face it has, muzzle and all, with its eyes darkened.
        for eye in found.iter().flat_map(|at| [*at, mirrored(at)]) {
            out.get_mut(&eye).unwrap().colour = Vec3::splat(0.012);
        }
        return Ok(out);
    }

    // A person's face is made flat, as games that draw people in cubes make it: at this size
    // a modelled brow and cheek only hide the eyes. The skin on the front of the head is
    // brought to one plane and one colour, and the features are single cubes on it: two eyes,
    // a nose that stands one cube proud, a mouth.
    let lowest = front.iter().map(|c| c.y).min().unwrap_or(0);
    let highest = front.iter().map(|c| c.y).max().unwrap_or(0);
    let bucket = |c: Vec3| (c.powf(1.0 / 2.2) * 5.0).round().as_ivec3();
    let mut counts: HashMap<IVec3, (Vec3, f32)> = HashMap::new();
    // Skin is what the bottom third of the head's front mostly is: hair can be the top half.
    for at in front.iter().filter(|c| c.y <= lowest + (highest - lowest) / 3) {
        let entry = counts.entry(bucket(out[at].colour)).or_default();
        *entry = (entry.0 + out[at].colour, entry.1 + 1.0);
    }
    let Some(skin) = counts.into_iter().max_by(|a, b| a.1.1.total_cmp(&b.1.1).then(a.0.to_array().cmp(&b.0.to_array()))).map(|(_, (sum, n))| sum / n) else { return Ok(out) };
    let skinned: Vec<IVec3> = front.iter().filter(|at| out[*at].colour.distance(skin) < 0.12).copied().collect();
    if skinned.len() < 6 {
        return Ok(out);
    }
    // The face is a box on the front of the head: as wide as the skin showing, from the chin
    // up to the highest row that is nearly half skin (above that is fringe and is left alone).
    let (left, right) = (skinned.iter().map(|c| c.x).min().unwrap(), skinned.iter().map(|c| c.x).max().unwrap());
    let bottom = skinned.iter().map(|c| c.y).min().unwrap();
    let across = |y: i32| skinned.iter().filter(|c| c.y == y).count() as i32;
    let widest = (bottom..=highest).map(across).max().unwrap_or(0);
    let brow = (bottom..=highest).rev().find(|&y| across(y) * 10 >= widest * 4).unwrap_or(bottom);
    let mut depths: Vec<i32> = skinned.iter().map(|c| c.z).collect();
    depths.sort();
    let plane = depths[depths.len() / 2];
    // Everything on the front of the head inside the box is brought to one plane and one
    // colour: what stands proud goes, what falls short is filled.
    for at in front.iter().filter(|c| (left..=right).contains(&c.x) && (bottom..=brow).contains(&c.y)) {
        for z in plane + 1..=at.z {
            out.remove(&IVec3::new(at.x, at.y, z));
        }
        for z in at.z.min(plane)..=plane {
            out.insert(IVec3::new(at.x, at.y, z), Cube { colour: skin, part: head, bone: driver });
        }
    }
    cut.face = Some((right - left + 1, brow - bottom + 1));
    let mut feature = |x: i32, y: i32, z: i32, colour: Vec3| {
        if (left..=right).contains(&x) && (bottom..=brow).contains(&y) {
            out.insert(IVec3::new(x, y, z), Cube { colour, part: head, bone: driver });
        }
    };
    // Features are placed, not found: eyes on the top row either side of the middle, the nose
    // under them one cube proud, the mouth lower. A face an odd number of cubes wide has a
    // middle cube, so its nose and mouth are one cube; an even one's are two.
    let wide = right - left + 1;
    let middle: Vec<i32> = if wide % 2 == 1 { vec![left + wide / 2] } else { vec![left + wide / 2 - 1, left + wide / 2] };
    let out_by = if wide >= 5 { 2 } else { 1 };
    for x in [middle[0] - out_by, middle[middle.len() - 1] + out_by] {
        feature(x, brow, plane, Vec3::splat(0.012));
    }
    for &x in &middle {
        feature(x, brow - 1, plane + 1, skin * 0.82);
        feature(x, (brow - 3).max(bottom), plane, Vec3::new(0.30, 0.07, 0.06));
    }
    Ok(out)
}

/// Makes `ID-cubes` from the finished model of `id`, its head `head_scale` times its size.
/// Returns one line of what it made, and the checks it was held to.
pub fn run(id: &str, size: f32, head_scale: f32, style: &spec::Style) -> Result<(String, Vec<(bool, String)>)> {
    let from = spec::dir(id);
    let mut glb = Glb::read(&from.join("model.glb"))?;
    let sk = Skeleton::read(&glb)?;
    let clips = clips::read(&glb, &sk)?;
    let stand = clips.iter().find(|c| c.name == "idle").map_or(sk.rest(), |c| c.frames[0].clone());
    let world = sk.pose_world(&stand);
    let mut meta: spec::Meta = spec::load(&from.join("meta.toml"))?;
    let roles = Roles::read(&sk, Facing::Told(Vec3::Z));
    let mut cut = Cut::default();
    let made = cubes(&glb, &sk, &roles, &world, size, head_scale, &mut cut)?;

    // Every face of a cube that is not against another cube of the same part, taken back to
    // where its bone has it at rest, which is where a skin's vertices are kept.
    let back: Vec<Mat4> = world.iter().zip(&sk.inverse_bind).map(|(w, ib)| (*w * *ib).inverse()).collect();
    let mut keys: Vec<IVec3> = made.keys().copied().collect();
    keys.sort_by_key(|k| (k.y, k.z, k.x));
    let (mut positions, mut normals, mut colours, mut weights) = (Vec::<f32>::new(), Vec::<f32>::new(), Vec::<f32>::new(), Vec::<f32>::new());
    let (mut joints, mut indices) = (Vec::<u8>::new(), Vec::<u8>::new());
    let (mut lo, mut hi, mut count) = (Vec3::MAX, Vec3::MIN, 0u32);
    for at in keys {
        let cube = &made[&at];
        // A little lighter or darker a cube, as the ground is, so a flat colour has grain.
        let grain = 0.92 + 0.16 * ((at.x.wrapping_mul(73856093) ^ at.y.wrapping_mul(19349663) ^ at.z.wrapping_mul(83492791)) as u32 % 1000) as f32 / 1000.0;
        for (n, u, v) in FACES {
            if made.get(&(at + n)).is_some_and(|other| other.part == cube.part) {
                continue;
            }
            let normal = back[cube.bone].transform_vector3(n.as_vec3()).normalize();
            for (su, sv) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                let corner = back[cube.bone].transform_point3((at + n.max(IVec3::ZERO) + u * su + v * sv).as_vec3() * size);
                (lo, hi) = (lo.min(corner), hi.max(corner));
                positions.extend(corner.to_array());
                normals.extend(normal.to_array());
                colours.extend([cube.colour.x * grain, cube.colour.y * grain, cube.colour.z * grain, 1.0]);
                weights.extend([1.0, 0.0, 0.0, 0.0]);
                joints.extend([cube.bone as u16, 0, 0, 0].iter().flat_map(|j| j.to_le_bytes()));
            }
            indices.extend([count, count + 1, count + 2, count, count + 2, count + 3].iter().flat_map(|i| i.to_le_bytes()));
            count += 4;
        }
    }

    let meshes = glb.json["meshes"].as_array().map_or(0, Vec::len);
    if meshes != 1 {
        bail!("{id} has {meshes} meshes; cubes are made of a model with one");
    }
    let position = glb.add_floats(&positions, "VEC3", false);
    glb.json["accessors"][position]["min"] = json!(lo.to_array());
    glb.json["accessors"][position]["max"] = json!(hi.to_array());
    let attributes = json!({
        "POSITION": position,
        "NORMAL": glb.add_floats(&normals, "VEC3", false),
        "COLOR_0": glb.add_floats(&colours, "VEC4", false),
        "WEIGHTS_0": glb.add_floats(&weights, "VEC4", false),
        "JOINTS_0": glb.add_raw(&joints, 5123, "VEC4", count as usize),
    });
    let index = glb.add_raw(&indices, 5125, "SCALAR", indices.len() / 4);
    // Its colour is in its cubes: no texture, so none is kept.
    glb.json["materials"] = json!([{ "name": "cubes", "pbrMetallicRoughness": { "metallicFactor": 0.0, "roughnessFactor": 0.9 } }]);
    glb.json["meshes"][0]["primitives"] = json!([{ "attributes": attributes, "indices": index, "material": 0 }]);
    for unused in ["textures", "images", "samplers"] {
        glb.json.as_object_mut().context("a GLB's JSON is an object")?.remove(unused);
    }
    glb.drop_unused();

    let to = spec::dir(&format!("{id}-cubes"));
    glb.write(&to.join("model.glb"))?;
    meta.triangles = glb.triangles();
    spec::save(&to.join("meta.toml"), &meta, &format!("# Made by `cargo forge cubes {id}` from that model; do not edit.\n"))?;
    // The same creature, marked as made from another so forge does not try to build it, and
    // with no place in the world of its own.
    let mut text = String::new();
    for line in std::fs::read_to_string(from.join("creature.toml"))?.lines() {
        if line.trim() == "[life]" {
            break;
        }
        text += &match line.strip_prefix("name = \"") {
            Some(name) => format!("name = \"{} (cubes)\"\n", name.trim_end_matches('"')),
            None => format!("{line}\n"),
        };
    }
    std::fs::write(to.join("creature.toml"), format!("{}\ncubes_of = \"{id}\"\n", text.trim_end()))?;
    let bones: std::collections::HashSet<usize> = made.values().map(|c| c.bone).collect();
    let lines = judge(&made, &cut, &style.cubes, meta.triangles, style.checks.max_triangles);
    let failed = lines.iter().filter(|l| !l.0).count();
    let verdict = if failed == 0 { "ok".into() } else { format!("{failed} FAILED") };
    let summary = format!("{id}-cubes: {verdict} · {} cubes of {:.0} cm in {} parts (of {} bones) · {} triangles (was {}) · {} kB", made.len(), size * 100.0, bones.len(), sk.joints.len(), meta.triangles, spec::load::<spec::Meta>(&from.join("meta.toml"))?.triangles, std::fs::metadata(to.join("model.glb"))?.len() / 1000);
    Ok((summary, lines))
}
