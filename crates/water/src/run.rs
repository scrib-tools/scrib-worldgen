//! One run: guide folder in, Magna Urbis's water chain (`vector_water::condition_and_water`,
//! `terrain/src/vector_water/mod.rs:423`), carved guide folder and `water.json` out.
//!
//! Magna Urbis's frame: a square of `n` cells and `width` metres, sea at 0, row-major from the north-west corner;
//! cell (c, r) has its centre at `((c + 0.5)·cell, (r + 0.5)·cell)` metres, y pointing south
//! (`rivers/network.rs:193-194`, `rivers.rs:139`). The guide is padded (with sea) to a square of side `N` samples,
//! `N` a multiple of the recipe's `downsample` factor `d`, so guide sample (c, r) is fine cell (c, r) and the chain
//! runs on `N / d` cells of `d·s` metres over the same `N·s` metres.
use crate::guide::{Guide, StampMask, copy_except, f32_bytes};
use crate::polygon;
use crate::recipe::Recipe;
use magna_urbis_terrain::vector_water::{self, Water};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;
use std::time::Instant;

/// `water.json` format version.
pub const WATER_FORMAT: u32 = 1;
/// The Magna Urbis commit this tool is built on (`Cargo.toml`).
pub const MAGNA_URBIS_REV: &str = "21e22171f0b1766650b2cfef5d4bdaf574a1d325";
/// Largest square Magna Urbis's chain takes (`vector_water/wire.rs:268`).
pub const MU_MAX_N: usize = 8192;

#[derive(Serialize)]
pub struct LakeOut {
    pub id: usize,
    /// `open` (drains out) or `closed` (terminal), from Magna Urbis's lake kind; `lake` when it has none.
    pub kind: &'static str,
    pub level_m: f64,
    /// Counter-clockwise ring (x east, y north), last point not repeated.
    pub outline: Vec<[f64; 2]>,
    /// Clockwise rings of the islands.
    pub holes: Vec<Vec<[f64; 2]>>,
    /// Cell count times cell area.
    pub area_m2: f64,
    /// Set on a lake filling a dry closed basin that a river course crosses at its spill level (Magna Urbis kept
    /// no lake there): the id of the river flowing into it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_course: Option<usize>,
}

#[derive(Serialize)]
pub struct RiverOut {
    pub id: usize,
    /// The Magna Urbis network list it comes from: `river`, `braid`, `distributary` or `lake_link`.
    pub kind: &'static str,
    /// Magna Urbis's own line id, the id it joins and its outlet, passed through.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mu_id: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joins: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outlet: Option<usize>,
    /// True when the line was cut where it entered a stamp or a dropped lake (the downstream part dropped), or
    /// starts where it leaves one (the upstream part dropped).
    pub clipped: bool,
    /// Upstream to downstream: `[x, y, width_m, level_m]` (water surface, non-increasing downstream).
    pub points: Vec<[f64; 4]>,
}

#[derive(Serialize)]
pub struct WaterFile {
    pub format: u32,
    pub frame: &'static str,
    pub magna_urbis_rev: &'static str,
    pub sea_level_m: f64,
    pub lakes: Vec<LakeOut>,
    pub rivers: Vec<RiverOut>,
    pub stats: Stats,
}

/// Run figures (no clock time: the output files stay byte-identical across runs).
#[derive(Clone, Debug, Default, Serialize)]
pub struct Stats {
    /// Side of the square Magna Urbis ran on (cells) and of the padded guide square (samples).
    pub grid_n: usize,
    pub padded_n: usize,
    pub downsample: u32,
    pub lakes: usize,
    pub rivers: usize,
    pub river_km: f64,
    pub max_abs_delta_m: f64,
    pub lakes_dropped: usize,
    pub rivers_clipped: usize,
    pub rivers_dropped: usize,
    /// River points (off lakes) whose surface was raised over `ground + fill·bed_depth` to stay non-increasing and
    /// above `ground - bed_depth`: pools behind a bed that rises downstream.
    pub levels_pooled: usize,
    /// River level minus carved centre ground (m), off lakes: median and 90th percentile.
    pub river_level_p50_m: f64,
    pub river_level_p90_m: f64,
    /// Lake outline vertices (outlines and holes), and those over ground more than 1 cm under their lake's level.
    pub lake_vertices: usize,
    pub lake_vertices_below: usize,
    /// Dry closed basins a course crossed at its spill level, filled as lakes at that level.
    pub basin_lakes: usize,
    /// Courses ended at the rim of such a basin instead (the downstream part dropped), by cause: the basin touches
    /// a stamp, is larger than `max_basin_lake_km2`, or reaches other water (a lake, the sea, the guide's edge).
    pub courses_ended_stamp: usize,
    pub courses_ended_size: usize,
    pub courses_ended_water: usize,
    /// Written river points (off lakes) whose level stands more than a bed depth over the ground (0 expected).
    pub points_floating: usize,
}

pub struct Report {
    pub stats: Stats,
    pub time_s: f64,
    /// The height change applied to `heights.f32`, guide grid (row 0 = north).
    pub delta: Vec<f32>,
}

