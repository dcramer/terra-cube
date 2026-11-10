//! A GLB file as its JSON and its one binary buffer, with just enough reading and writing of
//! accessors to rework skeletons and animations. No glTF library: the format is small and we
//! touch only a little of it.

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::path::Path;

pub struct Glb {
    pub json: Value,
    pub bin: Vec<u8>,
}

impl Glb {
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        ensure!(bytes.len() > 20 && &bytes[0..4] == b"glTF", "{} is not a GLB", path.display());
        let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        let json_len = u32_at(12);
        let json: Value = serde_json::from_slice(&bytes[20..20 + json_len])?;
        let mut bin = Vec::new();
        let at = 20 + json_len;
        if at + 8 <= bytes.len() {
            let len = u32_at(at);
            bin = bytes[at + 8..at + 8 + len].to_vec();
        }
        if let Some(used) = json.get("extensionsRequired").and_then(Value::as_array) {
            ensure!(used.is_empty(), "{} needs extensions {used:?}; export a plain GLB", path.display());
        }
        Ok(Self { json, bin })
    }

    pub fn write(&mut self, path: &Path) -> Result<()> {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        self.json["buffers"] = json!([{ "byteLength": self.bin.len() }]);
        let mut text = serde_json::to_vec(&self.json)?;
        while !text.len().is_multiple_of(4) {
            text.push(b' ');
        }
        let total = 12 + 8 + text.len() + 8 + self.bin.len();
        let mut out = Vec::with_capacity(total);
        out.extend(b"glTF");
        out.extend(2u32.to_le_bytes());
        out.extend((total as u32).to_le_bytes());
        out.extend((text.len() as u32).to_le_bytes());
        out.extend(b"JSON");
        out.extend(&text);
        out.extend((self.bin.len() as u32).to_le_bytes());
        out.extend(b"BIN\0");
        out.extend(&self.bin);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, out).with_context(|| format!("writing {}", path.display()))
    }

    pub fn array(&self, key: &str) -> &[Value] {
        self.json.get(key).and_then(Value::as_array).map_or(&[], |v| v.as_slice())
    }

    fn push(&mut self, key: &str, value: Value) -> usize {
        if !self.json[key].is_array() {
            self.json[key] = json!([]);
        }
        let list = self.json[key].as_array_mut().unwrap();
        list.push(value);
        list.len() - 1
    }

    /// An accessor's values as f32, `width` to an element. Integers come as they are, or
    /// scaled to 0..1 when the accessor is normalized.
    pub fn floats(&self, accessor: usize) -> Result<(Vec<f32>, usize)> {
        let a = &self.array("accessors")[accessor];
        let width = match a["type"].as_str().unwrap_or("") {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            "MAT4" => 16,
            other => bail!("accessor type {other} not handled"),
        };
        let count = a["count"].as_u64().unwrap_or(0) as usize;
        let view = &self.array("bufferViews")[a["bufferView"].as_u64().context("sparse accessors not handled")? as usize];
        let comp = a["componentType"].as_u64().unwrap_or(0);
        let size = match comp {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            other => bail!("component type {other} not handled"),
        };
        let start = view["byteOffset"].as_u64().unwrap_or(0) as usize + a["byteOffset"].as_u64().unwrap_or(0) as usize;
        let stride = view["byteStride"].as_u64().map_or(size * width, |s| s as usize);
        let normalized = a["normalized"].as_bool().unwrap_or(false);
        let mut out = Vec::with_capacity(count * width);
        for i in 0..count {
            for c in 0..width {
                let at = start + i * stride + c * size;
                let b = &self.bin[at..at + size];
                out.push(match comp {
                    5120 => (b[0] as i8) as f32 / if normalized { 127.0 } else { 1.0 },
                    5121 => b[0] as f32 / if normalized { 255.0 } else { 1.0 },
                    5122 => i16::from_le_bytes([b[0], b[1]]) as f32 / if normalized { 32767.0 } else { 1.0 },
                    5123 => u16::from_le_bytes([b[0], b[1]]) as f32 / if normalized { 65535.0 } else { 1.0 },
                    5125 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
                    _ => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                });
            }
        }
        Ok((out, width))
    }

    /// Adds float data as a new accessor and returns its index.
    pub fn add_floats(&mut self, data: &[f32], kind: &str, with_bounds: bool) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let offset = self.bin.len();
        for v in data {
            self.bin.extend(v.to_le_bytes());
        }
        let view = self.push("bufferViews", json!({ "buffer": 0, "byteOffset": offset, "byteLength": data.len() * 4 }));
        let width = match kind {
            "SCALAR" => 1,
            "VEC3" => 3,
            _ => 4,
        };
        let mut accessor = json!({ "bufferView": view, "componentType": 5126, "count": data.len() / width, "type": kind });
        if with_bounds {
            let (lo, hi) = data.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
            accessor["min"] = json!([lo]);
            accessor["max"] = json!([hi]);
        }
        self.push("accessors", accessor)
    }

    /// Adds whole-number data, already as little-endian bytes, as a new accessor.
    pub fn add_raw(&mut self, bytes: &[u8], component: u32, kind: &str, count: usize) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let offset = self.bin.len();
        self.bin.extend(bytes);
        let view = self.push("bufferViews", json!({ "buffer": 0, "byteOffset": offset, "byteLength": bytes.len() }));
        self.push("accessors", json!({ "bufferView": view, "componentType": component, "count": count, "type": kind }))
    }

    /// Cuts out of the binary whatever no accessor or image still points at: after a mesh or
    /// a texture is replaced, the old one's bytes.
    pub fn drop_unused(&mut self) {
        let mut used = vec![false; self.array("bufferViews").len()];
        let mark = |v: &Value, used: &mut Vec<bool>| {
            if let Some(i) = v["bufferView"].as_u64() {
                used[i as usize] = true;
            }
        };
        // Accessors nothing refers to go first: a mesh's, once its primitives are replaced.
        let mut wanted = vec![false; self.array("accessors").len()];
        let mut want = |v: &Value| {
            if let Some(i) = v.as_u64() {
                wanted[i as usize] = true;
            }
        };
        for mesh in self.array("meshes") {
            for p in mesh["primitives"].as_array().into_iter().flatten() {
                want(&p["indices"]);
                p["attributes"].as_object().into_iter().flatten().for_each(|(_, a)| want(a));
            }
        }
        for skin in self.array("skins") {
            want(&skin["inverseBindMatrices"]);
        }
        for animation in self.array("animations") {
            for sampler in animation["samplers"].as_array().into_iter().flatten() {
                want(&sampler["input"]);
                want(&sampler["output"]);
            }
        }
        for (accessor, wanted) in self.array("accessors").iter().zip(&wanted) {
            if *wanted {
                mark(accessor, &mut used);
            }
        }
        for image in self.array("images") {
            mark(image, &mut used);
        }
        // Accessors and views keep their numbers; the bytes of unused views are closed up.
        let mut bin = Vec::with_capacity(self.bin.len());
        let views = self.json["bufferViews"].as_array_mut().map(std::mem::take).unwrap_or_default();
        let mut kept = Vec::with_capacity(views.len());
        for (mut view, used) in views.into_iter().zip(used) {
            let (from, len) = (view["byteOffset"].as_u64().unwrap_or(0) as usize, view["byteLength"].as_u64().unwrap_or(0) as usize);
            while !bin.len().is_multiple_of(4) {
                bin.push(0);
            }
            view["byteOffset"] = json!(bin.len());
            if used {
                bin.extend(&self.bin[from..from + len]);
            } else {
                view["byteLength"] = json!(0);
            }
            kept.push(view);
        }
        self.json["bufferViews"] = Value::Array(kept);
        self.bin = bin;
    }

    /// Triangles over every mesh primitive.
    pub fn triangles(&self) -> usize {
        let mut n = 0;
        for mesh in self.array("meshes") {
            for p in mesh["primitives"].as_array().into_iter().flatten() {
                let counted = p.get("indices").or_else(|| p["attributes"].get("POSITION")).and_then(Value::as_u64);
                if let Some(a) = counted {
                    n += self.array("accessors")[a as usize]["count"].as_u64().unwrap_or(0) as usize / 3;
                }
            }
        }
        n
    }
}
