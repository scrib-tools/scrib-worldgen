//! Slice PL-2: fit kit buildings into Magna Urbis's building footprints -> `placements.json`.
//!
//! The catalogue best-fit of rule-based town builders (`CityEngine`'s rule inserts, `cityforge`'s stamps): for each
//! footprint (`Lot::preview_parts`), in lot id order, every kit model of kind `building` is turned so its street door
//! faces the street, slid along the frontage with its door side on the footprint's front edge, and accepted when its
//! hull lies inside the footprint, overlaps no hull placed so far and the ground under it stays within the pad step.
//! The best covered share wins; candidates within `score_margin` of it are drawn by kit weight with a seeded draw
//! (recipe seed + lot id + footprint index), so one seed gives one town and the town is not one model.
//!
//! Frontage frame of a lot: `n` = outward normal of its main access segment (else its `front`, else its `axis` turned
//! clockwise), pointing away from the lot's centroid; `u` = `n` turned clockwise (along the street).
//!
//! Pad: z = site ground `door_probe_m` outside the street door (doorless: under the hull centroid); the model's
//! pivot sits at pad z - the model's `ground_z_m` (the ground vanilla placed it on; slice PL-2b), else at pad z - door
//! `sill_z_m` (doorless: pad z). Rejected when the ground under the hull drops below the mesh bottom (with
//! `ground_z_m`: pad z - `foundation_depth_m`, absent `ground_z_m - base_offset_m`; else pad z - (`sill_z_m` -
//! `base_offset_m`): the foundation would show) or rises above pad z + `pad_rise_max_m` (buried walls).
//!
//! Domain (`[fit] domain`): `footprint` (default) fits into each of Magna Urbis's footprints as above. `lot` fits one
//! building into the whole lot, so a house wider than Magna Urbis's preview footprint fills its plot to the street
//! (the medina's street wall, owner 2026-11-24): the lot polygon shrunk by `lot_setback_m` on every side but the
//! street side, and lots without a footprint but with a street access are tried too. A lot edge is on the street side
//! when its outward normal is nearer `n` than `u`, `-u` or `-n`; the hull keeps `lot_setback_m` from every other edge
//! (and from holes). The door side stands on the street side's edges: for each frontage position, the hull goes as far
//! forward as they allow (`setback_m` behind them), so a lot front a few degrees off the access normal still takes a
//! house flush with the street at its nearest corner. Everything else (door to street, slide, pad, overlap, draw) is
//! the footprint rule.
//!
//! `match` (slice C5b) maps each Magna Urbis building to one kit model instead of requiring a model inside it: Magna
//! Urbis decides where buildings stand, the fit only picks the kit model nearest each one. For each `plot` lot, each
//! footprint labelled `building` (other labels are counted and left alone) is the target: every model, turned with its
//! street door to the street as above, stands with its door side `setback_m` behind the footprint's front (its
//! furthest point along `n`), centred along `u` on the centre of the footprint's minimum-area oriented box, then slid
//! along `u` as above. A position is accepted when the hull lies inside the **lot** (`inside_tolerance_m`), overlaps
//! no hull placed so far and passes the pad step. The score is the intersection-over-union (area) of the hull and the
//! footprint; the seeded draw within `score_margin` of the best is the footprint rule's. One model per footprint; an
//! unmatched footprint stays empty with the furthest test passed (over every stage), like a lot.
//!
//! Fallback stages (slice C5e), run only for a footprint no model matches at its front, so a matched footprint keeps
//! its placement: an oblique street edge or a concave lot corner pushes a front corner of the hull out of the lot
//! while a position stands behind it (`docs/research/fit-match-diagnosis-2026-11-24.md`). (1) `back_slide`: the same
//! search with the hull also set back along `-n` in `slide_step_m` steps up to `back_slide_max_m` (each step tries
//! every `u` slide before the next). (2) `part_frame`: the footprint's minimum-area box axis nearest `n`, when within
//! `part_frame_max_deg` of it, replaces the street frame (`n`, `u`, front and anchor taken in it; the door faces
//! that axis), at the front and then set back as in (1). (3) `sink_back` (slice C5f, owner 2026-11-24: "sink the
//! back"), for footprints still empty after (1) and (2): the front, then (1) and (2) again, with the pad rule's rise
//! limit at `sink_back_max_m` in place of `pad_rise_max_m`, so on a slope the door stays at street level and the
//! ground covers the lower part of the back wall (the house is not raised: pad z stays at the door, the drop side is
//! unchanged). (4) `small` (owner 2026-11-24: "towers where no house fits"), for footprints still empty after (3): the
//! front, then (1) and (2) again with the candidates = kit models of a `small_kinds` kind (e.g. `tower`) in place of
//! the `building`s, and the rise limit of (3) when it is set; a model's first door faces the street as above (a doorless
//! one turns its local +y to it and takes the pad under its hull centroid). Same accept rules, `IoU` score and seeded
//! draw; absent tunables skip their stage. Placed with the `building` role, so the filler (`crate::filler`, run after
//! the fit and merge passes) counts their hulls as placed: its corner towers and walls stand clear of them.
//! `FitStats::parts_filled_by_stage` counts the stage that matched; `FitStats::sink_back_rise_m` the buried depth.
//!
//! Lots without a fit stay `empty` with the furthest test any candidate passed; `crate::merge` (PL-3a) then joins
//! narrow empty lots along their street and fits the joined domain with the same [`Pass`] (filler walls are PL-3b).
// Sample counts and indices are small.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// Planar geometry: a, b, p, n, u, t as in the formulas.
#![allow(clippy::many_single_char_names, clippy::similar_names)]
use crate::formats::{Door, Instance, KitFile, KitKind, KitModel, LotStatus, PLACEMENTS_FORMAT, PlacedLot, PlacementsFile, rotate, to_world};
use crate::lots::{Lot, Xy};
use crate::recipe::FitRecipe;
use magna_urbis_geom::{Point, Shape, area as geom_area, contains, intersection};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

/// Instance role of a fitted building.
pub const BUILDING_ROLE: &str = "building";

/// Magna Urbis's label of a building footprint (`Part::label`), the `match` domain's targets.
pub const BUILDING_LABEL: &str = "building";

/// Magna Urbis's lot use of a building plot (`Lot::use_kind`), the lots the `match` domain fills.
pub const PLOT_USE: &str = "plot";

/// The `match` domain's stages, in the order tried (module docs): at the footprint's front, then the fallbacks.
pub const MATCH_STAGES: [&str; 5] = ["front", "back_slide", "part_frame", "sink_back", "small"];