impl Report {
    #[must_use]
    pub fn summary(&self) -> String {
        let s = &self.stats;
        format!(
            "worldgen-water: grid {} (padded {}, /{}), {:.1} s, {} lakes, {} rivers, {:.1} river km, max |delta| {:.2} m, \
             dropped {} lakes, clipped {} rivers, dropped {} rivers, {} levels pooled; river level - centre ground p50 {:.2} m, \
             p90 {:.2} m; lake outline vertices below level {:.1} % ({} of {}); {} basins filled as lakes, \
             {} courses ended at a basin rim ({} stamp, {} size, {} water), {} points floating",
            s.grid_n,
            s.padded_n,
            s.downsample,
            self.time_s,
            s.lakes,
            s.rivers,
            s.river_km,
            s.max_abs_delta_m,
            s.lakes_dropped,
            s.rivers_clipped,
            s.rivers_dropped,
            s.levels_pooled,
            s.river_level_p50_m,
            s.river_level_p90_m,
            share(s.lake_vertices_below, s.lake_vertices),
            s.lake_vertices_below,
            s.lake_vertices,
            s.basin_lakes,
            s.courses_ended_stamp + s.courses_ended_size + s.courses_ended_water,
            s.courses_ended_stamp,
            s.courses_ended_size,
            s.courses_ended_water,
            s.points_floating,
        )
    }
}

#[allow(clippy::cast_precision_loss)]
fn share(part: usize, whole: usize) -> f64 {
    if whole == 0 { 0.0 } else { 100.0 * part as f64 / whole as f64 }
}

/// Converts between Magna Urbis metres (from the north-west corner, y south) and the guide frame.
struct Frame<'a> {
    guide: &'a Guide,
    /// Coarse cell size (m) and side of the coarse square (cells).
    cell: f64,
    m: usize,
}

impl Frame<'_> {
    /// Guide position of a Magna Urbis point.
    fn to_guide(&self, x: f64, y: f64) -> [f64; 2] {
        let s = self.guide.spacing_m;
        self.guide.position(x / s - 0.5, y / s - 0.5)
    }
    /// Guide position of fractional coarse cell index (x, y) (cell centres at integers).
    /// The coarse cell a guide point lies in (clamped to the square).
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn coarse_cell(&self, p: [f64; 2]) -> usize {
        let s = self.guide.spacing_m;
        let at = |v: f64| (((v + 0.5) * s / self.cell).floor().max(0.0) as usize).min(self.m - 1);
        let (cu, ru) = ((p[0] - self.guide.origin_m[0]) / s, (self.guide.origin_m[1] - p[1]) / s);
        at(ru) * self.m + at(cu)
    }
}

/// Bilinear ground (m, guide frame) of guide-grid `heights` at guide point `p`, clamped to the grid.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn ground(guide: &Guide, heights: &[f32], p: [f64; 2]) -> f64 {
    let (w, h, s) = (guide.width, guide.height, guide.spacing_m);
    let cu = ((p[0] - guide.origin_m[0]) / s).clamp(0.0, (w - 1) as f64);
    let ru = ((guide.origin_m[1] - p[1]) / s).clamp(0.0, (h - 1) as f64);
    let (c, r) = ((cu as usize).min(w.saturating_sub(2)), (ru as usize).min(h.saturating_sub(2)));
    let (u, v) = (cu - c as f64, ru - r as f64);
    let z = |cc: usize, rr: usize| f64::from(heights[(rr * w + cc).min(heights.len() - 1)]);
    (z(c, r) * (1.0 - u) + z(c + 1, r) * u) * (1.0 - v) + (z(c, r + 1) * (1.0 - u) + z(c + 1, r + 1) * u) * v
}

/// Nearest-rank percentile of `values` (sorted in place); 0 when empty.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn percentile(values: &mut [f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    values[((p * values.len() as f64) as usize).min(values.len() - 1)]
}

