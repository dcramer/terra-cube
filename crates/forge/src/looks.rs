//! Trying a look before it is the house style: `cargo forge looks` draws the same few
//! creatures in each look listed in `assets/creatures/looks.toml` and lays them out as one
//! picture, a row a look. Only concept images are made (2 credits each, once: an image
//! already under `assets/creatures/looks/` is not drawn again). The look that wins is copied
//! into `style.toml` by hand; nothing here changes a creature.

use crate::spec::{self, Creature, Style};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

#[derive(Deserialize)]
struct Looks {
    /// The creatures every look is drawn for: their subjects and plans are used as they are.
    of: Vec<String>,
    /// Ends every prompt: how the picture is framed, whatever the look.
    frame: String,
    looks: BTreeMap<String, Look>,
}

#[derive(Deserialize)]
struct Look {
    /// What the look is for and what it risks, for whoever reads the sheet.
    note: String,
    /// Stands where the house style does in a concept prompt.
    style: String,
}

/// Draws what is missing and writes `shots/looks.jpg`. Returns the one line to print.
pub fn run(style: &Style) -> Result<String> {
    let root = Path::new(spec::ROOT);
    let looks: Looks = spec::load(&root.join("looks.toml"))?;
    let mut spent = 0.0;
    let mut rows: Vec<(String, Vec<std::path::PathBuf>)> = vec![("house".into(), looks.of.iter().map(|id| spec::dir(id).join("concept.jpg")).collect())];
    for (name, look) in &looks.looks {
        let dir = root.join("looks").join(name);
        std::fs::create_dir_all(&dir)?;
        let mut row = Vec::new();
        for id in &looks.of {
            let to = dir.join(format!("{id}.jpg"));
            if !to.exists() {
                let creature: Creature = spec::load(&spec::dir(id).join("creature.toml"))?;
                let plan = style.plans.get(&creature.plan).with_context(|| format!("no plan `{}`", creature.plan))?;
                let prompt = format!("{}, {}, {}, {}", creature.subject.trim(), plan.pose.trim(), look.style.trim(), looks.frame.trim()).replace('\n', " ");
                let scratch = Path::new(".work").join("looks");
                std::fs::create_dir_all(&scratch)?;
                let png = scratch.join(format!("{name}-{id}.png"));
                eprintln!("{name}: {id}…");
                let mut args = vec!["tools/tripo/tripo.ts".to_owned(), "image".into(), prompt, "--model".into(), style.tripo.image_model.clone(), "-o".into(), png.to_string_lossy().into_owned()];
                if plan.rig == "humanoid" {
                    args.push("--t-pose".into());
                }
                let out = Command::new("node").args(&args).output().context("running node tools/tripo/tripo.ts")?;
                anyhow::ensure!(out.status.success(), "tripo image failed: {}", String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or(""));
                let report: serde_json::Value = String::from_utf8_lossy(&out.stdout).lines().rev().find(|l| l.trim_start().starts_with('{')).and_then(|l| serde_json::from_str(l).ok()).unwrap_or_default();
                spent += report["credits"].as_f64().unwrap_or(0.0);
                image::open(&png)?.thumbnail(512, 512).to_rgb8().save(&to)?;
            }
            row.push(to);
        }
        rows.push((name.clone(), row));
    }
    const CELL: u32 = 384;
    let mut sheet = image::RgbImage::from_pixel(CELL * looks.of.len() as u32, CELL * rows.len() as u32, image::Rgb([200, 200, 200]));
    for (r, (_, row)) in rows.iter().enumerate() {
        for (c, path) in row.iter().enumerate() {
            if let Ok(img) = image::open(path) {
                image::imageops::replace(&mut sheet, &img.thumbnail(CELL, CELL).to_rgb8(), (c as u32 * CELL) as i64, (r as u32 * CELL) as i64);
            }
        }
    }
    std::fs::create_dir_all("shots")?;
    sheet.save("shots/looks.jpg")?;
    let names: Vec<String> = rows.iter().map(|(name, _)| match looks.looks.get(name) {
        Some(look) => format!("{name} ({})", look.note.trim()),
        None => "house (as it is now)".into(),
    }).collect();
    Ok(format!("shots/looks.jpg · rows top to bottom: {} · columns: {} · {spent} credits this run", names.join("; "), looks.of.join(" ")))
}