/// Why a lot with footprints got no building: the furthest test any candidate passed. Ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyReason {
    /// No candidate's hull fits inside any footprint of the lot.
    Inside,
    /// Some fit inside, every one overlaps a hull placed before.
    Overlap,
    /// Some fit inside without overlap, the ground under every one breaks the pad step.
    PadStep,
}

impl EmptyReason {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Inside => "inside",
            Self::Overlap => "overlap",
            Self::PadStep => "pad_step",
        }
    }
}

/// Counts for `stats.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct FitStats {
    pub lots: usize,
    /// Lots tried: with at least one footprint (`lot` domain: or a street access, `tried`).
    pub tried: usize,
    pub filled: usize,
    /// Lots not tried (without a footprint; `lot` domain: and without a street access; `empty` in the file).
    pub no_footprint: usize,
    pub empty_by_reason: BTreeMap<&'static str, usize>,
    /// The recipe's domain fits into whole lots (`domain = "lot"`).
    pub lot_domain: bool,
    /// `lot` domain only: lots without a footprint tried (street access) and filled.
    pub tried_without_footprint: usize,
    pub filled_without_footprint: usize,
    pub parts_tried: usize,
    pub parts_filled: usize,
    pub instances: usize,
    pub distinct_models: usize,
    /// Per lot role (`-` without one): `[tried, filled]`.
    pub by_role: BTreeMap<String, [usize; 2]>,
    /// The recipe's domain matches one model per Magna Urbis building (`domain = "match"`).
    pub match_domain: bool,
    /// `match` domain only: `building` footprints on `plot` lots (`parts_tried` = those on a lot with a street
    /// normal, `parts_filled` = matched).
    pub building_parts: usize,
    /// `match` domain only: footprints left alone, by label (`<label> on <use>` off `plot` lots).
    pub parts_skipped_by_label: BTreeMap<String, usize>,
    /// `match` domain only: unmatched building footprints by the furthest test passed.
    pub parts_empty_by_reason: BTreeMap<&'static str, usize>,
    /// `match` domain only: matched building footprints by the stage that matched them ([`MATCH_STAGES`]).
    pub parts_filled_by_stage: BTreeMap<&'static str, usize>,
    /// `match` domain only: intersection-over-union of the accepted matches, `[p10, p50, p90]`.
    pub match_iou: Option<[f64; 3]>,
    /// `match` domain only: the buried depth (m, ground rise above the pad under the hull) of the `sink_back`
    /// matches, `[p50, max]`.
    pub sink_back_rise_m: Option<[f64; 2]>,
    /// Kit models of kind `building`.
    pub candidates: usize,
    /// `match` domain only: kit models of a `small_kinds` kind (the `small` stage's candidates).
    pub small_candidates: usize,
    pub fit_ms: f64,
}

/// What fitting a culture's lots gives.
#[derive(Clone, Debug, PartialEq)]
pub struct Fitted {
    /// Every lot, in lot id order.
    pub lots: Vec<PlacedLot>,
    pub instances: Vec<Instance>,
    /// Lots with footprints left empty, and why.
    pub empty: BTreeMap<String, EmptyReason>,
    pub stats: FitStats,
}

/// `placements.json` header values.
pub struct Header {
    pub name: String,
    pub producer: String,
    pub recipe: String,
    pub seed: String,
    pub kit_blake3: String,
    pub origin_m: Xy,
}

impl Fitted {
    /// The `placements.json` body (streets, features and grade come with later slices).
    #[must_use]
    pub fn file(&self, header: Header) -> PlacementsFile {
        PlacementsFile {
            format: PLACEMENTS_FORMAT,
            name: header.name,
            producer: header.producer,
            recipe: header.recipe,
            seed: header.seed,
            kit_blake3: header.kit_blake3,
            origin_m: header.origin_m,
            lots: self.lots.clone(),
            instances: self.instances.clone(),
            streets: Vec::new(),
            features: Vec::new(),
            grade: None,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Planar helpers

pub(crate) fn dot(a: Xy, b: Xy) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

pub(crate) fn sub(a: Xy, b: Xy) -> Xy {
    [a[0] - b[0], a[1] - b[1]]
}

fn cross(o: Xy, a: Xy, b: Xy) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

pub(crate) fn unit(v: Xy) -> Option<Xy> {
    let l = v[0].hypot(v[1]);
    (l.is_finite() && l > 0.0).then(|| [v[0] / l, v[1] / l])
}

/// `v` turned clockwise by a quarter turn.
pub(crate) fn clockwise(v: Xy) -> Xy {
    [v[1], -v[0]]
}

/// Direction of a unit vector in degrees, `[0, 360)`, exact on the axes (so quarter-turn yaws stay exact).
#[allow(clippy::float_cmp)]
pub(crate) fn angle_deg(v: Xy) -> f64 {
    if v[1] == 0.0 {
        if v[0] >= 0.0 { 0.0 } else { 180.0 }
    } else if v[0] == 0.0 {
        if v[1] > 0.0 { 90.0 } else { 270.0 }
    } else {
        v[1].atan2(v[0]).to_degrees().rem_euclid(360.0)
    }
}

/// Signed area (m², CCW positive) of a ring.
pub(crate) fn signed_area(r: &[Xy]) -> f64 {
    (0..r.len()).map(|i| {
        let (a, b) = (r[i], r[(i + 1) % r.len()]);
        a[0] * b[1] - b[0] * a[1]
    }).sum::<f64>() / 2.0
}

/// Area centroid of a ring (vertex mean for a degenerate one).
pub(crate) fn centroid(r: &[Xy]) -> Xy {
    let a = signed_area(r);
    if a.abs() > 0.0 {
        let mut c = [0.0, 0.0];
        for i in 0..r.len() {
            let (p, q) = (r[i], r[(i + 1) % r.len()]);
            let k = p[0] * q[1] - q[0] * p[1];
            c[0] += (p[0] + q[0]) * k;
            c[1] += (p[1] + q[1]) * k;
        }
        [c[0] / (6.0 * a), c[1] / (6.0 * a)]
    } else {
        let n = r.len().max(1) as f64;
        [r.iter().map(|p| p[0]).sum::<f64>() / n, r.iter().map(|p| p[1]).sum::<f64>() / n]
    }
}

/// Convex hull, CCW, without duplicate or collinear points (Andrew's monotone chain).
pub(crate) fn convex_hull(points: &[Xy]) -> Vec<Xy> {
    fn chain(hull: &mut Vec<Xy>, q: Xy, start: usize) {
        while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], q) <= 0.0 {
            hull.pop();
        }
        hull.push(q);
    }
    let mut p: Vec<Xy> = points.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let mut hull: Vec<Xy> = Vec::with_capacity(p.len() * 2);
    for &q in &p {
        chain(&mut hull, q, 0);
    }
    // Upper chain: starts on the last lower point, which it pops at the end with the lower chain's closing point.
    let lower = hull.len();
    for &q in p.iter().rev().skip(1) {
        chain(&mut hull, q, lower - 1);
    }
    hull.pop();
    hull
}

/// Centre and one unit axis of the minimum-area oriented box of `points` (rotating calipers: one side of the box lies
/// on a hull edge; the axis is that edge's direction).
fn min_area_box(points: &[Xy]) -> Option<(Xy, Xy)> {
    let hull = convex_hull(points);
    let mut best: Option<(f64, Xy, Xy)> = None;
    for (a, b) in edges(&hull) {
        let Some(e) = unit(sub(b, a)) else { continue };
        let f = [-e[1], e[0]];
        let span = |v: Xy| hull.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &p| (lo.min(dot(v, p)), hi.max(dot(v, p))));
        let ((elo, ehi), (flo, fhi)) = (span(e), span(f));
        let area = (ehi - elo) * (fhi - flo);
        if best.is_none_or(|(a, _, _)| area < a) {
            let (ce, cf) = (f64::midpoint(elo, ehi), f64::midpoint(flo, fhi));
            best = Some((area, [ce * e[0] + cf * f[0], ce * e[1] + cf * f[1]], e));
        }
    }
    best.map(|b| (b.1, b.2))
}