/// Read, run, write. `recipe_text` is the recipe file's text (hashed into the provenance).
///
/// # Errors
/// Bad guide or recipe, a grid out of Magna Urbis's range, I/O errors.
#[allow(clippy::too_many_lines, clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn run(guide_dir: &Path, recipe: &Recipe, recipe_text: &str, out: &Path) -> Result<Report, String> {
    let started = Instant::now();
    if let (Ok(a), Ok(b)) = (guide_dir.canonicalize(), out.canonicalize())
        && (b.starts_with(&a) || a.starts_with(&b))
    {
        return Err("--out must not be the guide folder, inside it or around it".into());
    }
    let guide = Guide::read(guide_dir)?;
    let stamps = StampMask::read(guide_dir, &guide)?;
    let d = recipe.downsample.value as usize;
    let (gw, gh) = (guide.width, guide.height);
    let big = gw.max(gh).div_ceil(d) * d;
    let m = big / d;
    if !(2..=MU_MAX_N).contains(&m) {
        return Err(format!("Magna Urbis grid {m} out of 2..={MU_MAX_N}: change `downsample`"));
    }
    // Sea at 0, padded with sea.
    let sea = guide.sea_level_m;
    let pad = -recipe.pad_depth_m.value;
    let mut fine = vec![pad as f32; big * big];
    for r in 0..gh {
        for c in 0..gw {
            fine[r * big + c] = (f64::from(guide.heights[r * gw + c]) - sea) as f32;
        }
    }
    let coarse = vector_water::downsample(&fine, big, m);
    let width = big as f64 * guide.spacing_m;
    let settings = recipe.mu_settings()?;
    let (result, water) = vector_water::condition_and_water(
        &coarse,
        m,
        width,
        &recipe.mu_conditioning()?,
        &settings,
        recipe.mu_climate(),
        &recipe.seed.value,
        false,
    );
    // Back to the guide grid. Downsampled, the change lands only on land, as Magna Urbis's own upsampled erosion
    // (`vector_water/conditioning.rs:782-783`): sea cells stay as they are.
    let up = vector_water::upsample_delta(&result.delta, m, big);
    let mut delta = vec![0.0_f32; gw * gh];
    let mut heights = guide.heights.clone();
    let mut max_abs = 0.0_f64;
    for r in 0..gh {
        for c in 0..gw {
            let v = up[r * big + c];
            let v = if d > 1 && fine[r * big + c] <= 0.0 { 0.0 } else { v };
            delta[r * gw + c] = v;
            heights[r * gw + c] += v;
            max_abs = max_abs.max(f64::from(v).abs());
        }
    }
    let frame = Frame { guide: &guide, cell: width / m as f64, m };
    let coarse_stamp = coarse_mask(&stamps, &guide, d, m);
    let mut stats = Stats { grid_n: m, padded_n: big, downsample: recipe.downsample.value, max_abs_delta_m: max_abs, ..Stats::default() };
    let (labels, comps) = polygon::components(&water.lake, m);
    let fit = Fit {
        frame: &frame,
        stamps: &stamps,
        heights: &heights,
        labels: &labels,
        sea_mask: &water.hydro.grids.sea,
        // Lakes touching a stamp are dropped (the runtime restores the stamp's ground there): no level.
        lake_level: {
            let grid = water.lake_level.as_deref().unwrap_or(&water.hydro.grids.filled);
            comps
                .iter()
                .map(|cells| {
                    let level = cells.iter().map(|&c| f64::from(grid[c])).fold(f64::NEG_INFINITY, f64::max) + sea;
                    (!cells.iter().any(|&c| coarse_stamp[c])).then_some(level)
                })
                .collect()
        },
        sea,
        d,
    };
    let mut lakes = lakes(&water, &comps, &fit, recipe, &mut stats);
    let width_scale = water.rivers.as_ref().map_or(0.0, |n| n.width_scale);
    let width_scale = if width_scale > 0.0 { width_scale } else { settings.width_scale };
    let rivers = rivers(&water, &fit, width_scale, recipe, comps.len(), &mut lakes, &mut stats);
    stats.lakes = lakes.len();

    // Output folder: the guide copied, heights carved, meta stamped with its provenance.
    copy_except(guide_dir, out, &["heights.f32", "meta.json", "water.json"])?;
    let write = |name: &str, bytes: &[u8]| fs::write(out.join(name), bytes).map_err(|e| format!("{name}: {e}"));
    let raw = f32_bytes(&heights);
    write("heights.f32", &raw)?;
    let mut meta = guide.meta.clone();
    if let Value::Object(map) = &mut meta {
        // A producer's hash of heights.f32 stays true of the file (the input's moves into the provenance).
        let input_hash = map.get("heights_blake3").cloned();
        if input_hash.is_some() {
            map.insert("heights_blake3".into(), blake3::hash(&raw).to_hex().to_string().into());
        }
        map.insert(
            "water_from".into(),
            json!({
                "tool": format!("worldgen-water {}", env!("CARGO_PKG_VERSION")),
                "guide": guide_dir.display().to_string(),
                "magna_urbis_rev": MAGNA_URBIS_REV,
                "recipe_blake3": blake3::hash(recipe_text.as_bytes()).to_hex().to_string(),
                "input_heights_blake3": input_hash,
            }),
        );
    }
    write("meta.json", serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?.as_bytes())?;
    let file = WaterFile {
        format: WATER_FORMAT,
        frame: "guide: x east, y north, metres",
        magna_urbis_rev: MAGNA_URBIS_REV,
        sea_level_m: sea,
        lakes,
        rivers,
        stats: stats.clone(),
    };
    write("water.json", serde_json::to_string(&file).map_err(|e| e.to_string())?.as_bytes())?;
    Ok(Report { stats, time_s: started.elapsed().as_secs_f64(), delta })
}

