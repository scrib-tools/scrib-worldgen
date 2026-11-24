//! The town's regional roads from a guide's `roads.json` (`worldgen-roads`, format 1, guide frame metres).
//!
//! Magna Urbis's region stage hands every town the regional network as urban `Street`s, each road segment split at
//! its crossings so a bridge, ford or causeway keeps its span (`rust/bridge/urbanRoads.ts:6` `regionalStreets`,
//! rev `21e22171f0b1766650b2cfef5d4bdaf574a1d325`). [`regional_streets`] is a faithful port of that function; the
//! rest crops the network to the site: segments whose bounds meet the crop are converted, moved into Magna Urbis's
//! local frame (`crate::grid::Frame::to_local`, the y flip; an isometry, so the converter's stations are unchanged)
//! and clipped to the crop less one cell, as `crate::crossing` keeps its founding road. Magna Urbis then fits decks
//! and approaches (`network::bridge_water`, `urban/mod.rs:121`), clips the roads to the town envelope and grows the
//! town along them. Given a roads file, the founding crossing is not added: the region's own roads replace it.
use crate::grid::Grid;
use magna_urbis_core::urban::{Point, Street};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// `roads.json` format this reader takes.
pub const ROADS_FORMAT: u32 = 1;

/// The part of a `roads.json` the town reads (other fields are ignored).
#[derive(Clone, Debug, Deserialize)]
pub struct RoadsJson {
    pub format: u32,
    pub segments: Vec<SegmentJson>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SegmentJson {
    pub id: usize,
    /// `highway | main | local | track | trail`.
    pub class: String,
    pub width_m: f64,
    /// Guide frame (x east, y north), metres.
    pub points: Vec<[f64; 2]>,
    #[serde(default)]
    pub crossings: Vec<CrossingJson>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CrossingJson {
    /// `bridge | ford | causeway`.
    pub kind: String,
    pub points: Vec<[f64; 2]>,
}

/// A read `roads.json`.
#[derive(Clone, Debug)]
pub struct RegionalRoads {
    pub file: RoadsJson,
    /// BLAKE3 (hex) of the file's bytes.
    pub blake3: String,
}

impl RegionalRoads {
    /// Parse a `roads.json` text.
    ///
    /// # Errors
    /// Bad JSON, a format other than [`ROADS_FORMAT`].
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: RoadsJson = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if file.format != ROADS_FORMAT {
            return Err(format!("roads.json format {} (this reader takes {ROADS_FORMAT})", file.format));
        }
        Ok(Self { file, blake3: blake3::hash(text.as_bytes()).to_hex().to_string() })
    }

    /// Read a `roads.json`.
    ///
    /// # Errors
    /// I/O errors and everything [`RegionalRoads::parse`] refuses.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// One regional road segment as Magna Urbis's converter reads it (`RoadOutput.segments[]`).
#[derive(Clone, Debug)]
pub struct RoadSegment {
    pub class: String,
    pub width: f64,
    pub path: Vec<Point>,
    /// `(kind, path)` of each crossing.
    pub crossings: Vec<(String, Vec<Point>)>,
}

/// Port of `regionalStreets` (`rust/bridge/urbanRoads.ts:6-58`): each segment cut at its crossings' ends (projected
/// onto the road), each piece a street with the crossing's kind as role (`regional` elsewhere), the segment's width at
/// every vertex and rank 0 highway, 1 main, 2 otherwise.
#[must_use]
pub fn regional_streets(segments: &[RoadSegment]) -> Vec<Street> {
    let mut out = Vec::new();
    for segment in segments {
        let points = &segment.path;
        if points.len() < 2 {
            continue;
        }
        let rank = match segment.class.as_str() {
            "highway" => 0,
            "main" => 1,
            _ => 2,
        };
        let mut station = vec![0.0];
        for i in 1..points.len() {
            station.push(station[i - 1] + (points[i].x - points[i - 1].x).hypot(points[i].y - points[i - 1].y));
        }
        let project = |p: Point| -> f64 {
            let (mut best, mut at) = (f64::INFINITY, 0.0);
            for i in 1..points.len() {
                let (a, b) = (points[i - 1], points[i]);
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let len2 = dx * dx + dy * dy;
                let len2 = if len2 == 0.0 { 1.0 } else { len2 };
                let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
                let d = (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy);
                if d < best {
                    best = d;
                    at = station[i - 1] + t * (station[i] - station[i - 1]);
                }
            }
            at
        };
        let crossings: Vec<(f64, f64, &str)> = segment
            .crossings
            .iter()
            .filter(|(_, path)| path.len() > 1)
            .map(|(kind, path)| {
                let (a, b) = (project(path[0]), project(path[path.len() - 1]));
                (a.min(b), a.max(b), kind.as_str())
            })
            .collect();
        let total = station[station.len() - 1];
        let mut cuts = vec![0.0, total];
        cuts.extend(crossings.iter().flat_map(|&(from, to, _)| [from, to]));
        cuts.sort_by(f64::total_cmp);
        cuts.dedup();
        let point_at = |d: f64| -> Point {
            let mut i = 1;
            while i < station.len() - 1 && station[i] < d {
                i += 1;
            }
            let span = station[i] - station[i - 1];
            let t = (d - station[i - 1]) / if span == 0.0 { 1.0 } else { span };
            let (a, b) = (points[i - 1], points[i]);
            Point { x: a.x + t * (b.x - a.x), y: a.y + t * (b.y - a.y) }
        };
        for pair in cuts.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            if to - from < 0.001 {
                continue;
            }
            let mid = f64::midpoint(from, to);
            let role = crossings.iter().find(|&&(f, t, _)| mid >= f && mid <= t).map_or("regional", |c| c.2);
            let mut path = vec![point_at(from)];
            path.extend(points.iter().zip(&station).filter(|&(_, &s)| s > from && s < to).map(|(&p, _)| p));
            path.push(point_at(to));
            out.push(Street { widths: vec![segment.width; path.len()], path, rank, role: role.into() });
        }
    }
    out
}

/// Liang-Barsky: the parameter range of `a + t (b - a)`, `t` in [0, 1], inside the square `[lo, hi]²`.
fn clip_segment(a: Point, b: Point, lo: f64, hi: f64) -> Option<(f64, f64)> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [(-dx, a.x - lo), (dx, hi - a.x), (-dy, a.y - lo), (dy, hi - a.y)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    (t0 <= t1).then_some((t0, t1))
}

/// The pieces of `street` inside the square `[lo, hi]²` (a street leaving and re-entering it gives two), each with
/// the street's first width at every vertex (the converter's streets are of one width), its rank and role.
#[must_use]
pub fn clip(street: &Street, lo: f64, hi: f64) -> Vec<Street> {
    let width = street.widths.first().copied().unwrap_or(0.0);
    let mut pieces: Vec<Vec<Point>> = Vec::new();
    let mut run: Vec<Point> = Vec::new();
    let lerp = |a: Point, b: Point, t: f64| Point { x: a.x + t * (b.x - a.x), y: a.y + t * (b.y - a.y) };
    for pair in street.path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let Some((t0, t1)) = clip_segment(a, b, lo, hi) else {
            pieces.extend((run.len() > 1).then(|| std::mem::take(&mut run)));
            run.clear();
            continue;
        };
        if t0 > 0.0 || run.is_empty() {
            pieces.extend((run.len() > 1).then(|| std::mem::take(&mut run)));
            run.clear();
            run.push(lerp(a, b, t0));
        }
        run.push(lerp(a, b, t1));
        if t1 < 1.0 {
            pieces.extend((run.len() > 1).then(|| std::mem::take(&mut run)));
            run.clear();
        }
    }
    pieces.extend((run.len() > 1).then_some(run));
    pieces
        .into_iter()
        .filter(|p| p.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>() >= 0.001)
        .map(|path| Street { widths: vec![width; path.len()], path, rank: street.rank, role: street.role.clone() })
        .collect()
}