/// The `match` fallback's part frame: the axis of `part`'s minimum-area box nearest street normal `n`, when it is
/// within `max_deg` of `n` and not `n` itself (that frame was tried already).
fn part_frame(part: &crate::lots::Part, n: Xy, max_deg: f64) -> Option<Xy> {
    let (_, e) = min_area_box(&part.polygon)?;
    let axis = [e, [-e[1], e[0]], [-e[0], -e[1]], [e[1], -e[0]]].into_iter().reduce(|a, b| if dot(b, n) > dot(a, n) { b } else { a })?;
    let cos = dot(axis, n);
    (cos < 1.0 && cos.clamp(-1.0, 1.0).acos().to_degrees() <= max_deg).then_some(axis)
}

fn segment_distance(p: Xy, a: Xy, b: Xy) -> f64 {
    let ab = sub(b, a);
    let len2 = dot(ab, ab);
    let t = if len2 > 0.0 { (dot(sub(p, a), ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let c = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    (p[0] - c[0]).hypot(p[1] - c[1])
}

fn rings_of(part: &crate::lots::Part) -> impl Iterator<Item = &Vec<Xy>> {
    std::iter::once(&part.polygon).chain(&part.holes)
}

fn edges(r: &[Xy]) -> impl Iterator<Item = (Xy, Xy)> + '_ {
    (0..r.len()).map(move |i| (r[i], r[(i + 1) % r.len()]))
}

/// A convex CCW hull's half-planes: outward unit normal `n` and offset `c` (inside: `n · x <= c`).
fn half_planes(hull: &[Xy]) -> Vec<(Xy, f64)> {
    edges(hull).filter_map(|(a, b)| unit(clockwise(sub(b, a))).map(|n| (n, dot(n, a)))).collect()
}

#[allow(clippy::float_cmp)] // a segment parallel to a plane
/// Does segment `p0 p1` reach into the convex region `n · x <= c - shrink` for every plane (Cyrus-Beck)?
fn segment_enters(planes: &[(Xy, f64)], shrink: f64, p0: Xy, p1: Xy) -> bool {
    let d = sub(p1, p0);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for &(n, c) in planes {
        let num = c - shrink - dot(n, p0);
        let den = dot(n, d);
        if den == 0.0 {
            if num < 0.0 {
                return false;
            }
        } else if den < 0.0 {
            t0 = t0.max(num / den);
        } else {
            t1 = t1.min(num / den);
        }
        if t0 >= t1 {
            return false;
        }
    }
    t0 < t1
}

/// Penetration depth (m) of two convex hulls (separating axis); 0 or less = apart or touching.
pub(crate) fn penetration(a: &[Xy], b: &[Xy]) -> f64 {
    let mut depth = f64::INFINITY;
    for hull in [a, b] {
        for (n, _) in half_planes(hull) {
            let span = |h: &[Xy]| h.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &p| (lo.min(dot(n, p)), hi.max(dot(n, p))));
            let ((alo, ahi), (blo, bhi)) = (span(a), span(b));
            depth = depth.min(ahi.min(bhi) - alo.max(blo));
            if depth <= 0.0 {
                return depth;
            }
        }
    }
    depth
}

pub(crate) type Bbox = [f64; 4];

pub(crate) fn bbox(h: &[Xy]) -> Bbox {
    h.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, p| {
        [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
    })
}

pub(crate) fn boxes_meet(a: Bbox, b: Bbox, pad: f64) -> bool {
    a[0] < b[2] - pad && b[0] < a[2] - pad && a[1] < b[3] - pad && b[1] < a[3] - pad
}

/// Uniform draw in `[0, 1)` from the recipe seed, lot id and footprint index.
fn draw(seed: &str, lot: &str, part: usize) -> f64 {
    let h = blake3::hash(format!("{seed}\u{0}{lot}\u{0}{part}").as_bytes());
    let mut b = [0_u8; 8];
    b.copy_from_slice(&h.as_bytes()[..8]);
    // 53 random bits: every value exactly representable.
    (u64::from_le_bytes(b) >> 11) as f64 / (1_u64 << 53) as f64
}

// ---------------------------------------------------------------------------------------------------------------
// Candidates

/// A kit building ready to fit: its convex hull and the doors that may face the street.
struct Candidate<'a> {
    model: &'a KitModel,
    hull: Vec<Xy>,
    area: f64,
    /// Street door per turn tried: the first door, then a back door if the model has one; `None` = doorless.
    street_doors: Vec<Option<&'a Door>>,
}

/// The kit models of the given kinds, ready to fit.
fn candidates<'a>(kit: &'a KitFile, recipe: &FitRecipe, kinds: &[KitKind]) -> Vec<Candidate<'a>> {
    kit.models
        .iter()
        .filter(|m| kinds.contains(&m.kind))
        .filter_map(|m| {
            let hull = convex_hull(&m.footprint);
            let area = signed_area(&hull);
            if hull.len() < 3 || area <= 0.0 {
                return None;
            }
            let street_doors = match m.doors.first() {
                None => vec![None],
                Some(first) => {
                    let tol = recipe.back_door_tolerance_deg.value;
                    let back = m.doors[1..].iter().find(|d| {
                        let off = (d.facing_deg - first.facing_deg - 180.0).rem_euclid(360.0);
                        off.min(360.0 - off) <= tol
                    });
                    std::iter::once(Some(first)).chain(back.map(Some)).collect()
                }
            };
            Some(Candidate { model: m, hull, area, street_doors })
        })
        .collect()
}

