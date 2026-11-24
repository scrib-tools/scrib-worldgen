//! ST-3e: an 80 m river in a gorge (cliff banks 20 m high, terraces between cliff bands, as on the 10x Odai), no
//! roads, arm B's recipe (`recipes/balmora-x10s.toml`). Without a regional road Magna Urbis's town never crosses it:
//! its own urban crossings are capped at the builders' span (about 21 m for 3000 people,
//! `urban/footprint/crossings.rs:133`) and the cliffs stop the rest, so the town stays on one bank. With the founding
//! crossing (`settle::crossing`, the road its region stage would give a river town) both banks get streets and lots
//! and the river a bridge.
// Fixture raster maths: sizes are small constants.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
use magna_urbis_core::urban::Point;
use settle::site::{SiteMeta, SiteRaster};
use settle::{Prepared, Recipe, crossing, prepare, run_culture};
use std::collections::BTreeMap;

mod common;

const CELL_M: f64 = 5.0;
const SIDE_CELLS: usize = 480;
const ORIGIN_M: [f64; 2] = [10_000.0, 20_000.0];
const RIVER_HALF_WIDTH_M: f64 = 40.0;
const WATER_LEVEL_M: f64 = 10.0;
const BED_DEPTH_M: f32 = 3.0;
/// Each bank: a cliff `CLIFF_M` high over `CLIFF_RUN_M`, a terrace `TERRACE_M` deep rising `TERRACE_SLOPE`, repeated.
const CLIFF_M: f32 = 20.0;
const CLIFF_RUN_M: f32 = 10.0;
const TERRACE_M: f32 = 120.0;
const TERRACE_SLOPE: f32 = 0.02;
/// The two cultures that stay on one bank without the crossing (korean crosses on its own here).
const CULTURES: [&str; 2] = ["european-organic", "medina"];

fn centre() -> [f64; 2] {
    let half = SIDE_CELLS as f64 * CELL_M / 2.0;
    [ORIGIN_M[0] + half, ORIGIN_M[1] - half]
}

/// Height above the water level `d` m from the bank.
fn ground(d: f32) -> f32 {
    let period = CLIFF_RUN_M + TERRACE_M;
    let k = (d / period).floor();
    let r = d - k * period;
    k * CLIFF_M + if r < CLIFF_RUN_M { CLIFF_M * r / CLIFF_RUN_M } else { CLIFF_M } + TERRACE_SLOPE * r
}

/// The fixture: a straight east-west river through `centre()` across the whole site.
fn gorge_site() -> SiteRaster {
    let [_, cy] = centre();
    let mut heights = Vec::new();
    let mut water = Vec::new();
    for row in 0..SIDE_CELLS {
        let y = ORIGIN_M[1] - (row as f64 + 0.5) * CELL_M;
        let off = (y - cy).abs();
        for _ in 0..SIDE_CELLS {
            let wet = off < RIVER_HALF_WIDTH_M;
            water.push(u8::from(wet));
            heights.push(if wet {
                WATER_LEVEL_M as f32 - BED_DEPTH_M
            } else {
                WATER_LEVEL_M as f32 + ground((off - RIVER_HALF_WIDTH_M) as f32)
            });
        }
    }
    SiteRaster {
        meta: SiteMeta {
            format: 1,
            name: "gorge fixture".into(),
            producer: "settle tests".into(),
            origin_m: ORIGIN_M,
            cell_m: CELL_M,
            dims: [SIDE_CELLS, SIDE_CELLS],
            water_level_m: WATER_LEVEL_M,
            centre_m: centre(),
            source: "synthetic: cliff-banked river, terraces".into(),
            claim: None,
            blake3: BTreeMap::new(),
        },
        heights,
        water,
        roads: None,
    }
}

fn recipe() -> Recipe {
    Recipe::parse(include_str!("../../../recipes/balmora-x10s.toml")).unwrap()
}

/// The river band in the local frame (y south from the crop's north edge).
fn band(prepared: &Prepared) -> (f64, f64) {
    let mid = prepared.grid.frame.north_m - centre()[1];
    (mid - RIVER_HALF_WIDTH_M, mid + RIVER_HALF_WIDTH_M)
}

