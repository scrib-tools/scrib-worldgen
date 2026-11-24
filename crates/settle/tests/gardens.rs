//! Slice G1a (temporary scaffold, owner 2026-11-24): garden features on Magna Urbis's garden lots and on the plot
//! lots no kit building went on. One garden lot, one built plot, one empty plot, one 3 m sliver: three features
//! with the right sources, world (guide) frame coordinates, counter-clockwise; switch off, none.
#![allow(clippy::float_cmp)] // exact assertions are the point
use settle::formats::{Feature, Garden, GardenKind, GardenSource, Instance};
use settle::gardens::{gardens, stats};
use settle::lots::{Lot, Xy};
use settle::recipe::GardensRecipe;

fn recipe(enabled: bool) -> GardensRecipe {
    toml::from_str(&format!(
        r#"
        enabled = {{ value = {enabled}, unit = "bool", source = "test" }}
        sliver_width_m = {{ value = 5.0, unit = "m", source = "test" }}
        "#
    ))
    .expect("gardens table parses")
}

fn lot(id: &str, use_kind: &str, polygon: Vec<Xy>) -> Lot {
    Lot {
        id: id.into(),
        use_kind: use_kind.into(),
        polygon,
        holes: Vec::new(),
        front: None,
        axis: [1.0, 0.0],
        access: None,
        role: None,
        function: None,
        wealth: None,
        floors: None,
        preview_parts: Vec::new(),
        ground_z_m: 0.0,
    }
}

fn rect(x0: f64, y0: f64, w: f64, d: f64) -> Vec<Xy> {
    vec![[x0, y0], [x0 + w, y0], [x0 + w, y0 + d], [x0, y0 + d]]
}

fn instance(lot: &str, role: &str) -> Instance {
    Instance { id: format!("i-{lot}-{role}"), kit: "k".into(), lot: Some(lot.into()), pos: [0.0, 0.0, 0.0], yaw_deg: 0.0, role: role.into(), door: None }
}

fn fixture() -> (Vec<Lot>, Vec<Instance>) {
    // The empty plot is clockwise (as a ring flipped from Magna Urbis's y-south frame would be): it comes out reversed.
    let mut empty = rect(1020.0, -480.0, 12.0, 15.0);
    empty.reverse();
    let lots = vec![
        lot("g1", "garden", rect(1000.0, -500.0, 10.0, 8.0)),
        lot("p-built", "plot", rect(1000.0, -480.0, 12.0, 15.0)),
        lot("p-empty", "plot", empty),
        lot("p-sliver", "plot", rect(1040.0, -480.0, 3.0, 15.0)),
        lot("r1", "reserve", rect(1060.0, -480.0, 20.0, 20.0)),
    ];
    // A wall on the empty plot does not make it built.
    let instances = vec![instance("p-built", "building"), instance("p-empty", "wall")];
    (lots, instances)
}

#[test]
fn three_gardens_with_sources_frame_and_winding() {
    let (lots, instances) = fixture();
    let g = gardens(&lots, &instances, &[], &recipe(true));
    let got: Vec<&Garden> = g.features.iter().map(|f| match f {
        Feature::Garden(g) => g,
        Feature::Grade(_) => panic!("only gardens"),
    }).collect();
    let ids: Vec<(&str, GardenSource)> = got.iter().map(|g| (g.lot.as_str(), g.source)).collect();
    assert_eq!(ids, [("g1", GardenSource::MuGarden), ("p-empty", GardenSource::EmptyPlot), ("p-sliver", GardenSource::Sliver)]);
    assert!(got.iter().all(|g| g.kind == GardenKind::Garden));
    assert_eq!(got[0].polygon, rect(1000.0, -500.0, 10.0, 8.0));
    assert_eq!(got[1].polygon, rect(1020.0, -480.0, 12.0, 15.0));
    assert_eq!(got[2].polygon, rect(1040.0, -480.0, 3.0, 15.0));
    for g in &got {
        let r = &g.polygon;
        let area: f64 = (0..r.len()).map(|i| r[i][0] * r[(i + 1) % r.len()][1] - r[(i + 1) % r.len()][0] * r[i][1]).sum::<f64>() / 2.0;
        assert!(area > 0.0, "{} is counter-clockwise", g.lot);
    }
    assert_eq!(g.by_source[&GardenSource::MuGarden], (1, 80.0));
    assert_eq!(g.by_source[&GardenSource::EmptyPlot], (1, 180.0));
    assert_eq!(g.by_source[&GardenSource::Sliver], (1, 45.0));
    let s = stats(&g);
    assert_eq!(s["features"], 3);
    assert_eq!(s["area_m2"], 305.0);
    assert_eq!(s["by_source"]["sliver"]["count"], 1);
}

#[test]
fn garden_entry_has_the_fixed_format_and_round_trips() {
    let (lots, instances) = fixture();
    let g = gardens(&lots, &instances, &[], &recipe(true));
    let json = serde_json::to_value(&g.features[2]).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"kind": "garden", "source": "sliver", "lot": "p-sliver",
            "polygon": [[1040.0, -480.0], [1043.0, -480.0], [1043.0, -465.0], [1040.0, -465.0]]})
    );
    let back: Feature = serde_json::from_value(json).unwrap();
    assert_eq!(back, g.features[2]);
}

#[test]
fn switch_off_adds_no_features() {
    let (lots, instances) = fixture();
    let g = gardens(&lots, &instances, &[], &recipe(false));
    assert!(g.features.is_empty());
    assert!(g.by_source.is_empty());
}
