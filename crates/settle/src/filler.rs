//! Slice PL-3b: close the street wall (the medina's continuous street front, owner 2026-11-24). After fitting and
//! merging (`crate::merge`), kit walls fill every stretch of lot face no building covers, and arches span the lane
//! mouths between frontage runs on one street line.
//!
//! The filler step of rule-based town builders (`CityEngine`'s street-front rules and Magna Urbis's own garden walls in
//! `urban/parcels`): a modular wall kit laid end to end along the building line, whole pieces only, a gate piece where a
//! lane leaves the street.
//!
//! 1. **Frontage runs**: every lot with a frontage, chained along its street exactly as the merge pass chains its
//!    candidates (`crate::merge` frontage neighbours, `[merge]` tunables; `max_offset_m` is the street-face reach). A
//!    run is a 1-D line: its members' street faces end to end in walk order, each face in its own frontage frame.
//! 2. **Covered**: a building hull covers the part of a face its projection on the face line spans, when it reaches
//!    within `cover_depth_m` behind the line and not wholly into the street. The rest of the run, in stretches of at
//!    least `min_gap_m`, is a gap.
//! 3. **Walls**: each gap gets the combination of `walls` pieces whose span covers it best: a span may overhang the gap
//!    by up to `overlap_max_m` at either end (into the building, or past the run's end) and pieces may overlap each
//!    other by up to `overlap_max_m`; ties within `length_tie_m` go to the pieces listed first, then to fewer pieces.
//!    The pieces are laid centred on the gap, end to end with equal overlaps, each on the face of the member under its
//!    centre: local x along the face, the outer face (local +y extreme) on the face line, local +y to the street.
//! 4. **Arches**: two run ends facing each other across a gap along one street line (normals within `max_angle_deg`,
//!    offset within `max_offset_m`, each the other's nearest) are a lane mouth; one whose width lies within the arches'
//!    widths widened by `arch_slack_m` gets the arch of nearest width, centred, on the mean face line. Wider mouths are
//!    streets and stay open.
//! 5. **Towers** (only with `towers`), where the wall line ends or turns (Balmora's own walls; Magna Urbis's
//!    `urban/types.rs` `Enclosure.towers`, gate towers first, then corners): at every run end that is no lane mouth
//!    within the arches' widths (an open street corner; decided by width, before the arch is placed), then at every
//!    vertex where two members' faces turn more than `tower_turn_deg`. A tower is laid in the wall frame, the middle
//!    of its outer face on the corner point and local +y along the bisector of the faces' normals; at a run end, the
//!    listed towers whose footprint centroid leans out past the end (their wall stub along the run) are tried first,
//!    the first that stands wins. Towers stand before the walls: a tower hull covers the faces as a building does, so
//!    the walls stop against it (overhanging it by up to `overlap_max_m`). A tower whose corner point lies within
//!    `tower_min_spacing_m` of a placed tower's is left out (`tower_spacing`).
//!
//! Every piece takes its height from the pad rule of doorless buildings (`crate::fit::pad`) and is left out, never cut,
//! when the ground under it breaks the pad step (`pad_step`) or it interpenetrates a placed hull (buildings, filler of
//! earlier gaps) deeper than `overlap_max_m` (`overlap`). A wall instance belongs to the (placed) lot under its centre,
//! which takes status `filler` if it was `empty`; an arch or a tower belongs to no lot.
// Piece counts are small.
#![allow(clippy::cast_precision_loss)]
// Planar geometry: a, b, n, u, s, t, w as in the formulas.
#![allow(clippy::many_single_char_names, clippy::similar_names)]
use crate::fit::{
    BUILDING_ROLE, Bbox, Fitted, Pad, angle_deg, bbox, centroid, clockwise, convex_hull, dot, pad, penetration, sub, unit,
};
use crate::formats::{Instance, KitFile, KitModel, LotStatus, rotate};
use crate::lots::{Lot, Xy};
use crate::merge::{Frontage, MergeStats, face, fit_and_merge, frontage, runs};
use crate::recipe::{FillerRecipe, FitRecipe, MergeRecipe};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

/// Instance role of a street-wall piece.
pub const WALL_ROLE: &str = "wall";
/// Instance role of a lane-mouth arch.
pub const ARCH_ROLE: &str = "arch";
/// Instance role of a street-wall tower.
pub const TOWER_ROLE: &str = "tower";

