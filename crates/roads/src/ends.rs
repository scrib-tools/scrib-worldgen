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

/// Contact samples per `handover_m` of road in [`clip_at_streets`]: a ratio of the band (no contact narrower than a
/// quarter of it is missed), not a tunable.
const SAMPLES_PER_BAND: f64 = 4.0;

/// A point of a polyline tested for contact by [`clip_at_streets`].
struct Sample {
    /// Edge it lies on (`points[edge]` to `points[edge + 1]`).
    edge: usize,
    /// Arc length (m) from the polyline's start.
    s: f64,
    p: [f64; 2],
    /// Set when the sample is a vertex of the polyline (its index).
    vertex: Option<usize>,
}

/// Cut `points` where it runs along a settled site's streets: the run from its first to its last contact
/// (a point nearer than `handover_m` to any street) is dropped, the pieces before and after are kept and each cut end
/// gets one more vertex, the nearest point on a street, so the road meets the street's centreline.
///
/// Contact is tested on the vertices and on samples at most `handover_m / 4` apart between them; the cut points are
/// the first and last contact samples. Head = start to the first contact plus its snap vertex, `cut [None, Some(site)]`;
/// tail = the last contact's snap vertex, then the last contact to the end, `cut [Some(site), None]`. As in [`clip`],
/// `trimmed_m` at a cut end is the length of the dropped run beside it: the arc length from the first to the last
/// contact (0 for a single contact; a dropped stub is not counted). A head or tail shorter than `handover_m` (before
/// its snap vertex) is a stub and is dropped, so a polyline wholly along streets gives none. With no contact, fewer
/// than 2 points, no streets or `handover_m <= 0`, the result is as [`clip`]'s with no sites: the input uncut (none for
/// fewer than 2 points).
#[must_use]
pub fn clip_at_streets(points: &[[f64; 2]], site: usize, streets: &[Vec<[f64; 2]>], handover_m: f64) -> Vec<Piece> {
    if points.len() < 2 {
        return Vec::new();
    }
    let uncut = || vec![Piece { points: points.to_vec(), cut: [None, None], trimmed_m: [0.0, 0.0] }];
    if streets.is_empty() || handover_m <= 0.0 || handover_m.is_nan() {
        return uncut();
    }
    // Each street with its bounding box grown by the band: a sample outside the box is no contact.
    let boxed: Vec<(&[[f64; 2]], [f64; 4])> = streets
        .iter()
        .filter(|st| !st.is_empty())
        .map(|st| {
            let b = st.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, q| {
                [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])]
            });
            (st.as_slice(), [b[0] - handover_m, b[1] - handover_m, b[2] + handover_m, b[3] + handover_m])
        })
        .collect();
    // Nearest street point to `p` within the band, if any.
    let snap = |p: [f64; 2]| {
        boxed
            .iter()
            .filter(|(_, b)| p[0] >= b[0] && p[1] >= b[1] && p[0] <= b[2] && p[1] <= b[3])
            .map(|(st, _)| nearest_on(st, p))
            .filter(|&(_, d)| d < handover_m)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(q, _)| q)
    };
    let step = handover_m / SAMPLES_PER_BAND;
    let mut samples = vec![Sample { edge: 0, s: 0.0, p: points[0], vertex: Some(0) }];
    let mut s0 = 0.0;
    for (i, w) in points.windows(2).enumerate() {
        let len = dist(w[0], w[1]);
        let n = (len / step).ceil().max(1.0) as usize;
        samples.extend((1..n).map(|j| {
            let t = j as f64 / n as f64;
            Sample { edge: i, s: s0 + t * len, p: lerp(w[0], w[1], t), vertex: None }
        }));
        s0 += len;
        samples.push(Sample { edge: i, s: s0, p: w[1], vertex: Some(i + 1) });
    }
    let total = s0;
    let Some(first) = samples.iter().position(|x| snap(x.p).is_some()) else { return uncut() };
    let last = samples.iter().rposition(|x| snap(x.p).is_some()).unwrap_or(first);
    let (a, b) = (&samples[first], &samples[last]);
    let dropped = b.s - a.s;
    let mut pieces = Vec::new();
    if a.s >= handover_m {
        let mut head = match a.vertex {
            Some(v) => points[..=v].to_vec(),
            None => points[..=a.edge].iter().copied().chain(std::iter::once(a.p)).collect(),
        };
        head.extend(snap(a.p));
        pieces.push(Piece { points: head, cut: [None, Some(site)], trimmed_m: [0.0, dropped] });
    }
    if total - b.s >= handover_m {
        let mut tail: Vec<[f64; 2]> = snap(b.p).into_iter().collect();
        match b.vertex {
            Some(v) => tail.extend_from_slice(&points[v..]),
            None => tail.extend(std::iter::once(b.p).chain(points[b.edge + 1..].iter().copied())),
        }
        pieces.push(Piece { points: tail, cut: [Some(site), None], trimmed_m: [dropped, 0.0] });
    }
    pieces
}

