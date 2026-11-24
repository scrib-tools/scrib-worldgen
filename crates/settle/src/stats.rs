//! `stats.json`: lot count (total and by use), street length by rank, built area, run time.
use crate::claim;
use crate::recipe::Recipe;
use crate::run::{CultureRun, Prepared};
use magna_urbis_core::urban::{Point, Shape};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Signed area (m²) of a closed ring (shoelace).
fn ring_area(ring: &[Point]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

/// Area (m²) of a shape: outer ring less its holes.
#[must_use]
pub fn shape_area(shape: &Shape) -> f64 {
    ring_area(&shape.outer).abs() - shape.holes.iter().map(|h| ring_area(h).abs()).sum::<f64>()
}

/// Length (m) of a polyline.
#[must_use]
pub fn path_length(path: &[Point]) -> f64 {
    path.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// The stats of one run. `built_area_m2.new` is the land cut into lots (building footprints come at massing, a later
/// slice); `built_area_m2.kept` is the kept town's land (its claim), 0 without one; `ratio` is new over kept.
#[must_use]
pub fn stats(recipe: &Recipe, prepared: &Prepared, run: &CultureRun) -> Value {
    let mut by_use: BTreeMap<&str, (usize, f64)> = BTreeMap::new();
    for lot in &run.partition.parcels {
        let entry = by_use.entry(lot.use_kind.as_str()).or_default();
        entry.0 += 1;
        entry.1 += shape_area(&lot.shape);
    }
    let mut by_rank: BTreeMap<usize, f64> = BTreeMap::new();
    for street in &run.layout.streets {
        *by_rank.entry(street.rank).or_default() += path_length(&street.path);
    }
    let lot_area: f64 = by_use.values().map(|(_, a)| a).sum();
    let block_area: f64 = run.layout.blocks.iter().map(|b| shape_area(&b.shape)).sum();
    let kept_land = prepared.kept.as_ref().map_or(0.0, |k| k.land_m2);
    let kept = prepared.kept.as_ref().map(|k| {
        let tolerance = recipe.claim.as_ref().map_or(0.0, |c| c.join_tolerance_m.value);
        let joined = claim::joined_exits(&k.claim, &run.layout.streets, tolerance);
        let centre = run.layout.center;
        json!({
            "footprints": k.footprints.len(),
            "footprint_area_m2": k.footprint_m2.round(),
            "land_pieces": k.claim.land.len(),
            "land_area_m2": k.land_m2.round(),
            "exits": k.claim.streets.len(),
            "exits_joined": joined.len(),
            "centre_moved_m": centre.distance(prepared.centre).round(),
        })
    });
    json!({
        "culture": run.culture,
        "population": recipe.population.value,
        "seed": recipe.seed.value,
        "site_radius_m": run.site_radius_m,
        "lots": run.partition.parcels.len(),
        "lots_by_use": by_use.iter().map(|(k, (n, a))| ((*k).to_string(), json!({"count": n, "area_m2": a.round()}))).collect::<BTreeMap<_, _>>(),
        "streets": run.layout.streets.len(),
        "street_length_m_by_rank": by_rank.iter().map(|(k, v)| (k.to_string(), v.round())).collect::<BTreeMap<_, _>>(),
        "street_length_m": by_rank.values().sum::<f64>().round(),
        "regional_roads": prepared.roads.len(),
        "bridges": run.layout.streets.iter().filter(|s| s.role == "bridge").count(),
        "blocks": run.layout.blocks.len(),
        "block_area_m2": block_area.round(),
        "built_area_m2": {
            "new": lot_area.round(),
            "kept": kept_land.round(),
            "ratio": if kept_land > 0.0 { (lot_area / kept_land * 100.0).round() / 100.0 } else { 0.0 },
        },
        "kept": kept,
        "warnings": run.layout.warnings.len() + run.partition.warnings.len(),
        "run_time_s": {
            "quarters": run.quarters_s,
            "parcels": run.parcels_s,
            "total": run.quarters_s + run.parcels_s,
        },
    })
}
