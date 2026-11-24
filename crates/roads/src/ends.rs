//! Road ends at a settlement's edge, on land.
//!
//! Magna Urbis routes every road to its site's centre and lets it cross the cells within the site radius freely
//! (`settlements/roads/mod.rs:820-834` marks them settled, `roads/search.rs:637` frees the turns there), so a road runs
//! through a kept town's buildings to its centre, and a centre on water is a route end in the water. Two fixes around
//! the router, both in the guide frame:
//! - before routing, a site whose centre lies on a wet routing cell is routed from its **landing**, the nearest
//!   routing cell Magna Urbis pools to dry within the recipe's `landing_search_m` ([`landing`]);
//! - after routing, every polyline is cut where it enters a site's disc (radius around its centre, and around its
//!   landing if it has one) and the parts inside are dropped ([`clip`]): roads stop on the town's edge.
// Raster block indices and metres: values bounded by the raster dims. Geometry: p, q, c, r as in the formulas.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss, clippy::many_single_char_names)]
use crate::raster::{DRY, LAKE, MuRaster, RIVER, SEA};

/// Magna Urbis's water code of routing block `(bx, by)`, `k` raster cells a side: sea or lake when it fills half the
/// block, river when any cell is river, else dry (`settlements/grid.rs:87-121` `Land::pooled`).
#[must_use]
pub fn block_water(raster: &MuRaster, k: usize, bx: usize, by: usize) -> u8 {
    let n = raster.frame.n;
    let mut count = [0_usize; 4];
    for y in by * k..(by + 1) * k {
        for x in bx * k..(bx + 1) * k {
            count[usize::from(raster.water[y * n + x]).min(3)] += 1;
        }
    }
    let half = k * k;
    if 2 * count[usize::from(SEA)] >= half {
        SEA
    } else if 2 * count[usize::from(LAKE)] >= half {
        LAKE
    } else if count[usize::from(RIVER)] > 0 {
        RIVER
    } else {
        DRY
    }
}

/// Where a site is routed from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Landing {
    /// Its centre is on a dry routing cell.
    Dry,
    /// Its centre is wet; routed from this dry routing cell's centre (guide frame).
    Moved([f64; 2]),
    /// Its centre is wet and no dry routing cell lies within the search radius (or it is off the raster).
    Stranded,
}

/// The landing of a site centred on `centre` (guide frame), with the raster pooled by `k` cells a side for routing.
/// Nearest dry block centre within `search_m` of `centre`; ties go to the first block in row-major order.
#[must_use]
pub fn landing(raster: &MuRaster, k: usize, centre: [f64; 2], search_m: f64) -> Landing {
    let f = &raster.frame;
    let Some(i) = f.cell(centre) else { return Landing::Stranded };
    let (bcx, bcy) = (i % f.n / k, i / f.n / k);
    if block_water(raster, k, bcx, bcy) == DRY {
        return Landing::Dry;
    }
    let (m, block_m) = (f.n / k, f.cell_m * k as f64);
    let reach = (search_m / block_m).ceil() as usize;
    let mut best: Option<(f64, [f64; 2])> = None;
    for by in bcy.saturating_sub(reach)..=(bcy + reach).min(m - 1) {
        for bx in bcx.saturating_sub(reach)..=(bcx + reach).min(m - 1) {
            let p = [f.west_m + (bx as f64 + 0.5) * block_m, f.north_m - (by as f64 + 0.5) * block_m];
            let d = dist(p, centre);
            if d <= search_m && best.is_none_or(|(b, _)| d < b) && block_water(raster, k, bx, by) == DRY {
                best = Some((d, p));
            }
        }
    }
    best.map_or(Landing::Stranded, |(_, p)| Landing::Moved(p))
}

/// A site's extent as [`clip`] sees it (guide frame).
#[derive(Clone, Debug)]
pub struct Extent {
    pub centre: [f64; 2],
    pub landing: Option<[f64; 2]>,
    /// Effective radius (m): the site's own, else Magna Urbis's from its population.
    pub radius: f64,
}

/// A run of a routed polyline outside every site.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub points: Vec<[f64; 2]>,
    /// Per end (first, last point): the index of the site whose edge cut it, `None` for an uncut end.
    pub cut: [Option<usize>; 2],
    /// Per end: length (m) of the dropped run beyond it, 0 for an uncut end.
    pub trimmed_m: [f64; 2],
}

