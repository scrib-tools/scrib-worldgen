//! Milestone 2 file formats (slice PL-1, `docs/research/settlements-m2-placement-2026-11-24.md` section 5), format 1
//! each, versioned like `site.json`: a reader refuses any other `format` before looking at the rest.
//!
//! - `kit.json` (read; written by Scrib): the prefab catalogue. Local model frame: x right, y front (door side),
//!   z up, pivot at the model origin; `facing_deg` is a door normal, 0 = +x, counter-clockwise.
//! - `placements.json` (written here, read back by tests and Scrib): lots, instances, planned streets, features.
//! - `grade.json` + `grade.f32`, `grade_w.f32`, `ground.u8` (written here): planned ground, aligned with Scrib's
//!   terrain stamp format v1 so Scrib copies it without resampling: `origin_m` is the centre of sample (0, 0),
//!   row 0 = north, `spacing_m` between samples, `dims` = `[w, h]`; heights in final metres, weights f32 0..1.
//!
//! World frame everywhere: metres, x east, y north, z up; yaw counter-clockwise about +z seen from above;
//! world = pos + R(yaw) · local. No game names: ground classes are generic, Scrib maps them to textures.
// Raster sample positions: indices are bounded by the dims.
#![allow(clippy::cast_precision_loss)]
use crate::lots::Xy;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// A world point with height (m, x east, y north, z up).
pub type Xyz = [f64; 3];

/// `kit.json` format version.
pub const KIT_FORMAT: u32 = 1;
/// `placements.json` format version.
pub const PLACEMENTS_FORMAT: u32 = 1;
/// `grade.json` format version.
pub const GRADE_FORMAT: u32 = 1;

pub const KIT_FILE: &str = "kit.json";
pub const PLACEMENTS_FILE: &str = "placements.json";
pub const GRADE_FILE: &str = "grade.json";
pub const GRADE_HEIGHTS_FILE: &str = "grade.f32";
pub const GRADE_WEIGHTS_FILE: &str = "grade_w.f32";
pub const GROUND_FILE: &str = "ground.u8";

/// The `frame` text `grade.json` carries.
pub const GRADE_FRAME: &str = "world m, x east, y north, z up; origin_m = centre of sample (0,0); row 0 = north";

/// Ground class legend written by default: index = class value in `ground.u8`, 0 = none.
pub const GROUND_LEGEND: [&str; 10] =
    ["none", "street_rank0", "street_rank1", "street_rank2", "street_rank3", "street_rank4", "plaza", "yard", "quay", "verge"];

/// Only the `format` field, read first so a newer file fails on its version, not on a schema detail.
#[derive(Deserialize)]
struct Probe {
    format: u32,
}

/// Parse a versioned JSON file: `format` must be `want`, then the full schema.
fn parse_versioned<T: DeserializeOwned>(text: &str, what: &str, want: u32) -> Result<T, String> {
    let probe: Probe = serde_json::from_str(text).map_err(|e| format!("{what}: {e}"))?;
    if probe.format != want {
        return Err(format!("{what}: format {}, this tool reads format {want}", probe.format));
    }
    serde_json::from_str(text).map_err(|e| format!("{what}: {e}"))
}

fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn to_json<T: Serialize>(value: &T) -> String {
    // Plain structs, `Vec`s and `BTreeMap`s with string keys: serialising cannot fail, the output is ordered.
    serde_json::to_string_pretty(value).expect("format types serialise")
}

/// Rotate a local vector by `yaw_deg` counter-clockwise. Quarter turns are exact (no `cos(90°) = 6e-17`), so a
/// kit with `yaw_step_deg: 90` lands on exact coordinates.
#[must_use]
#[allow(clippy::float_cmp)] // exact quarter turns are the point
pub fn rotate(v: Xy, yaw_deg: f64) -> Xy {
    let q = yaw_deg.rem_euclid(360.0);
    let (s, c) = if q == 0.0 {
        (0.0, 1.0)
    } else if q == 90.0 {
        (1.0, 0.0)
    } else if q == 180.0 {
        (0.0, -1.0)
    } else if q == 270.0 {
        (-1.0, 0.0)
    } else {
        q.to_radians().sin_cos()
    };
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}