/// The lot's outward street normal (unit), see the module docs.
pub(crate) fn street_normal(lot: &Lot) -> Option<Xy> {
    let c = centroid(&lot.polygon);
    let from_segment = |[a, b]: [Xy; 2]| {
        let n = unit(clockwise(sub(b, a)))?;
        let mid = [f64::midpoint(a[0], b[0]), f64::midpoint(a[1], b[1])];
        Some(if dot(n, sub(mid, c)) >= 0.0 { n } else { [-n[0], -n[1]] })
    };
    lot.access
        .as_ref()
        .and_then(|a| from_segment(a.segment))
        .or_else(|| lot.front.and_then(from_segment))
        .or_else(|| unit(lot.axis).map(clockwise))
}

/// One accepted placement.
struct Placement {
    pos: Xy,
    yaw_deg: f64,
    z: f64,
    pad_z: f64,
    door: Option<Xy>,
    hull: Vec<Xy>,
}

/// The `lot` domain's extra rules (module docs).
struct LotRule {
    /// Street-side edges of the outer ring.
    street: Vec<(Xy, Xy)>,
    /// Every other edge, holes included: the hull keeps `setback` from them.
    sides: Vec<(Xy, Xy)>,
    setback: f64,
}

impl LotRule {
    fn new(part: &crate::lots::Part, n: Xy, setback: f64) -> Self {
        let u = clockwise(n);
        // Outward normal of a ring edge: clockwise of the edge on a CCW ring.
        let sign = if signed_area(&part.polygon) >= 0.0 { 1.0 } else { -1.0 };
        let (mut street, mut sides) = (Vec::new(), Vec::new());
        for (a, b) in edges(&part.polygon) {
            let o = clockwise(sub(b, a));
            let o = [o[0] * sign, o[1] * sign];
            if dot(o, n) > dot(o, u).abs() { street.push((a, b)) } else { sides.push((a, b)) }
        }
        sides.extend(part.holes.iter().flat_map(|h| edges(h)));
        Self { street, sides, setback }
    }

    /// The convex `hull` (inside the lot) keeps `setback` from every side edge.
    fn clear(&self, hull: &[Xy]) -> bool {
        let need = self.setback - 1e-9;
        let [x0, y0, x1, y1] = bbox(hull);
        self.sides.iter().all(|&(a, b)| {
            if a[0].min(b[0]) > x1 + need || a[0].max(b[0]) < x0 - need || a[1].min(b[1]) > y1 + need || a[1].max(b[1]) < y0 - need {
                return true;
            }
            hull.iter().all(|&p| segment_distance(p, a, b) >= need)
                && edges(hull).all(|(p, q)| segment_distance(a, p, q) >= need && segment_distance(b, p, q) >= need)
        })
    }
}

/// What a footprint and its frame give every candidate.
struct Target<'a> {
    rings: Vec<&'a Vec<Xy>>,
    shape: Shape,
    area: f64,
    n: Xy,
    u: Xy,
    n_max: f64,
    u_range: [f64; 2],
    perimeter: f64,
    lot: Option<LotRule>,
    /// `match` domain: the hull's centre along `u` (the matched footprint's box centre); `n_max` is then that
    /// footprint's front. `None`: centred in `u_range`.
    anchor_u: Option<f64>,
    /// Offsets (m) the hull is set back along `-n` from its depth, tried in order (outer loop over the `u` slides):
    /// `[0]`, or the `match` fallback's back-slide steps.
    back: Vec<f64>,
}

impl<'a> Target<'a> {
    fn new(part: &'a crate::lots::Part, n: Xy, lot_setback: Option<f64>) -> Self {
        let u = clockwise(n);
        let pt = |p: &Xy| Point { x: p[0], y: p[1] };
        let shape = Shape { outer: part.polygon.iter().map(pt).collect(), holes: part.holes.iter().map(|h| h.iter().map(pt).collect()).collect() };
        let area = signed_area(&part.polygon).abs() - part.holes.iter().map(|h| signed_area(h).abs()).sum::<f64>();
        let along = |v: Xy| part.polygon.iter().map(move |&p| dot(v, p));
        let perimeter = rings_of(part).flat_map(|r| edges(r)).map(|(a, b)| (b[0] - a[0]).hypot(b[1] - a[1])).sum();
        Self {
            perimeter,
            rings: rings_of(part).collect(),
            shape,
            area,
            n,
            u,
            n_max: along(n).fold(f64::NEG_INFINITY, f64::max),
            u_range: [along(u).fold(f64::INFINITY, f64::min), along(u).fold(f64::NEG_INFINITY, f64::max)],
            lot: lot_setback.map(|s| LotRule::new(part, n, s)),
            anchor_u: None,
            back: vec![0.0],
        }
    }

    /// The `match` domain's target: the domain (the lot) with its front and centre taken from footprint `part`
    /// (module docs). `None` for a degenerate footprint.
    fn anchored(mut self, part: &crate::lots::Part) -> Option<Self> {
        let (c, _) = min_area_box(&part.polygon)?;
        self.n_max = part.polygon.iter().map(|&p| dot(self.n, p)).fold(f64::NEG_INFINITY, f64::max);
        self.anchor_u = Some(dot(self.u, c));
        Some(self)
    }

    /// Pivot offset along `n` for a hull (turned, pivot-relative `local`) at frontage offset `s`: `default` (door side
    /// `setback` behind the domain's front), and in the `lot` domain no further forward than `setback` behind every
    /// street-side edge over the hull's span along `u`.
    fn depth(&self, local: &[Xy], s: f64, setback: f64, default: f64) -> f64 {
        let Some(rule) = &self.lot else { return default };
        let (u, n) = (self.u, self.n);
        // The hull in (u, n-offset from the pivot).
        let pts: Vec<Xy> = local.iter().map(|&p| [s + dot(u, p), dot(n, p)]).collect();
        // Highest n-offset of the hull at `at` along u, if the hull spans it.
        let top = |at: f64| {
            edges(&pts)
                .filter_map(|(p, q)| {
                    let (lo, hi) = (p[0].min(q[0]), p[0].max(q[0]));
                    if at < lo || at > hi {
                        None
                    } else if hi > lo {
                        Some(p[1] + (at - p[0]) / (q[0] - p[0]) * (q[1] - p[1]))
                    } else {
                        Some(p[1].max(q[1]))
                    }
                })
                .reduce(f64::max)
        };
        let mut d = default;
        for &(a, b) in &rule.street {
            let ([ua, na], [ub, nb]) = ([dot(u, a), dot(n, a)], [dot(u, b), dot(n, b)]);
            if (ub - ua).abs() > 0.0 {
                for p in &pts {
                    let t = (p[0] - ua) / (ub - ua);
                    if (0.0..=1.0).contains(&t) {
                        d = d.min(na + t * (nb - na) - setback - p[1]);
                    }
                }
            }
            for [eu, en] in [[ua, na], [ub, nb]] {
                if let Some(h) = top(eu) {
                    d = d.min(en - setback - h);
                }
            }
        }
        d
    }

