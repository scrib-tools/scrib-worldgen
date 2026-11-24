//! The site folder, format 1 (`docs/specs/settlements.md` "Contract: files only"): metres, x east, y north,
//! raster row 0 = north edge. Written by Scrib's exporter, read here.
// Raster, pixel and metre conversions: values are bounded by the raster dims and the sheet size.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// The only site format this tool reads.
pub const FORMAT: u32 = 1;
/// Raster file names inside a site folder (format 1).
pub const HEIGHTS_FILE: &str = "heights.f32";
pub const WATER_FILE: &str = "water.u8";
pub const ROADS_FILE: &str = "roads.u8";
pub const META_FILE: &str = "site.json";

/// `site.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SiteMeta {
    pub format: u32,
    pub name: String,
    pub producer: String,
    /// World position (m) of the north-west corner of raster cell (0, 0).
    pub origin_m: [f64; 2],
    /// Raster spacing (m).
    pub cell_m: f64,
    /// Raster size in cells: `[w, h]`.
    pub dims: [usize; 2],
    pub water_level_m: f64,
    /// Where the town's centre should be (world m).
    pub centre_m: [f64; 2],
    pub source: String,
    /// Kept content (arm A). Read by ST-3b; carried through untouched here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<serde_json::Value>,
    /// BLAKE3 (hex) of each raster, keyed by its file name (`heights.f32`, ...).
    #[serde(default)]
    pub blake3: BTreeMap<String, String>,
}

/// A loaded site folder.
#[derive(Clone, Debug)]
pub struct SiteRaster {
    pub meta: SiteMeta,
    /// Metres, row-major, `w * h`, row 0 = north.
    pub heights: Vec<f32>,
    /// 1 = open water.
    pub water: Vec<u8>,
    /// 1 = ground textured as road or path (optional).
    pub roads: Option<Vec<u8>>,
}

fn read_checked(dir: &Path, file: &str, meta: &SiteMeta, len: usize) -> Result<Vec<u8>, String> {
    let path = dir.join(file);
    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() != len {
        return Err(format!("{}: {} bytes, expected {len} for dims {:?}", path.display(), bytes.len(), meta.dims));
    }
    if let Some(want) = meta.blake3.get(file) {
        let got = blake3::hash(&bytes).to_hex();
        if !want.eq_ignore_ascii_case(got.as_str()) {
            return Err(format!("{}: blake3 {got} does not match site.json {want}", path.display()));
        }
    }
    Ok(bytes)
}