/// Local point -> world point for a model placed at `pos` with `yaw_deg`.
#[must_use]
pub fn to_world(local: Xy, pos: Xy, yaw_deg: f64) -> Xy {
    let r = rotate(local, yaw_deg);
    [pos[0] + r[0], pos[1] + r[1]]
}

// ---------------------------------------------------------------------------------------------------------------
// kit.json

/// `kit.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitFile {
    pub format: u32,
    pub name: String,
    pub producer: String,
    /// Always `m`.
    pub units: String,
    /// Free text naming the local model frame.
    pub frame: String,
    pub models: Vec<KitModel>,
}

/// What a kit model is for: the research schema's kinds plus `bridge`, `canal`, `lamp`, `tower` (Scrib's exporter).
/// Fitting places only `building`s; the filler places the `wall`, `arch` and `tower` ids its recipe names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KitKind {
    Building,
    Wall,
    Arch,
    Stall,
    Yard,
    Prop,
    Stairs,
    RetainingWall,
    Bridge,
    Canal,
    Lamp,
    /// A street-wall tower (`crate::filler`), never a lot building.
    Tower,
}

/// One prefab of the kit, in its local frame (x right, y front, z up, pivot at the origin).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitModel {
    /// Stable id (Scrib's own id for the model).
    pub id: String,
    pub kind: KitKind,
    /// Free text (`wealth:low`, ...); absent or null = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Roles or functions this model may serve (`dwelling`, `commerce.inn`); absent or null = not filled yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roles: Option<Vec<String>>,
    /// Convex hull of the mesh on the ground (local m).
    pub footprint: Vec<Xy>,
    /// Oriented rectangle of the hull, a fast pre-filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obb: Option<Obb>,
    pub doors: Vec<Door>,
    pub height_m: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floors: Option<u32>,
    /// Lowest mesh point relative to the pivot z.
    pub base_offset_m: f64,
    /// Terrain height in the model's frame as vanilla placed it (median over references, at the street door's outside
    /// point or under the hull centroid); absent = seat on the street door's sill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_z_m: Option<f64>,
    /// Mesh below the ground (with `ground_z_m`: `ground_z_m - base_offset_m`) or below the sill that may bury.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foundation_depth_m: Option<f64>,
    /// 0 or absent = any yaw, 90 = quarter turns only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yaw_step_deg: Option<f64>,
    pub weight: f64,
    /// BLAKE3 (hex) of the source mesh.
    pub blake3: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obb {
    pub centre: Xy,
    pub half: Xy,
    pub yaw_deg: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Door {
    /// Local position on the ground.
    pub at: Xy,
    /// Door normal in the local frame, 0 = +x, counter-clockwise.
    pub facing_deg: f64,
    pub width_m: f64,
    pub sill_z_m: f64,
}

/// A door after placing its model: world position and outward facing (degrees in `[0, 360)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldDoor {
    pub at: Xy,
    pub facing_deg: f64,
}

impl KitFile {
    /// Parse `kit.json` text.
    ///
    /// # Errors
    /// Malformed JSON, a format other than 1, an unknown field or kind, duplicate model ids, a footprint with
    /// fewer than 3 points.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let kit: Self = parse_versioned(text, KIT_FILE, KIT_FORMAT)?;
        let mut ids = BTreeSet::new();
        for m in &kit.models {
            if !ids.insert(m.id.as_str()) {
                return Err(format!("{KIT_FILE}: duplicate model id {:?}", m.id));
            }
            if m.footprint.len() < 3 {
                return Err(format!("{KIT_FILE}: model {:?} footprint has {} points", m.id, m.footprint.len()));
            }
        }
        Ok(kit)
    }

    /// Read a `kit.json` file.
    ///
    /// # Errors
    /// I/O errors and everything [`KitFile::from_json`] refuses.
    pub fn load(path: &Path) -> Result<Self, String> {
        Self::from_json(&read_text(path)?).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The model with this id.
    #[must_use]
    pub fn model(&self, id: &str) -> Option<&KitModel> {
        self.models.iter().find(|m| m.id == id)
    }
}