    fn inside(&self, hull: &[Xy], tol: f64) -> bool {
        let near = |p: Xy| self.rings.iter().any(|r| edges(r).any(|(a, b)| segment_distance(p, a, b) <= tol));
        if !hull.iter().all(|&p| contains(&self.shape, Point { x: p[0], y: p[1] }) || near(p)) {
            return false;
        }
        let planes = half_planes(hull);
        !self.rings.iter().any(|r| edges(r).any(|(a, b)| segment_enters(&planes, tol, a, b)))
            && self.lot.as_ref().is_none_or(|l| l.clear(hull))
    }
}

/// Stage a candidate reached: `Err` = the furthest test it failed.
type Tried = Result<Placement, EmptyReason>;

/// The ground sample points under a convex CCW hull: its corners, then a grid at `ground_sample_m` inside it.
fn hull_samples<'h>(recipe: &FitRecipe, hull: &'h [Xy]) -> impl Iterator<Item = Xy> + 'h {
    let planes = half_planes(hull);
    let [x0, y0, x1, y1] = bbox(hull);
    let step = recipe.ground_sample_m.value;
    let (nx, ny) = (((x1 - x0) / step).floor() as usize, ((y1 - y0) / step).floor() as usize);
    let grid = (0..=ny)
        .flat_map(move |j| (0..=nx).map(move |i| [x0 + i as f64 * step, y0 + j as f64 * step]))
        .filter(move |&p| !planes.iter().any(|&(n, c)| dot(n, p) > c));
    hull.iter().copied().chain(grid)
}

/// Ground under a hull within `[low, high]` (hull corners and a grid at `ground_sample_m`).
fn pad_holds(recipe: &FitRecipe, ground: &dyn Fn(f64, f64) -> f64, hull: &[Xy], low: f64, high: f64) -> bool {
    hull_samples(recipe, hull).all(|p| {
        let z = ground(p[0], p[1]);
        z >= low && z <= high
    })
}

/// How far (m) the ground under a hull rises above `pad_z` at most (0 when it never does): the buried depth of the
/// back wall.
fn ground_rise(recipe: &FitRecipe, ground: &dyn Fn(f64, f64) -> f64, hull: &[Xy], pad_z: f64) -> f64 {
    hull_samples(recipe, hull).map(|p| ground(p[0], p[1]) - pad_z).fold(0.0, f64::max)
}

/// Heights of a placed piece (the pad rule, module docs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pad {
    /// Pivot height.
    pub z: f64,
    pub pad_z: f64,
}

/// The pad rule for one placed piece (buildings and the street wall's filler, `crate::filler`): pad z = site ground
/// at `probe` (`None`: under the hull centroid). With the model's `ground_z_m`, pivot z = pad z - `ground_z_m` and the
/// mesh bottom is pad z - `foundation_depth_m` (absent: `ground_z_m - base_offset_m`); without it, pivot z = pad z -
/// `sill` and the mesh bottom is pad z - (`sill` - `base_offset_m`). `None` when the ground under the convex CCW `hull`
/// drops below the mesh bottom or rises above pad z + `pad_rise_max_m`.
pub(crate) fn pad(recipe: &FitRecipe, ground: &dyn Fn(f64, f64) -> f64, hull: &[Xy], probe: Option<Xy>, sill: f64, model: &KitModel) -> Option<Pad> {
    pad_within(recipe, ground, hull, probe, sill, model, recipe.pad_rise_max_m.value)
}

/// [`pad`] with the rise limit `rise` (m) in place of `pad_rise_max_m` (the `sink_back` stage: `sink_back_max_m`).
fn pad_within(recipe: &FitRecipe, ground: &dyn Fn(f64, f64) -> f64, hull: &[Xy], probe: Option<Xy>, sill: f64, model: &KitModel, rise: f64) -> Option<Pad> {
    let probe = probe.unwrap_or_else(|| centroid(hull));
    let pad_z = ground(probe[0], probe[1]);
    let (seat, below) = match model.ground_z_m {
        Some(g) => (g, model.foundation_depth_m.unwrap_or(g - model.base_offset_m)),
        None => (sill, sill - model.base_offset_m),
    };
    pad_holds(recipe, ground, hull, pad_z - below, pad_z + rise).then_some(Pad { z: pad_z - seat, pad_z })
}

struct Fitter<'a> {
    recipe: &'a FitRecipe,
    ground: &'a dyn Fn(f64, f64) -> f64,
    placed: Vec<(Bbox, Vec<Xy>)>,
    /// The pad rule's rise limit (m) while the `sink_back` stage runs; `None` = `pad_rise_max_m`.
    rise: Option<f64>,
}

