//! The files forge reads and writes for a creature, all under `assets/creatures/`:
//! `style.toml` (shared: the art style and each body plan), `<id>/creature.toml` (written by
//! hand: what the creature is), `<id>/forge.lock` (what has been paid for at Tripo) and
//! `<id>/meta.toml` (measured from the finished model; the game reads it).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const ROOT: &str = "assets/creatures";

#[derive(Deserialize)]
pub struct Style {
    /// Appended to every concept prompt.
    pub style: String,
    pub tripo: TripoOptions,
    pub plans: BTreeMap<String, Plan>,
    pub checks: Bars,
    #[serde(default)]
    pub cubes: CubeBars,
}

/// The bars `forge cubes` holds a figure of cubes to.
#[derive(Deserialize)]
pub struct CubeBars {
    /// Shortest figure worth converting, in cubes.
    pub min_tall: i32,
    /// Thinnest a limb may be across, in cubes.
    pub min_limb: i32,
    /// Most cubes that may hang on by one face or none, as a part of all.
    pub max_loose: f32,
    /// Most surface cubes that may share their colour with no neighbour, as a part of all.
    pub max_speckle: f32,
    /// Most of any arm or leg piece that may be lost inside another part.
    pub max_swallowed: f32,
}

impl Default for CubeBars {
    fn default() -> Self {
        Self { min_tall: 10, min_limb: 2, max_loose: 0.03, max_speckle: 0.06, max_swallowed: 0.5 }
    }
}

#[derive(Deserialize)]
pub struct TripoOptions {
    pub faces: u32,
    pub texture_size: u32,
    pub image_model: String,
    /// Extra arguments for `tripo.ts model`.
    pub model_args: Vec<String>,
}

#[derive(Deserialize)]
pub struct Plan {
    /// How the concept image poses the creature, so it rigs.
    pub pose: String,
    /// `humanoid` or `other`, as `tripo.ts rig --type`.
    pub rig: String,
    #[serde(default)]
    pub skeleton: Option<String>,
    /// Our clip name → Tripo's preset for this rig.
    #[serde(default)]
    pub presets: BTreeMap<String, String>,
    /// Motions Tripo made from a description, once: the name a preset knows it by (the start
    /// of its description) → its asset id at Tripo. Each is put on every creature of the plan.
    #[serde(default)]
    pub motions: BTreeMap<String, String>,
    /// Our clip name → a clip of the hand-animated library (`library.rs`), carried onto the
    /// creature's own skeleton. Taken before a preset of the same name.
    #[serde(default)]
    pub library: BTreeMap<String, String>,
    /// Every clip a creature of this plan must have; ones without a preset are made.
    pub clips: Vec<String>,
    /// Stands upright (falls backward, not onto its side).
    #[serde(default)]
    pub upright: bool,
    /// A cut of a preset that runs long: clip → [from seconds, to seconds, speed].
    #[serde(default)]
    pub trim: BTreeMap<String, [f32; 3]>,
    /// This plan's own stretch bar, where its rest pose is far from how it stands (a T-pose).
    #[serde(default)]
    pub max_stretch: Option<f32>,
}