impl KitModel {
    /// The footprint hull in the world for the model placed at `pos` with `yaw_deg` (same point order).
    #[must_use]
    pub fn world_hull(&self, pos: Xy, yaw_deg: f64) -> Vec<Xy> {
        self.footprint.iter().map(|&p| to_world(p, pos, yaw_deg)).collect()
    }

    /// Every door in the world for the model placed at `pos` with `yaw_deg`.
    #[must_use]
    pub fn world_doors(&self, pos: Xy, yaw_deg: f64) -> Vec<WorldDoor> {
        self.doors.iter().map(|d| d.to_world(pos, yaw_deg)).collect()
    }
}

impl Door {
    #[must_use]
    pub fn to_world(&self, pos: Xy, yaw_deg: f64) -> WorldDoor {
        WorldDoor { at: to_world(self.at, pos, yaw_deg), facing_deg: (self.facing_deg + yaw_deg).rem_euclid(360.0) }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// placements.json

/// `placements.json`: what the tool placed, world frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementsFile {
    pub format: u32,
    pub name: String,
    pub producer: String,
    pub recipe: String,
    pub seed: String,
    /// BLAKE3 (hex) of the `kit.json` the instances name.
    pub kit_blake3: String,
    /// World reference point of the plan (m); all coordinates in the file are absolute world metres.
    pub origin_m: Xy,
    pub lots: Vec<PlacedLot>,
    pub instances: Vec<Instance>,
    pub streets: Vec<PlannedStreet>,
    pub features: Vec<Feature>,
    /// File name of the grade header next to this file (`grade.json`), absent before grading exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grade: Option<String>,
}

/// How a lot was used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LotStatus {
    /// One prefab on the lot.
    Filled,
    /// Joined with neighbours (`merged_from`) under one prefab.
    Merged,
    /// Walls or yard instead of a building.
    Filler,
    Empty,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacedLot {
    /// Magna Urbis `Parcel.id`, as in `lots.json`.
    pub id: String,
    pub polygon: Vec<Xy>,
    pub status: LotStatus,
    pub role: Option<String>,
    /// Pad height (m) under the lot's prefab, if it has one.
    pub pad_z_m: Option<f64>,
    pub front: Option<[Xy; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merged_from: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    pub id: String,
    /// Kit model id.
    pub kit: String,
    /// Lot id, if the instance stands on one.
    pub lot: Option<String>,
    /// Pivot in the world.
    pub pos: Xyz,
    pub yaw_deg: f64,
    /// `building`, `filler`, `wall`, ...
    pub role: String,
    /// World position of the door that faces the street, if the model has one.
    pub door: Option<Xy>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedStreet {
    pub id: String,
    pub rank: u32,
    pub role: String,
    /// Centreline with planned heights.
    pub polyline: Vec<Xyz>,
    /// Width per polyline vertex (as Magna Urbis's `Street.widths`).
    pub widths_m: Vec<f64>,
    /// Ground class name from the grade legend.
    pub surface: String,
}

/// A grade break the tool could not smooth away.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureKind {
    RetainingWall,
    Stairs,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GradeBreak {
    pub kind: FeatureKind,
    pub polyline: Vec<Xyz>,
    pub base_z_m: f64,
    pub top_z_m: f64,
}

/// One `features` entry: told apart by its `kind` (`retaining_wall` / `stairs`, or `garden`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Feature {
    Grade(GradeBreak),
    Garden(Garden),
}

/// The only kind of a [`Garden`] feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GardenKind {
    Garden,
}

/// Why a lot became a garden (`crate::gardens`, a temporary scaffold).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GardenSource {
    /// A plot lot no kit building went on.
    EmptyPlot,
    /// A Magna Urbis `garden` lot.
    MuGarden,
    /// A plot lot no kit building went on, too narrow for any (`gardens.sliver_width_m`).
    Sliver,
}