/// The site's regional roads in Magna Urbis's local frame: the segments of `roads` whose bounds meet the crop,
/// converted ([`regional_streets`]) and clipped to the crop less one cell.
#[must_use]
pub fn roads(grid: &Grid, roads: &RegionalRoads) -> Vec<Street> {
    let frame = grid.frame;
    let (west, east) = (frame.west_m, frame.west_m + frame.width_m);
    let (south, north) = (frame.north_m - frame.width_m, frame.north_m);
    let local = |pts: &[[f64; 2]]| pts.iter().map(|&[x, y]| frame.to_local(x, y)).collect::<Vec<_>>();
    let segments: Vec<RoadSegment> = roads
        .file
        .segments
        .iter()
        .filter(|s| {
            let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
            for &[x, y] in &s.points {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
            x1 >= west && x0 <= east && y1 >= south && y0 <= north
        })
        .map(|s| RoadSegment {
            class: s.class.clone(),
            width: s.width_m,
            path: local(&s.points),
            crossings: s.crossings.iter().map(|c| (c.kind.clone(), local(&c.points))).collect(),
        })
        .collect();
    #[allow(clippy::cast_precision_loss)] // grid side: a few thousand cells at most
    let margin = frame.width_m / frame.n as f64;
    regional_streets(&segments).iter().flat_map(|s| clip(s, margin, frame.width_m - margin)).collect()
}

/// Street count by role.
#[must_use]
pub fn by_role(streets: &[Street]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for s in streets {
        *out.entry(s.role.clone()).or_insert(0) += 1;
    }
    out
}
