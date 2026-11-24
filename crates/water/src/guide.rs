//! Reading a Scrib guide folder (format 1, Scrib `superfargoth/crates/sf_worldgen/src/guide.rs:1-22`) and its
//! height stamps (`stamps.rs:1-22`), and copying it.
//!
//! Guide frame: x east, y north, metres; sample (c, r) sits at `(origin_x + c·s, origin_y − r·s)`, row 0 = north.
use serde_json::Value;
use std::fs;
use std::path::Path;

pub struct Guide {
    /// `meta.json` as read (every key kept, so the copy keeps them).
    pub meta: Value,
    pub width: usize,
    pub height: usize,
    pub spacing_m: f64,
    pub sea_level_m: f64,
    pub origin_m: [f64; 2],
    /// Heights, metres, row-major, row 0 = north.
    pub heights: Vec<f32>,
}

fn read_f32(path: &Path) -> Result<Vec<f32>, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() % 4 != 0 {
        return Err(format!("{}: length is not a whole number of f32", path.display()));
    }
    Ok(bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect())
}

/// Little-endian f32 bytes.
#[must_use]
pub fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn num(meta: &Value, key: &str) -> Option<f64> {
    meta.get(key).and_then(Value::as_f64)
}

impl Guide {
    /// # Errors
    /// Missing or malformed `meta.json` / `heights.f32`.
    pub fn read(dir: &Path) -> Result<Self, String> {
        let meta_path = dir.join("meta.json");
        let text = fs::read_to_string(&meta_path).map_err(|e| format!("{}: {e}", meta_path.display()))?;
        let meta: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", meta_path.display()))?;
        if meta.get("format").and_then(Value::as_u64) != Some(1) {
            return Err("meta.json: format 1 expected".into());
        }
        let dims = meta.get("dims").and_then(Value::as_array).ok_or("meta.json: dims missing")?;
        let dim = |k: usize| dims.get(k).and_then(Value::as_u64).and_then(|v| usize::try_from(v).ok());
        let (width, height) = (dim(0).ok_or("meta.json: dims[0]")?, dim(1).ok_or("meta.json: dims[1]")?);
        let spacing_m = num(&meta, "sample_spacing_m").ok_or("meta.json: sample_spacing_m missing")?;
        let sea_level_m = num(&meta, "sea_level_m").ok_or("meta.json: sea_level_m missing")?;
        let origin_m = match meta.get("origin_m").and_then(Value::as_array) {
            Some(o) => [o.first().and_then(Value::as_f64).unwrap_or(0.0), o.get(1).and_then(Value::as_f64).unwrap_or(0.0)],
            None => [0.0, 0.0],
        };
        let heights = read_f32(&dir.join("heights.f32"))?;
        if heights.len() != width * height {
            return Err(format!("heights.f32: {} samples, dims say {}", heights.len(), width * height));
        }
        Ok(Self { meta, width, height, spacing_m, sea_level_m, origin_m, heights })
    }

    /// Guide-frame position of fractional sample index (c, r).
    #[must_use]
    pub fn position(&self, c: f64, r: f64) -> [f64; 2] {
        [self.origin_m[0] + c * self.spacing_m, self.origin_m[1] - r * self.spacing_m]
    }
}

/// One height stamp's header (`stamps.json`).
#[derive(serde::Deserialize)]
struct StampHeader {
    origin_m: [f64; 2],
    dims: [usize; 2],
    spacing_m: f64,
    offset: usize,
}

#[derive(serde::Deserialize)]
struct StampsFile {
    format: u32,
    stamps: Vec<StampHeader>,
}

/// Where the guide's stamps have weight: a guide-grid mask, conservative. A sample is set when it lies within one
/// stamp spacing plus one guide spacing (on each axis) of a stamp sample of weight > 0, so every point where a
/// stamp's bilinear weight is > 0 has its four surrounding guide samples set.
pub struct StampMask {
    pub width: usize,
    pub height: usize,
    pub set: Vec<bool>,
    pub stamps: usize,
}

impl StampMask {
    /// No stamps (an empty mask).
    #[must_use]
    pub fn empty(guide: &Guide) -> Self {
        Self { width: guide.width, height: guide.height, set: vec![false; guide.width * guide.height], stamps: 0 }
    }