impl Fitter<'_> {
    fn overlaps(&self, hull: &[Xy]) -> bool {
        let b = bbox(hull);
        let tol = self.recipe.overlap_tolerance_m.value;
        self.placed.iter().any(|(pb, ph)| boxes_meet(b, *pb, tol) && penetration(hull, ph) > tol)
    }

    /// First accepted position of one candidate turned for one street door, or the furthest test passed.
    fn try_turn(&self, t: &Target, cand: &Candidate, door: Option<&Door>) -> Tried {
        let facing = door.map_or(90.0, |d| d.facing_deg);
        let mut yaw = (angle_deg(t.n) - facing).rem_euclid(360.0);
        if let Some(step) = cand.model.yaw_step_deg.filter(|s| *s > 0.0) {
            yaw = ((yaw / step).round() * step).rem_euclid(360.0);
        }
        let local: Vec<Xy> = cand.hull.iter().map(|&p| rotate(p, yaw)).collect();
        let span = |v: Xy| local.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &p| (lo.min(dot(v, p)), hi.max(dot(v, p))));
        let (h_ulo, h_uhi) = span(t.u);
        let (_, h_nhi) = span(t.n);
        let r = self.recipe;
        let tol = r.inside_tolerance_m.value;
        // Pivot offsets along u keeping the hull within the footprint's frontage extent (the lot's: minus its setback).
        let side = t.lot.as_ref().map_or(0.0, |l| l.setback);
        let (lo, hi) = (t.u_range[0] + side - h_ulo, t.u_range[1] - side - h_uhi);
        if hi < lo - 2.0 * tol {
            return Err(EmptyReason::Inside);
        }
        let depth = t.n_max - r.setback_m.value - h_nhi;
        // Centred in the frontage extent, or (`match`) on the anchor: then each side slides while it stays in it.
        let mid = t.anchor_u.map_or_else(|| f64::midpoint(lo, hi), |a| a - f64::midpoint(h_ulo, h_uhi));
        let step = r.slide_step_m.value;
        let mut slides = vec![mid];
        let mut k = 1.0;
        loop {
            let (plus, minus) = (mid + k * step, mid - k * step);
            let plus_ok = plus <= hi;
            let minus_ok = if t.anchor_u.is_some() { minus >= lo } else { plus_ok };
            if slides.len() >= r.slide_max.value || !(plus_ok || minus_ok) {
                break;
            }
            if plus_ok {
                slides.push(plus);
            }
            if minus_ok && slides.len() < r.slide_max.value {
                slides.push(minus);
            }
            k += 1.0;
        }
        let mut furthest = EmptyReason::Inside;
        for (&back, &s) in t.back.iter().flat_map(|b| slides.iter().map(move |s| (b, s))) {
            let depth = t.depth(&local, s, r.setback_m.value, depth) - back;
            let pos = [s * t.u[0] + depth * t.n[0], s * t.u[1] + depth * t.n[1]];
            let hull: Vec<Xy> = cand.hull.iter().map(|&p| to_world(p, pos, yaw)).collect();
            if !t.inside(&hull, tol) {
                continue;
            }
            furthest = furthest.max(EmptyReason::Overlap);
            if self.overlaps(&hull) {
                continue;
            }
            furthest = EmptyReason::PadStep;
            let world_door = door.map(|d| d.to_world(pos, yaw));
            let probe = world_door.map(|d| {
                let (s, c) = d.facing_deg.to_radians().sin_cos();
                [d.at[0] + c * r.door_probe_m.value, d.at[1] + s * r.door_probe_m.value]
            });
            let sill = door.map_or(0.0, |d| d.sill_z_m);
            let rise = self.rise.unwrap_or(r.pad_rise_max_m.value);
            let Some(Pad { z, pad_z }) = pad_within(r, self.ground, &hull, probe, sill, cand.model, rise) else { continue };
            return Ok(Placement { pos, yaw_deg: yaw, z, pad_z, door: world_door.map(|d| d.at), hull });
        }
        Err(furthest)
    }

    fn try_candidate(&self, t: &Target, cand: &Candidate) -> Tried {
        let mut furthest = EmptyReason::Inside;
        // A hull larger than the footprint (with its tolerance band) cannot fit.
        if cand.area > t.area + self.recipe.inside_tolerance_m.value * t.perimeter {
            return Err(furthest);
        }
        for &door in &cand.street_doors {
            match self.try_turn(t, cand, door) {
                Ok(p) => return Ok(p),
                Err(e) => furthest = furthest.max(e),
            }
        }
        Err(furthest)
    }
}

/// One building fitted into a domain.
pub(crate) struct Fit<'a> {
    pub model: &'a KitModel,
    pub pos: Xy,
    pub yaw_deg: f64,
    /// Pivot height.
    pub z: f64,
    pub pad_z: f64,
    pub door: Option<Xy>,
    /// The score it won with: covered share of the domain (`match`: intersection-over-union with the footprint).
    pub score: f64,
    /// How far (m) the ground under the hull rises above the pad at most: the buried depth of the back wall.
    pub rise_m: f64,
}

/// Intersection-over-union (area) of a convex hull of area `hull_area` and a shape of area `shape_area`.
fn iou(hull: &[Xy], hull_area: f64, shape: &Shape, shape_area: f64) -> f64 {
    let h = Shape { outer: hull.iter().map(|p| Point { x: p[0], y: p[1] }).collect(), holes: Vec::new() };
    let inter: f64 = intersection(&h, shape).iter().map(geom_area).sum();
    let union = hull_area + shape_area - inter;
    if union > 0.0 { (inter / union).clamp(0.0, 1.0) } else { 0.0 }
}

/// A fit pass: the kit's candidates and the hulls placed so far. The first pass ([`fit`]) and the merge pass
/// (`crate::merge`) share one, so a merged building never overlaps a building fitted before it.
pub(crate) struct Pass<'a> {
    cands: Vec<Candidate<'a>>,
    /// The `match` domain's `small` stage candidates (`small_kinds`; empty = no such stage).
    small: Vec<Candidate<'a>>,
    fitter: Fitter<'a>,
    seed: &'a str,
}

impl<'a> Pass<'a> {
    pub(crate) fn new(kit: &'a KitFile, recipe: &'a FitRecipe, seed: &'a str, ground: &'a dyn Fn(f64, f64) -> f64) -> Self {
        let small = match (recipe.match_domain(), &recipe.small_kinds) {
            (true, Some(kinds)) => candidates(kit, recipe, &kinds.value),
            _ => Vec::new(),
        };
        Self { cands: candidates(kit, recipe, &[KitKind::Building]), small, fitter: Fitter { recipe, ground, placed: Vec::new(), rise: None }, seed }
    }

