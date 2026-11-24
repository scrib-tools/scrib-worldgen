//! Slice PL-3a: join narrow empty lots along their street until a kit building fits (the medina's continuous street
//! wall, owner 2026-11-24). Runs after the first fit pass (`crate::fit`), on the same [`Pass`], so a merged building
//! never overlaps one fitted before it and never undoes a filled lot.
//!
//! The street-wall step of rule-based town builders (`CityEngine`'s lot merge before a rule insert; Magna Urbis's own
//! narrow-lot pairing in `urban/parcels`): a building wider than one plot spans several plots of one frontage.
//!
//! 1. **Candidates**: lots left `empty` for `inside` (no model is narrow enough) that the fit tried (with footprints;
//!    in the `lot` domain also with a street access only, `fit::tried`).
//! 2. **Frontage neighbours** among the lots left `empty` that the fit tried: they share at least `min_shared_m` of
//!    boundary, their street normals (`fit::street_normal`) differ by at most `max_angle_deg`, the nearer end of the
//!    one's frontage (access segment, else `front`) lies within `max_offset_m` of the other's frontage line (the
//!    nearer end, so a lane winding by a few degrees per plot keeps its plots), and their street faces are at most
//!    `max_gap_m` apart along it. A street face is the span along the street of the lot's frontage and its vertices
//!    within `max_offset_m` of the frontage line: Magna Urbis's access segment is the entry, often shorter than the
//!    lot's face. Each lot keeps its nearest neighbour on either side along the
//!    street; mutual links chain into frontage runs, each walked from its end with the lower lot id.
//! 3. **Greedy merge**: runs in order of their lowest lot id, each along the street; from each lot not yet merged,
//!    the next lots are added one at a time (up to `max_lots` members and `max_front_m` of frontage) and the joined
//!    domain, once it holds a candidate, is fitted after each addition, in the frame of the first member's street
//!    normal; the first fit makes a group and the walk goes on after it. A lot in no group stays as it was. (A lot
//!    left empty for `overlap` or `pad_step` may join, and open a group that reaches a candidate.)
//!
//! Joined domain: the union of the members' lot polygons, cut to the depth band of their building footprints along
//! the street normal (from the deepest footprint back to the most forward footprint front). The lots are a partition,
//! so their union never reaches into a neighbour that is not a member; the band keeps Magna Urbis's building line
//! (the front setback it drew, which the fit's `setback_m` is measured from) and its building depth, and takes in
//! the side strips between the members' footprints, which is the gap a merge closes. (Closing the footprints across
//! the shared boundaries instead needs a polygon offset Magna Urbis's geometry crate does not have, for the same
//! result on the rectangular footprints Magna Urbis draws.)
//!
//! In the `lot` domain (`[fit] domain`) the joined domain is the members' union whole, with no depth band: the fit's
//! lot rule (`crate::fit` module docs) shrinks it by `lot_setback_m` on every side but the street side, so the
//! members' party walls vanish under one building.
//!
//! A group is one `PlacedLot` with status `merged`, id = its first member along the street, `merged_from` = every
//! member id along the street, polygon = the members' union; its building is appended to the instances.
// Planar geometry: a, b, n, u, t as in the formulas.
#![allow(clippy::many_single_char_names, clippy::similar_names)]
use crate::fit::{EmptyReason, Fitted, Pass, clockwise, dot, first_pass, street_normal, sub, tried, unit};
use crate::formats::{KitFile, LotStatus, PlacedLot};
use crate::lots::{Lot, Part, Xy};
use crate::recipe::{FitRecipe, MergeRecipe};
use magna_urbis_geom::{Point, Shape, area, intersection, union};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

/// Why a lot is still empty after merging (`stats.json` `merge.still_empty_by_reason`): `tried` = the furthest test a
/// joined domain holding it passed.
fn still_empty_name(first: EmptyReason, tried: Option<EmptyReason>, has_neighbour: bool) -> &'static str {
    match (tried, first) {
        (Some(EmptyReason::Inside), _) => "merged_inside",
        (Some(EmptyReason::Overlap), _) => "merged_overlap",
        (Some(EmptyReason::PadStep), _) => "merged_pad_step",
        (None, EmptyReason::Inside) if has_neighbour => "merge_limit",
        (None, EmptyReason::Inside) => "no_neighbour",
        // Not a candidate and never in a joined domain: the first pass's reason.
        (None, other) => other.name(),
    }
}