/// Counts for `stats.json` `filler` (lengths in m).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct FillerStats {
    pub runs: usize,
    /// Lot street faces, summed over the runs.
    pub faces_m: f64,
    /// Faces plus the lane mouths that got an arch: the street wall's length.
    pub frontage_m: f64,
    /// Face length covered by building hulls.
    pub buildings_m: f64,
    /// Face length covered by placed walls (beyond the buildings and towers).
    pub walls_m: f64,
    /// Face length covered by placed towers (beyond the buildings).
    pub towers_m: f64,
    /// Lane-mouth width spanned by placed arches.
    pub arches_m: f64,
    /// Face length neither building, tower nor wall covers.
    pub open_m: f64,
    /// Buildings / faces: the street wall before filling.
    pub coverage_before: f64,
    /// (Buildings + towers + walls + arches) / frontage.
    pub coverage: f64,
    pub gaps: usize,
    pub walls: usize,
    pub arches: usize,
    pub towers: usize,
    /// Mutual facing run ends (lane mouths and street crossings).
    pub mouths: usize,
    /// Lots that went from `empty` to `filler`.
    pub lots_filler: usize,
    /// Pieces left out: `wall_pad_step`, `wall_overlap`, `arch_pad_step`, `arch_overlap`; gaps no piece fits
    /// (`gap_too_short`); mouths outside the arches' widths (`mouth_too_narrow`, `mouth_too_wide`); tower spots
    /// (`tower_spacing`, `tower_pad_step`, `tower_overlap`).
    pub skipped_by_reason: BTreeMap<&'static str, usize>,
    pub filler_ms: f64,
}

/// A wall, arch or tower model: its length along local x, the centre of that span, its outer face (local +y extreme)
/// and how far its footprint's centroid lies from that centre along local x.
struct Piece<'a> {
    model: &'a KitModel,
    len: f64,
    x_mid: f64,
    y_out: f64,
    lean: f64,
}

fn piece<'a>(kit: &'a KitFile, id: &str) -> Result<Piece<'a>, String> {
    let model = kit.model(id).ok_or_else(|| format!("filler: model `{id}` is not in the kit"))?;
    let (mut x0, mut x1, mut y1) = (f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in &model.footprint {
        (x0, x1, y1) = (x0.min(p[0]), x1.max(p[0]), y1.max(p[1]));
    }
    let len = x1 - x0;
    if !(len.is_finite() && len > 0.0 && y1.is_finite()) {
        return Err(format!("filler: model `{id}` has no length along local x"));
    }
    let x_mid = f64::midpoint(x0, x1);
    let hull = convex_hull(&model.footprint);
    let lean = if hull.len() >= 3 { centroid(&hull)[0] - x_mid } else { 0.0 };
    Ok(Piece { model, len, x_mid, y_out: y1, lean })
}

fn pieces<'a>(kit: &'a KitFile, ids: &[String]) -> Result<Vec<Piece<'a>>, String> {
    ids.iter().map(|id| piece(kit, id)).collect()
}

/// One lot face of a run, in its frontage frame: `t` along `u`, the face line `n · p = c`, face `[lo, hi]` on `u`,
/// walked along `sign * u` from run position `offset`.
struct Member {
    lot: usize,
    n: Xy,
    u: Xy,
    c: f64,
    lo: f64,
    hi: f64,
    sign: f64,
    offset: f64,
}

impl Member {
    fn len(&self) -> f64 {
        self.hi - self.lo
    }

    /// `t` of walk distance `w` into the face.
    fn t(&self, w: f64) -> f64 {
        if self.sign > 0.0 { self.lo + w } else { self.hi - w }
    }

    fn w(&self, t: f64) -> f64 {
        if self.sign > 0.0 { t - self.lo } else { self.hi - t }
    }

    fn at(&self, t: f64) -> Xy {
        [self.u[0] * t + self.n[0] * self.c, self.u[1] * t + self.n[1] * self.c]
    }
}