/// Coarse cells that touch a stamp: any guide sample of the cell's block set, then grown by one cell (the outline
/// of a lake runs half a cell beyond its cells).
fn coarse_mask(stamps: &StampMask, guide: &Guide, d: usize, m: usize) -> Vec<bool> {
    let mut block = vec![false; m * m];
    for r in 0..guide.height {
        for c in 0..guide.width {
            if stamps.at(c, r) {
                block[(r / d) * m + c / d] = true;
            }
        }
    }
    let mut grown = block.clone();
    for y in 0..m {
        for x in 0..m {
            if block[y * m + x] {
                for (yy, xx) in [(y.wrapping_sub(1), x), (y + 1, x), (y, x.wrapping_sub(1)), (y, x + 1)] {
                    if yy < m && xx < m {
                        grown[yy * m + xx] = true;
                    }
                }
            }
        }
    }
    grown
}

/// What the water geometry is fitted to: the carved guide heights, Magna Urbis's lake and sea masks.
struct Fit<'a> {
    frame: &'a Frame<'a>,
    stamps: &'a StampMask,
    /// Carved heights, guide grid (guide frame, m).
    heights: &'a [f32],
    /// Lake component of each coarse cell (-1 = none).
    labels: &'a [i32],
    sea_mask: &'a [u8],
    /// Level of each lake component (guide frame): the highest of Magna Urbis's levels on its cells; `None` for a
    /// dropped lake.
    lake_level: Vec<Option<f64>>,
    sea: f64,
    d: usize,
}

impl Fit<'_> {
    fn ground(&self, p: [f64; 2]) -> f64 {
        ground(self.frame.guide, self.heights, p)
    }
    /// The level of the lake at guide point `p` (guide frame), if it lies on a kept lake of Magna Urbis's mask.
    fn lake_at(&self, p: [f64; 2]) -> Option<f64> {
        let id = self.labels[self.frame.coarse_cell(p)];
        usize::try_from(id).ok().and_then(|id| self.lake_level[id])
    }
}

/// A shoreline vertex counts as below its lake when its ground is this far under the level (m): float noise of
/// the bilinear sample on the contour.
const BELOW_TOLERANCE_M: f64 = 0.01;
/// Field value given to a sample forced in or out of a lake where the ground disagrees with the mask (m): the
/// contour then runs next to that sample, at the mask edge.
const MASK_EPSILON_M: f64 = 1e-3;

/// Lakes: each component of Magna Urbis's lake mask, its outline traced as the shoreline (`shoreline`: the
/// iso-contour ground = level of the carved heights, marching squares on the guide grid), then finished by
/// `finish_lake` (simplified, pushed out under the bank).
#[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
fn lakes(water: &Water, comps: &[Vec<usize>], fit: &Fit, recipe: &Recipe, stats: &mut Stats) -> Vec<LakeOut> {
    let (frame, guide, d, m) = (fit.frame, fit.frame.guide, fit.d, fit.frame.m);
    let cell_area = frame.cell * frame.cell;
    let mut out = Vec::new();
    for (id, cells) in comps.iter().enumerate() {
        let Some(level) = fit.lake_level[id] else {
            stats.lakes_dropped += 1;
            continue;
        };
        let kind = match water.lake_kind.as_ref().map(|k| k[cells[0]]) {
            Some(1) => "open",
            Some(2) => "closed",
            _ => "lake",
        };
        // The component's guide samples; the wet region grows `d` samples past them (the coarse mask's edge is
        // uncertain by a cell; an outlet channel cut under the level is closed a cell past the mask edge).
        let (mut c0, mut r0, mut c1, mut r1) = (usize::MAX, usize::MAX, 0, 0);
        for &c in cells {
            let (x, y) = (c % m, c / m);
            (c0, r0, c1, r1) = (c0.min(x * d), r0.min(y * d), c1.max(x * d + d - 1), r1.max(y * d + d - 1));
        }
        let bbox = [c0, r0, c1.min(guide.width - 1), r1.min(guide.height - 1)];
        let rings = shoreline(fit, bbox, d, level, |c, r| fit.labels[(r / d) * m + c / d] == id as i32);
        let other = |p: [f64; 2]| {
            let c = frame.coarse_cell(p);
            fit.labels[c] >= 0 && fit.labels[c] != id as i32
        };
        let Some((outline, holes)) = finish_lake(fit, &rings, level, recipe, stats, other) else { continue };
        out.push(LakeOut { id, kind, level_m: level, outline, holes, area_m2: cells.len() as f64 * cell_area, from_course: None });
    }
    out
}

/// A lake's outline (counter-clockwise) and holes (clockwise), guide frame.
type Outline = (Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>);