    /// Best-fit one domain (a footprint, or a merged group's domain) in the frontage frame of street normal `n`. The
    /// weighted draw is keyed by `key` and `index`. An accepted building's hull is placed (later fits avoid it); `Err`
    /// = the furthest test any candidate passed.
    pub(crate) fn fit_domain(&mut self, domain: &crate::lots::Part, n: Xy, key: &str, index: usize) -> Result<Fit<'a>, EmptyReason> {
        let target = Target::new(domain, n, self.fitter.recipe.lot_setback());
        let area = target.area;
        self.best_fit(&target, &|cand, _| cand.area / area, key, index)
    }

    /// The `match` domain (module docs): one model for Magna Urbis building footprint `part` of `lot` (its polygon
    /// and holes as a part), scored by intersection-over-union with the footprint, at the footprint's front.
    pub(crate) fn fit_match(&mut self, lot: &crate::lots::Part, part: &crate::lots::Part, n: Xy, key: &str, index: usize) -> Result<Fit<'a>, EmptyReason> {
        self.match_stages(lot, part, n, key, index, false).map(|(f, _)| f)
    }

    /// The `match` domain's fallback stages for a footprint [`Self::fit_match`] left empty, in order: the back-slide
    /// (`back_slide_max_m`), then the footprint's own box frame (`part_frame_max_deg`). `Ok` carries the stage that
    /// matched ([`MATCH_STAGES`]).
    pub(crate) fn fit_match_fallback(
        &mut self,
        lot: &crate::lots::Part,
        part: &crate::lots::Part,
        n: Xy,
        key: &str,
        index: usize,
    ) -> Result<(Fit<'a>, &'static str), EmptyReason> {
        self.match_stages(lot, part, n, key, index, true)
    }

    fn match_stages(
        &mut self,
        lot: &crate::lots::Part,
        part: &crate::lots::Part,
        n: Xy,
        key: &str,
        index: usize,
        fallback: bool,
    ) -> Result<(Fit<'a>, &'static str), EmptyReason> {
        let Some(target) = Target::new(lot, n, None).anchored(part) else { return Err(EmptyReason::Inside) };
        let goal = Target::new(part, n, None);
        if goal.area <= 0.0 {
            return Err(EmptyReason::Inside);
        }
        let score = |cand: &Candidate, p: &Placement| iou(&p.hull, cand.area, &goal.shape, goal.area);
        if !fallback {
            return self.best_fit(&target, &score, key, index).map(|f| (f, MATCH_STAGES[0]));
        }
        let mut furthest = EmptyReason::Inside;
        let recipe = self.fitter.recipe;
        // Back-slide steps: `slide_step_m` apart, up to `back_slide_max_m` (none when absent).
        let step = recipe.slide_step_m.value;
        let max = recipe.back_slide_max_m.as_ref().map_or(0.0, |t| t.value);
        let steps: Vec<f64> = (1..=(max / step + 1e-9).floor() as usize).map(|k| k as f64 * step).collect();
        let mut stages: Vec<(&'static str, Target)> = Vec::new();
        if !steps.is_empty() {
            stages.push((MATCH_STAGES[1], Target { back: steps.clone(), ..target }));
        }
        let frame = recipe.part_frame_max_deg.as_ref().and_then(|t| part_frame(part, n, t.value));
        if let Some(t) = frame.and_then(|pn| Target::new(lot, pn, None).anchored(part)) {
            stages.push((MATCH_STAGES[2], Target { back: std::iter::once(0.0).chain(steps).collect(), ..t }));
        }
        for (stage, t) in &stages {
            match self.best_fit(t, &score, key, index) {
                Ok(f) => return Ok((f, *stage)),
                Err(e) => furthest = furthest.max(e),
            }
        }
        Err(furthest)
    }

    /// Every candidate's first accepted placement in `target`, scored; the seeded draw within `score_margin` of the
    /// best (keyed by `key` and `index`) is placed.
    fn best_fit(&mut self, target: &Target, score: &dyn Fn(&Candidate, &Placement) -> f64, key: &str, index: usize) -> Result<Fit<'a>, EmptyReason> {
        let recipe = self.fitter.recipe;
        let mut furthest = EmptyReason::Inside;
        if target.area <= 0.0 {
            return Err(furthest);
        }
        let mut fits: Vec<(usize, f64, Placement)> = Vec::new();
        for (ci, cand) in self.cands.iter().enumerate() {
            match self.fitter.try_candidate(target, cand) {
                Ok(p) => fits.push((ci, score(cand, &p), p)),
                Err(e) => furthest = furthest.max(e),
            }
        }
        let Some(best) = fits.iter().map(|f| f.1).reduce(f64::max) else { return Err(furthest) };
        let weight = |ci: usize| self.cands[ci].model.weight.max(0.0);
        let pool: Vec<&(usize, f64, Placement)> = fits.iter().filter(|f| f.1 >= best - recipe.score_margin.value).collect();
        let total: f64 = pool.iter().map(|f| weight(f.0)).sum();
        let pick = if total > 0.0 {
            let mut r = draw(self.seed, key, index) * total;
            pool.iter().find(|f| {
                r -= weight(f.0);
                r < 0.0
            }).or(pool.last()).copied()
        } else {
            pool.iter().find(|f| f.1 >= best).copied()
        };
        let Some((ci, score, p)) = pick else { return Err(furthest) };
        let model = self.cands[*ci].model;
        let rise_m = ground_rise(recipe, self.fitter.ground, &p.hull, p.pad_z);
        self.fitter.placed.push((bbox(&p.hull), p.hull.clone()));
        Ok(Fit { model, pos: p.pos, yaw_deg: p.yaw_deg, z: p.z, pad_z: p.pad_z, door: p.door, score: *score, rise_m })
    }

    /// Every domain of lot `key` fitted, in footprint order, with the stage that matched (`match` domain: `matched`
    /// = the whole lot; else `""`). The `match` fallbacks run only after every footprint of the lot was tried at its
    /// front, so a fallback hull never takes a front match's place.
    fn fit_lot(
        &mut self,
        key: &str,
        matched: Option<&crate::lots::Part>,
        domains: &[(usize, &crate::lots::Part)],
        n: Xy,
    ) -> Vec<Result<(Fit<'a>, &'static str), EmptyReason>> {
        let mut results: Vec<_> = domains
            .iter()
            .map(|&(pi, part)| match matched {
                Some(w) => self.fit_match(w, part, n, key, pi).map(|f| (f, MATCH_STAGES[0])),
                None => self.fit_domain(part, n, key, pi).map(|f| (f, "")),
            })
            .collect();
        if let Some(w) = matched {
            for (&(pi, part), r) in domains.iter().zip(&mut results) {
                if let Err(e) = *r {
                    *r = self.fit_match_fallback(w, part, n, key, pi).map_err(|f| f.max(e));
                }
            }
            // `sink_back`: every stage again with the rise limit at `sink_back_max_m`, front first for the whole lot.
            let sink = self.fitter.recipe.sink_back_max_m.as_ref().map(|t| t.value);
            if sink.is_some() {
                self.again(w, domains, n, key, &mut results, sink, MATCH_STAGES[3]);
            }
            // `small`: every stage again with the `small_kinds` models as the candidates (and the sink allowance).
            if !self.small.is_empty() {
                std::mem::swap(&mut self.cands, &mut self.small);
                self.again(w, domains, n, key, &mut results, sink, MATCH_STAGES[4]);
                std::mem::swap(&mut self.cands, &mut self.small);
            }
        }
        results
    }

    /// Every `match` stage again, front first for the whole lot, for the footprints of `results` still empty, with
    /// the pad rule's rise limit at `rise` (`None` = `pad_rise_max_m`); a match is recorded as `stage`.
    #[allow(clippy::too_many_arguments)]
    fn again(
        &mut self,
        lot: &crate::lots::Part,
        domains: &[(usize, &crate::lots::Part)],
        n: Xy,
        key: &str,
        results: &mut [Result<(Fit<'a>, &'static str), EmptyReason>],
        rise: Option<f64>,
        stage: &'static str,
    ) {
        self.fitter.rise = rise;
        for fallback in [false, true] {
            for (&(pi, part), r) in domains.iter().zip(results.iter_mut()) {
                if let Err(e) = *r {
                    *r = self.match_stages(lot, part, n, key, pi, fallback).map(|(f, _)| (f, stage)).map_err(|f| f.max(e));
                }
            }
        }
        self.fitter.rise = None;
    }

    pub(crate) fn candidates(&self) -> usize {
        self.cands.len()
    }

    pub(crate) fn small_candidates(&self) -> usize {
        self.small.len()
    }

    /// The recipe fits into whole lots (`domain = "lot"`).
    pub(crate) fn lot_domain(&self) -> bool {
        self.fitter.recipe.lot_setback().is_some()
    }
}