fn members(run: &[usize], order: &[&Lot], fronts: &[Option<Frontage>], reach: f64) -> Vec<Member> {
    let mut out: Vec<Member> = run
        .iter()
        .filter_map(|&i| {
            let f = fronts[i].as_ref()?;
            let u = clockwise(f.n);
            let [lo, hi] = face(order[i], f, reach, u);
            (lo < hi).then(|| Member { lot: i, n: f.n, u, c: dot(f.n, f.seg[0]), lo, hi, sign: 1.0, offset: 0.0 })
        })
        .collect();
    let mid = |m: &Member| m.at(f64::midpoint(m.lo, m.hi));
    let centres: Vec<Xy> = out.iter().map(mid).collect();
    let k = out.len();
    let mut offset = 0.0;
    for (j, m) in out.iter_mut().enumerate() {
        let own = f64::midpoint(m.lo, m.hi);
        m.sign = if k < 2 {
            1.0
        } else if j + 1 < k {
            if dot(m.u, centres[j + 1]) >= own { 1.0 } else { -1.0 }
        } else if dot(m.u, centres[j - 1]) <= own {
            1.0
        } else {
            -1.0
        };
        m.offset = offset;
        offset += m.len();
    }
    out
}

/// Sorted disjoint union of intervals.
fn union(mut v: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    v.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let mut out: Vec<[f64; 2]> = Vec::new();
    for iv in v {
        match out.last_mut() {
            Some(last) if iv[0] <= last[1] => last[1] = last[1].max(iv[1]),
            _ => out.push(iv),
        }
    }
    out
}

fn measure(v: &[[f64; 2]]) -> f64 {
    v.iter().map(|iv| iv[1] - iv[0]).sum()
}

/// The parts of a run (`[0, total]`) the building hulls cover.
fn covered(ms: &[Member], hulls: &[(Bbox, Vec<Xy>)], depth: f64) -> Vec<[f64; 2]> {
    let mut v = Vec::new();
    for m in ms {
        let mut b = bbox(&[m.at(m.lo), m.at(m.hi)]);
        b = [b[0] - depth, b[1] - depth, b[2] + depth, b[3] + depth];
        for (hb, h) in hulls {
            if hb[0] > b[2] || b[0] > hb[2] || hb[1] > b[3] || b[1] > hb[3] {
                continue;
            }
            let span = |a: Xy| h.iter().fold([f64::INFINITY, f64::NEG_INFINITY], |[lo, hi], p| [lo.min(dot(a, *p)), hi.max(dot(a, *p))]);
            let ([nlo, nhi], [ulo, uhi]) = (span(m.n), span(m.u));
            if nhi < m.c - depth || nlo > m.c {
                continue;
            }
            let (t0, t1) = (ulo.max(m.lo), uhi.min(m.hi));
            if t0 < t1 {
                let (w0, w1) = (m.w(t0), m.w(t1));
                v.push([m.offset + w0.min(w1), m.offset + w0.max(w1)]);
            }
        }
    }
    union(v)
}

/// Best wall combination for a gap of length `g` (module docs): piece indices in laying order and the span laid.
fn combination(lens: &[f64], g: f64, o: f64, tie: f64) -> Option<(Vec<usize>, f64)> {
    struct Best {
        counts: Vec<usize>,
        cover: f64,
        total: f64,
        k: usize,
        span: f64,
    }
    fn better(a: &Best, b: &Best, tie: f64) -> bool {
        if (a.cover - b.cover).abs() > tie {
            return a.cover > b.cover;
        }
        if (a.total - b.total).abs() > tie {
            return a.total < b.total;
        }
        if a.counts != b.counts {
            return a.counts > b.counts;
        }
        a.k < b.k
    }
    fn walk(lens: &[f64], g: f64, o: f64, tie: f64, counts: &mut Vec<usize>, best: &mut Option<Best>) {
        let i = counts.len();
        let k: usize = counts.iter().sum();
        // Pieces are longer than `o`, so the shortest span grows with every piece.
        let total: f64 = counts.iter().zip(lens).map(|(&n, &l)| n as f64 * l).sum();
        if k > 0 && total - (k - 1) as f64 * o > g + 2.0 * o {
            return;
        }
        if i == lens.len() {
            if k == 0 {
                return;
            }
            let short = total - (k - 1) as f64 * o;
            let span = if total >= g { short.max(g) } else { total };
            let cand = Best { counts: counts.clone(), cover: span.min(g), total, k, span };
            if best.as_ref().is_none_or(|b| better(&cand, b, tie)) {
                *best = Some(cand);
            }
            return;
        }
        for n in 0.. {
            let t = total + n as f64 * lens[i];
            let kk = k + n;
            if n > 0 && t - (kk - 1) as f64 * o > g + 2.0 * o {
                break;
            }
            counts.push(n);
            walk(lens, g, o, tie, counts, best);
            counts.pop();
        }
    }
    let mut best = None;
    walk(lens, g, o, tie, &mut Vec::new(), &mut best);
    best.map(|b| (b.counts.iter().enumerate().flat_map(|(i, &n)| std::iter::repeat_n(i, n)).collect(), b.span))
}