    /// Read `stamps.json` + `stamps.f32` if the guide has them.
    ///
    /// # Errors
    /// Malformed stamp files.
    pub fn read(dir: &Path, guide: &Guide) -> Result<Self, String> {
        let mut mask = Self::empty(guide);
        let json = dir.join("stamps.json");
        if !json.exists() {
            return Ok(mask);
        }
        let text = fs::read_to_string(&json).map_err(|e| format!("{}: {e}", json.display()))?;
        let file: StampsFile = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", json.display()))?;
        if file.format != 1 {
            return Err("stamps.json: format 1 expected".into());
        }
        let data = read_f32(&dir.join("stamps.f32"))?;
        let s = guide.spacing_m;
        for st in &file.stamps {
            let [w, h] = st.dims;
            let weights = data.get(st.offset + w * h..st.offset + 2 * w * h).ok_or("stamps.f32: stamp out of range")?;
            let reach = st.spacing_m + s;
            // Per row, each run of weight > 0 marks one box (the union of its samples' boxes).
            for r in 0..h {
                let row = &weights[r * w..(r + 1) * w];
                let mut c = 0;
                while c < w {
                    if row[c] <= 0.0 {
                        c += 1;
                        continue;
                    }
                    let start = c;
                    while c < w && row[c] > 0.0 {
                        c += 1;
                    }
                    #[allow(clippy::cast_precision_loss)]
                    let (xa, xb, y) = (
                        st.origin_m[0] + start as f64 * st.spacing_m,
                        st.origin_m[0] + (c - 1) as f64 * st.spacing_m,
                        st.origin_m[1] - r as f64 * st.spacing_m,
                    );
                    mask.mark(guide, [xa, xb], y, reach);
                }
            }
            mask.stamps += 1;
        }
        Ok(mask)
    }

    /// Set every guide sample within `reach` (m, per axis) of the guide-frame segment from (x[0], y) to (x[1], y).
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
    fn mark(&mut self, guide: &Guide, x: [f64; 2], y: f64, reach: f64) {
        let s = guide.spacing_m;
        let (ca, cb, ru) = ((x[0] - guide.origin_m[0]) / s, (x[1] - guide.origin_m[0]) / s, (guide.origin_m[1] - y) / s);
        let k = reach / s;
        let lo = |v: f64| (v - k).ceil().max(0.0) as usize;
        let (c0, c1) = (lo(ca), ((cb + k).floor()).min(self.width as f64 - 1.0));
        let (r0, r1) = (lo(ru), ((ru + k).floor()).min(self.height as f64 - 1.0));
        if c1 < 0.0 || r1 < 0.0 || (c0 as f64) > c1 || (r0 as f64) > r1 {
            return;
        }
        for r in r0..=r1 as usize {
            for c in c0..=c1 as usize {
                self.set[r * self.width + c] = true;
            }
        }
    }

    #[must_use]
    pub fn at(&self, c: usize, r: usize) -> bool {
        self.set[r * self.width + c]
    }

    /// True when guide-frame point (x, y) may carry stamp weight > 0: one of its four surrounding samples is set
    /// (outside the guide: false).
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
    pub fn touches(&self, guide: &Guide, x: f64, y: f64) -> bool {
        let s = guide.spacing_m;
        let (cu, ru) = ((x - guide.origin_m[0]) / s, (guide.origin_m[1] - y) / s);
        let (cf, rf) = (cu.floor(), ru.floor());
        for (dc, dr) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let (c, r) = (cf + dc, rf + dr);
            if c >= 0.0 && r >= 0.0 && c < self.width as f64 && r < self.height as f64 && self.at(c as usize, r as usize) {
                return true;
            }
        }
        false
    }
}

/// Copy every file and folder of `from` into `to`, recursively, except the top-level names in `skip`.
///
/// # Errors
/// I/O errors.
pub fn copy_except(from: &Path, to: &Path, skip: &[&str]) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let mut entries: Vec<_> = fs::read_dir(from)
        .map_err(|e| format!("{}: {e}", from.display()))?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        if skip.iter().any(|s| name == *s) {
            continue;
        }
        let (src, dst) = (entry.path(), to.join(&name));
        if src.is_dir() {
            copy_except(&src, &dst, &[])?;
        } else {
            fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
}
