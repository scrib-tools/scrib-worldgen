//! A Scrib guide folder, format 1 (Scrib `superfargoth/crates/sf_worldgen/src/guide.rs:1-22`): `meta.json` and
//! `heights.f32`. Sample `(col, row)` stands at `(origin_x + col * spacing, origin_y - row * spacing)`: row 0 = north,
//! x east, y north, metres.
// Sample indices and metres: values are bounded by the guide dims.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
use serde::Deserialize;
use std::path::Path;

/// Guide format this tool reads.
pub const GUIDE_FORMAT: u32 = 1;

#[derive(Deserialize)]
struct Meta {
    format: u32,
    dims: [usize; 2],
    sample_spacing_m: f64,
    sea_level_m: f64,
    #[serde(default)]
    origin_m: Option<[f64; 2]>,
}

pub struct Guide {
    pub width: usize,
    pub height: usize,
    pub spacing_m: f64,
    /// Guide-frame position of sample (0, 0) (its north-west corner sample).
    pub origin_m: [f64; 2],
    pub sea_level_m: f64,
    /// Row-major, row 0 = north.
    pub heights: Vec<f32>,
}

impl Guide {
    /// # Errors
    /// Missing or malformed `meta.json` / `heights.f32`, another format, a size mismatch.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let meta_path = dir.join("meta.json");
        let text = std::fs::read_to_string(&meta_path).map_err(|e| format!("{}: {e}", meta_path.display()))?;
        let meta: Meta = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", meta_path.display()))?;
        if meta.format != GUIDE_FORMAT {
            return Err(format!("{}: format {} (this tool reads {GUIDE_FORMAT})", meta_path.display(), meta.format));
        }
        let [width, height] = meta.dims;
        if width < 2 || height < 2 || !(meta.sample_spacing_m.is_finite() && meta.sample_spacing_m > 0.0) {
            return Err(format!("{}: bad dims or sample spacing", meta_path.display()));
        }
        let path = dir.join("heights.f32");
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if bytes.len() != width * height * 4 {
            return Err(format!("{}: {} bytes, want {width} x {height} f32", path.display(), bytes.len()));
        }
        let heights = bytes.as_chunks::<4>().0.iter().map(|&b| f32::from_le_bytes(b)).collect();
        Ok(Self {
            width,
            height,
            spacing_m: meta.sample_spacing_m,
            origin_m: meta.origin_m.unwrap_or([0.0, 0.0]),
            sea_level_m: meta.sea_level_m,
            heights,
        })
    }

    /// Guide-frame position of sample `(col, row)`.
    #[must_use]
    pub fn sample_pos(&self, col: usize, row: usize) -> [f64; 2] {
        [self.origin_m[0] + col as f64 * self.spacing_m, self.origin_m[1] - row as f64 * self.spacing_m]
    }

    /// The area the samples cover, each sample standing for a square of one spacing: `(west, north, east, south)`.
    #[must_use]
    pub fn extent(&self) -> (f64, f64, f64, f64) {
        let half = 0.5 * self.spacing_m;
        let [ox, oy] = self.origin_m;
        (
            ox - half,
            oy + half,
            ox + (self.width - 1) as f64 * self.spacing_m + half,
            oy - (self.height - 1) as f64 * self.spacing_m - half,
        )
    }

    /// Bilinear height at a guide-frame point, clamped to the edge samples; `None` outside [`Guide::extent`].
    #[must_use]
    pub fn height_at(&self, x: f64, y: f64) -> Option<f32> {
        let (west, north, east, south) = self.extent();
        if !(x >= west && x <= east && y <= north && y >= south) {
            return None;
        }
        let fc = ((x - self.origin_m[0]) / self.spacing_m).clamp(0.0, (self.width - 1) as f64);
        let fr = ((self.origin_m[1] - y) / self.spacing_m).clamp(0.0, (self.height - 1) as f64);
        let (c0, r0) = (fc.floor() as usize, fr.floor() as usize);
        let (c1, r1) = ((c0 + 1).min(self.width - 1), (r0 + 1).min(self.height - 1));
        let (tx, ty) = (fc - c0 as f64, fr - r0 as f64);
        let h = |c: usize, r: usize| f64::from(self.heights[r * self.width + c]);
        let top = h(c0, r0) * (1.0 - tx) + h(c1, r0) * tx;
        let bottom = h(c0, r1) * (1.0 - tx) + h(c1, r1) * tx;
        Some((top * (1.0 - ty) + bottom * ty) as f32)
    }
}
