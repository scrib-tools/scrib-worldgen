//! `streets.json` (slice S1a): the town's planned streets as a road network, format v1, in the guide frame, so the
//! game draws them with the ribbons it draws the regional roads with (Scrib `sf_worldgen::roads`: `{ "format": 1,
//! "segments": [{ "id", "class", "surface", "width_m", "a", "b", "points", "crossings", "length_m", "grade" }],
//! "junctions": [{ "id", "pos", "kind", "sites" }] }`, x east, y north, metres; extra keys are ignored there).
//!
//! One segment per Magna Urbis street (`Layout.streets`, its local frame moved to the guide frame by
//! `crate::grid::Frame::to_world`, the y flip):
//! - class and surface from the recipe's `[streets]` tables (role first, then rank);
//! - `width_m` = the mean of the street's vertex widths; `length_m` along the path;
//! - `a` / `b` = junctions: street ends within `junction_merge_m` are one junction (greedy, first end seen holds the
//!   position) and the ends are moved onto it, so ribbons meet;
//! - crossings: a street Magna Urbis made a `bridge`, `causeway` or `ford` is that crossing end to end; any other
//!   street gets a `bridge` over each stretch whose samples (every `water_sample_m`) fall in the site's water raster
//!   (`water.u8`), padded by `bridge_pad_m` each side, so the ribbon never drapes into a river bed;
//! - `grade` = the steepest rise over run between samples `grade_run_m` apart on the site heights, off the deck spans;
//! - `regional`: every water sample within `regional_match_m` of one of the regional roads the town was laid along
//!   (`crate::regional`), so the game can join it to the regional network later.
//!
//! Extra keys per segment: `regional`, Magna Urbis's `rank` and `role`. Squares and plazas (polygons) are not written.
// Sample counts: bounded by street lengths over metre spacings.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
use crate::grid::Frame;
use crate::recipe::StreetsRecipe;
use crate::site::SiteRaster;
use magna_urbis_core::urban::{Point, Street};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// `streets.json` format version (road network v1).
pub const STREETS_FORMAT: u32 = 1;
pub const STREETS_FILE: &str = "streets.json";
/// Magna Urbis street roles that are a crossing end to end; the role is the road network crossing kind.
pub const CROSSING_ROLES: [&str; 3] = ["bridge", "causeway", "ford"];
/// Crossing kinds the game's ribbon skips (a deck carries the road there).
pub const DECK_KINDS: [&str; 2] = ["bridge", "causeway"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreetsFile {
    pub format: u32,
    pub producer: String,
    pub culture: String,
    pub segments: Vec<Segment>,
    pub junctions: Vec<Junction>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub id: u32,
    pub class: String,
    pub surface: String,
    pub width_m: f64,
    pub a: u32,
    pub b: u32,
    /// Guide frame (x east, y north), metres.
    pub points: Vec<[f64; 2]>,
    pub crossings: Vec<Crossing>,
    pub length_m: f64,
    /// Steepest rise over run along the path, off deck spans.
    pub grade: f64,
    /// Laid along one of the guide's regional roads.
    pub regional: bool,
    /// Magna Urbis's `Street.rank` and `Street.role`.
    pub rank: usize,
    pub role: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Crossing {
    /// `bridge | ford | causeway`.
    pub kind: String,
    pub points: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Junction {
    pub id: u32,
    pub pos: [f64; 2],
    /// `end` (one street) or `junction` (two or more).
    pub kind: String,
    pub sites: Vec<String>,
}

/// Station (m along the path) of every vertex.
fn stations(path: &[Point]) -> Vec<f64> {
    let mut out = vec![0.0];
    for w in path.windows(2) {
        out.push(out[out.len() - 1] + w[0].distance(w[1]));
    }
    out
}

/// The point at station `d`.
fn point_at(path: &[Point], st: &[f64], at: f64) -> Point {
    let mut i = 1;
    while i < st.len() - 1 && st[i] < at {
        i += 1;
    }
    let span = st[i] - st[i - 1];
    let t = if span > 0.0 { ((at - st[i - 1]) / span).clamp(0.0, 1.0) } else { 0.0 };
    let (from, to) = (path[i - 1], path[i]);
    Point { x: from.x + t * (to.x - from.x), y: from.y + t * (to.y - from.y) }
}

/// The path between stations `from` and `to`.
fn sub_path(path: &[Point], st: &[f64], from: f64, to: f64) -> Vec<Point> {
    let mut out = vec![point_at(path, st, from)];
    out.extend(path.iter().zip(st).filter(|&(_, &s)| s > from && s < to).map(|(&p, _)| p));
    out.push(point_at(path, st, to));
    out
}

/// `n + 1` evenly spaced stations over `[0, length]`, at most `step` apart.
fn even_stations(length: f64, step: f64) -> Vec<f64> {
    let n = (length / step).ceil().max(1.0) as usize;
    (0..=n).map(|k| length * k as f64 / n as f64).collect()
}

fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

fn near_any(p: Point, roads: &[Street], tol: f64) -> bool {
    roads.iter().any(|r| r.path.windows(2).any(|w| segment_distance(p, w[0], w[1]) <= tol))
}

/// Deck and ford spans `(from, to, kind)` of one street, in stations.
fn spans(street: &Street, path: &[Point], st: &[f64], wet: &dyn Fn(Point) -> bool, spec: &StreetsRecipe) -> Vec<(f64, f64, String)> {
    let length = st[st.len() - 1];
    if CROSSING_ROLES.contains(&street.role.as_str()) {
        return vec![(0.0, length, street.role.clone())];
    }
    let samples = even_stations(length, spec.water_sample_m.value);
    let half = if samples.len() > 1 { samples[1] / 2.0 } else { 0.0 };
    let pad = spec.bridge_pad_m.value;
    let mut out: Vec<(f64, f64, String)> = Vec::new();
    for &s in &samples {
        if !wet(point_at(path, st, s)) {
            continue;
        }
        let (from, to) = ((s - half - pad).max(0.0), (s + half + pad).min(length));
        match out.last_mut() {
            Some(last) if from <= last.1 => last.1 = last.1.max(to),
            _ => out.push((from, to, "bridge".into())),
        }
    }
    out
}

/// Steepest rise over run between samples `grade_run_m` apart, skipping runs that touch a deck span.
fn grade(path: &[Point], st: &[f64], decks: &[(f64, f64)], height: &dyn Fn(Point) -> f64, run_m: f64) -> f64 {
    let samples = even_stations(st[st.len() - 1], run_m);
    let on_deck = |s: f64| decks.iter().any(|&(f, t)| s >= f && s <= t);
    samples
        .windows(2)
        .filter(|w| w[1] > w[0] && !on_deck(w[0]) && !on_deck(w[1]))
        .map(|w| (height(point_at(path, st, w[1])) - height(point_at(path, st, w[0]))).abs() / (w[1] - w[0]))
        .fold(0.0, f64::max)
}

/// The junction holding street end `p` (one within `merge` m, else a new one), its degree counted.
fn junction(junctions: &mut Vec<([f64; 2], usize)>, p: [f64; 2], merge: f64) -> u32 {
    let found = junctions.iter().position(|(q, _)| (q[0] - p[0]).hypot(q[1] - p[1]) <= merge);
    let i = found.unwrap_or_else(|| {
        junctions.push((p, 0));
        junctions.len() - 1
    });
    junctions[i].1 += 1;
    i as u32
}

/// The road network of a town's streets (local frame in, guide frame out).
///
/// `regional` = the regional roads the town was laid along (local frame), for the `regional` flag.
#[must_use]
pub fn streets(culture: &str, streets: &[Street], frame: &Frame, site: &SiteRaster, regional: &[Street], spec: &StreetsRecipe) -> StreetsFile {
    let world = |p: Point| frame.to_world(p);
    let wet = |p: Point| {
        let [x, y] = world(p);
        site.cell_index(x, y).is_some_and(|i| site.water[i] != 0)
    };
    let height = |p: Point| {
        let [x, y] = world(p);
        f64::from(site.height_at(x, y))
    };
    let merge = spec.junction_merge_m.value;
    let mut junctions: Vec<([f64; 2], usize)> = Vec::new();
    let (mut segments, mut warnings) = (Vec::new(), Vec::new());
    for (n, street) in streets.iter().enumerate() {
        let path = &street.path;
        let st = stations(path);
        let length = st[st.len() - 1];
        if path.len() < 2 || length.is_nan() || length <= 0.0 {
            warnings.push(format!("street {n} ({}): no length, skipped", street.role));
            continue;
        }
        let spans = spans(street, path, &st, &wet, spec);
        let decks: Vec<(f64, f64)> =
            spans.iter().filter(|s| DECK_KINDS.contains(&s.2.as_str())).map(|s| (s.0, s.1)).collect();
        let regional_flag = !regional.is_empty()
            && even_stations(length, spec.water_sample_m.value)
                .iter()
                .all(|&s| near_any(point_at(path, &st, s), regional, spec.regional_match_m.value));
        let mut points: Vec<[f64; 2]> = path.iter().map(|&p| world(p)).collect();
        let last = points.len() - 1;
        let (a, b) = (junction(&mut junctions, points[0], merge), junction(&mut junctions, points[last], merge));
        let style = spec.style(&street.role, street.rank);
        let width_m = if street.widths.is_empty() { 0.0 } else { street.widths.iter().sum::<f64>() / street.widths.len() as f64 };
        let crossings = spans
            .iter()
            .map(|(from, to, kind)| Crossing { kind: kind.clone(), points: sub_path(path, &st, *from, *to).into_iter().map(world).collect() })
            .collect();
        // Ends on their junction (first end seen holds it).
        points[0] = junctions[a as usize].0;
        points[last] = junctions[b as usize].0;
        segments.push(Segment {
            id: segments.len() as u32,
            class: style.class.clone(),
            surface: style.surface.clone(),
            width_m,
            a,
            b,
            points,
            crossings,
            length_m: length,
            grade: grade(path, &st, &decks, &height, spec.grade_run_m.value),
            regional: regional_flag,
            rank: street.rank,
            role: street.role.clone(),
        });
    }
    let junctions = junctions
        .into_iter()
        .enumerate()
        .map(|(i, (pos, degree))| Junction {
            id: i as u32,
            pos,
            kind: if degree > 1 { "junction" } else { "end" }.into(),
            sites: Vec::new(),
        })
        .collect();
    StreetsFile { format: STREETS_FORMAT, producer: format!("settle {}", env!("CARGO_PKG_VERSION")), culture: culture.into(), segments, junctions, warnings }
}

/// `stats.json` `streets_export`: segment and junction counts, km by class and by surface, crossings by kind, the
/// regional count and the steepest grade.
#[must_use]
pub fn summary(file: &StreetsFile) -> Value {
    let (mut km_class, mut km_surface, mut crossings) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    for s in &file.segments {
        *km_class.entry(s.class.clone()).or_insert(0.0) += s.length_m / 1000.0;
        *km_surface.entry(s.surface.clone()).or_insert(0.0) += s.length_m / 1000.0;
        for c in &s.crossings {
            *crossings.entry(c.kind.clone()).or_insert(0_usize) += 1;
        }
    }
    let round = |m: BTreeMap<String, f64>| m.into_iter().map(|(k, v)| (k, (v * 1000.0).round() / 1000.0)).collect::<BTreeMap<_, _>>();
    let max_grade = file.segments.iter().map(|s| s.grade).fold(0.0, f64::max);
    json!({
        "segments": file.segments.len(),
        "junctions": file.junctions.len(),
        "km_by_class": round(km_class),
        "km_by_surface": round(km_surface),
        "crossings_by_kind": crossings,
        "regional": file.segments.iter().filter(|s| s.regional).count(),
        "max_grade": (max_grade * 1000.0).round() / 1000.0,
        "warnings": file.warnings.len(),
    })
}
