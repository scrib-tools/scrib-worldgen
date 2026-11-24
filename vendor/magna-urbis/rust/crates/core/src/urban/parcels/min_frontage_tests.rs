//! A culture's declared smallest street front (`minFrontage`, scrib mu-patch 03):
//! no house lot is cut narrower, and without it the cut is unchanged.

use super::generate;
use crate::urban::geometry::rectangle;
use crate::urban::{Block, Layout, Morph, ParcelConfig, Point, Street};
use serde_json::json;

/// One 96 x 84 m block of a culture whose native witness house (EUR-MED) is
/// about 5 m wide, ringed by a public street, cut in 5-9 m burgage strips.
fn fixture() -> Layout {
    let mut layout: Layout = serde_json::from_value(json!({
        "key": "min-frontage-block", "culture": "mediterranean-modern",
        "center": {"x": 48.0, "y": 42.0},
        "quarters": [], "blocks": [], "warnings": [], "streets": []
    }))
    .unwrap();
    layout.blocks = vec![Block {
        phase: None,
        shape: rectangle(0.0, 0.0, 96.0, 84.0),
        quarter: 0,
        kind: "block".into(),
        axis: Point { x: 1.0, y: 0.0 },
        zone: "core".into(),
    }];
    layout.streets = vec![Street {
        path: vec![
            Point { x: -3.0, y: -3.0 },
            Point { x: 99.0, y: -3.0 },
            Point { x: 99.0, y: 87.0 },
            Point { x: -3.0, y: 87.0 },
            Point { x: -3.0, y: -3.0 },
        ],
        widths: vec![6.0; 5],
        rank: 2,
        role: "street".into(),
    }];
    layout
}

fn cut(min_frontage: Option<f64>) -> serde_json::Value {
    let mut value = json!({"plotOp":"burgage","streetOp":"organic","plotTilt":0.0,"wideLotChance":0.05,
        "lotMergeChance":0.1,
        "frontage":{"core":[5.0,9.0]},"plotDepth":{"core":[18.0,26.0]},
        "houseArea":{"core":[100.0,240.0]},"coverage":{"core":[0.8,0.9]}});
    if let Some(m) = min_frontage {
        value["minFrontage"] = json!(m);
    }
    let result = generate("min-frontage", &fixture(), &ParcelConfig::default(), &Morph { value }).unwrap();
    serde_json::to_value(result).unwrap()
}

/// Street fronts of the house lots (`plot`), sorted.
fn fronts(cut: &serde_json::Value) -> Vec<f64> {
    let mut f: Vec<f64> = cut["parcels"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["use"] == "plot")
        .map(|p| p["access"]["frontLength"].as_f64().unwrap_or(0.0))
        .collect();
    f.sort_by(f64::total_cmp);
    f
}

#[test]
fn min_frontage_floors_every_house_lot_front() {
    let native = fronts(&cut(None));
    assert!(native.len() > 10, "the fixture cuts a street of lots: {native:?}");
    assert!(native[0] < 8.4, "the fixture has lots under the floor without it: {native:?}");
    let floored = fronts(&cut(Some(8.4)));
    assert!(floored.len() > 5, "the floor still cuts lots: {floored:?}");
    assert!(floored[0] + 1e-6 >= 8.4, "a house lot under minFrontage: {floored:?}");
    assert!(floored.len() < native.len(), "wider lots, fewer of them");
}

#[test]
fn min_frontage_absent_zero_or_below_the_native_house_changes_nothing() {
    let native = cut(None);
    assert_eq!(cut(Some(0.0)), native);
    // Below the native witness front (EUR-MED's smallest house, ~5 m + 0.6 m).
    assert_eq!(cut(Some(1.0)), native);
}