/// A lake's traced shoreline rings made into its outline and holes: simplified, oriented (outer counter-clockwise,
/// holes clockwise, y north: water on the left), then pushed out by `lake_overlap_m` so the edge lies under the
/// bank (UE5's Water plugin draws a lake past its shoreline and lets the landscape hide the edge). The push stops
/// before other water (`other`), the sea, a stamp, or ground that falls back under the level (the far side of a
/// spit). Counts the outline vertices into `stats`; `None` when nothing was traced.
fn finish_lake(
    fit: &Fit,
    rings: &[Vec<[f64; 2]>],
    level: f64,
    recipe: &Recipe,
    stats: &mut Stats,
    other: impl Fn([f64; 2]) -> bool,
) -> Option<Outline> {
    let (frame, guide) = (fit.frame, fit.frame.guide);
    // Push in steps of at most a quarter sample, so a spit or a neighbour narrower than a sample is not jumped.
    let step = 0.25 * guide.spacing_m;
    let mut rings: Vec<Vec<[f64; 2]>> = rings.iter().map(|g| polygon::simplify_ring(g, recipe.lake_simplify_m.value)).collect();
    if rings.is_empty() {
        return None;
    }
    let outer = (0..rings.len())
        .max_by(|&a, &b| polygon::signed_area(&rings[a]).abs().total_cmp(&polygon::signed_area(&rings[b]).abs()))
        .unwrap_or(0);
    for (k, ring) in rings.iter_mut().enumerate() {
        let ccw = polygon::signed_area(ring) > 0.0;
        if ccw != (k == outer) {
            ring.reverse();
        }
    }
    let blocked = |from: [f64; 2], to: [f64; 2]| {
        other(to)
            || fit.sea_mask.get(frame.coarse_cell(to)).is_some_and(|&v| v != 0)
            || fit.stamps.touches(guide, to[0], to[1])
            || (fit.ground(to) < level && fit.ground(from) >= level)
    };
    for ring in &mut rings {
        *ring = polygon::push_out(ring, recipe.lake_overlap_m.value, step, blocked);
    }
    let outline = rings.swap_remove(outer);
    rings.retain(|r| r.len() >= 3);
    for p in outline.iter().chain(rings.iter().flatten()) {
        stats.lake_vertices += 1;
        if fit.ground(*p) < level - BELOW_TOLERANCE_M {
            stats.lake_vertices_below += 1;
        }
    }
    Some((outline, rings))
}

/// The shoreline rings of one lake (guide frame, unsimplified, any orientation): the zero contour of
/// `ground - level` on the carved heights over guide samples `bbox` (`[c0, r0, c1, r1]`, inclusive), wet being the
/// `seed` samples and those under the level within `grow` steps of them, off Magna Urbis's lakes and the sea.
#[allow(clippy::cast_precision_loss)]
fn shoreline(fit: &Fit, bbox: [usize; 4], grow: usize, level: f64, seed: impl Fn(usize, usize) -> bool) -> Vec<Vec<[f64; 2]>> {
    let (guide, d, m) = (fit.frame.guide, fit.d, fit.frame.m);
    let (gw, gh) = (guide.width, guide.height);
    let [c0, r0, c1, r1] = bbox;
    if c0 > c1 || r0 > r1 {
        return Vec::new();
    }
    // Local (i, j) = sample (c0 + i - pad, r0 + j - pad). The outermost local ring stays dry, so every contour
    // closes.
    let pad = grow + 1;
    let (w, h) = (c1 - c0 + 1 + 2 * pad, r1 - r0 + 1 + 2 * pad);
    let sample = |i: usize, j: usize| -> Option<(usize, usize)> {
        let (c, r) = ((c0 + i).checked_sub(pad)?, (r0 + j).checked_sub(pad)?);
        (i > 0 && j > 0 && i + 1 < w && j + 1 < h && c < gw && r < gh).then_some((c, r))
    };
    let z = |c: usize, r: usize| f64::from(fit.heights[r * gw + c]) - level;
    let mut steps = vec![usize::MAX; w * h];
    let mut queue = VecDeque::new();
    for j in 0..h {
        for i in 0..w {
            if let Some((c, r)) = sample(i, j)
                && seed(c, r)
            {
                steps[j * w + i] = 0;
                queue.push_back((i, j));
            }
        }
    }
    while let Some((i, j)) = queue.pop_front() {
        let next = steps[j * w + i] + 1;
        if next > grow {
            continue;
        }
        for (ii, jj) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
            let Some((c, r)) = sample(ii, jj) else { continue };
            let cell = (r / d) * m + c / d;
            if steps[jj * w + ii] == usize::MAX && z(c, r) < 0.0 && fit.labels[cell] < 0 && fit.sea_mask.get(cell).is_none_or(|&v| v == 0) {
                steps[jj * w + ii] = next;
                queue.push_back((ii, jj));
            }
        }
    }
    let mut field = vec![1.0; w * h];
    for j in 0..h {
        for i in 0..w {
            if let Some((c, r)) = sample(i, j) {
                let v = z(c, r);
                field[j * w + i] = if steps[j * w + i] == usize::MAX { v.max(MASK_EPSILON_M) } else { v.min(-MASK_EPSILON_M) };
            }
        }
    }
    polygon::iso_rings(&field, w, h)
        .into_iter()
        .map(|ring| ring.iter().map(|p| guide.position(c0 as f64 + p[0] - pad as f64, r0 as f64 + p[1] - pad as f64)).collect())
        .collect()
}

/// Magna Urbis's channel width (`vector_water/rivers.rs:93`, `pub(super)` there): `scale · 3 · (A / 1 km²)^0.45`.
fn channel_width(area_m2: f64, scale: f64) -> f64 {
    scale * 3.0 * (area_m2 / 1e6).powf(0.45)
}

