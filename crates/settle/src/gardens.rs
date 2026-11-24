//! Garden features (slice G1a): `placements.json` `features` entries of kind `garden`, so the game can plant trees
//! where the town has no house.
//!
//! **Temporary scaffold (owner 2026-11-24).** One garden per lot, the lot's outer ring as its polygon:
//! - a Magna Urbis `garden` lot → `mu_garden`;
//! - a `plot` lot no kit `building` instance went on (walls, towers and arches don't count; the members of a merged
//!   group count as built) → `empty_plot`, or `sliver` when its minimum-area oriented box has a short side under
//!   `[gardens] sliver_width_m`.
//!
//! Polygons are in the world (guide) frame like every `placements.json` coordinate (x east, y north, m; the lots
//! were converted from Magna Urbis's local frame by `crate::lots`), turned counter-clockwise. A lot with holes gives
//! its outer ring only (counted in `outer_ring_only`).
//!
//! Removal path: Magna Urbis's own land-use planting (gardens, orchards, yards drawn by the planner), flora garden
//! zones on the game side, and kit structures (sheds, stalls, wall gardens) on the slivers. Delete this module and
//! the `[gardens]` recipe table then.
use crate::fit::{BUILDING_ROLE, convex_hull, dot, signed_area, sub, unit};
use crate::formats::{Feature, Garden, GardenKind, GardenSource, Instance, PlacedLot};
use crate::lots::{Lot, Xy};
use crate::recipe::GardensRecipe;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Magna Urbis's lot uses this module reads.
const USE_GARDEN: &str = "garden";
const USE_PLOT: &str = "plot";

/// What planting gardens gives.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gardens {
    pub features: Vec<Feature>,
    /// Count and area (m², of the written polygon) per source.
    pub by_source: BTreeMap<GardenSource, (usize, f64)>,
    /// Lots with holes that gave their outer ring only.
    pub outer_ring_only: usize,
}

/// Short side (m) of the minimum-area oriented box of `points` (rotating calipers over the hull edges).
fn box_short_side(points: &[Xy]) -> f64 {
    let hull = convex_hull(points);
    let mut best: Option<(f64, f64)> = None;
    for i in 0..hull.len() {
        let Some(e) = unit(sub(hull[(i + 1) % hull.len()], hull[i])) else { continue };
        let f = [-e[1], e[0]];
        let span = |v: Xy| {
            let (lo, hi) = hull.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &p| (lo.min(dot(v, p)), hi.max(dot(v, p))));
            hi - lo
        };
        let (a, b) = (span(e), span(f));
        if best.is_none_or(|(area, _)| a * b < area) {
            best = Some((a * b, a.min(b)));
        }
    }
    best.map_or(0.0, |b| b.1)
}

/// The garden features of a culture's lots, in lot order, given what the fit placed (`instances`, `placed`).
#[must_use]
pub fn gardens(lots: &[Lot], instances: &[Instance], placed: &[PlacedLot], recipe: &GardensRecipe) -> Gardens {
    let mut out = Gardens::default();
    if !recipe.on() {
        return out;
    }
    let mut built: BTreeSet<&str> = instances.iter().filter(|i| i.role == BUILDING_ROLE).filter_map(|i| i.lot.as_deref()).collect();
    let members: Vec<&str> =
        placed.iter().filter(|p| built.contains(p.id.as_str())).flat_map(|p| p.merged_from.iter().map(String::as_str)).collect();
    built.extend(members);
    for lot in lots {
        let source = match lot.use_kind.as_str() {
            USE_GARDEN => GardenSource::MuGarden,
            USE_PLOT if !built.contains(lot.id.as_str()) => {
                if box_short_side(&lot.polygon) < recipe.sliver_width_m.value { GardenSource::Sliver } else { GardenSource::EmptyPlot }
            }
            _ => continue,
        };
        if lot.polygon.len() < 3 {
            continue;
        }
        let mut polygon = lot.polygon.clone();
        let area = signed_area(&polygon);
        if area < 0.0 {
            polygon.reverse();
        }
        if !lot.holes.is_empty() {
            out.outer_ring_only += 1;
        }
        let e = out.by_source.entry(source).or_default();
        e.0 += 1;
        e.1 += area.abs();
        out.features.push(Feature::Garden(Garden { kind: GardenKind::Garden, source, lot: lot.id.clone(), polygon }));
    }
    out
}

/// `stats.json` `gardens` block.
#[must_use]
pub fn stats(g: &Gardens) -> Value {
    let by_source: BTreeMap<&str, Value> =
        g.by_source.iter().map(|(s, (n, a))| (s.name(), json!({"count": n, "area_m2": a.round()}))).collect();
    json!({
        "features": g.features.len(),
        "area_m2": g.by_source.values().map(|v| v.1).sum::<f64>().round(),
        "by_source": by_source,
        "outer_ring_only": g.outer_ring_only,
    })
}
