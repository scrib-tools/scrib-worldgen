//! The square Magna Urbis raster (`settlements/mod.rs:139` `Raster`) resampled from a guide.
//!
//! Magna Urbis's frame (`settlements/grid.rs:545-553`): cell `i` = `row * n + col`, its centre at
//! `((col + 0.5) * cell, (row + 0.5) * cell)`, `y` growing with the row. We lay row 0 on the guide's north edge, as
//! the guide's own rows, so the local frame is `x` east from the square's west edge, `y` **south** from its north edge
//! (the same flip `crates/settle/src/grid.rs` makes for the town stage). Magna Urbis's routing does not depend on the
//! frame's handedness (no sun, aspect or compass in `roads/`).
// Raster, cell and metre conversions: values are bounded by the raster dims.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
use crate::guide::Guide;
use crate::water::WaterFile;
use magna_urbis_core::settlements::{Point, Raster};
use serde::{Deserialize, Serialize};

/// Magna Urbis water codes (`settlements/mod.rs:138`).
pub const DRY: u8 = 0;
pub const SEA: u8 = 1;
pub const LAKE: u8 = 2;
pub const RIVER: u8 = 3;

/// Where Magna Urbis's local frame sits in the guide frame: `guide = (west_m + x, north_m - y)`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub west_m: f64,
    pub north_m: f64,
    /// Side of the square (m).
    pub width_m: f64,
    /// Raster side (cells).
    pub n: usize,
    /// Cell side (m).
    pub cell_m: f64,
}

impl Frame {
    #[must_use]
    pub fn to_guide(&self, p: Point) -> [f64; 2] {
        [self.west_m + p.x, self.north_m - p.y]
    }
    #[must_use]
    pub fn to_local(&self, [x, y]: [f64; 2]) -> Point {
        Point { x: x - self.west_m, y: self.north_m - y }
    }
    /// Guide-frame centre of cell `(col, row)`.
    #[must_use]
    pub fn centre(&self, col: usize, row: usize) -> [f64; 2] {
        [self.west_m + (col as f64 + 0.5) * self.cell_m, self.north_m - (row as f64 + 0.5) * self.cell_m]
    }
    /// Cell holding a guide-frame point, `None` outside the square.
    #[must_use]
    pub fn cell(&self, [x, y]: [f64; 2]) -> Option<usize> {
        let col = ((x - self.west_m) / self.cell_m).floor();
        let row = ((self.north_m - y) / self.cell_m).floor();
        let n = self.n as f64;
        (col >= 0.0 && row >= 0.0 && col < n && row < n).then(|| row as usize * self.n + col as usize)
    }
}

/// Owned arrays behind a Magna Urbis [`Raster`].
pub struct MuRaster {
    pub frame: Frame,
    pub height: Vec<f32>,
    pub water: Vec<u8>,
    pub water_level: Vec<f32>,
    pub river_width: Vec<f32>,
    pub river_ids: Vec<i32>,
}

impl MuRaster {
    /// Resample `guide` to an `n`-cell square covering it, centred on it, padded with sea `pad_depth_m` below sea level.
    ///
    /// Heights: mean of the guide samples in a cell (cells finer than the samples: bilinear at the centre). Water:
    /// sea where the height is below the guide's sea level; then each `water.json` lake fills the cells whose centre
    /// is inside its outline and outside its holes (lakes are data, they win over the sea test); then each river marks
    /// the dry cells within half its width of its centreline, plus every cell its centreline passes through, so a
    /// river narrower than a cell stays one unbroken run of cells. A cell between two rivers takes the nearer one.
    #[must_use]
    pub fn build(guide: &Guide, water_file: Option<&WaterFile>, n: usize, pad_depth_m: f64) -> Self {
        let (west, north, east, south) = guide.extent();
        let width_m = (east - west).max(north - south);
        let cell_m = width_m / n as f64;
        let (cx, cy) = (f64::midpoint(west, east), f64::midpoint(north, south));
        let frame = Frame { west_m: cx - width_m / 2.0, north_m: cy + width_m / 2.0, width_m, n, cell_m };
        let size = n * n;
        let mut sum = vec![0.0_f64; size];
        let mut count = vec![0_u32; size];
        for row in 0..guide.height {
            for col in 0..guide.width {
                let p = guide.sample_pos(col, row);
                // Clamp: a sample on the square's east or south edge belongs to the last cell.
                let c = (((p[0] - frame.west_m) / cell_m).floor() as usize).min(n - 1);
                let r = (((frame.north_m - p[1]) / cell_m).floor() as usize).min(n - 1);
                sum[r * n + c] += f64::from(guide.heights[row * guide.width + col]);
                count[r * n + c] += 1;
            }
        }
        let sea_level = guide.sea_level_m as f32;
        let pad = (guide.sea_level_m - pad_depth_m) as f32;
        let mut height = vec![pad; size];
        for row in 0..n {
            for col in 0..n {
                let i = row * n + col;
                if count[i] > 0 {
                    height[i] = (sum[i] / f64::from(count[i])) as f32;
                } else {
                    let [x, y] = frame.centre(col, row);
                    if let Some(h) = guide.height_at(x, y) {
                        height[i] = h;
                    }
                }
            }
        }
        let mut water: Vec<u8> = height.iter().map(|&h| if h < sea_level { SEA } else { DRY }).collect();
        let mut water_level: Vec<f32> = height.iter().map(|&h| if h < sea_level { sea_level } else { h }).collect();
        let mut river_width = vec![0.0_f32; size];
        let mut river_ids = vec![-1_i32; size];
        if let Some(w) = water_file {
            for lake in &w.lakes {
                fill_lake(&frame, lake, &mut water, &mut water_level);
            }
            let mut best = vec![f64::INFINITY; size];
            for (id, river) in w.rivers.iter().enumerate() {
                let id = i32::try_from(id).unwrap_or(i32::MAX);
                for seg in river.points.windows(2) {
                    mark_river(&frame, seg[0], seg[1], id, &mut best, &mut water, &mut water_level, &mut river_width, &mut river_ids);
                }
            }
        }
        Self { frame, height, water, water_level, river_width, river_ids }
    }

