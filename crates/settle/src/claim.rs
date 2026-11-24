//! The kept town as Magna Urbis's `Claim` (`urban/neighbours.rs:21`): land the new town never builds on (plus Magna
//! Urbis's own 8 m margin, `neighbours.rs:53`) and edge streets it joins (`neighbours::join`, `:466`).
//!
//! Land: the site's `claim.footprints` rasterized, grown by a margin and closed over the gaps between them (a
//! morphological closing on an exact Euclidean distance transform, Felzenszwalb and Huttenlocher 2012), small holes
//! filled, stray pieces dropped, traced into rings and simplified: the same raster closing Magna Urbis applies to its
//! own towns' claims (`neighbours.rs:697` `Raster`), with our recipe values instead of its constants.
//!
//! Exits: road texels (`roads.u8`) just outside the land with a road texel inside next to them, clustered, each traced
//! along the road texels outward and inward into one street that ends on its outer side, like Magna Urbis's own claim
//! streets (`neighbours.rs:870` `stubs`). They go in `Claim::streets`, not in the regional roads: Magna Urbis adds a
//! neighbour's edge streets to the roads the town grows against (`neighbours::roads`, `:312`) *and* ties its dead
//! ends to them at the end (`join`); regional roads are only cut at the land and never joined.
//!
//! Everything here is in Magna Urbis's local frame (x east, y south, [`Frame`]).
// Raster and metre conversions: values are bounded by the raster dims.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss, clippy::cast_possible_wrap)]
// The distance transform keeps the paper's names (f, d, v, z, k, q).
#![allow(clippy::many_single_char_names)]
use crate::grid::Frame;
use crate::recipe::ClaimRecipe;
use crate::site::SiteRaster;
use magna_urbis_core::urban::neighbours::Claim;
use magna_urbis_core::urban::{Point, Shape, Street};
use magna_urbis_geom as g;
use serde::Deserialize;
use std::collections::VecDeque;

/// Site key of the kept town's claim.
pub const KEPT_KEY: &str = "kept";
/// Exits are regional roads to Magna Urbis (rank 0, `geography.rs:564`).
pub const EXIT_RANK: usize = 0;
pub const EXIT_ROLE: &str = "regional";

#[derive(Deserialize)]
struct ClaimFile {
    #[serde(default)]
    footprints: Vec<FootprintFile>,
}

#[derive(Deserialize)]
struct FootprintFile {
    polygon: Vec<[f64; 2]>,
}

/// The kept town, ready for Magna Urbis.
#[derive(Clone, Debug)]
pub struct Kept {
    /// The site's footprints (local frame), for the sheet.
    pub footprints: Vec<Shape>,
    /// Land and exits (local frame).
    pub claim: Claim,
    pub footprint_m2: f64,
    pub land_m2: f64,
}

/// A raster over the local frame; cell `(i, j)` covers `[x0 + i c, x0 + (i+1) c] x [y0 + j c, ...]`.
struct Raster {
    x0: f64,
    y0: f64,
    cell: f64,
    nx: usize,
    ny: usize,
    on: Vec<bool>,
}