/// Nearest point of the polyline `points` (one point or more) to `p`, and its distance (m).
fn nearest_on(points: &[[f64; 2]], p: [f64; 2]) -> ([f64; 2], f64) {
    let on_edge = |a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let q = lerp(a, b, t);
        (q, dist(p, q))
    };
    if points.len() == 1 {
        return (points[0], dist(points[0], p));
    }
    points.windows(2).map(|w| on_edge(w[0], w[1])).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or((p, f64::INFINITY))
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
// Cut and snap points are asserted exactly: they are vertices or nearest points the inputs make exact.
#[allow(clippy::float_cmp)]
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

    /// The x axis from (0, 0) to (1000, 0), a vertex every 100 m.
    fn road() -> Vec<[f64; 2]> {
        (0..=10).map(|i| [f64::from(i) * 100.0, 0.0]).collect()
    }

    fn street_at(x: f64, y0: f64, y1: f64) -> Vec<[f64; 2]> {
        vec![[x, y0], [x, y1]]
    }

    const BAND: f64 = 8.0;
    const SITE: usize = 3;

    #[test]
    fn road_through_a_town_is_cut_at_both_contacts() {
        let streets = [street_at(600.0, -50.0, 50.0), street_at(800.0, -50.0, 50.0)];
        let pieces = clip_at_streets(&road(), SITE, &streets, BAND);
        assert_eq!(pieces.len(), 2);
        let (head, tail) = (&pieces[0], &pieces[1]);
        assert_eq!(head.points[0], [0.0, 0.0]);
        assert_eq!(*head.points.last().unwrap(), [600.0, 0.0]);
        let before = head.points[head.points.len() - 2];
        assert!((before[0] - 592.0).abs() <= 2.0, "{before:?}");
        assert_eq!(before[1], 0.0);
        assert_eq!(head.cut, [None, Some(SITE)]);
        assert_eq!(tail.points[0], [800.0, 0.0]);
        assert!((tail.points[1][0] - 808.0).abs() <= 2.0, "{:?}", tail.points[1]);
        assert_eq!(tail.points[1][1], 0.0);
        assert_eq!(*tail.points.last().unwrap(), [1000.0, 0.0]);
        assert_eq!(tail.cut, [Some(SITE), None]);
    }

    #[test]
    fn road_clear_of_the_streets_is_untouched() {
        let streets = [street_at(600.0, 100.0, 200.0), street_at(800.0, 100.0, 200.0)];
        let pieces = clip_at_streets(&road(), SITE, &streets, BAND);
        assert_eq!(pieces, vec![Piece { points: road(), cut: [None, None], trimmed_m: [0.0, 0.0] }]);
    }

    #[test]
    fn road_ending_in_town_loses_its_stub() {
        let pieces = clip_at_streets(&road(), SITE, &[street_at(995.0, -50.0, 50.0)], BAND);
        assert_eq!(pieces.len(), 1);
        assert_eq!(*pieces[0].points.last().unwrap(), [995.0, 0.0]);
    }

    #[test]
    fn road_along_a_street_gives_nothing() {
        let streets = [vec![[-10.0, 2.0], [30.0, 2.0]]];
        assert!(clip_at_streets(&[[0.0, 0.0], [20.0, 0.0]], SITE, &streets, BAND).is_empty());
    }

    #[test]
    fn degenerate_input_comes_back() {
        let uncut = vec![Piece { points: road(), cut: [None, None], trimmed_m: [0.0, 0.0] }];
        assert_eq!(clip_at_streets(&road(), SITE, &[], BAND), uncut);
        assert_eq!(clip_at_streets(&road(), SITE, &[street_at(600.0, -50.0, 50.0)], 0.0), uncut);
        assert!(clip_at_streets(&[[0.0, 0.0]], SITE, &[street_at(0.0, -50.0, 50.0)], BAND).is_empty());
    }
}
