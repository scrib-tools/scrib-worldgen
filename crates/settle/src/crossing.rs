//! The town's founding crossing: the regional road a river town is founded on.
//!
//! Magna Urbis's region stage hands every town its regional roads, split at their river crossings
//! (`rust/bridge/urbanRoads.ts:6` `regionalStreets`), and its town stage spans a river only through the crossing it
//! was founded on: "Water stays a barrier except along existing regional crossings" (`urban/footprint/growth.rs:219`);
//! its own urban crossings are capped at the builders' span, about 21 m for 3000 people
//! (`urban/footprint/crossings.rs:123-133`) and need bank approaches within the street grade. A site without a
//! claim brings no road, so a town on a cliff-banked river stays on one bank (ST-3e). This module gives such a site
//! the regional road Magna Urbis's region stage would: straight across the narrowest water near the centre, on
//! through both banks. Magna Urbis fits the deck and its approaches itself (`generate_quarters` runs
//! `network::bridge_water` on the roads, `urban/mod.rs:113`).
// Grid indices and marching steps: values are bounded by the grid side.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
use crate::claim::sq_distance;
use crate::grid::Grid;
use crate::recipe::CrossingRecipe;
use magna_urbis_core::urban::{Point, Street};

/// Magna Urbis's role of a regional road (`rust/bridge/urbanRoads.ts:52`).
pub const REGIONAL_ROLE: &str = "regional";

/// The narrowest water crossing found near the centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    /// Last wet and first dry point on each side, local frame.
    pub entry: Point,
    pub exit: Point,
    /// Water crossed along the line (m).
    pub span_m: f64,
}

impl Crossing {
    #[must_use]
    pub fn middle(&self) -> Point {
        Point { x: f64::midpoint(self.entry.x, self.exit.x), y: f64::midpoint(self.entry.y, self.exit.y) }
    }
}

fn wet_at(grid: &Grid, p: Point) -> Option<bool> {
    let n = grid.frame.n;
    let cell = grid.frame.width_m / n as f64;
    if p.x < 0.0 || p.y < 0.0 || p.x >= grid.frame.width_m || p.y >= grid.frame.width_m {
        return None;
    }
    let (col, row) = ((p.x / cell) as usize, (p.y / cell) as usize);
    Some(grid.water[row.min(n - 1) * n + col.min(n - 1)] != 0)
}

/// From wet `p`, march along `(ux, uy)` until dry: the first dry point and the distance to it; `None` if the water
/// runs past `limit` m or off the crop (no far bank inside it).
fn bank(grid: &Grid, p: Point, (ux, uy): (f64, f64), step: f64, limit: f64) -> Option<(Point, f64)> {
    let mut t = step;
    while t <= limit {
        let q = Point { x: p.x + ux * t, y: p.y + uy * t };
        if !wet_at(grid, q)? {
            return Some((q, t));
        }
        t += step;
    }
    None
}

/// The channel's middle nearest `centre`: from the wet cell nearest it (within `search_m`, else `None`), steepest
/// ascent on the distance to dry ground to a local maximum, the medial axis (as the site export picks `centre_m`).
fn medial_point(grid: &Grid, centre: Point, search_m: f64) -> Option<usize> {
    let n = grid.frame.n;
    let cell = grid.frame.width_m / n as f64;
    let at = |i: usize| Point { x: ((i % n) as f64 + 0.5) * cell, y: ((i / n) as f64 + 0.5) * cell };
    let start = (0..n * n)
        .filter(|&i| grid.water[i] != 0)
        .map(|i| (at(i).distance(centre), i))
        .filter(|&(d, _)| d <= search_m)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))?
        .1;
    let dry: Vec<bool> = grid.water.iter().map(|&w| w == 0).collect();
    let depth = sq_distance(&dry, n, n);
    let mut i = start;
    loop {
        let (x, y) = (i % n, i / n);
        let next = (y.saturating_sub(1)..=(y + 1).min(n - 1))
            .flat_map(|cy| (x.saturating_sub(1)..=(x + 1).min(n - 1)).map(move |cx| cy * n + cx))
            .max_by(|&a, &b| depth[a].total_cmp(&depth[b]).then(b.cmp(&a)))?;
        if depth[next] <= depth[i] {
            return Some(i);
        }
        i = next;
    }
}

/// The narrowest line (`spec.max_span_m` of water at most, both banks inside the crop) through the channel's middle
/// nearest `centre`, lines tried every `spec.angle_step_deg`.
#[must_use]
pub fn narrowest(grid: &Grid, centre: Point, spec: &CrossingRecipe) -> Option<Crossing> {
    let side = grid.frame.n;
    let cell = grid.frame.width_m / side as f64;
    let medial = medial_point(grid, centre, spec.search_m.value)?;
    let middle = Point { x: ((medial % side) as f64 + 0.5) * cell, y: ((medial / side) as f64 + 0.5) * cell };
    let (step, limit) = (cell * 0.5, spec.max_span_m.value);
    let lines = (180.0 / spec.angle_step_deg.value).ceil().max(1.0) as usize;
    let mut best: Option<Crossing> = None;
    for line in 0..lines {
        let angle = (line as f64 * spec.angle_step_deg.value).to_radians();
        let dir = (angle.cos(), angle.sin());
        let Some((exit, ahead)) = bank(grid, middle, dir, step, limit) else { continue };
        let Some((entry, behind)) = bank(grid, middle, (-dir.0, -dir.1), step, limit - ahead) else { continue };
        if best.is_none_or(|b| ahead + behind < b.span_m) {
            best = Some(Crossing { entry, exit, span_m: ahead + behind });
        }
    }
    best
}

/// The founding crossing as Magna Urbis's regional road: a straight street through the crossing's middle,
/// `spec.reach_m` each way, clipped to the crop less one cell.
#[must_use]
pub fn regional_road(grid: &Grid, crossing: &Crossing, spec: &CrossingRecipe) -> Street {
    let m = crossing.middle();
    let len = crossing.entry.distance(crossing.exit).max(1e-9);
    let u = ((crossing.exit.x - crossing.entry.x) / len, (crossing.exit.y - crossing.entry.y) / len);
    let margin = grid.frame.width_m / grid.frame.n as f64;
    let hi = grid.frame.width_m - margin;
    // Largest t keeping m + t u inside [margin, hi] on both axes.
    let reach = |s: f64| {
        let along = |c: f64, v: f64| if v * s > 0.0 { (hi - c) / (v * s) } else if v * s < 0.0 { (margin - c) / (v * s) } else { f64::INFINITY };
        spec.reach_m.value.min(along(m.x, u.0)).min(along(m.y, u.1)).max(0.0)
    };
    let (ta, tb) = (reach(-1.0), reach(1.0));
    let a = Point { x: m.x - u.0 * ta, y: m.y - u.1 * ta };
    let b = Point { x: m.x + u.0 * tb, y: m.y + u.1 * tb };
    Street { path: vec![a, b], widths: vec![spec.width_m.value; 2], rank: spec.rank.value, role: REGIONAL_ROLE.into() }
}

/// The site's regional roads: its founding crossing, when the recipe asks for one and water lies near the centre.
#[must_use]
pub fn roads(grid: &Grid, centre: Point, spec: Option<&CrossingRecipe>) -> Vec<Street> {
    spec.and_then(|s| narrowest(grid, centre, s).map(|c| regional_road(grid, &c, s))).into_iter().collect()
}