/// Whether the pass tries a lot: it has a footprint, or (`lot` domain) a street access.
pub(crate) fn tried(lot: &Lot, lot_domain: bool) -> bool {
    !lot.preview_parts.is_empty() || (lot_domain && lot.access.is_some())
}

impl Fit<'_> {
    /// The fitted building as instance `i<index>` on lot `lot`.
    pub(crate) fn instance(&self, index: usize, lot: &str) -> Instance {
        Instance {
            id: format!("i{index}"),
            kit: self.model.id.clone(),
            lot: Some(lot.to_string()),
            pos: [self.pos[0], self.pos[1], self.z],
            yaw_deg: self.yaw_deg,
            role: BUILDING_ROLE.into(),
            door: self.door,
        }
    }
}

/// Fit the kit's buildings into every lot's footprints. `ground(x, y)` = site height (m) at a world point.
#[must_use]
pub fn fit(lots: &[Lot], kit: &KitFile, recipe: &FitRecipe, seed: &str, ground: &dyn Fn(f64, f64) -> f64) -> Fitted {
    first_pass(lots, kit, recipe, seed, ground).0
}

/// Nearest-rank quantiles `ps` of `v` (sorted in place); `None` when it is empty.
fn quantiles<const N: usize>(v: &mut [f64], ps: [f64; N]) -> Option<[f64; N]> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(ps.map(|p| v[((p * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1]))
}

/// [`fit`], also handing back the pass (its placed hulls) for the merge pass.
pub(crate) fn first_pass<'a>(
    lots: &[Lot],
    kit: &'a KitFile,
    recipe: &'a FitRecipe,
    seed: &'a str,
    ground: &'a dyn Fn(f64, f64) -> f64,
) -> (Fitted, Pass<'a>) {
    let start = Instant::now();
    let mut pass = Pass::new(kit, recipe, seed, ground);
    let lot_domain = pass.lot_domain();
    let mut order: Vec<&Lot> = lots.iter().collect();
    order.sort_by(|a, b| a.id.cmp(&b.id));
    let mut out_lots = Vec::with_capacity(order.len());
    let mut instances: Vec<Instance> = Vec::new();
    let mut empty = BTreeMap::new();
    let match_domain = recipe.match_domain();
    let small_candidates = pass.small_candidates();
    let mut stats = FitStats { lots: lots.len(), candidates: pass.candidates(), small_candidates, lot_domain, match_domain, ..FitStats::default() };
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut ious: Vec<f64> = Vec::new();
    let mut sunk: Vec<f64> = Vec::new();
    for lot in order {
        let mut placed_lot = PlacedLot {
            id: lot.id.clone(),
            polygon: lot.polygon.clone(),
            status: LotStatus::Empty,
            role: lot.role.clone(),
            pad_z_m: None,
            front: lot.front,
            merged_from: Vec::new(),
        };
        let role = lot.role.clone().unwrap_or_else(|| "-".into());
        // The `match` domain's targets: the `building` footprints of a `plot` lot (the rest is counted and left alone).
        let matched: Vec<(usize, &crate::lots::Part)> = if match_domain {
            let plot = lot.use_kind == PLOT_USE;
            let (building, skipped): (Vec<_>, Vec<_>) = lot.preview_parts.iter().enumerate().partition(|(_, p)| plot && p.label == BUILDING_LABEL);
            for (_, p) in skipped {
                let key = if plot { p.label.clone() } else { format!("{} on {}", p.label, lot.use_kind) };
                *stats.parts_skipped_by_label.entry(key).or_default() += 1;
            }
            stats.building_parts += building.len();
            building
        } else {
            Vec::new()
        };
        if if match_domain { matched.is_empty() } else { !tried(lot, lot_domain) } {
            stats.no_footprint += 1;
            out_lots.push(placed_lot);
            continue;
        }
        let bare = lot.preview_parts.is_empty();
        stats.tried += 1;
        stats.tried_without_footprint += usize::from(bare);
        stats.by_role.entry(role.clone()).or_default()[0] += 1;
        let mut furthest = EmptyReason::Inside;
        // The `lot` domain: the whole lot, one domain; the `match` domain: the whole lot, one target per footprint.
        let whole = (lot_domain || match_domain).then(|| crate::lots::Part { label: "lot".into(), polygon: lot.polygon.clone(), holes: lot.holes.clone() });
        let domains: Vec<(usize, &crate::lots::Part)> = match &whole {
            Some(w) if lot_domain => vec![(0, w)],
            _ if match_domain => matched,
            _ => lot.preview_parts.iter().enumerate().collect(),
        };
        if let Some(n) = street_normal(lot) {
            let results = pass.fit_lot(&lot.id, whole.as_ref().filter(|_| match_domain), &domains, n);
            stats.parts_tried += results.len();
            for fitted in results {
                match fitted {
                    Ok((f, stage)) => {
                        used.insert(f.model.id.clone());
                        stats.parts_filled += 1;
                        if match_domain {
                            ious.push(f.score);
                            *stats.parts_filled_by_stage.entry(stage).or_default() += 1;
                            if stage == MATCH_STAGES[3] {
                                sunk.push(f.rise_m);
                            }
                        }
                        instances.push(f.instance(instances.len(), &lot.id));
                        placed_lot.status = LotStatus::Filled;
                        placed_lot.pad_z_m = placed_lot.pad_z_m.or(Some(f.pad_z));
                    }
                    Err(e) => {
                        furthest = furthest.max(e);
                        if match_domain {
                            *stats.parts_empty_by_reason.entry(e.name()).or_default() += 1;
                        }
                    }
                }
            }
        }
        if placed_lot.status == LotStatus::Filled {
            stats.filled += 1;
            stats.filled_without_footprint += usize::from(bare);
            stats.by_role.entry(role).or_default()[1] += 1;
        } else {
            *stats.empty_by_reason.entry(furthest.name()).or_default() += 1;
            empty.insert(lot.id.clone(), furthest);
        }
        out_lots.push(placed_lot);
    }
    stats.instances = instances.len();
    stats.distinct_models = used.len();
    stats.match_iou = quantiles(&mut ious, [0.1, 0.5, 0.9]);
    stats.sink_back_rise_m = quantiles(&mut sunk, [0.5, 1.0]);
    stats.fit_ms = start.elapsed().as_secs_f64() * 1000.0;
    (Fitted { lots: out_lots, instances, empty, stats }, pass)
}