/// Cut `points` where it enters any site's disc and keep the runs outside, in order. A polyline wholly inside gives
/// none; one that never enters a disc gives itself, uncut.
#[must_use]
pub fn clip(points: &[[f64; 2]], sites: &[Extent]) -> Vec<Piece> {
    if points.len() < 2 {
        return Vec::new();
    }
    let discs: Vec<([f64; 2], f64, usize)> = sites
        .iter()
        .enumerate()
        .flat_map(|(i, s)| std::iter::once(s.centre).chain(s.landing).map(move |c| (c, s.radius, i)))
        .collect();
    // The polyline with every circle crossing inserted: each sub-edge is wholly inside or wholly outside.
    let mut pts = vec![points[0]];
    let mut ts = Vec::new();
    for w in points.windows(2) {
        let (p, q) = (w[0], w[1]);
        ts.clear();
        ts.extend(discs.iter().flat_map(|&(c, r, _)| circle_hits(p, q, c, r)).filter(|t| *t > 0.0 && *t < 1.0));
        ts.sort_by(f64::total_cmp);
        pts.extend(ts.iter().map(|&t| lerp(p, q, t)));
        pts.push(q);
    }
    let outside = |w: &[[f64; 2]]| {
        let m = lerp(w[0], w[1], 0.5);
        !discs.iter().any(|&(c, r, _)| dist(m, c) < r)
    };
    // Runs of sub-edges with the same side: (outside, first sub-edge, end sub-edge, length).
    let mut runs: Vec<(bool, usize, usize, f64)> = Vec::new();
    for (i, w) in pts.windows(2).enumerate() {
        let (out, len) = (outside(w), dist(w[0], w[1]));
        match runs.last_mut() {
            Some(run) if run.0 == out => {
                run.2 = i + 1;
                run.3 += len;
            }
            _ => runs.push((out, i, i + 1, len)),
        }
    }
    let edge_site = |p: [f64; 2]| {
        discs.iter().min_by(|a, b| (dist(p, a.0) - a.1).abs().total_cmp(&(dist(p, b.0) - b.1).abs())).map(|d| d.2)
    };
    let mut pieces = Vec::new();
    for (k, &(out, a, b, len)) in runs.iter().enumerate() {
        if !out || len <= 0.0 {
            continue;
        }
        let before = k.checked_sub(1).map(|j| runs[j].3);
        let after = runs.get(k + 1).map(|r| r.3);
        let (first, last) = (pts[a], pts[b]);
        pieces.push(Piece {
            points: pts[a..=b].to_vec(),
            cut: [before.and_then(|_| edge_site(first)), after.and_then(|_| edge_site(last))],
            trimmed_m: [before.unwrap_or(0.0), after.unwrap_or(0.0)],
        });
    }
    pieces
}

/// Polyline length (m).
#[must_use]
pub fn length(points: &[[f64; 2]]) -> f64 {
    points.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Distance (m) from `p` to the polyline `points`.
#[must_use]
pub fn distance_to(points: &[[f64; 2]], p: [f64; 2]) -> f64 {
    let to_edge = |a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
        dist(p, lerp(a, b, t))
    };
    points.windows(2).map(|w| to_edge(w[0], w[1])).fold(f64::INFINITY, f64::min)
}

#[must_use]
pub fn dist(p: [f64; 2], q: [f64; 2]) -> f64 {
    (p[0] - q[0]).hypot(p[1] - q[1])
}

fn lerp(p: [f64; 2], q: [f64; 2], t: f64) -> [f64; 2] {
    [p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])]
}

/// Parameters `t` where `p + t (q - p)` meets the circle `(c, r)` (none for a tangent or a point).
fn circle_hits(p: [f64; 2], q: [f64; 2], c: [f64; 2], r: f64) -> impl Iterator<Item = f64> {
    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
    let (fx, fy) = (p[0] - c[0], p[1] - c[1]);
    let a = dx * dx + dy * dy;
    let b = 2.0 * (fx * dx + fy * dy);
    let disc = b * b - 4.0 * a * (fx * fx + fy * fy - r * r);
    let hits = (a > 0.0 && disc > 0.0).then(|| {
        let s = disc.sqrt();
        [(-b - s) / (2.0 * a), (-b + s) / (2.0 * a)]
    });
    hits.into_iter().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_road_through_a_town_is_cut_at_both_edges_and_one_inside_is_dropped() {
        let town = Extent { centre: [0.0, 0.0], landing: None, radius: 10.0 };
        let pieces = clip(&[[-30.0, 0.0], [0.0, 0.0], [30.0, 0.0]], std::slice::from_ref(&town));
        let round = |p: &Piece| {
            let r = |v: f64| (v * 1e6).round() / 1e6;
            let pts: Vec<[f64; 2]> = p.points.iter().map(|q| [r(q[0]), r(q[1])]).collect();
            (pts, p.cut, p.trimmed_m.map(r))
        };
        assert_eq!(pieces.len(), 2);
        assert_eq!(round(&pieces[0]), (vec![[-30.0, 0.0], [-10.0, 0.0]], [None, Some(0)], [0.0, 20.0]));
        assert_eq!(round(&pieces[1]), (vec![[10.0, 0.0], [30.0, 0.0]], [Some(0), None], [20.0, 0.0]));
        assert!(clip(&[[-5.0, 0.0], [5.0, 1.0]], &[town]).is_empty());
    }
}