/// Counts north of, south of and touching the band of shapes or polylines given by their points.
fn banks<'a>(items: impl Iterator<Item = &'a [Point]>, (lo, hi): (f64, f64)) -> [usize; 3] {
    let mut out = [0; 3];
    for points in items {
        let i = if points.iter().all(|p| p.y <= lo) {
            0
        } else if points.iter().all(|p| p.y >= hi) {
            1
        } else {
            2
        };
        out[i] += 1;
    }
    out
}

#[test]
fn without_a_crossing_the_gorge_keeps_the_town_on_one_bank() {
    let site = gorge_site();
    let mut recipe = recipe();
    recipe.crossing = None;
    let prepared = prepare(&site, &recipe).unwrap();
    assert!(prepared.roads.is_empty());
    for culture in CULTURES {
        let run = run_culture(&prepared, &recipe, culture).unwrap();
        let [north, south, _] = banks(run.partition.parcels.iter().map(|l| l.shape.outer.as_slice()), band(&prepared));
        eprintln!("{culture} without crossing: lots N {north} S {south}");
        assert!(north + south > 0, "{culture}: no lots");
        assert!(north == 0 || south == 0, "{culture}: lots on both banks without a crossing (N {north}, S {south})");
        assert_eq!(run.layout.streets.iter().filter(|s| s.role == "bridge").count(), 0, "{culture}: a bridge");
    }
}

#[test]
fn the_founding_crossing_spans_the_river_and_the_town_takes_both_banks() {
    let site = gorge_site();
    let recipe = recipe();
    let spec = recipe.crossing.as_ref().unwrap();
    let prepared = prepare(&site, &recipe).unwrap();
    let band = band(&prepared);
    let found = crossing::narrowest(&prepared.grid, prepared.centre, spec).unwrap();
    // Perpendicular to the river: the narrowest line, its span the river's width to the first dry sample.
    assert!((found.entry.x - found.exit.x).abs() < 1e-6, "oblique crossing {found:?}");
    assert!((2.0 * RIVER_HALF_WIDTH_M..=2.0 * RIVER_HALF_WIDTH_M + CELL_M).contains(&found.span_m), "{found:?}");
    assert_eq!(prepared.roads.len(), 1);
    let road = &prepared.roads[0];
    assert_eq!(road.role, crossing::REGIONAL_ROLE);
    let (a, b) = (road.path[0], road.path[1]);
    assert!(a.y.min(b.y) < band.0 && a.y.max(b.y) > band.1, "the road does not cross the river: {road:?}");

    for culture in CULTURES {
        let run = run_culture(&prepared, &recipe, culture).unwrap();
        let lots = banks(run.partition.parcels.iter().map(|l| l.shape.outer.as_slice()), band);
        let streets = banks(run.layout.streets.iter().map(|s| s.path.as_slice()), band);
        let bridges = run.layout.streets.iter().filter(|s| s.role == "bridge").count();
        eprintln!("{culture} with crossing: lots N/S/wet {lots:?}, streets {streets:?}, bridges {bridges}");
        assert!(lots[0] > 0 && lots[1] > 0, "{culture}: lots on one bank only {lots:?}");
        assert_eq!(lots[2], 0, "{culture}: lots touching the river");
        assert!(streets[0] > 0 && streets[1] > 0, "{culture}: streets on one bank only {streets:?}");
        assert!(bridges >= 1, "{culture}: no bridge");

        // PL-0 on arm B's recipe: lots.json contract and determinism for this culture.
        let text = settle::lots::lots_json(&settle::lots::lots(&site, &recipe.seed.value, &prepared, &run).file).unwrap();
        let with = common::check_lots(&text, &run.partition, culture);
        eprintln!("{culture}: {} lots, {with} with a footprint", run.partition.parcels.len());
        assert!(with > 0, "{culture}: no lot carries a building footprint");
        let again = settle::lots::lots_json(&settle::lots::lots(&site, &recipe.seed.value, &prepared, &run).file).unwrap();
        assert!(text == again, "{culture}: lots.json differs between two calls");
    }
}