impl GardenSource {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::EmptyPlot => "empty_plot",
            Self::MuGarden => "mu_garden",
            Self::Sliver => "sliver",
        }
    }
}

/// A lot planted as a garden (`crate::gardens`): its outer ring, world frame, counter-clockwise, no holes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Garden {
    pub kind: GardenKind,
    pub source: GardenSource,
    pub lot: String,
    pub polygon: Vec<Xy>,
}

impl PlacementsFile {
    /// Deterministic text: struct field order, vectors in order, no maps.
    #[must_use]
    pub fn to_json(&self) -> String {
        to_json(self)
    }

    /// Parse `placements.json` text.
    ///
    /// # Errors
    /// Malformed JSON, a format other than 1, an unknown field, a street whose widths do not match its vertices.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let file: Self = parse_versioned(text, PLACEMENTS_FILE, PLACEMENTS_FORMAT)?;
        for s in &file.streets {
            if s.widths_m.len() != s.polyline.len() {
                return Err(format!(
                    "{PLACEMENTS_FILE}: street {:?} has {} widths for {} vertices",
                    s.id,
                    s.widths_m.len(),
                    s.polyline.len()
                ));
            }
        }
        Ok(file)
    }

    /// Write `placements.json` into `dir`.
    ///
    /// # Errors
    /// I/O errors.
    pub fn write(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(PLACEMENTS_FILE);
        fs::write(&path, self.to_json()).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Read `placements.json` from `dir`.
    ///
    /// # Errors
    /// I/O errors and everything [`PlacementsFile::from_json`] refuses.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join(PLACEMENTS_FILE);
        Self::from_json(&read_text(&path)?).map_err(|e| format!("{}: {e}", path.display()))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// grade.json + rasters

/// `grade.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GradeMeta {
    pub format: u32,
    /// [`GRADE_FRAME`].
    pub frame: String,
    /// World position (m) of the centre of sample (0, 0), the north-west sample.
    pub origin_m: Xy,
    /// Distance between samples (m).
    pub spacing_m: f64,
    /// Samples: `[w, h]`.
    pub dims: [usize; 2],
    /// Ground classes: index = value in `ground.u8`; entry 0 is `none`.
    pub ground_legend: Vec<String>,
    /// BLAKE3 (hex) of each raster, keyed by its file name; all three are required.
    pub blake3: BTreeMap<String, String>,
    pub max_cut_m: f64,
    pub max_fill_m: f64,
    pub cut_m3: f64,
    pub fill_m3: f64,
    /// Width of the band blending the plan into untouched ground (m).
    pub blend_band_m: f64,
    /// Recipe name and seed.
    pub source: String,
}

/// A grade: header plus its rasters (row-major, `w * h`, row 0 = north).
#[derive(Clone, Debug, PartialEq)]
pub struct Grade {
    pub meta: GradeMeta,
    /// Planned heights, final metres (same datum as `site.json` heights).
    pub heights: Vec<f32>,
    /// Blend weight 0..1: Scrib applies `h = mix(h_gen, h_plan, w)`.
    pub weights: Vec<f32>,
    /// Ground class per sample, an index into `ground_legend`.
    pub ground: Vec<u8>,
}