/// Water surface of one course, upstream to downstream, fitted to the carved heights. Returns the levels and the
/// points pooled (raised over their own `ground + fill·depth`).
///
/// Magna Urbis sets a course's level in `incise_lines` (`vector_water/incision.rs:183-190`: in the bed at `FILL` of
/// its depth, never rising) against the relief as it stands when that course is cut; the courses cut after it
/// (smaller ones, with valleys up to 600 m wide, `VALLEY_REACH`) lower the ground under it again, and a lake link
/// (`rivers/network.rs:1932`) carries its lake's level all the way to its outlet. Measured on x10s-b2w: level minus
/// carved centre ground p50 20 m, p90 69 m (lake links 56 m, 191 m). So the level is set here on the final heights,
/// by Magna Urbis's rule: at most `ground + fill·depth` (its lake's level on a lake), never rising downstream
/// (running minimum), never more than a bed depth under the centre ground off lakes (running maximum of
/// `ground - depth` from the mouth up: the water pools behind a bed that rises downstream), never under the sea.
#[must_use]
pub fn fit_levels(ground: &[f64], depth: &[f64], lake: &[Option<f64>], fill: f64, sea: f64) -> (Vec<f64>, usize) {
    let m = ground.len();
    let mut floor = vec![f64::NEG_INFINITY; m];
    let mut low = f64::NEG_INFINITY;
    for k in (0..m).rev() {
        if lake[k].is_none() {
            low = low.max(ground[k] - depth[k]);
        }
        floor[k] = low;
    }
    let (mut top, mut pooled) = (f64::INFINITY, 0);
    let levels = (0..m)
        .map(|k| {
            let hi = lake[k].unwrap_or(ground[k] + fill * depth[k]);
            top = top.min(hi);
            let level = top.max(floor[k]);
            if level > hi + 1e-9 {
                pooled += 1;
            }
            level.max(sea)
        })
        .collect();
    (levels, pooled)
}

/// A river point floats when its level stands more than this over `ground + bed_depth` (m): float noise.
const FLOAT_EPSILON_M: f64 = 1e-6;

/// Lakes filling dry closed basins (guide-sample regions), made from the courses that cross them.
#[derive(Default)]
struct Basins {
    level: Vec<f64>,
    /// Guide sample -> basin.
    of: HashMap<usize, usize>,
}

impl Basins {
    /// The basin lake guide point `p` lies on: its ground under that lake's level and one of its four surrounding
    /// samples in the lake's region.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn at(&self, fit: &Fit, p: [f64; 2]) -> Option<usize> {
        if self.of.is_empty() {
            return None;
        }
        let guide = fit.frame.guide;
        let s = guide.spacing_m;
        let (cu, ru) = ((p[0] - guide.origin_m[0]) / s, (guide.origin_m[1] - p[1]) / s);
        if cu < 0.0 || ru < 0.0 {
            return None;
        }
        let (c, r, g) = (cu.floor() as usize, ru.floor() as usize, fit.ground(p));
        [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().find_map(|(dc, dr)| {
            let (cc, rr) = (c + dc, r + dr);
            if cc >= guide.width || rr >= guide.height {
                return None;
            }
            let id = *self.of.get(&(rr * guide.width + cc))?;
            (g < self.level[id]).then_some(id)
        })
    }
}

/// Why a basin is not filled.
enum Stop {
    Stamp,
    Size,
    Water,
}

/// The dry closed basin under guide point `p` at `level`: the guide samples with ground under the level
/// 4-connected to `p`'s surrounding ones, in flood order. Refused when the region or its rim touches a stamp, holds
/// more than `max_samples` samples, or reaches other water (a Magna Urbis lake, a basin lake, the sea, the guide's
/// edge: the basin is not closed at that level).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn flood(fit: &Fit, basins: &Basins, p: [f64; 2], level: f64, max_samples: usize) -> Result<Vec<usize>, Stop> {
    let (guide, d, m) = (fit.frame.guide, fit.d, fit.frame.m);
    let (gw, gh, s) = (guide.width, guide.height, guide.spacing_m);
    let h = |i: usize| f64::from(fit.heights[i]);
    let cell = |i: usize| ((i / gw) / d) * m + (i % gw) / d;
    let lake = |i: usize| fit.labels[cell(i)] >= 0 || basins.of.contains_key(&i);
    let (cu, ru) = (((p[0] - guide.origin_m[0]) / s).max(0.0), ((guide.origin_m[1] - p[1]) / s).max(0.0));
    let (c, r) = (cu.floor() as usize, ru.floor() as usize);
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();
    for (cc, rr) in [(c, r), (c + 1, r), (c, r + 1), (c + 1, r + 1)] {
        let i = rr * gw + cc;
        if cc < gw && rr < gh && h(i) < level && seen.insert(i) {
            queue.push_back(i);
        }
    }
    let mut region = Vec::new();
    while let Some(i) = queue.pop_front() {
        let (c, r) = (i % gw, i / gw);
        if c == 0
            || r == 0
            || c + 1 == gw
            || r + 1 == gh
            || h(i) <= fit.sea
            || lake(i)
            || fit.sea_mask.get(cell(i)).is_some_and(|&v| v != 0)
        {
            return Err(Stop::Water);
        }
        if fit.stamps.at(c, r) {
            return Err(Stop::Stamp);
        }
        region.push(i);
        if region.len() > max_samples {
            return Err(Stop::Size);
        }
        for j in [i - 1, i + 1, i - gw, i + gw] {
            if h(j) < level {
                if seen.insert(j) {
                    queue.push_back(j);
                }
            } else if fit.stamps.at(j % gw, j / gw) {
                return Err(Stop::Stamp);
            } else if lake(j) {
                return Err(Stop::Water);
            }
        }
    }
    if region.is_empty() { Err(Stop::Water) } else { Ok(region) }
}