/// Counts for `stats.json` `merge`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct MergeStats {
    /// Lots `empty` for `inside` with footprints.
    pub candidates: usize,
    /// Lots `empty` with footprints that have a frontage neighbour.
    pub lots_with_neighbour: usize,
    pub runs: usize,
    pub groups: usize,
    /// Members of all groups.
    pub lots_merged: usize,
    /// Lots carrying a building before merging (`filled`).
    pub lots_filled_before: usize,
    /// Lots carrying a building after merging (`filled` + merged members).
    pub lots_filled_after: usize,
    pub instances_added: usize,
    /// Groups by member count.
    pub groups_by_size: BTreeMap<usize, usize>,
    /// Lots with footprints still empty: the furthest test a joined domain holding the lot passed (`merged_inside`,
    /// `merged_overlap`, `merged_pad_step`); else, for a candidate, `no_neighbour` or `merge_limit` (`max_lots` /
    /// `max_front_m` stopped every group before a fit was tried); else the first pass's reason.
    pub still_empty_by_reason: BTreeMap<&'static str, usize>,
    pub merge_ms: f64,
}

/// A lot's frontage on its street.
pub(crate) struct Frontage {
    /// Outward street normal (unit).
    pub n: Xy,
    pub seg: [Xy; 2],
    pub len: f64,
}

pub(crate) fn frontage(lot: &Lot) -> Option<Frontage> {
    let seg = lot.access.as_ref().map(|a| a.segment).or(lot.front)?;
    let n = street_normal(lot)?;
    let len = (seg[1][0] - seg[0][0]).hypot(seg[1][1] - seg[0][1]);
    (len.is_finite() && len > 0.0).then_some(Frontage { n, seg, len })
}

/// Span on axis `u` of a lot's street face: its frontage and its vertices within `reach` (m) of the frontage line.
pub(crate) fn face(lot: &Lot, f: &Frontage, reach: f64, u: Xy) -> [f64; 2] {
    let line = dot(f.n, f.seg[0]);
    f.seg.iter().chain(lot.polygon.iter().filter(|p| dot(f.n, **p) >= line - reach)).fold([f64::INFINITY, f64::NEG_INFINITY], |[lo, hi], p| {
        [lo.min(dot(u, *p)), hi.max(dot(u, *p))]
    })
}

type Bbox = [f64; 4];

fn bbox(r: &[Xy]) -> Bbox {
    r.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, p| {
        [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
    })
}

fn edges(r: &[Xy]) -> impl Iterator<Item = (Xy, Xy)> + '_ {
    (0..r.len()).map(move |i| (r[i], r[(i + 1) % r.len()]))
}

/// Length (m) of boundary two rings share: overlaps of edge pairs whose other edge's ends both lie within `tol` of
/// the edge's line (a T-junction counts its overlap).
#[must_use]
pub fn shared_boundary(a: &[Xy], b: &[Xy], tol: f64) -> f64 {
    let mut total = 0.0;
    for (a0, a1) in edges(a) {
        let Some(d) = unit(sub(a1, a0)) else { continue };
        let len = dot(d, sub(a1, a0));
        let side = clockwise(d);
        for (b0, b1) in edges(b) {
            let (h0, h1) = (dot(side, sub(b0, a0)), dot(side, sub(b1, a0)));
            if h0.abs() > tol || h1.abs() > tol {
                continue;
            }
            let (t0, t1) = (dot(d, sub(b0, a0)), dot(d, sub(b1, a0)));
            let overlap = t0.max(t1).min(len) - t0.min(t1).max(0.0);
            if overlap > 0.0 {
                total += overlap;
            }
        }
    }
    total
}

/// Signed gap (m) along the street from `a` to `b` (negative: overlapping frontages) and whether `b` lies ahead of
/// `a` along `a`'s `u`, if `b` is `a`'s frontage neighbour.
fn neighbour(r: &MergeRecipe, a: (&Lot, &Frontage), b: (&Lot, &Frontage)) -> Option<(f64, bool)> {
    let ((la, fa), (lb, fb)) = (a, b);
    if dot(fa.n, fb.n) < r.max_angle_deg.value.to_radians().cos() {
        return None;
    }
    let off = |p: Xy| dot(fa.n, sub(p, fa.seg[0])).abs();
    if off(fb.seg[0]).min(off(fb.seg[1])) > r.max_offset_m.value {
        return None;
    }
    let u = clockwise(fa.n);
    let reach = r.max_offset_m.value;
    let ([alo, ahi], [blo, bhi]) = (face(la, fa, reach, u), face(lb, fb, reach, u));
    let gap = (blo - ahi).max(alo - bhi);
    if gap > r.max_gap_m.value {
        return None;
    }
    if shared_boundary(&la.polygon, &lb.polygon, r.shared_tolerance_m.value) < r.min_shared_m.value {
        return None;
    }
    Some((gap, blo + bhi > alo + ahi))
}