impl Raster {
    /// Sets the cells whose centre lies inside `ring` (even-odd), and the cells holding its vertices, so a footprint
    /// smaller than a cell still counts.
    fn fill(&mut self, ring: &[Point]) {
        let ys = ring.iter().map(|p| p.y);
        let (lo, hi) = ys.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), y| (a.min(y), b.max(y)));
        let row = |y: f64| (y - self.y0) / self.cell - 0.5;
        let j0 = row(lo).ceil().max(0.0) as usize;
        let j1 = (row(hi).floor() + 1.0).clamp(0.0, self.ny as f64) as usize;
        let mut xs = Vec::new();
        for j in j0..j1 {
            let y = self.y0 + (j as f64 + 0.5) * self.cell;
            xs.clear();
            for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()) {
                if (a.y <= y) != (b.y <= y) {
                    xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
                }
            }
            xs.sort_by(f64::total_cmp);
            for pair in xs.as_chunks::<2>().0 {
                let i0 = ((pair[0] - self.x0) / self.cell - 0.5).ceil().max(0.0) as usize;
                let i1 = (((pair[1] - self.x0) / self.cell - 0.5).floor() + 1.0).clamp(0.0, self.nx as f64) as usize;
                for i in i0..i1.max(i0) {
                    self.on[j * self.nx + i] = true;
                }
            }
        }
        for p in ring {
            let (i, j) = (((p.x - self.x0) / self.cell).floor(), ((p.y - self.y0) / self.cell).floor());
            if i >= 0.0 && j >= 0.0 && (i as usize) < self.nx && (j as usize) < self.ny {
                self.on[j as usize * self.nx + i as usize] = true;
            }
        }
    }

    /// Cells set: closing of `on` grown by `margin` with radius `close` (m).
    fn close(&mut self, margin: f64, close: f64) {
        let grow = (margin + close) / self.cell;
        let d = sq_distance(&self.on, self.nx, self.ny);
        let grown: Vec<bool> = d.iter().map(|&v| v <= grow * grow).collect();
        let off: Vec<bool> = grown.iter().map(|&b| !b).collect();
        let shrink = close / self.cell;
        let d = sq_distance(&off, self.nx, self.ny);
        self.on = grown.iter().zip(&d).map(|(&g, &v)| g && v > shrink * shrink).collect();
    }

    /// 4-connected components of cells equal to `value`: (cells, touches the raster border).
    fn components(&self, value: bool) -> Vec<(Vec<usize>, bool)> {
        let (nx, ny) = (self.nx, self.ny);
        let mut seen = vec![false; nx * ny];
        let mut out = Vec::new();
        for start in 0..nx * ny {
            if seen[start] || self.on[start] != value {
                continue;
            }
            seen[start] = true;
            let (mut stack, mut cells, mut border) = (vec![start], Vec::new(), false);
            while let Some(k) = stack.pop() {
                cells.push(k);
                let (i, j) = (k % nx, k / nx);
                border |= i == 0 || j == 0 || i + 1 == nx || j + 1 == ny;
                let near = [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)];
                for n in near.into_iter().flatten() {
                    if !seen[n] && self.on[n] == value {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
            out.push((cells, border));
        }
        out
    }

    /// Fill holes under `min_hole` and drop pieces under `min_land` (m²).
    fn clean(&mut self, min_hole: f64, min_land: f64) {
        let area = self.cell * self.cell;
        for (cells, border) in self.components(false) {
            if !border && (cells.len() as f64) * area < min_hole {
                for k in cells {
                    self.on[k] = true;
                }
            }
        }
        for (cells, _) in self.components(true) {
            if (cells.len() as f64) * area < min_land {
                for k in cells {
                    self.on[k] = false;
                }
            }
        }
    }

    /// Outlines of the set cells: boundary edges traced into rings (land on the left, diagonal cells kept apart at a
    /// saddle), simplified, holes matched to the smallest outer ring holding them.
    fn outline(&self, simplify: f64) -> Vec<Shape> {
        let (nx, ny) = (self.nx, self.ny);
        let on = |i: i64, j: i64| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && self.on[j as usize * nx + i as usize];
        let vid = |i: usize, j: usize| j * (nx + 1) + i;
        let mut out = vec![[usize::MAX; 2]; (nx + 1) * (ny + 1)];
        let mut add = |a: usize, b: usize| {
            let slot = usize::from(out[a][0] != usize::MAX);
            out[a][slot] = b;
        };
        for j in 0..ny {
            for i in 0..nx {
                if !self.on[j * nx + i] {
                    continue;
                }
                let (ii, jj) = (i as i64, j as i64);
                if !on(ii, jj - 1) {
                    add(vid(i, j), vid(i + 1, j));
                }
                if !on(ii + 1, jj) {
                    add(vid(i + 1, j), vid(i + 1, j + 1));
                }
                if !on(ii, jj + 1) {
                    add(vid(i + 1, j + 1), vid(i, j + 1));
                }
                if !on(ii - 1, jj) {
                    add(vid(i, j + 1), vid(i, j));
                }
            }
        }
        let coords = |v: usize| ((v % (nx + 1)) as i64, (v / (nx + 1)) as i64);
        let point = |v: usize| {
            let (i, j) = coords(v);
            Point { x: self.x0 + i as f64 * self.cell, y: self.y0 + j as f64 * self.cell }
        };
        let direction = |a: usize, b: usize| {
            let ((ai, aj), (bi, bj)) = (coords(a), coords(b));
            (bi - ai, bj - aj)
        };
        let mut rings: Vec<Vec<Point>> = Vec::new();
        for start in 0..out.len() {
            while out[start][0] != usize::MAX {
                let (mut ring, mut cur, mut incoming) = (Vec::new(), start, None::<(i64, i64)>);
                loop {
                    let slots = out[cur];
                    let pick = match incoming {
                        Some((dx, dy)) if slots[1] != usize::MAX => {
                            let turn = |s: usize| {
                                let (cx, cy) = direction(cur, slots[s]);
                                dx * cy - dy * cx
                            };
                            usize::from(turn(0) > turn(1))
                        }
                        _ => 0,
                    };
                    let next = slots[pick];
                    out[cur][pick] = out[cur][1 - pick];
                    out[cur][1] = usize::MAX;
                    let d = direction(cur, next);
                    if incoming != Some(d) {
                        ring.push(point(cur));
                    }
                    incoming = Some(d);
                    cur = next;
                    if cur == start {
                        break;
                    }
                }
                if ring.len() >= 3 {
                    rings.push(ring);
                }
            }
        }
        let simplified = rings.into_iter().map(|r| {
            let mut closed = r.clone();
            closed.push(r[0]);
            let mut s = g::simplify(&closed, simplify, |_| false);
            s.pop();
            s
        });
        let (outers, holes): (Vec<_>, Vec<_>) = simplified.filter(|r| r.len() >= 3 && g::signed(r) != 0.0).partition(|r| g::signed(r) > 0.0);
        let mut shapes: Vec<Shape> = outers.into_iter().map(|outer| Shape { outer, holes: vec![] }).collect();
        for h in holes {
            let holder = shapes
                .iter_mut()
                .filter(|s| g::contains(&Shape { outer: s.outer.clone(), holes: vec![] }, h[0]))
                .min_by(|a, b| g::signed(&a.outer).total_cmp(&g::signed(&b.outer)));
            if let Some(s) = holder {
                s.holes.push(h);
            }
        }
        shapes.into_iter().map(g::normalized).collect()
    }
}

/// Squared Euclidean distance (in cells) from each cell centre to the nearest set cell: Felzenszwalb and
/// Huttenlocher's exact transform, one pass per axis. Cells with none set get a value above `(nx + ny)^2`.
pub(crate) fn sq_distance(on: &[bool], nx: usize, ny: usize) -> Vec<f64> {
    let far = ((nx + ny) as f64).powi(2) * 2.0 + 1.0;
    let mut f: Vec<f64> = on.iter().map(|&b| if b { 0.0 } else { far }).collect();
    let n = nx.max(ny);
    let (mut line, mut d, mut v, mut z) = (vec![0.0; n], vec![0.0; n], vec![0_usize; n], vec![0.0; n + 1]);
    for i in 0..nx {
        for j in 0..ny {
            line[j] = f[j * nx + i];
        }
        transform_1d(&line[..ny], &mut d[..ny], &mut v, &mut z);
        for j in 0..ny {
            f[j * nx + i] = d[j];
        }
    }
    for j in 0..ny {
        line[..nx].copy_from_slice(&f[j * nx..(j + 1) * nx]);
        transform_1d(&line[..nx], &mut d[..nx], &mut v, &mut z);
        f[j * nx..(j + 1) * nx].copy_from_slice(&d[..nx]);
    }
    f
}

fn transform_1d(f: &[f64], d: &mut [f64], v: &mut [usize], z: &mut [f64]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let sq = |q: usize| (q * q) as f64;
    let mut k = 0;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        let mut s;
        loop {
            let p = v[k];
            s = ((f[q] + sq(q)) - (f[p] + sq(p))) / (2.0 * (q - p) as f64);
            if s > z[k] {
                break;
            }
            k -= 1;
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        let dq = q as f64 - p as f64;
        *out = dq * dq + f[p];
    }
}

/// The kept land (local frame) from footprints (local frame).
fn land(footprints: &[Shape], spec: &ClaimRecipe) -> Vec<Shape> {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in footprints.iter().flat_map(|s| &s.outer) {
        (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
    }
    if !(x0.is_finite() && y0.is_finite()) {
        return Vec::new();
    }
    let cell = spec.cell_m.value;
    let pad = ((spec.margin_m.value + spec.close_m.value) / cell).ceil() * cell + 3.0 * cell;
    // Cells anchored on whole multiples of the cell side, so the outline does not depend on where the crop starts.
    let (x0, y0) = ((x0 / cell).floor() * cell - pad, (y0 / cell).floor() * cell - pad);
    let nx = ((x1 + pad - x0) / cell).ceil() as usize + 1;
    let ny = ((y1 + pad - y0) / cell).ceil() as usize + 1;
    let mut raster = Raster { x0, y0, cell, nx, ny, on: vec![false; nx * ny] };
    for s in footprints {
        raster.fill(&s.outer);
    }
    raster.close(spec.margin_m.value, spec.close_m.value);
    raster.clean(spec.min_hole_m2.value, spec.min_land_m2.value);
    raster.outline(spec.simplify_m.value)
}

/// One exit before merging: its street path (inside end first) and its outward length (m).
struct Exit {
    path: Vec<Point>,
    out_m: f64,
}

/// The road texels of the site window around the land, in local coordinates.
struct RoadWindow {
    w: usize,
    h: usize,
    /// Cell centres (local frame).
    centres: Vec<Point>,
    road: Vec<bool>,
    inside: Vec<bool>,
}

impl RoadWindow {
    fn new(site: &SiteRaster, frame: &Frame, land: &[Shape], reach: f64) -> Option<Self> {
        let roads = site.roads.as_ref()?;
        let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in land.iter().flat_map(|s| &s.outer) {
            (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
        }
        if !x0.is_finite() {
            return None;
        }
        let [cols, rows] = site.meta.dims;
        let [west, north] = site.meta.origin_m;
        let c = site.meta.cell_m;
        // Local (x east, y south) to site columns and rows (row 0 = north).
        let [wx0, wy0] = frame.to_world(Point { x: x0 - reach, y: y0 - reach });
        let [wx1, wy1] = frame.to_world(Point { x: x1 + reach, y: y1 + reach });
        let col = |x: f64| ((x - west) / c).floor().clamp(0.0, cols as f64) as usize;
        let row = |y: f64| ((north - y) / c).floor().clamp(0.0, rows as f64) as usize;
        let (c0, c1) = (col(wx0), (col(wx1) + 1).min(cols));
        let (r0, r1) = (row(wy0), (row(wy1) + 1).min(rows));
        let (w, h) = (c1.saturating_sub(c0), r1.saturating_sub(r0));
        if w < 2 || h < 2 {
            return None;
        }
        let mut centres = Vec::with_capacity(w * h);
        let mut road = Vec::with_capacity(w * h);
        let mut inside = Vec::with_capacity(w * h);
        for r in r0..r1 {
            for col in c0..c1 {
                let p = frame.to_local(west + (col as f64 + 0.5) * c, north - (r as f64 + 0.5) * c);
                let is_road = roads[r * cols + col] != 0;
                centres.push(p);
                road.push(is_road);
                inside.push(is_road && land.iter().any(|s| g::contains(s, p)));
            }
        }
        Some(Self { w, h, centres, road, inside })
    }

    fn neighbours(&self, k: usize, diagonal: bool) -> impl Iterator<Item = usize> + '_ {
        let (i, j) = ((k % self.w) as i64, (k / self.w) as i64);
        let steps: &[(i64, i64)] = if diagonal {
            &[(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)]
        } else {
            &[(1, 0), (-1, 0), (0, 1), (0, -1)]
        };
        steps.iter().filter_map(move |(di, dj)| {
            let (a, b) = (i + di, j + dj);
            (a >= 0 && b >= 0 && (a as usize) < self.w && (b as usize) < self.h).then(|| b as usize * self.w + a as usize)
        })
    }

    /// Breadth-first along road cells where `allowed`, from `roots`, within `reach` (m) of `from`; returns the
    /// reached cell farthest from `from` (the first reached on a tie) and the path to it, root first.
    fn trace(&self, roots: &[usize], allowed: impl Fn(usize) -> bool, from: Point, reach: f64) -> Option<(Vec<usize>, f64)> {
        let mut parent = vec![usize::MAX; self.w * self.h];
        let mut queue = VecDeque::new();
        for &r in roots {
            if parent[r] == usize::MAX {
                parent[r] = r;
                queue.push_back(r);
            }
        }
        let mut best: Option<(f64, usize)> = None;
        while let Some(k) = queue.pop_front() {
            let d = self.centres[k].distance(from);
            if best.is_none_or(|(bd, _)| d > bd) {
                best = Some((d, k));
            }
            for n in self.neighbours(k, true) {
                if parent[n] == usize::MAX && allowed(n) && self.centres[n].distance(from) <= reach {
                    parent[n] = k;
                    queue.push_back(n);
                }
            }
        }
        let (d, mut k) = best?;
        let mut path = vec![k];
        while parent[k] != k {
            k = parent[k];
            path.push(k);
        }
        path.reverse();
        Some((path, d))
    }

    fn exits(&self, spec: &ClaimRecipe) -> Vec<Exit> {
        let n = self.w * self.h;
        let outside = |k: usize| self.road[k] && !self.inside[k];
        let gate: Vec<bool> = (0..n).map(|k| outside(k) && self.neighbours(k, false).any(|m| self.inside[m])).collect();
        let mut seen = vec![false; n];
        let mut found = Vec::new();
        for start in 0..n {
            if !gate[start] || seen[start] {
                continue;
            }
            seen[start] = true;
            let (mut stack, mut cluster) = (vec![start], Vec::new());
            while let Some(k) = stack.pop() {
                cluster.push(k);
                for m in self.neighbours(k, true) {
                    if gate[m] && !seen[m] {
                        seen[m] = true;
                        stack.push(m);
                    }
                }
            }
            cluster.sort_unstable();
            let inv = 1.0 / cluster.len() as f64;
            let from = Point {
                x: cluster.iter().map(|&k| self.centres[k].x).sum::<f64>() * inv,
                y: cluster.iter().map(|&k| self.centres[k].y).sum::<f64>() * inv,
            };
            let Some((out, out_m)) = self.trace(&cluster, outside, from, spec.exit_out_m.value) else { continue };
            if out_m < spec.exit_min_m.value {
                continue;
            }
            let mut inner: Vec<usize> = cluster.iter().flat_map(|&k| self.neighbours(k, false)).filter(|&m| self.inside[m]).collect();
            inner.sort_unstable();
            inner.dedup();
            let inward = self.trace(&inner, |k| self.inside[k], from, spec.exit_in_m.value).map(|(p, _)| p).unwrap_or_default();
            let cells: Vec<usize> = inward.into_iter().rev().chain(out).collect();
            let points: Vec<Point> = cells.iter().map(|&k| self.centres[k]).collect();
            let path = g::simplify(&points, spec.simplify_m.value, |_| false);
            if path.len() >= 2 {
                found.push(Exit { path, out_m });
            }
        }
        // Longest first; an exit whose outer end is near a kept one's is the same road.
        let mut order: Vec<usize> = (0..found.len()).collect();
        order.sort_by(|&a, &b| found[b].out_m.total_cmp(&found[a].out_m).then(a.cmp(&b)));
        let mut kept: Vec<usize> = Vec::new();
        for i in order {
            let end = *found[i].path.last().unwrap_or(&found[i].path[0]);
            if kept.iter().all(|&k| found[k].path.last().is_none_or(|e| e.distance(end) >= spec.exit_merge_m.value)) {
                kept.push(i);
            }
        }
        kept.sort_unstable();
        let mut found: Vec<Option<Exit>> = found.into_iter().map(Some).collect();
        kept.into_iter().filter_map(|i| found[i].take()).collect()
    }
}

/// The kept town of a site, if its `site.json` carries a claim with footprints.
///
/// # Errors
/// A malformed `claim`.
pub fn derive(site: &SiteRaster, frame: &Frame, spec: &ClaimRecipe) -> Result<Option<Kept>, String> {
    let Some(value) = &site.meta.claim else { return Ok(None) };
    let file: ClaimFile = serde_json::from_value(value.clone()).map_err(|e| format!("site.json claim: {e}"))?;
    let footprints: Vec<Shape> = file
        .footprints
        .iter()
        .filter(|f| f.polygon.len() >= 3)
        .map(|f| g::normalized(Shape { outer: f.polygon.iter().map(|&[x, y]| frame.to_local(x, y)).collect(), holes: vec![] }))
        .collect();
    if footprints.is_empty() {
        return Ok(None);
    }
    let land = land(&footprints, spec);
    let reach = spec.exit_out_m.value.max(spec.exit_in_m.value) + site.meta.cell_m;
    let streets = RoadWindow::new(site, frame, &land, reach)
        .map(|w| w.exits(spec))
        .unwrap_or_default()
        .into_iter()
        .map(|e| Street { widths: vec![spec.exit_width_m.value; e.path.len()], path: e.path, rank: EXIT_RANK, role: EXIT_ROLE.into() })
        .collect();
    let footprint_m2 = footprints.iter().map(g::area).sum();
    let land_m2 = land.iter().map(g::area).sum();
    Ok(Some(Kept { footprints, claim: Claim { key: KEPT_KEY.into(), land, streets, reserve: None }, footprint_m2, land_m2 }))
}

/// The exits (by index in `claim.streets`) some street of `streets` ends on, within `tolerance` (m).
#[must_use]
pub fn joined_exits(claim: &Claim, streets: &[Street], tolerance: f64) -> Vec<usize> {
    let ends: Vec<Point> = streets.iter().filter(|s| s.path.len() >= 2).flat_map(|s| [s.path[0], s.path[s.path.len() - 1]]).collect();
    claim
        .streets
        .iter()
        .enumerate()
        .filter(|(_, exit)| {
            exit.path.windows(2).any(|w| ends.iter().any(|&p| g::distance(p, w[0], w[1]).0 <= tolerance))
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{sq_distance, transform_1d};

    #[test]
    fn distance_transform_is_exact_on_a_small_grid() {
        let (nx, ny) = (7, 5);
        let mut on = vec![false; nx * ny];
        on[2 * nx + 3] = true;
        let d = sq_distance(&on, nx, ny);
        for j in 0..ny {
            for i in 0..nx {
                let (di, dj) = (i as f64 - 3.0, j as f64 - 2.0);
                assert!((d[j * nx + i] - (di * di + dj * dj)).abs() < 1e-9);
            }
        }
        let (mut out, mut v, mut z) = (vec![0.0; 3], vec![0; 3], vec![0.0; 4]);
        transform_1d(&[0.0, 9.0, 0.0], &mut out, &mut v, &mut z);
        assert_eq!(out, vec![0.0, 1.0, 0.0]);
    }
}