/// A basin region traced as a lake: its outline and holes (`shoreline` with no growth, then `finish_lake`).
fn basin_outline(
    fit: &Fit,
    basins: &Basins,
    region: &[usize],
    level: f64,
    recipe: &Recipe,
    stats: &mut Stats,
) -> Option<Outline> {
    let gw = fit.frame.guide.width;
    let set: HashSet<usize> = region.iter().copied().collect();
    let bbox = region
        .iter()
        .fold([usize::MAX, usize::MAX, 0, 0], |b, &i| [b[0].min(i % gw), b[1].min(i / gw), b[2].max(i % gw), b[3].max(i / gw)]);
    let rings = shoreline(fit, bbox, 0, level, |c, r| set.contains(&(r * gw + c)));
    let other = |p: [f64; 2]| fit.labels[fit.frame.coarse_cell(p)] >= 0 || basins.at(fit, p).is_some();
    finish_lake(fit, &rings, level, recipe, stats, other)
}

/// Rivers: each Magna Urbis course in guide points (cut at stamps and dropped lakes), its levels fitted
/// (`fit_levels`), then worked in pieces, upstream first. Where a piece floats (a point more than a bed depth over
/// the ground: Magna Urbis's route crosses a dry closed basin at its spill level, and the fitted level pools to
/// that spill), the basin is flooded at the pooled level and becomes a lake (`from_course`), the hydrologically
/// right state of a basin a river flows through; the course is split at it like Magna Urbis's lake links (in and
/// out pieces, each keeping one point on the lake). A basin touching a stamp, larger than `max_basin_lake_km2` or
/// reaching other water ends the course at its rim instead (a terminal sink, the downstream part dropped).
#[allow(clippy::too_many_lines, clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn rivers(
    water: &Water,
    fit: &Fit,
    scale: f64,
    recipe: &Recipe,
    first_basin_id: usize,
    lakes: &mut Vec<LakeOut>,
    stats: &mut Stats,
) -> Vec<RiverOut> {
    let Some(net) = &water.rivers else { return Vec::new() };
    let guide = fit.frame.guide;
    let fill = recipe.river_fill.value;
    let sample_area = guide.spacing_m * guide.spacing_m;
    let max_samples = (recipe.max_basin_lake_km2.value * 1e6 / sample_area) as usize;
    let lists: [(&'static str, &Vec<_>); 4] =
        [("river", &net.rivers), ("braid", &net.braids), ("distributary", &net.distributaries), ("lake_link", &net.lake_links)];
    // Segments are tested for stamps every half guide spacing.
    let step = 0.5 * guide.spacing_m;
    // A stamp, or a lake dropped for touching one: no water drawn there. A course is cut where it enters one, and
    // one that starts in one (a lake link leaving a dropped lake) starts where it leaves it.
    let inside = |p: [f64; 2]| {
        let id = fit.labels[fit.frame.coarse_cell(p)];
        fit.stamps.touches(guide, p[0], p[1]) || usize::try_from(id).is_ok_and(|id| fit.lake_level[id].is_none())
    };
    let mut basins = Basins::default();
    let mut out = Vec::new();
    let mut above = Vec::new();
    for (kind, lines) in lists {
        for line in lines {
            let count = line.points.len() / 2;
            let mut pts: Vec<[f64; 3]> = Vec::with_capacity(count);
            let mut clipped = false;
            for k in 0..count {
                let p = fit.frame.to_guide(line.points[2 * k], line.points[2 * k + 1]);
                let blocked = inside(p)
                    || pts.last().is_some_and(|q| {
                        let len = (p[0] - q[0]).hypot(p[1] - q[1]);
                        let steps = (len / step).ceil().max(1.0) as usize;
                        (1..steps).any(|i| {
                            let t = i as f64 / steps as f64;
                            inside([q[0] + (p[0] - q[0]) * t, q[1] + (p[1] - q[1]) * t])
                        })
                    });
                if blocked {
                    clipped = true;
                    if pts.is_empty() {
                        continue;
                    }
                    break;
                }
                let area = line.area.get(k).copied().unwrap_or(0.0);
                pts.push([p[0], p[1], channel_width(area, scale)]);
            }
            if clipped {
                stats.rivers_clipped += 1;
            }
            let n = pts.len();
            let before = out.len();
            // Pieces of the course, as inclusive index ranges into `pts`, upstream first.
            let mut queue: VecDeque<(usize, usize)> = if n >= 2 { VecDeque::from([(0, n - 1)]) } else { VecDeque::new() };
            // Basin lakes made from this course, given the id of the next piece written.
            let mut pending: Vec<usize> = Vec::new();
            let mut ended = false;
            while let Some((s, e)) = queue.pop_front() {
                if e <= s {
                    continue;
                }
                let seg = &pts[s..=e];
                let xy = |k: usize| [seg[k][0], seg[k][1]];
                let basin: Vec<Option<usize>> = (0..seg.len()).map(|k| basins.at(fit, xy(k))).collect();
                if basin.iter().all(Option::is_some) {
                    continue;
                }
                // Points inside a basin lake split the piece: each run off basin lakes, with one lake point at
                // either end where it meets one.
                if (1..seg.len() - 1).any(|k| basin[k].is_some()) {
                    let (mut k, mut subs) = (0, Vec::new());
                    while k < seg.len() {
                        if basin[k].is_some() {
                            k += 1;
                            continue;
                        }
                        let i = k;
                        while k < seg.len() && basin[k].is_none() {
                            k += 1;
                        }
                        subs.push((s + i.saturating_sub(1), s + k.min(seg.len() - 1)));
                    }
                    for sub in subs.into_iter().rev() {
                        queue.push_front(sub);
                    }
                    continue;
                }
                let ground: Vec<f64> = (0..seg.len()).map(|k| fit.ground(xy(k))).collect();
                let depth: Vec<f64> = seg.iter().map(|p| vector_water::bed_depth(p[2])).collect();
                let lake: Vec<Option<f64>> =
                    (0..seg.len()).map(|k| fit.lake_at(xy(k)).or(basin[k].map(|b| basins.level[b]))).collect();
                let (levels, pooled) = fit_levels(&ground, &depth, &lake, fill, fit.sea);
                let floats = |k: usize| lake[k].is_none() && levels[k] - ground[k] > depth[k] + FLOAT_EPSILON_M;
                if let Some(k) = (0..seg.len()).find(|&k| floats(k)) {
                    let level = levels[k];
                    let traced = flood(fit, &basins, xy(k), level, max_samples).and_then(|region| {
                        basin_outline(fit, &basins, &region, level, recipe, stats).map(|ring| (region, ring)).ok_or(Stop::Water)
                    });
                    match traced {
                        Ok((region, (outline, holes))) => {
                            let b = basins.level.len();
                            basins.level.push(level);
                            for &i in &region {
                                basins.of.insert(i, b);
                            }
                            pending.push(lakes.len());
                            lakes.push(LakeOut {
                                id: first_basin_id + b,
                                kind: "open",
                                level_m: level,
                                outline,
                                holes,
                                area_m2: region.len() as f64 * sample_area,
                                from_course: None,
                            });
                            stats.basin_lakes += 1;
                            // Worked again: its points in the new lake split it.
                            queue.push_front((s, e));
                        }
                        Err(stop) => {
                            match stop {
                                Stop::Stamp => stats.courses_ended_stamp += 1,
                                Stop::Size => stats.courses_ended_size += 1,
                                Stop::Water => stats.courses_ended_water += 1,
                            }
                            // The rim: the last point before the run under the level that holds the floating point.
                            let mut a = k;
                            while a > 0 && ground[a - 1] < level {
                                a -= 1;
                            }
                            queue.clear();
                            if a >= 2 {
                                queue.push_front((s, s + a - 1));
                            }
                            ended = true;
                        }
                    }
                    continue;
                }
                stats.levels_pooled += pooled;
                above.extend((0..seg.len()).filter(|&k| lake[k].is_none()).map(|k| levels[k] - ground[k]));
                let points: Vec<[f64; 4]> = seg.iter().zip(&levels).map(|(p, &l)| [p[0], p[1], p[2], l]).collect();
                stats.river_km += points.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum::<f64>() / 1000.0;
                let id = out.len();
                for l in pending.drain(..) {
                    lakes[l].from_course = Some(id);
                }
                // Only the piece that ends where the course ends reaches what it joins.
                let to_end = e == n - 1 && !ended;
                out.push(RiverOut {
                    id,
                    kind,
                    mu_id: line.id,
                    joins: line.joins.filter(|_| to_end),
                    outlet: line.outlet.filter(|_| to_end),
                    clipped: clipped || ended,
                    points,
                });
            }
            if out.len() == before {
                stats.rivers_dropped += 1;
            }
        }
    }
    stats.points_floating = out
        .iter()
        .flat_map(|r| r.points.iter())
        .filter(|p| {
            let q = [p[0], p[1]];
            fit.lake_at(q).is_none()
                && basins.at(fit, q).is_none()
                && p[3] - fit.ground(q) > vector_water::bed_depth(p[2]) + FLOAT_EPSILON_M
        })
        .count();
    stats.river_level_p50_m = percentile(&mut above, 0.5);
    stats.river_level_p90_m = percentile(&mut above, 0.9);
    stats.rivers = out.len();
    out
}
