//! `lots.json` (slice PL-0): every lot of a culture's partition with Magna Urbis's own building request for it
//! (`building_sites::requests`, `urban/building_sites.rs:1671`), reduced to what fitting a prefab needs: the lot, its
//! frontage, its main access, its programme (role, function, wealth, floors) and the building footprint Magna Urbis
//! drew on it (`preview` + `preview_parts`, `:1371-1372`).
//!
//! World frame (x east, y north, metres), like the planned `placements.json`: the one conversion from Magna Urbis's
//! local frame (y south, `layout.json` + its `frame`) lives here. Deterministic for one seed: no timings in the file
//! (the call's wall time goes to `stats.json`).
//!
//! Magna Urbis fails the whole call on the first failing request (`:1780`); then, or if it panics, the lots come
//! from the parcels alone with empty footprints and the header says why (`error`), so the run still finishes.
use crate::grid::Frame;
use crate::run::{CultureRun, Prepared};
use crate::site::SiteRaster;
use magna_urbis_core::urban::programme::Allocation;
use magna_urbis_core::urban::{Parcel, Point, Shape, building_sites};
use magna_urbis_geom as g;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

/// `lots.json` format version.
pub const LOTS_FORMAT: u32 = 1;

/// A world point (m, x east, y north).
pub type Xy = [f64; 2];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LotsFile {
    pub format: u32,
    pub culture: String,
    pub seed: String,
    /// Why the lots carry no building request (Magna Urbis's `requests` failed or panicked); absent when it ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub lots: Vec<Lot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lot {
    /// Magna Urbis `Parcel.id` (stable per seed).
    pub id: String,
    /// Magna Urbis's lot use (`Parcel.use`: plot, compound, garden, reserve...).
    #[serde(rename = "use")]
    pub use_kind: String,
    /// Outer ring of the lot.
    pub polygon: Vec<Xy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<Xy>>,
    /// Frontage segment (`Parcel.front`), if the lot has one.
    pub front: Option<[Xy; 2]>,
    /// Frontage direction (`Parcel.axis`), a world vector.
    pub axis: Xy,
    /// Main access: the widest public entry of the request's `accesses` (else its first), if any.
    pub access: Option<Access>,
    /// Programme allocation (`partition.programme.allocations`, joined by parcel id), if the lot has one.
    pub role: Option<String>,
    pub function: Option<String>,
    pub wealth: Option<f64>,
    /// Storeys `[min, max]` from the request's programme, if it sets them.
    pub floors: Option<[u32; 2]>,
    /// Building footprints Magna Urbis drew on the lot (several on a craft yard), empty without a request.
    pub preview_parts: Vec<Part>,
    /// Site height (m, the game's own relief, not the planning grid's scaled one) at the lot's interior point.
    pub ground_z_m: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Access {
    /// `public`, `private` or `collective`.
    pub kind: String,
    pub segment: [Xy; 2],
    pub width_m: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    /// Magna Urbis's label of the piece (`building`, a court, a prayer space...).
    pub label: String,
    pub polygon: Vec<Xy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<Xy>>,
}

/// A culture's lots and what the request call cost.
pub struct Lots {
    pub file: LotsFile,
    /// Wall time of Magna Urbis's `requests` call (ms).
    pub requests_ms: f64,
}

impl Lots {
    /// Lots carrying at least one footprint.
    #[must_use]
    pub fn with_preview(&self) -> usize {
        self.file.lots.iter().filter(|l| !l.preview_parts.is_empty()).count()
    }
}

fn ring(frame: &Frame, ring: &[Point]) -> Vec<Xy> {
    ring.iter().map(|&p| frame.to_world(p)).collect()
}

fn point(v: &Value) -> Option<Point> {
    Some(Point { x: v.get("x")?.as_f64()?, y: v.get("y")?.as_f64()? })
}

fn segment(frame: &Frame, v: &Value) -> Option<[Xy; 2]> {
    let s = v.as_array()?;
    Some([frame.to_world(point(s.first()?)?), frame.to_world(point(s.get(1)?)?)])
}

/// The request's main access: widest public first (`accesses` are sorted widest first, `building_sites.rs:23-28`).
fn access(frame: &Frame, request: &Value) -> Option<Access> {
    let all = request.get("accesses")?.as_array()?;
    let pick = all.iter().find(|a| a["kind"] == "public").or_else(|| all.first())?;
    Some(Access {
        kind: pick["kind"].as_str()?.to_string(),
        segment: segment(frame, &pick["segment"])?,
        width_m: pick["width_m"].as_f64()?,
    })
}

fn floors(request: &Value) -> Option<[u32; 2]> {
    let f = request.get("programme")?.get("floors")?.as_array()?;
    // Storey counts are small whole numbers.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = |v: &Value| v.as_u64().map(|n| n as u32).or_else(|| v.as_f64().map(|n| n.round() as u32));
    Some([n(f.first()?)?, n(f.get(1)?)?])
}

fn parts(frame: &Frame, request: &Value) -> Vec<Part> {
    let shapes: Vec<Shape> = request.get("preview").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    let labels = request.get("preview_parts").and_then(Value::as_array);
    shapes
        .iter()
        .enumerate()
        .map(|(i, s)| Part {
            label: labels.and_then(|l| l.get(i)).and_then(Value::as_str).unwrap_or("building").to_string(),
            polygon: ring(frame, &s.outer),
            holes: s.holes.iter().map(|h| ring(frame, h)).collect(),
        })
        .collect()
}

/// Magna Urbis's building requests for a run: `Ok` with one request per allocated, non-garden lot, or why not.
fn requests(prepared: &Prepared, seed: &str, run: &CultureRun, water_level_m: f64) -> Result<Vec<Value>, String> {
    // Only read where the planning grid is wet (`building_sites.rs:994`); the relief is scaled about the site's water
    // level (`Grid::crop`), so the level itself is unchanged on the grid.
    #[allow(clippy::cast_possible_truncation)]
    let water_level = vec![water_level_m as f32; prepared.grid.heights.len()];
    let call = || {
        building_sites::requests(
            seed,
            std::slice::from_ref(&run.layout),
            std::slice::from_ref(&run.partition),
            prepared.grid.surface(),
            &water_level,
            None,
        )
    };
    match catch_unwind(AssertUnwindSafe(call)) {
        Ok(result) => result,
        Err(panic) => {
            let what = panic.downcast_ref::<&str>().map(ToString::to_string).or_else(|| panic.downcast_ref::<String>().cloned());
            Err(format!("panicked: {}", what.unwrap_or_else(|| "?".into())))
        }
    }
}

fn lot(frame: &Frame, site: &SiteRaster, parcel: &Parcel, alloc: Option<&Allocation>, requests: &[&Value]) -> Lot {
    let inside = frame.to_world(g::interior_point(&parcel.shape));
    let first = requests.first().copied();
    Lot {
        id: parcel.id.clone(),
        use_kind: parcel.use_kind.clone(),
        polygon: ring(frame, &parcel.shape.outer),
        holes: parcel.shape.holes.iter().map(|h| ring(frame, h)).collect(),
        front: parcel.front.map(|[a, b]| [frame.to_world(a), frame.to_world(b)]),
        // A direction: only y flips (y south to y north).
        axis: [parcel.axis.x, -parcel.axis.y],
        access: first.and_then(|r| access(frame, r)),
        role: alloc.map(|a| a.role.clone()),
        function: alloc.map(|a| a.function.clone()),
        wealth: alloc.map(|a| a.wealth),
        floors: first.and_then(floors),
        preview_parts: requests.iter().flat_map(|r| parts(frame, r)).collect(),
        ground_z_m: f64::from(site.height_at(inside[0], inside[1])),
    }
}

/// Every lot of a culture's partition, in partition order, with its building request if Magna Urbis made one.
#[must_use]
pub fn lots(site: &SiteRaster, seed: &str, prepared: &Prepared, run: &CultureRun) -> Lots {
    let start = Instant::now();
    let result = requests(prepared, seed, run, site.meta.water_level_m);
    let requests_ms = start.elapsed().as_secs_f64() * 1000.0;
    let (requests, error) = match result {
        Ok(r) => (r, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let mut by_parcel: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for r in &requests {
        if let Some(id) = r["parcel_id"].as_str() {
            by_parcel.entry(id).or_default().push(r);
        }
    }
    let allocations: BTreeMap<&str, _> =
        run.partition.programme.iter().flat_map(|p| &p.allocations).map(|a| (a.parcel_id.as_str(), a)).collect();
    let frame = &prepared.grid.frame;
    let lots = run
        .partition
        .parcels
        .iter()
        .map(|p| {
            let reqs = by_parcel.get(p.id.as_str()).map_or(&[][..], Vec::as_slice);
            lot(frame, site, p, allocations.get(p.id.as_str()).copied(), reqs)
        })
        .collect();
    Lots { file: LotsFile { format: LOTS_FORMAT, culture: run.culture.clone(), seed: seed.into(), error, lots }, requests_ms }
}

/// `lots.json` text.
///
/// # Errors
/// Serialization errors.
pub fn lots_json(file: &LotsFile) -> Result<String, String> {
    serde_json::to_string(file).map_err(|e| e.to_string())
}