impl GradeMeta {
    /// World position (m) of sample (`col`, `row`).
    #[must_use]
    pub fn sample_pos(&self, col: usize, row: usize) -> Xy {
        [self.origin_m[0] + col as f64 * self.spacing_m, self.origin_m[1] - row as f64 * self.spacing_m]
    }
}

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn f32_from(bytes: &[u8]) -> Vec<f32> {
    bytes.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

impl Grade {
    /// Write the rasters and `grade.json` into `dir`; fills `meta.blake3`. Same input, same bytes.
    ///
    /// # Errors
    /// Rasters whose sizes do not match `dims`, I/O errors.
    pub fn write(&mut self, dir: &Path) -> Result<(), String> {
        let n = self.meta.dims[0] * self.meta.dims[1];
        if self.heights.len() != n || self.weights.len() != n || self.ground.len() != n {
            return Err(format!(
                "{GRADE_FILE}: rasters {}/{}/{} samples, dims {:?} need {n}",
                self.heights.len(),
                self.weights.len(),
                self.ground.len(),
                self.meta.dims
            ));
        }
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let files: [(&str, Vec<u8>); 3] = [
            (GRADE_HEIGHTS_FILE, f32_bytes(&self.heights)),
            (GRADE_WEIGHTS_FILE, f32_bytes(&self.weights)),
            (GROUND_FILE, self.ground.clone()),
        ];
        self.meta.blake3.clear();
        for (file, bytes) in &files {
            self.meta.blake3.insert((*file).into(), blake3::hash(bytes).to_hex().to_string());
            let path = dir.join(file);
            fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        let path = dir.join(GRADE_FILE);
        fs::write(&path, to_json(&self.meta)).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Read `grade.json` and its rasters from `dir`.
    ///
    /// # Errors
    /// Missing or malformed files, a format other than 1, bad dims or spacing, a legend not starting with `none`,
    /// a raster of the wrong size, a missing or mismatched blake3, weights outside 0..1, a class past the legend.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let meta_path = dir.join(GRADE_FILE);
        let at = |e: String| format!("{}: {e}", meta_path.display());
        let meta: GradeMeta = parse_versioned(&read_text(&meta_path)?, GRADE_FILE, GRADE_FORMAT).map_err(at)?;
        let [w, h] = meta.dims;
        if w == 0 || h == 0 || !(meta.spacing_m.is_finite() && meta.spacing_m > 0.0) {
            return Err(at(format!("bad dims {:?} or spacing_m {}", meta.dims, meta.spacing_m)));
        }
        if meta.ground_legend.first().map(String::as_str) != Some("none") || meta.ground_legend.len() > 256 {
            return Err(at(format!("ground_legend must start with \"none\" and hold at most 256 classes: {:?}", meta.ground_legend)));
        }
        let n = w * h;
        let read = |file: &str, len: usize| -> Result<Vec<u8>, String> {
            let path = dir.join(file);
            let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            if bytes.len() != len {
                return Err(format!("{}: {} bytes, expected {len} for dims {:?}", path.display(), bytes.len(), meta.dims));
            }
            let want = meta.blake3.get(file).ok_or_else(|| format!("{}: no blake3 in {GRADE_FILE}", path.display()))?;
            let got = blake3::hash(&bytes).to_hex();
            if got.as_str() != want {
                return Err(format!("{}: blake3 {got} does not match {GRADE_FILE} {want}", path.display()));
            }
            Ok(bytes)
        };
        let heights = f32_from(&read(GRADE_HEIGHTS_FILE, n * 4)?);
        let weights = f32_from(&read(GRADE_WEIGHTS_FILE, n * 4)?);
        let ground = read(GROUND_FILE, n)?;
        if let Some(i) = weights.iter().position(|v| !(0.0..=1.0).contains(v)) {
            return Err(format!("{GRADE_WEIGHTS_FILE}: weight {} at sample {i} outside 0..1", weights[i]));
        }
        if let Some(i) = ground.iter().position(|&c| usize::from(c) >= meta.ground_legend.len()) {
            return Err(format!("{GROUND_FILE}: class {} at sample {i} past the legend", ground[i]));
        }
        Ok(Self { meta, heights, weights, ground })
    }
}