/// Pivot and yaw of a piece whose outer face's middle sits at `at`, its local +y along `n`.
fn place(p: &Piece, at: Xy, n: Xy) -> (Xy, f64) {
    let mut yaw = (angle_deg(n) - 90.0).rem_euclid(360.0);
    if let Some(step) = p.model.yaw_step_deg.filter(|s| *s > 0.0) {
        yaw = ((yaw / step).round() * step).rem_euclid(360.0);
    }
    let r = rotate([p.x_mid, p.y_out], yaw);
    ([at[0] - r[0], at[1] - r[1]], yaw)
}

struct Placer<'a> {
    fit: &'a FitRecipe,
    ground: &'a dyn Fn(f64, f64) -> f64,
    overlap: f64,
    placed: Vec<(Bbox, Vec<Xy>)>,
}

impl Placer<'_> {
    /// The piece's hull and heights if it may stand there, else the reason (`overlap` / `pad_step`).
    fn check(&self, p: &Piece, pos: Xy, yaw: f64) -> Result<(Vec<Xy>, Pad), &'static str> {
        let hull = convex_hull(&p.model.world_hull(pos, yaw));
        let b = bbox(&hull);
        let o = self.overlap;
        let hits = |(pb, ph): &(Bbox, Vec<Xy>)| b[0] < pb[2] && pb[0] < b[2] && b[1] < pb[3] && pb[1] < b[3] && penetration(&hull, ph) > o;
        if self.placed.iter().any(hits) {
            return Err("overlap");
        }
        let heights = pad(self.fit, self.ground, &hull, None, 0.0, p.model).ok_or("pad_step")?;
        Ok((hull, heights))
    }
}

/// One end of a run: its point on the face line, normal, and the outward direction along the street.
struct End {
    run: usize,
    at: Xy,
    n: Xy,
    out: Xy,
}

/// A member's walk direction.
fn dir(m: &Member) -> Xy {
    [m.u[0] * m.sign, m.u[1] * m.sign]
}

fn ends(ms: &[Member], run: usize) -> [End; 2] {
    let (first, last) = (&ms[0], &ms[ms.len() - 1]);
    let d0 = dir(first);
    [
        End { run, at: first.at(first.t(0.0)), n: first.n, out: [-d0[0], -d0[1]] },
        End { run, at: last.at(last.t(last.len())), n: last.n, out: dir(last) },
    ]
}

/// Gap along the street from `a` to `b` if `b` faces `a` across it on one street line.
fn across(a: &End, b: &End, m: &MergeRecipe) -> Option<f64> {
    if a.run == b.run || dot(a.n, b.n) < m.max_angle_deg.value.to_radians().cos() {
        return None;
    }
    let d = sub(b.at, a.at);
    if dot(a.n, d).abs() > m.max_offset_m.value {
        return None;
    }
    let (g, back) = (dot(a.out, d), -dot(b.out, d));
    (g > 0.0 && back > 0.0).then_some(g)
}

fn skip(stats: &mut FillerStats, why: &'static str) {
    *stats.skipped_by_reason.entry(why).or_default() += 1;
}

/// The arch of nearest width for a lane mouth `g` wide, else why the mouth stays open.
fn arch_for<'p, 'a>(arches: &'p [Piece<'a>], g: f64, slack: f64) -> Result<&'p Piece<'a>, &'static str> {
    let (wmin, wmax) = arches.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.len), hi.max(p.len)));
    if arches.is_empty() || g > wmax + slack {
        return Err("mouth_too_wide");
    }
    if g < wmin - slack {
        return Err("mouth_too_narrow");
    }
    arches.iter().min_by(|x, y| (x.len - g).abs().total_cmp(&(y.len - g).abs())).ok_or("mouth_too_wide")
}

/// Where a tower may stand: the corner point on the face line, the normal it faces, at a run end the outward
/// direction along the street.
struct Spot {
    at: Xy,
    n: Xy,
    out: Option<Xy>,
}

