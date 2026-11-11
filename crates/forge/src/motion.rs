//! Captured motion for a biped: `[motion]` in a `creature.toml` names, for a clip, a take of
//! the CMU motion capture library and how long one cycle of it may be (`walk = "07_02:0.85-1.35"`),
//! and Blender puts it on the creature's skeleton (`tools/blender/retarget.py`). What comes
//! back is `.work/ID/motion.glb`, Tripo's export with those clips in it, and `motion.toml`
//! beside it with how fast each goes; `finish` reads them in place of the export.
//!
//! Blender is run as a Python module from an environment of its own (`make blender` makes
//! it): the one program forge starts besides `node` and `cargo`, and only for this. Tripo's
//! own walk and run for a person, and the gaits made here, pass every check and are stiff: a
//! recorded person walking has the weight shift, the arm swing and the heel strike that no
//! recipe has.

use crate::spec::{self, Creature};
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::process::Command;

/// The Python that has Blender in it.
fn python() -> PathBuf {
    let env = std::path::Path::new(".work").join("blender-venv");
    // (Windows keeps a virtual environment's programs in another folder under another name.)
    if cfg!(windows) { env.join("Scripts").join("python.exe") } else { env.join("bin").join("python") }
}

/// Blender's version, if it is here to be run.
pub fn version() -> Option<String> {
    let out = Command::new(python()).args(["-c", "import bpy; print(bpy.app.version_string)"]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Whether a creature asks for captured motion that has not been made.
pub fn wanted(id: &str, creature: &Creature) -> bool {
    !creature.motion.is_empty() && spec::work(id).join("rigged.glb").exists() && !spec::work(id).join("motion.glb").exists()
}

/// Makes a creature's captured clips. Says in a line what was made.
pub fn run(id: &str, creature: &Creature) -> Result<String> {
    if creature.motion.is_empty() {
        bail!("{id}: no [motion] in its creature.toml (walk = \"07_02:0.85-1.35\")");
    }
    let work = spec::work(id);
    if !work.join("rigged.glb").exists() {
        bail!("{id}: {} is missing; `cargo forge build {id}` fetches it", work.join("rigged.glb").display());
    }
    if version().is_none() {
        bail!("Blender is not set up here: `make blender` (it is a download of several hundred MB, once)");
    }
    let mut command = Command::new(python());
    command.arg(std::path::Path::new("tools").join("blender").join("retarget.py")).arg(work.join("rigged.glb")).arg(work.join("motion.glb"));
    for (clip, take) in &creature.motion {
        command.arg(format!("{clip}={take}"));
    }
    let out = command.output().context("starting Blender's Python")?;
    let said = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() || !said.lines().any(|l| l.starts_with("ok retarget")) {
        let why = String::from_utf8_lossy(&out.stderr);
        bail!("{id}: Blender could not make its motion: {}", why.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no reason given"));
    }
    let made: Vec<&str> = said.lines().filter(|l| l.starts_with("retarget ")).collect();
    Ok(format!("{id}: {} captured clips · {}", made.len(), made.join(" · ")))
}