    #[must_use]
    pub fn raster(&self) -> Raster<'_> {
        Raster {
            width: self.frame.width_m,
            n: self.frame.n,
            height: &self.height,
            water: &self.water,
            water_level: &self.water_level,
            river_width: &self.river_width,
            river_ids: &self.river_ids,
        }
    }

    /// Water code under a guide-frame point (sea outside the square).
    #[must_use]
    pub fn water_at(&self, p: [f64; 2]) -> u8 {
        self.frame.cell(p).map_or(SEA, |i| self.water[i])
    }
}

/// Even-odd scanline fill of a lake's outline and holes at the cell centres.
fn fill_lake(frame: &Frame, lake: &crate::water::Lake, water: &mut [u8], level: &mut [f32]) {
    let n = frame.n;
    let rings: Vec<&[[f64; 2]]> = std::iter::once(lake.outline.as_slice()).chain(lake.holes.iter().map(Vec::as_slice)).collect();
    let mut xs = Vec::new();
    for row in 0..n {
        let y = frame.centre(0, row)[1];
        xs.clear();
        for ring in &rings {
            for k in 0..ring.len() {
                let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
                if (a[1] > y) != (b[1] > y) {
                    xs.push(a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.as_chunks::<2>().0 {
            // Columns whose centre x lies in [pair[0], pair[1]).
            let c0 = ((pair[0] - frame.west_m) / frame.cell_m - 0.5).ceil().max(0.0);
            let c1 = ((pair[1] - frame.west_m) / frame.cell_m - 0.5).ceil().min(n as f64);
            for col in c0 as usize..c1.max(c0) as usize {
                water[row * n + col] = LAKE;
                level[row * n + col] = lake.level_m as f32;
            }
        }
    }
}

/// Mark the dry cells a river segment `[x, y, width_m, level_m]` covers.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::many_single_char_names)]
fn mark_river(
    frame: &Frame,
    a: [f64; 4],
    b: [f64; 4],
    id: i32,
    best: &mut [f64],
    water: &mut [u8],
    level: &mut [f32],
    widths: &mut [f32],
    ids: &mut [i32],
) {
    let n = frame.n;
    let reach = 0.5 * a[2].max(b[2]) + frame.cell_m;
    let col_of = |x: f64| ((x - frame.west_m) / frame.cell_m).floor().clamp(0.0, (n - 1) as f64) as usize;
    let row_of = |y: f64| ((frame.north_m - y) / frame.cell_m).floor().clamp(0.0, (n - 1) as f64) as usize;
    let (c0, c1) = (col_of(a[0].min(b[0]) - reach), col_of(a[0].max(b[0]) + reach));
    let (r0, r1) = (row_of(a[1].max(b[1]) + reach), row_of(a[1].min(b[1]) - reach));
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    for row in r0..=r1 {
        for col in c0..=c1 {
            let i = row * n + col;
            if water[i] != DRY && water[i] != RIVER {
                continue;
            }
            let [x, y] = frame.centre(col, row);
            let t = if len2 > 0.0 { (((x - a[0]) * dx + (y - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let d = (x - a[0] - t * dx).hypot(y - a[1] - t * dy);
            let width = a[2] + t * (b[2] - a[2]);
            let half = frame.cell_m * 0.5;
            let touched = segment_hits_box([a[0], a[1]], [b[0], b[1]], [x - half, y - half], [x + half, y + half]);
            if (d <= 0.5 * width || touched) && d < best[i] {
                best[i] = d;
                water[i] = RIVER;
                level[i] = (a[3] + t * (b[3] - a[3])) as f32;
                widths[i] = width as f32;
                ids[i] = id;
            }
        }
    }
}

/// Liang-Barsky: does segment `p`-`q` meet the box `lo`-`hi`?
fn segment_hits_box(p: [f64; 2], q: [f64; 2], lo: [f64; 2], hi: [f64; 2]) -> bool {
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for axis in 0..2 {
        let d = q[axis] - p[axis];
        for (num, den) in [(p[axis] - lo[axis], -d), (hi[axis] - p[axis], d)] {
            if den == 0.0 {
                if num < 0.0 {
                    return false;
                }
            } else {
                let t = num / den;
                if den < 0.0 {
                    t0 = t0.max(t);
                } else {
                    t1 = t1.min(t);
                }
            }
        }
    }
    t0 <= t1
}
