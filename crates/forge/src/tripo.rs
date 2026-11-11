//! The paid half of a build: concept image → model → rig → rigged export, each done once at
//! Tripo through `tools/tripo/tripo.ts` and noted in the creature's `forge.lock`, so a build
//! run again spends nothing on what is already made.

use crate::spec::{self, Creature, Lock, Plan, Style};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

pub const STEPS: [&str; 4] = ["concept", "model", "rig", "export"];

/// Runs the Tripo CLI and returns the one line of JSON it reports. Its progress chatter is
/// kept out of our output; on failure its last lines are the error.
fn tripo(args: &[String]) -> Result<Value> {
    let out = Command::new("node").arg("tools/tripo/tripo.ts").args(args).output().context("running node tools/tripo/tripo.ts")?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = err.lines().rev().take(4).collect();
        bail!("tripo {} failed: {}", args[0], tail.into_iter().rev().collect::<Vec<_>>().join(" | "));
    }
    let last = stdout.lines().rev().find(|l| l.trim_start().starts_with('{')).context("tripo printed no result")?;
    Ok(serde_json::from_str(last)?)
}

/// Credits left, which also proves the login in `.env` works.
pub fn balance() -> Result<f64> {
    tripo(&["balance".into()])?["credits"].as_f64().context("no credits in the balance")
}

/// The whole prompt for a creature's concept image: what it is, how it stands, the house style.
pub fn prompt(creature: &Creature, plan: &Plan, style: &Style) -> String {
    // A creature being built in a look that is on trial takes that look's style.
    let look = creature.look.as_deref().and_then(|name| crate::looks::style_of(name).ok());
    format!("{}, {}, {}", creature.subject.trim(), plan.pose.trim(), look.as_deref().unwrap_or(&style.style).trim()).replace('\n', " ")
}

fn s(v: impl ToString) -> String {
    v.to_string()
}

/// Does the Tripo steps not yet in the lock, up to and including `until`. Returns credits spent.
pub fn run(id: &str, creature: &Creature, plan: &Plan, style: &Style, until: &str) -> Result<f64> {
    let dir = spec::dir(id);
    let work = spec::work(id);
    std::fs::create_dir_all(&work)?;
    let lock_path = dir.join("forge.lock");
    let mut lock: Lock = if lock_path.exists() { spec::load(&lock_path)? } else { Lock::default() };
    let path = |name: &str| work.join(name).to_string_lossy().into_owned();
    let mut spent = 0.0;
    for step in STEPS {
        let have = lock.steps.iter().any(|d| d == step);
        // The export is free and its file is scratch, so fetch it again when it has gone.
        let refetch = step == "export" && have && !work.join("rigged.glb").exists();
        if !have || refetch {
            eprintln!("{id}: {step}…");
            let report = match step {
                // The picture already drawn and looked at, when there is one: the model is
                // made from what was chosen, and it is not paid for twice.
                "concept" if creature.look.as_deref().zip(creature.look_of.as_deref()).is_some_and(|(look, of)| crate::looks::picture(look, of).exists()) => {
                    let (look, of) = (creature.look.as_deref().unwrap(), creature.look_of.as_deref().unwrap());
                    std::fs::copy(crate::looks::picture(look, of), work.join("concept.png"))?;
                    thumbnail(&work.join("concept.png"), &dir.join("concept.jpg"))?;
                    serde_json::json!({ "credits": 0.0 })
                }
                "concept" => {
                    let mut args = vec![s("image"), prompt(creature, plan, style), s("--model"), style.tripo.image_model.clone(), s("-o"), path("concept.png")];
                    if plan.rig == "humanoid" {
                        args.push(s("--t-pose"));
                    }
                    let r = tripo(&args)?;
                    thumbnail(&work.join("concept.png"), &dir.join("concept.jpg"))?;
                    r
                }
                "model" => {
                    let mut args = vec![s("model"), path("concept.png"), s("--faces"), s(style.tripo.faces), s("-o"), path("model.glb")];
                    args.extend(style.tripo.model_args.iter().cloned());
                    let r = tripo(&args)?;
                    lock.project = r["project"].as_str().context("model made no project")?.to_owned();
                    r
                }
                // What flies is rigged here, from its shape (`shaped`): nothing is asked of
                // Tripo's rigger. (The export below is still wanted: the model as first downloaded is
                // packed in a way nothing here reads, and the export is plain.)
                "rig" if plan.rig == "forge" => serde_json::json!({ "credits": 0.0 }),
                "rig" => {
                    let mut args = vec![s("rig"), lock.project.clone(), s("--type"), plan.rig.clone(), s("-o"), path("rig.glb")];
                    if let Some(sk) = &plan.skeleton {
                        args.extend([s("--skeleton"), sk.clone()]);
                    }
                    if plan.rig == "humanoid" {
                        args.push(s("--anyway"));
                    }
                    tripo(&args)?
                }
                _ => {
                    let mut presets: Vec<String> = plan.presets.values().cloned().collect();
                    presets.sort();
                    presets.dedup();
                    let mut args = if presets.is_empty() { vec![s("export"), lock.project.clone()] } else { [vec![s("animate"), lock.project.clone()], presets].concat() };
                    args.extend([s("--in-place"), s("--texture-size"), s(style.tripo.texture_size), s("-o"), path("rigged.glb")]);
                    tripo(&args)?
                }
            };
            let credits = report["credits"].as_f64().unwrap_or(0.0);
            spent += credits;
            if !have {
                lock.steps.push(step.to_owned());
                *lock.credits.entry(step.to_owned()).or_default() += credits;
            }
            spec::save(&lock_path, &lock, "# What `cargo forge build` has made at Tripo. Delete a step (and those after it) to make it again.\n")?;
        }
        if step == until {
            break;
        }
    }
    Ok(spent)
}

/// A small copy of the concept image to keep beside the creature.
fn thumbnail(from: &Path, to: &Path) -> Result<()> {
    let img = image::open(from).with_context(|| format!("opening {}", from.display()))?;
    img.thumbnail(512, 512).to_rgb8().save(to)?;
    Ok(())
}