/// The vertices of a run where its faces turn more than `turn_deg`.
fn corners(ms: &[Member], turn_deg: f64, spots: &mut Vec<Spot>) {
    let cos = turn_deg.to_radians().cos();
    for p in ms.windows(2) {
        let (a, b) = (&p[0], &p[1]);
        if dot(dir(a), dir(b)) < cos {
            let (pa, pb) = (a.at(a.t(a.len())), b.at(b.t(0.0)));
            let n = unit([a.n[0] + b.n[0], a.n[1] + b.n[1]]).unwrap_or(a.n);
            spots.push(Spot { at: [f64::midpoint(pa[0], pb[0]), f64::midpoint(pa[1], pb[1])], n, out: None });
        }
    }
}

/// Raise a tower on every spot that takes one (module docs); returns their hulls.
fn raise_towers(
    towers: &[Piece],
    spots: &[Spot],
    spacing: f64,
    placer: &mut Placer,
    stats: &mut FillerStats,
    added: &mut Vec<Instance>,
) -> Vec<(Bbox, Vec<Xy>)> {
    let mut stood: Vec<Xy> = Vec::new();
    let mut hulls = Vec::new();
    for s in spots {
        if stood.iter().any(|q| dot(sub(*q, s.at), sub(*q, s.at)) < spacing * spacing) {
            skip(stats, "tower_spacing");
            continue;
        }
        let mut order: Vec<&Piece> = towers.iter().collect();
        if let Some(out) = s.out {
            // Stable: towers leaning out past the end first, list order otherwise.
            order.sort_by_key(|p| {
                let yaw = place(p, s.at, s.n).1;
                p.lean * dot(rotate([1.0, 0.0], yaw), out) <= 0.0
            });
        }
        let mut why = "overlap";
        let mut stands = false;
        for p in order {
            let (pos, yaw) = place(p, s.at, s.n);
            match placer.check(p, pos, yaw) {
                Ok((hull, h)) => {
                    added.push(Instance {
                        id: String::new(),
                        kit: p.model.id.clone(),
                        lot: None,
                        pos: [pos[0], pos[1], h.z],
                        yaw_deg: yaw,
                        role: TOWER_ROLE.into(),
                        door: None,
                    });
                    placer.placed.push((bbox(&hull), hull.clone()));
                    hulls.push((bbox(&hull), hull));
                    stood.push(s.at);
                    stats.towers += 1;
                    stands = true;
                    break;
                }
                Err(e) => why = e,
            }
        }
        if !stands {
            skip(stats, if why == "overlap" { "tower_overlap" } else { "tower_pad_step" });
        }
    }
    hulls
}