/// The bars `forge check` holds a model to.
#[derive(Deserialize)]
pub struct Bars {
    pub max_triangles: usize,
    pub max_joints: usize,
    /// Worst allowed 99th-percentile edge stretch over every clip, as a fraction.
    pub max_stretch: f32,
    /// How far a foot may sink below or the lowest foot float above the floor, of height.
    pub max_foot_error: f32,
    /// Largest joint turn between a looping clip's last and first frame, radians.
    pub max_loop_gap: f32,
    /// How fast a planted foot may move over the ground in a gait played at its measured
    /// speed, as a part of that speed.
    pub max_slide: f32,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Creature {
    pub name: String,
    pub kind: String,
    pub plan: String,
    /// What it is, without style words.
    pub subject: String,
    /// Metres to the top of the rest pose.
    pub height: f32,
    /// For what flies: its wingspan, metres. Its size is set by this and not its height, which
    /// with the wings spread is only how thick its body is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<f32>,
    /// A model made here of boxes, not at Tripo (`built`): the small life of air and water.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub built: Option<crate::built::Built>,
    #[serde(default = "auto", skip_serializing_if = "is_auto")]
    pub facing: String,
    /// Scales how far Tripo's walk and run swing the limbs; under 1 for a bulky body whose
    /// legs fold or tear (the stretch check says so).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub gait_gain: f32,
    /// How far this creature's feet may slide in a gait, where the style's bar is a hair too
    /// strict for its body: a recorded run that misses by half a point is still a better run
    /// than a made one. Say in `creature.toml` what it measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slide: Option<f32>,
    /// Clips to make for this creature where its plan would take Tripo's preset: for a body
    /// the preset tears (the stretch check says so).
    /// Scales how far the made clips that are not gaits (idle, jump, swim, attack, eat…) turn
    /// the joints from rest; under 1 for a body whose skin tears in them (the stretch check
    /// says so). The gaits have `gait_gain`.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub ease: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub made: Vec<String>,
    /// Clips of its plan this creature does not have, because it would not do them: a
    /// tortoise does not sprint, a golem of stone does not swim. The game plays the next
    /// thing down (a run for a sprint, a walk for a swim).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub without: Vec<String>,
    /// How it rests, which is not the same for every animal: `side` (flat on its side: a wolf,
    /// a bear), `belly` (down on its belly with its legs folded under: a goat, a badger),
    /// `crouch` (sunk on all fours, half way down: a rabbit, a deer bedded), `curl` (down and
    /// curled round, nose to its flank and tail over it: a fox, a cat), `stands`
    /// (on its feet with its head low: a horse; and the default, for a rig on which none of
    /// the others comes out right). Say one only after looking at it
    /// (`cargo sandbox --shot ID --clips lie --view side`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rests: String,
    /// For a person: the clips the two-legged walker makes for it in place of anybody's
    /// (`strider.rs`): of `idle`, `walk`, `run`, `sprint`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub walked: Vec<String>,
    /// It goes by hopping: hind feet together, then fore feet together (a rabbit, a hare).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hops: bool,
    /// A look from `looks.toml` to draw this creature in, where the house style would be:
    /// for building one creature in a look that is being tried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
    /// The creature whose picture in that look (already drawn by `cargo forge looks`) is this
    /// one's concept, so the model is made from the image that was looked at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_of: Option<String>,
}

fn one() -> f32 {
    1.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}

fn auto() -> String {
    "auto".into()
}
fn is_auto(s: &String) -> bool {
    s == "auto"
}

#[derive(Deserialize, Serialize, Default)]
pub struct Lock {
    #[serde(default)]
    pub project: String,
    /// Steps done at Tripo, in order: concept, model, rig, export.
    #[serde(default)]
    pub steps: Vec<String>,
    #[serde(default)]
    pub credits: BTreeMap<String, f64>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct Meta {
    pub height: f32,
    pub length: f32,
    /// Side to side as it stands, metres. With `length`, the shape other creatures bump into.
    #[serde(default)]
    pub width: f32,
    pub walk_speed: f32,
    pub run_speed: f32,
    /// 0 when the plan has no sprint.
    #[serde(default)]
    pub sprint_speed: f32,
    pub triangles: usize,
    pub joints: usize,
    pub clips: BTreeMap<String, ClipMeta>,
    /// Bones by role, for attaching gear and for the game's own aiming later.
    pub bones: BTreeMap<String, String>,
    /// What a body moved by its feet and no clip needs to know of it (`cube_anim`), where it
    /// has four legs that fold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk: Option<Walk>,
    /// How each hand (`Left`, `Right`) holds a handle, where it has hands (`grip.rs`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub grip: BTreeMap<String, Grip>,
}

/// A hand closed round a handle: where the handle's middle is and how it is turned, in the
/// hand bone's own frame (the thing runs along +y, out past the thumb, and faces +x, the
/// way the knuckles do), and the turn of each finger bone, by name.
#[derive(Deserialize, Serialize, Default)]
pub struct Grip {
    pub at: [f32; 3],
    pub turn: [f32; 4],
    pub bones: Vec<String>,
    pub turns: Vec<[f32; 4]>,
}

/// A four-legged body as it stands, metres, in the model's own frame (it faces +z, its feet
/// on y = 0): each leg in the walker's order (fore before hind, the one on -x first) as the
/// names of its hip, knee and end; where the hip is; how long its two bones are; how high its
/// end stands over the ground.
#[derive(Deserialize, Serialize, Default)]
pub struct Walk {
    pub legs: Vec<String>,
    pub hips: Vec<[f32; 3]>,
    /// And where its knee is, so that it goes on bending the way it is bent.
    #[serde(default)]
    pub knees: Vec<[f32; 3]>,
    pub bones: Vec<[f32; 2]>,
    pub ankle: Vec<f32>,
    /// Where each leg meets the ground, in its last joint's own frame: the leg is put down by
    /// that point, not by the joint (`bones` are to it, and `ankle` is nothing).
    #[serde(default)]
    pub sole: Vec<[f32; 3]>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct ClipMeta {
    pub seconds: f32,
    pub looping: bool,
    pub source: String,
    /// For a gait: how fast the ground passes under it played as it is, metres a second.
    /// Not always the speed the creature goes at in that gait (`walk_speed`…): the game
    /// mixes gaits to fit the speed it is going, and needs each one's own.
    #[serde(default)]
    pub speed: f32,
}

pub fn dir(id: &str) -> PathBuf {
    Path::new(ROOT).join(id)
}
/// Scratch for a creature: Tripo's downloads. Ignored by git; rebuilt from the project.
pub fn work(id: &str) -> PathBuf {
    Path::new(".work").join(id)
}

pub fn load<T: for<'a> Deserialize<'a>>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

pub fn save<T: Serialize>(path: &Path, value: &T, header: &str) -> Result<()> {
    std::fs::write(path, format!("{header}{}", toml::to_string(value)?)).with_context(|| format!("writing {}", path.display()))
}

pub fn ids() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(ROOT)
        .into_iter()
        .flatten()
        .flatten()
        // Not the ones made from another creature (`cargo forge cubes`): they are not built.
        .filter(|e| std::fs::read_to_string(e.path().join("creature.toml")).is_ok_and(|text| !text.contains("\ncubes_of = ")))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}