fn shape(r: &[Xy]) -> Shape {
    Shape { outer: r.iter().map(|p| Point { x: p[0], y: p[1] }).collect(), holes: Vec::new() }
}

fn ring(r: &[Point]) -> Vec<Xy> {
    r.iter().map(|p| [p.x, p.y]).collect()
}

fn largest(shapes: Vec<Shape>) -> Option<Shape> {
    shapes.into_iter().map(|s| (area(&s), s)).filter(|(a, _)| *a > 0.0).max_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, s)| s)
}

/// The members' lot polygons joined (largest piece).
fn joined(members: &[&Lot]) -> Option<Shape> {
    let lots: Vec<Shape> = members
        .iter()
        .map(|l| Shape { holes: l.holes.iter().map(|h| shape(h).outer).collect(), ..shape(&l.polygon) })
        .collect();
    largest(union(&lots))
}

/// The joined domain of a group in the frame of street normal `n` (module docs).
#[must_use]
pub fn domain(members: &[&Lot], n: Xy) -> Option<Part> {
    let u = clockwise(n);
    let lots = joined(members)?;
    // Building footprints: the parts labelled `building`, else all of a member's parts.
    let footprints = members.iter().flat_map(|l| {
        let buildings: Vec<&Part> = l.preview_parts.iter().filter(|p| p.label == "building").collect();
        if buildings.is_empty() { l.preview_parts.iter().collect() } else { buildings }
    });
    let (mut nlo, mut nhi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in footprints.flat_map(|p| &p.polygon) {
        nlo = nlo.min(dot(n, *p));
        nhi = nhi.max(dot(n, *p));
    }
    let (mut ulo, mut uhi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in &lots.outer {
        ulo = ulo.min(dot(u, [p.x, p.y]));
        uhi = uhi.max(dot(u, [p.x, p.y]));
    }
    if !(nlo < nhi && ulo < uhi) {
        return None;
    }
    // The band spans the lots along the street with their own width to spare, so only its depth cuts.
    let span = uhi - ulo;
    let (ulo, uhi) = (ulo - span, uhi + span);
    let at = |s: f64, t: f64| [s * u[0] + t * n[0], s * u[1] + t * n[1]];
    let band = shape(&[at(ulo, nlo), at(uhi, nlo), at(uhi, nhi), at(ulo, nhi)]);
    let cut = largest(intersection(&lots, &band))?;
    Some(Part { label: "building".into(), polygon: ring(&cut.outer), holes: cut.holes.iter().map(|h| ring(h)).collect() })
}

/// The joined domain of a group in the `lot` domain: the members' lot polygons joined (module docs).
fn lot_domain(members: &[&Lot]) -> Option<Part> {
    let lots = joined(members)?;
    Some(Part { label: "lot".into(), polygon: ring(&lots.outer), holes: lots.holes.iter().map(|h| ring(h)).collect() })
}

/// Frontage runs among `eligible` (indices into `lots`): chains of mutual nearest neighbours, each ordered along the
/// street, the runs in order of their lowest lot id.
pub(crate) fn runs(r: &MergeRecipe, lots: &[&Lot], fronts: &[Option<Frontage>], eligible: &[usize]) -> (Vec<Vec<usize>>, BTreeSet<usize>) {
    let tol = r.shared_tolerance_m.value;
    let boxes: Vec<Bbox> = lots.iter().map(|l| bbox(&l.polygon)).collect();
    // Nearest neighbour ahead and behind: (gap, id, index).
    let mut ahead: BTreeMap<usize, (f64, &str, usize)> = BTreeMap::new();
    let mut behind: BTreeMap<usize, (f64, &str, usize)> = BTreeMap::new();
    let mut with_neighbour = BTreeSet::new();
    for &i in eligible {
        let Some(fi) = &fronts[i] else { continue };
        for &j in eligible {
            let Some(fj) = &fronts[j] else { continue };
            let (bi, bj) = (boxes[i], boxes[j]);
            if i == j || bi[0] > bj[2] + tol || bj[0] > bi[2] + tol || bi[1] > bj[3] + tol || bj[1] > bi[3] + tol {
                continue;
            }
            let Some((gap, is_ahead)) = neighbour(r, (lots[i], fi), (lots[j], fj)) else { continue };
            with_neighbour.insert(i);
            let side = if is_ahead { &mut ahead } else { &mut behind };
            let key = (gap, lots[j].id.as_str(), j);
            let better = side.get(&i).is_none_or(|old| key.0.total_cmp(&old.0).then(key.1.cmp(old.1)).is_lt());
            if better {
                side.insert(i, key);
            }
        }
    }
    // Mutual links only: `next[i] = j` when j is i's nearest ahead and i is j's nearest behind.
    let next: BTreeMap<usize, usize> =
        ahead.iter().filter(|(i, (_, _, j))| behind.get(j).is_some_and(|b| b.2 == **i)).map(|(i, (_, _, j))| (*i, *j)).collect();
    let has_prev: BTreeSet<usize> = next.values().copied().collect();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let follow = |start: usize, seen: &mut BTreeSet<usize>| {
        let mut run = Vec::new();
        let mut at = Some(start);
        while let Some(i) = at.filter(|i| seen.insert(*i)) {
            run.push(i);
            at = next.get(&i).copied();
        }
        run
    };
    let mut by_id: Vec<usize> = eligible.iter().copied().filter(|i| fronts[*i].is_some()).collect();
    by_id.sort_by(|a, b| lots[*a].id.cmp(&lots[*b].id));
    for &i in &by_id {
        if !has_prev.contains(&i) && !seen.contains(&i) {
            out.push(follow(i, &mut seen));
        }
    }
    // Closed rings (a run around a square): start at their lowest id.
    for &i in &by_id {
        if !seen.contains(&i) {
            out.push(follow(i, &mut seen));
        }
    }
    // Each run walks from its end with the lower id; runs in order of their lowest id.
    for run in &mut out {
        if run.len() > 1 && lots[run[run.len() - 1]].id < lots[run[0]].id {
            run.reverse();
        }
    }
    let min_id = |run: &Vec<usize>| run.iter().map(|i| lots[*i].id.as_str()).min().unwrap_or_default().to_string();
    out.sort_by_cached_key(min_id);
    (out, with_neighbour)
}

/// The span of the members' fronts, in the direction of the first member front, if any member has one.
fn joined_front(members: &[&Lot]) -> Option<[Xy; 2]> {
    let first = members.iter().find_map(|l| l.front)?;
    let u = sub(first[1], first[0]);
    let ends: Vec<Xy> = members.iter().filter_map(|l| l.front).flatten().collect();
    let lo = ends.iter().copied().min_by(|a, b| dot(u, *a).total_cmp(&dot(u, *b)))?;
    let hi = ends.iter().copied().max_by(|a, b| dot(u, *a).total_cmp(&dot(u, *b)))?;
    Some([lo, hi])
}

/// The first fit pass ([`crate::fit::fit`]), then, with a merge recipe, the merge pass: the fit with groups replacing
/// their members, and the merge's counts.
#[must_use]
pub fn fit_and_merge(
    lots: &[Lot],
    kit: &KitFile,
    fit: &FitRecipe,
    merge: Option<&MergeRecipe>,
    seed: &str,
    ground: &dyn Fn(f64, f64) -> f64,
) -> (Fitted, Option<MergeStats>) {
    let (fitted, pass) = first_pass(lots, kit, fit, seed, ground);
    match merge {
        None => (fitted, None),
        Some(r) => {
            let (fitted, stats) = merge_pass(lots, fitted, pass, r);
            (fitted, Some(stats))
        }
    }
}

fn merge_pass(lots: &[Lot], mut fitted: Fitted, mut pass: Pass, r: &MergeRecipe) -> (Fitted, MergeStats) {
    let start = Instant::now();
    let mut order: Vec<&Lot> = lots.iter().collect();
    order.sort_by(|a, b| a.id.cmp(&b.id));
    let mut stats = MergeStats { lots_filled_before: fitted.stats.filled, ..MergeStats::default() };
    let whole = pass.lot_domain();
    let eligible: Vec<usize> = (0..order.len()).filter(|&i| tried(order[i], whole) && fitted.empty.contains_key(&order[i].id)).collect();
    let candidate = |i: usize| fitted.empty.get(&order[i].id) == Some(&EmptyReason::Inside);
    stats.candidates = eligible.iter().filter(|&&i| candidate(i)).count();
    let fronts: Vec<Option<Frontage>> = order.iter().map(|l| frontage(l)).collect();
    let (runs, with_neighbour) = runs(r, &order, &fronts, &eligible);
    stats.runs = runs.len();
    stats.lots_with_neighbour = with_neighbour.len();
    // Furthest test a joined domain holding the lot passed.
    let mut tried: BTreeMap<usize, EmptyReason> = BTreeMap::new();
    let mut groups: Vec<(Vec<usize>, PlacedLot)> = Vec::new();
    for run in &runs {
        let mut k = 0;
        while k < run.len() {
            let i = run[k];
            let Some(n) = fronts[i].as_ref().map(|f| f.n) else {
                k += 1;
                continue;
            };
            let mut front_m = fronts[i].as_ref().map_or(0.0, |f| f.len);
            let mut found = None;
            for end in k + 1..run.len() {
                if end - k + 1 > r.max_lots.value {
                    break;
                }
                front_m += fronts[run[end]].as_ref().map_or(0.0, |f| f.len);
                if front_m > r.max_front_m.value {
                    break;
                }
                if !run[k..=end].iter().any(|&m| candidate(m)) {
                    continue;
                }
                let members: Vec<&Lot> = run[k..=end].iter().map(|&m| order[m]).collect();
                let dom = if whole { lot_domain(&members) } else { domain(&members, n) };
                let Some(dom) = dom else { continue };
                let key = members.iter().map(|l| l.id.as_str()).collect::<Vec<_>>().join("+");
                match pass.fit_domain(&dom, n, &key, 0) {
                    Ok(f) => {
                        found = Some((end, members, f));
                        break;
                    }
                    Err(e) => {
                        for &m in &run[k..=end] {
                            let t = tried.entry(m).or_insert(e);
                            *t = (*t).max(e);
                        }
                    }
                }
            }
            let Some((end, members, f)) = found else {
                k += 1;
                continue;
            };
            let id = members[0].id.clone();
            fitted.instances.push(f.instance(fitted.instances.len(), &id));
            let polygon = joined(&members).map_or_else(|| members[0].polygon.clone(), |s| ring(&s.outer));
            let placed = PlacedLot {
                id,
                polygon,
                status: LotStatus::Merged,
                role: members[0].role.clone(),
                pad_z_m: Some(f.pad_z),
                front: joined_front(&members),
                merged_from: members.iter().map(|l| l.id.clone()).collect(),
            };
            stats.groups += 1;
            stats.lots_merged += members.len();
            stats.instances_added += 1;
            *stats.groups_by_size.entry(members.len()).or_default() += 1;
            groups.push((run[k..=end].to_vec(), placed));
            k = end + 1;
        }
    }
    let merged: BTreeSet<&str> = groups.iter().flat_map(|(m, _)| m.iter().map(|&i| order[i].id.as_str())).collect();
    let merged: BTreeSet<String> = merged.into_iter().map(String::from).collect();
    fitted.lots.retain(|l| !merged.contains(&l.id));
    fitted.lots.extend(groups.into_iter().map(|(_, p)| p));
    fitted.lots.sort_by(|a, b| a.id.cmp(&b.id));
    fitted.empty.retain(|id, _| !merged.contains(id));
    let index: BTreeMap<&str, usize> = order.iter().enumerate().map(|(i, l)| (l.id.as_str(), i)).collect();
    for (id, reason) in &fitted.empty {
        let i = index.get(id.as_str()).copied();
        let name = still_empty_name(*reason, i.and_then(|i| tried.get(&i)).copied(), i.is_some_and(|i| with_neighbour.contains(&i)));
        *stats.still_empty_by_reason.entry(name).or_default() += 1;
    }
    stats.lots_filled_after = stats.lots_filled_before + stats.lots_merged;
    stats.merge_ms = start.elapsed().as_secs_f64() * 1000.0;
    (fitted, stats)
}