/// Close the street wall of a fitted (and merged) culture (module docs). `lots` = the lots fitted.
///
/// # Errors
/// A recipe model id not in the kit, or a model without length; a wall piece not longer than `overlap_max_m`.
#[allow(clippy::too_many_lines)] // one pass: runs, towers, walls, arches, lot status, counts
pub fn fill(
    lots: &[Lot],
    kit: &KitFile,
    fit: &FitRecipe,
    merge: &MergeRecipe,
    r: &FillerRecipe,
    mut fitted: Fitted,
    ground: &dyn Fn(f64, f64) -> f64,
) -> Result<(Fitted, FillerStats), String> {
    let start = Instant::now();
    let o = r.overlap_max_m.value;
    let walls = pieces(kit, &r.walls.value)?;
    if let Some(w) = walls.iter().find(|w| w.len <= o) {
        return Err(format!("filler: wall `{}` is not longer than overlap_max_m", w.model.id));
    }
    let arches = pieces(kit, &r.arches.value)?;
    let towers = match &r.towers {
        Some(t) => pieces(kit, &t.value)?,
        None => Vec::new(),
    };
    let lens: Vec<f64> = walls.iter().map(|w| w.len).collect();
    let mut order: Vec<&Lot> = lots.iter().collect();
    order.sort_by(|a, b| a.id.cmp(&b.id));
    let fronts: Vec<Option<Frontage>> = order.iter().map(|l| frontage(l)).collect();
    let all: Vec<usize> = (0..order.len()).collect();
    let (runs, _) = runs(merge, &order, &fronts, &all);
    // Placed lot of each lot id (a merged member -> its group).
    let mut owner: BTreeMap<&str, &str> = BTreeMap::new();
    for pl in &fitted.lots {
        owner.insert(&pl.id, &pl.id);
        for m in &pl.merged_from {
            owner.insert(m, &pl.id);
        }
    }
    let buildings: Vec<(Bbox, Vec<Xy>)> = fitted
        .instances
        .iter()
        .filter(|i| i.role == BUILDING_ROLE)
        .filter_map(|i| kit.model(&i.kit).map(|m| convex_hull(&m.world_hull([i.pos[0], i.pos[1]], i.yaw_deg))))
        .map(|h| (bbox(&h), h))
        .collect();
    let mut placer = Placer { fit, ground, overlap: o, placed: buildings.clone() };
    let mut stats = FillerStats::default();
    let mut added: Vec<Instance> = Vec::new();
    let mut walled: BTreeSet<String> = BTreeSet::new();
    let lines: Vec<Vec<Member>> =
        runs.iter().map(|run| members(run, &order, &fronts, merge.max_offset_m.value)).filter(|ms| !ms.is_empty()).collect();
    let run_ends: Vec<End> = lines.iter().enumerate().flat_map(|(i, ms)| ends(ms, i)).collect();
    // Lane mouths: mutual nearest facing run ends.
    let nearest: Vec<Option<(f64, usize)>> = run_ends
        .iter()
        .map(|a| {
            run_ends.iter().enumerate().filter_map(|(j, b)| across(a, b, merge).map(|g| (g, j))).min_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)))
        })
        .collect();
    let mouths: Vec<(usize, usize, f64)> = nearest
        .iter()
        .enumerate()
        .filter_map(|(i, near)| {
            let (g, j) = (*near)?;
            (j > i && nearest[j].map(|n| n.1) == Some(i)).then_some((i, j, g))
        })
        .collect();
    let slack = r.arch_slack_m.value;
    let mut blockers = buildings.clone();
    if !towers.is_empty() {
        let arched: BTreeSet<usize> =
            mouths.iter().filter(|m| arch_for(&arches, m.2, slack).is_ok()).flat_map(|&(i, j, _)| [i, j]).collect();
        let mut spots: Vec<Spot> = run_ends
            .iter()
            .enumerate()
            .filter(|(i, _)| !arched.contains(i))
            .map(|(_, e)| Spot { at: e.at, n: e.n, out: Some(e.out) })
            .collect();
        let turn = r.tower_turn_deg.as_ref().map_or(180.0, |t| t.value);
        for ms in &lines {
            corners(ms, turn, &mut spots);
        }
        let spacing = r.tower_min_spacing_m.as_ref().map_or(0.0, |t| t.value);
        blockers.extend(raise_towers(&towers, &spots, spacing, &mut placer, &mut stats, &mut added));
    }
    let depth = r.cover_depth_m.value;
    for ms in &lines {
        let last = &ms[ms.len() - 1];
        let total = last.offset + last.len();
        stats.runs += 1;
        stats.faces_m += total;
        let mut cover = covered(ms, &buildings, depth);
        let built = measure(&cover);
        stats.buildings_m += built;
        if blockers.len() > buildings.len() {
            cover = covered(ms, &blockers, depth);
            stats.towers_m += measure(&cover) - built;
        }
        let mut gaps = Vec::new();
        let mut at = 0.0;
        for iv in cover.iter().chain(std::iter::once(&[total, total])) {
            if iv[0] - at >= r.min_gap_m.value {
                gaps.push([at, iv[0]]);
            }
            at = at.max(iv[1]);
        }
        for [a, b] in gaps {
            stats.gaps += 1;
            let Some((pieces, span)) = combination(&lens, b - a, o, r.length_tie_m.value) else {
                skip(&mut stats, "gap_too_short");
                continue;
            };
            let k = pieces.len();
            let total_len: f64 = pieces.iter().map(|&i| lens[i]).sum();
            let joint = if k > 1 { ((total_len - span) / (k - 1) as f64).max(0.0) } else { 0.0 };
            let mut s0 = f64::midpoint(a, b) - span / 2.0;
            let mut laid = Vec::new();
            let mut fresh = Vec::new();
            for &pi in &pieces {
                let p = &walls[pi];
                let sc = s0 + p.len / 2.0;
                let m = ms.iter().rev().find(|m| m.offset <= sc).unwrap_or(&ms[0]);
                let (pos, yaw) = place(p, m.at(m.t(sc - m.offset)), m.n);
                match placer.check(p, pos, yaw) {
                    Ok((hull, h)) => {
                        let lot = owner.get(order[m.lot].id.as_str()).map_or_else(|| order[m.lot].id.clone(), |s| (*s).to_string());
                        walled.insert(lot.clone());
                        added.push(Instance {
                            id: String::new(),
                            kit: p.model.id.clone(),
                            lot: Some(lot),
                            pos: [pos[0], pos[1], h.z],
                            yaw_deg: yaw,
                            role: WALL_ROLE.into(),
                            door: None,
                        });
                        laid.push([s0.max(a), (s0 + p.len).min(b)]);
                        fresh.push((bbox(&hull), hull));
                        stats.walls += 1;
                    }
                    Err(why) => skip(&mut stats, if why == "overlap" { "wall_overlap" } else { "wall_pad_step" }),
                }
                s0 += p.len - joint;
            }
            stats.walls_m += measure(&union(laid.into_iter().filter(|iv| iv[0] < iv[1]).collect()));
            placer.placed.extend(fresh);
        }
    }
    for &(i, j, g) in &mouths {
        stats.mouths += 1;
        let p = match arch_for(&arches, g, slack) {
            Ok(p) => p,
            Err(why) => {
                skip(&mut stats, why);
                continue;
            }
        };
        let (a, b) = (&run_ends[i], &run_ends[j]);
        let n = unit([a.n[0] + b.n[0], a.n[1] + b.n[1]]).unwrap_or(a.n);
        let (pos, yaw) = place(p, [f64::midpoint(a.at[0], b.at[0]), f64::midpoint(a.at[1], b.at[1])], n);
        match placer.check(p, pos, yaw) {
            Ok((hull, h)) => {
                added.push(Instance {
                    id: String::new(),
                    kit: p.model.id.clone(),
                    lot: None,
                    pos: [pos[0], pos[1], h.z],
                    yaw_deg: yaw,
                    role: ARCH_ROLE.into(),
                    door: None,
                });
                placer.placed.push((bbox(&hull), hull));
                stats.arches += 1;
                stats.arches_m += g.min(p.len);
                stats.frontage_m += g;
            }
            Err(why) => skip(&mut stats, if why == "overlap" { "arch_overlap" } else { "arch_pad_step" }),
        }
    }
    for mut inst in added {
        inst.id = format!("i{}", fitted.instances.len());
        fitted.instances.push(inst);
    }
    for pl in &mut fitted.lots {
        if pl.status == LotStatus::Empty && walled.contains(&pl.id) {
            pl.status = LotStatus::Filler;
            stats.lots_filler += 1;
        }
    }
    stats.frontage_m += stats.faces_m;
    stats.open_m = (stats.faces_m - stats.buildings_m - stats.towers_m - stats.walls_m).max(0.0);
    let share = |num: f64, den: f64| if den > 0.0 { num / den } else { 0.0 };
    stats.coverage_before = share(stats.buildings_m, stats.faces_m);
    stats.coverage = share(stats.buildings_m + stats.towers_m + stats.walls_m + stats.arches_m, stats.frontage_m);
    stats.filler_ms = start.elapsed().as_secs_f64() * 1000.0;
    Ok((fitted, stats))
}

/// Fitting, merging ([`fit_and_merge`]) and, with a filler recipe, the street wall ([`fill`]).
///
/// # Errors
/// A filler recipe without a merge recipe; everything [`fill`] refuses.
pub fn fit_merge_fill(
    lots: &[Lot],
    kit: &KitFile,
    fit: &FitRecipe,
    merge: Option<&MergeRecipe>,
    filler: Option<&FillerRecipe>,
    seed: &str,
    ground: &dyn Fn(f64, f64) -> f64,
) -> Result<(Fitted, Option<MergeStats>, Option<FillerStats>), String> {
    let (fitted, merged) = fit_and_merge(lots, kit, fit, merge, seed, ground);
    let Some(f) = filler else { return Ok((fitted, merged, None)) };
    let m = merge.ok_or("a [filler] table needs a [merge] table (its frontage runs)")?;
    let (fitted, stats) = fill(lots, kit, fit, m, f, fitted, ground)?;
    Ok((fitted, merged, Some(stats)))
}
