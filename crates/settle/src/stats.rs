//! `stats.json`: lot count (total and by use), street length by rank, built area, lots with a building footprint,
//! run time (with Magna Urbis's building-request call, `requests_ms`), the fit (`fit`, slice PL-2: the first pass)
//! when one ran, the merge (`merge`, slice PL-3a) and the street wall's filler (`filler`, slice PL-3b) when the recipe
//! has them.
use crate::claim;
use crate::filler::FillerStats;
use crate::fit::FitStats;
use crate::lots::Lots;
use crate::merge::MergeStats;
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
pub fn stats(recipe: &Recipe, prepared: &Prepared, run: &CultureRun, lots: &Lots) -> Value {
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
        "lots_with_preview": lots.with_preview(),
        "requests_error": lots.file.error,
        "requests_ms": lots.requests_ms.round(),
        "run_time_s": {
            "quarters": run.quarters_s,
            "parcels": run.parcels_s,
            "total": run.quarters_s + run.parcels_s,
        },
    })
}

/// The `fit` entry of `stats.json` (slice PL-2): lots tried, filled, empty by reason, instances, models, time.
#[must_use]
pub fn fit(s: &FitStats) -> Value {
    let mut v = json!({
        "lots": s.lots,
        "lots_tried": s.tried,
        "lots_filled": s.filled,
        "lots_without_footprint": s.no_footprint,
        "lots_empty_by_reason": s.empty_by_reason,
        "parts_tried": s.parts_tried,
        "parts_filled": s.parts_filled,
        "instances": s.instances,
        "candidates": s.candidates,
        "distinct_models": s.distinct_models,
        "filled_by_role": s.by_role.iter().map(|(k, [t, f])| (k.clone(), json!({"tried": t, "filled": f}))).collect::<BTreeMap<_, _>>(),
        "fit_ms": s.fit_ms.round(),
    });
    // `lot` domain only (the footprint domain's entry is unchanged): lots filled by what they had.
    if s.lot_domain {
        v["domain"] = json!("lot");
        v["lots_filled_by_domain"] = json!({
            "with_footprint": s.filled - s.filled_without_footprint,
            "without_footprint": s.filled_without_footprint,
        });
        v["lots_tried_without_footprint"] = json!(s.tried_without_footprint);
    }
    // `match` domain only: building footprints matched, the rest left alone, the matches' IoU.
    if s.match_domain {
        v["domain"] = json!("match");
        v["building_parts"] = json!(s.building_parts);
        v["parts_skipped_by_label"] = json!(s.parts_skipped_by_label);
        v["parts_empty_by_reason"] = json!(s.parts_empty_by_reason);
        v["parts_filled_by_stage"] = json!(s.parts_filled_by_stage);
        v["match_iou_p10_p50_p90"] = json!(s.match_iou);
        if s.sink_back_rise_m.is_some() {
            v["sink_back_rise_m_p50_max"] = json!(s.sink_back_rise_m);
        }
        if s.small_candidates > 0 {
            v["small_candidates"] = json!(s.small_candidates);
        }
    }
    v
}

/// The `merge` entry of `stats.json` (slice PL-3a): candidates, groups, lots merged, filled before and after, still
/// empty by reason, time.
#[must_use]
pub fn merge(s: &MergeStats) -> Value {
    json!({
        "candidates": s.candidates,
        "lots_with_neighbour": s.lots_with_neighbour,
        "runs": s.runs,
        "groups": s.groups,
        "groups_by_size": s.groups_by_size.iter().map(|(k, v)| (k.to_string(), *v)).collect::<BTreeMap<_, _>>(),
        "lots_merged": s.lots_merged,
        "lots_filled_before": s.lots_filled_before,
        "lots_filled_after": s.lots_filled_after,
        "instances_added": s.instances_added,
        "still_empty_by_reason": s.still_empty_by_reason,
        "merge_ms": s.merge_ms.round(),
    })
}

/// The `filler` entry of `stats.json` (slice PL-3b): frontage, what covers it (m, to the cm), coverage shares before
/// and after, pieces placed and skipped, time.
#[must_use]
pub fn filler(s: &FillerStats) -> Value {
    let cm = |v: f64| (v * 100.0).round() / 100.0;
    let share = |v: f64| (v * 1000.0).round() / 1000.0;
    json!({
        "runs": s.runs,
        "faces_m": cm(s.faces_m),
        "frontage_m": cm(s.frontage_m),
        "buildings_m": cm(s.buildings_m),
        "walls_m": cm(s.walls_m),
        "towers_m": cm(s.towers_m),
        "arches_m": cm(s.arches_m),
        "open_m": cm(s.open_m),
        "coverage_before": share(s.coverage_before),
        "coverage": share(s.coverage),
        "gaps": s.gaps,
        "walls": s.walls,
        "arches": s.arches,
        "towers": s.towers,
        "mouths": s.mouths,
        "lots_filler": s.lots_filler,
        "skipped_by_reason": s.skipped_by_reason,
        "filler_ms": s.filler_ms.round(),
    })
}