impl SiteRaster {
    /// Read a site folder and check its sizes and hashes.
    ///
    /// # Errors
    /// Missing or malformed files, a format other than 1, a size or hash mismatch.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let meta_path = dir.join(META_FILE);
        let text = fs::read_to_string(&meta_path).map_err(|e| format!("{}: {e}", meta_path.display()))?;
        let meta: SiteMeta = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", meta_path.display()))?;
        if meta.format != FORMAT {
            return Err(format!("{}: format {}, this tool reads format {FORMAT}", meta_path.display(), meta.format));
        }
        let [w, h] = meta.dims;
        if w < 2 || h < 2 || !(meta.cell_m.is_finite() && meta.cell_m > 0.0) {
            return Err(format!("{}: bad dims {:?} or cell_m {}", meta_path.display(), meta.dims, meta.cell_m));
        }
        let cells = w * h;
        let heights = read_checked(dir, HEIGHTS_FILE, &meta, cells * 4)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        let water = read_checked(dir, WATER_FILE, &meta, cells)?;
        let roads = if dir.join(ROADS_FILE).exists() { Some(read_checked(dir, ROADS_FILE, &meta, cells)?) } else { None };
        Ok(Self { meta, heights, water, roads })
    }

    /// Write this site as a folder (fixtures, round trips); fills `blake3`.
    ///
    /// # Errors
    /// I/O errors.
    pub fn write(&mut self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let heights: Vec<u8> = self.heights.iter().flat_map(|v| v.to_le_bytes()).collect();
        let mut files: Vec<(&str, &[u8])> = vec![(HEIGHTS_FILE, &heights), (WATER_FILE, &self.water)];
        if let Some(roads) = &self.roads {
            files.push((ROADS_FILE, roads));
        }
        for (file, bytes) in files {
            self.meta.blake3.insert(file.into(), blake3::hash(bytes).to_hex().to_string());
            fs::write(dir.join(file), bytes).map_err(|e| format!("{file}: {e}"))?;
        }
        let json = serde_json::to_string_pretty(&self.meta).map_err(|e| e.to_string())?;
        fs::write(dir.join(META_FILE), json).map_err(|e| format!("{META_FILE}: {e}"))
    }

    /// Fractional raster coordinates (column, row) of a world point, cell-centred.
    fn raster_pos(&self, x: f64, y: f64) -> (f64, f64) {
        let [ox, oy] = self.meta.origin_m;
        ((x - ox) / self.meta.cell_m - 0.5, (oy - y) / self.meta.cell_m - 0.5)
    }

    /// Bilinear height (m) at a world point, clamped to the raster.
    #[must_use]
    pub fn height_at(&self, x: f64, y: f64) -> f32 {
        let [cols, rows] = self.meta.dims;
        let (col, row) = self.raster_pos(x, y);
        let col = col.clamp(0.0, (cols - 1) as f64);
        let row = row.clamp(0.0, (rows - 1) as f64);
        let (c0, r0) = (col.floor() as usize, row.floor() as usize);
        let (c1, r1) = ((c0 + 1).min(cols - 1), (r0 + 1).min(rows - 1));
        let (tc, tr) = ((col - c0 as f64) as f32, (row - r0 as f64) as f32);
        let at = |c: usize, r: usize| self.heights[r * cols + c];
        (at(c0, r0) * (1.0 - tc) + at(c1, r0) * tc) * (1.0 - tr) + (at(c0, r1) * (1.0 - tc) + at(c1, r1) * tc) * tr
    }

    /// Whether the raster cell holding a world point is open water (outside the raster: false).
    /// Raster index of the cell holding world point `(x, y)`, if inside.
    #[must_use]
    pub fn cell_index(&self, x: f64, y: f64) -> Option<usize> {
        let [cols, rows] = self.meta.dims;
        let [west, north] = self.meta.origin_m;
        let col = ((x - west) / self.meta.cell_m).floor();
        let row = ((north - y) / self.meta.cell_m).floor();
        if col < 0.0 || row < 0.0 || col >= cols as f64 || row >= rows as f64 {
            return None;
        }
        Some(row as usize * cols + col as usize)
    }

    /// The water raster (1 = open water) without the bodies smaller than `min_area_m2`: 4-connected components of
    /// the whole site raster, so a river or coast cut by a later crop keeps its full size.
    #[must_use]
    pub fn open_water(&self, min_area_m2: f64) -> Vec<u8> {
        let [cols, rows] = self.meta.dims;
        let cell_area = self.meta.cell_m * self.meta.cell_m;
        let mut out = vec![0_u8; cols * rows];
        let mut seen = vec![false; cols * rows];
        let (mut stack, mut body) = (Vec::new(), Vec::new());
        for start in 0..cols * rows {
            if seen[start] || self.water[start] == 0 {
                continue;
            }
            seen[start] = true;
            stack.push(start);
            body.clear();
            while let Some(i) = stack.pop() {
                body.push(i);
                let (row, col) = (i / cols, i % cols);
                let near = [
                    (col > 0).then(|| i - 1),
                    (col + 1 < cols).then(|| i + 1),
                    (row > 0).then(|| i - cols),
                    (row + 1 < rows).then(|| i + cols),
                ];
                for j in near.into_iter().flatten() {
                    if !seen[j] && self.water[j] != 0 {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
            if body.len() as f64 * cell_area >= min_area_m2 {
                for &i in &body {
                    out[i] = 1;
                }
            }
        }
        out
    }

    #[must_use]
    pub fn extent(&self) -> (f64, f64, f64, f64) {
        let [ox, oy] = self.meta.origin_m;
        let [w, h] = self.meta.dims;
        (ox, oy, ox + w as f64 * self.meta.cell_m, oy - h as f64 * self.meta.cell_m)
    }
}
