//! ST-3b: the kept town as a Magna Urbis claim. A gentle slope, a straight river east of the centre, three clusters of
//! kept footprints (the centre on one of them) and one road leaving the central cluster westward: the kept land is
//! three pieces with one exit, no new block or lot overlaps the land, a new street ends on the exit, and one seed
//! gives one output.
// Fixture raster maths: sizes are small constants.
#![allow(clippy::cast_precision_loss)]
use magna_urbis_core::urban::Shape;
use magna_urbis_geom as g;
use serde_json::json;
use settle::claim::joined_exits;
use settle::run::layout_json;
use settle::site::{SiteMeta, SiteRaster};
use settle::{Recipe, prepare, run_culture};
use std::collections::BTreeMap;

const CELL_M: f64 = 2.5;
const SIDE_CELLS: usize = 800;
const ORIGIN_M: [f64; 2] = [10_000.0, 20_000.0];
const WATER_LEVEL_M: f64 = 10.0;
const BANK_M: f64 = 2.0;
const SLOPE: f64 = 0.02;
const RIVER_DX_M: f64 = 300.0;
const RIVER_HALF_WIDTH_M: f64 = 15.0;
/// Road from the centre west to this offset (m), this half width (m).
const ROAD_END_DX_M: f64 = -700.0;
const ROAD_HALF_WIDTH_M: f64 = 3.0;
/// Cluster centres (offsets from the site centre, m, y north); the first holds the centre and the road.
const CLUSTERS: [[f64; 2]; 3] = [[0.0, 0.0], [110.0, 90.0], [60.0, -120.0]];
/// Each cluster: 3 x 2 houses of `HOUSE_M`, on a `PITCH_M` grid, rows either side of the cluster's centre line.
const HOUSE_M: [f64; 2] = [10.0, 8.0];
const PITCH_M: [f64; 2] = [16.0, 18.0];
const CULTURE: &str = "european-organic";
/// Overlap (m²) under which a lot or block counts as clear of the land (float noise of Magna Urbis's clipping).
const OVERLAP_M2: f64 = 1e-6;

fn centre() -> [f64; 2] {
    let half = SIDE_CELLS as f64 * CELL_M / 2.0;
    [ORIGIN_M[0] + half, ORIGIN_M[1] - half]
}

fn footprints() -> serde_json::Value {
    let [cx, cy] = centre();
    let mut list = Vec::new();
    for (c, [ox, oy]) in CLUSTERS.iter().enumerate() {
        for col in 0..3_i32 {
            for row in [-1.0, 1.0] {
                let x = cx + ox + f64::from(col - 1) * PITCH_M[0];
                let y = cy + oy + row * PITCH_M[1] / 2.0;
                let [hw, hh] = HOUSE_M;
                let polygon = json!([[x - hw / 2.0, y - hh / 2.0], [x + hw / 2.0, y - hh / 2.0], [x + hw / 2.0, y + hh / 2.0], [x - hw / 2.0, y + hh / 2.0]]);
                list.push(json!({ "id": format!("c{c}-{col}-{row}"), "kind": "building", "polygon": polygon }));
            }
        }
    }
    json!({ "footprints": list })
}

fn site() -> SiteRaster {
    let [cx, cy] = centre();
    let n = SIDE_CELLS * SIDE_CELLS;
    let (mut heights, mut water, mut roads) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
    for row in 0..SIDE_CELLS {
        let dy = ORIGIN_M[1] - (row as f64 + 0.5) * CELL_M - cy;
        for col in 0..SIDE_CELLS {
            let dx = ORIGIN_M[0] + (col as f64 + 0.5) * CELL_M - cx;
            let wet = (dx - RIVER_DX_M).abs() < RIVER_HALF_WIDTH_M;
            let ground = WATER_LEVEL_M + BANK_M + SLOPE * (dx + 1000.0);
            #[allow(clippy::cast_possible_truncation)]
            heights.push(if wet { WATER_LEVEL_M - 1.5 } else { ground } as f32);
            water.push(u8::from(wet));
            roads.push(u8::from(dx <= 0.0 && dx > ROAD_END_DX_M && dy.abs() < ROAD_HALF_WIDTH_M));
        }
    }
    SiteRaster {
        meta: SiteMeta {
            format: 1,
            name: "claim fixture".into(),
            producer: "settle tests".into(),
            origin_m: ORIGIN_M,
            cell_m: CELL_M,
            dims: [SIDE_CELLS, SIDE_CELLS],
            water_level_m: WATER_LEVEL_M,
            centre_m: centre(),
            source: "synthetic: slope + river + three kept clusters + one road".into(),
            claim: Some(footprints()),
            blake3: BTreeMap::new(),
        },
        heights,
        water,
        roads: Some(roads),
    }
}

fn recipe() -> Recipe {
    let mut recipe = Recipe::parse(include_str!("../../../recipes/example.toml")).unwrap();
    recipe.cultures.value = vec![CULTURE.into()];
    recipe.validate().unwrap();
    recipe
}

fn overlap(a: &Shape, land: &[Shape]) -> f64 {
    land.iter().flat_map(|l| g::intersection(a, l)).map(|s| g::area(&s)).sum()
}

#[test]
fn new_town_grows_around_the_kept_land_and_joins_its_exit() {
    let (site, recipe) = (site(), recipe());
    let prepared = prepare(&site, &recipe).unwrap();
    let kept = prepared.kept.as_ref().expect("the site carries a claim");
    assert_eq!(kept.footprints.len(), 18);
    assert_eq!(kept.claim.land.len(), 3, "three clusters, three pieces of land");
    assert_eq!(kept.claim.streets.len(), 1, "one road leaves the kept land");

    let run = run_culture(&prepared, &recipe, CULTURE).unwrap();
    let land = &kept.claim.land;
    assert!(!run.partition.parcels.is_empty(), "the new town has lots");
    for block in &run.layout.blocks {
        assert!(overlap(&block.shape, land) <= OVERLAP_M2, "a block overlaps the kept land");
    }
    for lot in &run.partition.parcels {
        assert!(overlap(&lot.shape, land) <= OVERLAP_M2, "a lot overlaps the kept land");
    }
    let tolerance = recipe.claim.as_ref().unwrap().join_tolerance_m.value;
    assert_eq!(joined_exits(&kept.claim, &run.layout.streets, tolerance), vec![0], "a new street ends on the exit");

    let again = run_culture(&prepared, &recipe, CULTURE).unwrap();
    assert_eq!(layout_json(&site, &recipe, &prepared, &run).unwrap(), layout_json(&site, &recipe, &prepared, &again).unwrap());
}
