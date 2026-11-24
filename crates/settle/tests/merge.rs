//! Slice PL-3a: merging narrow empty lots along their street. A row of 6 m-front lots on a straight street (street
//! south, along +x) and a kit whose narrowest house is 10 m wide: pairs merge under one house each, a lot behind the
//! row (another frontage) and a filled lot are never absorbed, byte-identical output per seed.
#![allow(clippy::float_cmp)] // exact assertions are the point
#![allow(clippy::many_single_char_names)] // a, b, f, s, k as in tests/fit.rs
use settle::fit::Header;
use settle::formats::{KitFile, LotStatus, PLACEMENTS_FILE, PlacedLot};
use settle::lots::{Access, Lot, Part, Xy};
use settle::merge::{MergeStats, domain, fit_and_merge, shared_boundary};
use settle::recipe::{FitRecipe, MergeRecipe};
use std::fs;
use std::path::Path;

/// Two buildings, door on the front (+y local): 10 x 8 (the narrowest) and 14 x 8.
const KIT: &str = r#"{
  "format": 1, "name": "row", "producer": "test", "units": "m",
  "frame": "local x right, y front (door side), z up; pivot at model origin",
  "models": [
    { "id": "house_10x8", "kind": "building", "footprint": [[-5,-4],[5,-4],[5,4],[-5,4]],
      "doors": [ { "at": [0,4], "facing_deg": 90, "width_m": 1.4, "sill_z_m": 0.2 } ],
      "height_m": 7.5, "base_offset_m": -0.4, "weight": 1.0, "blake3": "aa" },
    { "id": "house_14x8", "kind": "building", "footprint": [[-7,-4],[7,-4],[7,4],[-7,4]],
      "doors": [ { "at": [0,4], "facing_deg": 90, "width_m": 1.6, "sill_z_m": 0.2 } ],
      "height_m": 9, "base_offset_m": -0.5, "weight": 0.5, "blake3": "bb" }
  ] }"#;

const SEED: &str = "merge-fixture";
/// The row's outward street normal (the lots stand north of the street): -y.
const STREET_NORMAL_DEG: f64 = 270.0;

fn kit() -> KitFile {
    KitFile::from_json(KIT).expect("synthetic kit parses")
}

fn recipes() -> (FitRecipe, MergeRecipe) {
    let r = settle::Recipe::parse(include_str!("../../../recipes/balmora-x10s.toml")).unwrap();
    (r.fit.expect("balmora-x10s has a [fit] table"), r.merge.expect("balmora-x10s has a [merge] table"))
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Xy> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

/// A lot `x0..x0 + w` along a street at `y = street` (`north`: the lot stands north of it), `d` deep, its one
/// footprint the whole lot.
fn lot_on(id: &str, x0: f64, w: f64, d: f64, street: f64, north: bool) -> Lot {
    let (y0, y1) = if north { (street, street + d) } else { (street - d, street) };
    let polygon = rect(x0, y0, x0 + w, y1);
    let front = [[x0, street], [x0 + w, street]];
    Lot {
        id: id.into(),
        use_kind: "plot".into(),
        polygon: polygon.clone(),
        holes: vec![],
        front: Some(front),
        axis: [1.0, 0.0],
        access: Some(Access { kind: "public".into(), segment: front, width_m: w }),
        role: Some("dwelling.house".into()),
        function: Some("domestic".into()),
        wealth: Some(0.5),
        floors: Some([2, 2]),
        preview_parts: vec![Part { label: "building".into(), polygon, holes: vec![] }],
        ground_z_m: 0.0,
    }
}

/// A 6 m by 10 m lot north of the street `y = 0`.
fn narrow(id: &str, x0: f64) -> Lot {
    lot_on(id, x0, 6.0, 10.0, 0.0, true)
}

/// Four 6 m lots in a row.
fn row() -> Vec<Lot> {
    (0..4).map(|i| narrow(&format!("lot-{i}"), 6.0 * f64::from(i))).collect()
}

fn flat(_: f64, _: f64) -> f64 {
    0.0
}

fn bounds(h: &[Xy]) -> [f64; 4] {
    h.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, p| {
        [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
    })
}

fn placed<'a>(lots: &'a [PlacedLot], id: &str) -> Option<&'a PlacedLot> {
    lots.iter().find(|l| l.id == id)
}

fn run(lots: &[Lot]) -> (settle::fit::Fitted, MergeStats) {
    let (k, (f, m)) = (kit(), recipes());
    let (fitted, stats) = fit_and_merge(lots, &k, &f, Some(&m), SEED, &flat);
    (fitted, stats.expect("a merge recipe gives merge stats"))
}

#[test]
fn shared_boundary_counts_party_walls_only() {
    let (a, b) = (rect(0.0, 0.0, 6.0, 10.0), rect(6.0, 0.0, 12.0, 10.0));
    assert_eq!(shared_boundary(&a, &b, 0.1), 10.0);
    // A T-junction: the shorter neighbour shares its own side.
    assert_eq!(shared_boundary(&a, &rect(6.0, 2.0, 9.0, 7.0), 0.1), 5.0);
    // A corner touch shares nothing; a lot 0.5 m away neither.
    assert_eq!(shared_boundary(&a, &rect(6.0, 10.0, 9.0, 13.0), 0.1), 0.0);
    assert_eq!(shared_boundary(&a, &rect(6.5, 0.0, 12.0, 10.0), 0.1), 0.0);
}

#[test]
fn domain_closes_the_side_strips_and_keeps_the_building_band() {
    // Footprints 1 m in from each side, 2 m behind the street, 3 m before the back of a 6 x 14 lot.
    let set_back = |id: &str, x0: f64| {
        let mut l = lot_on(id, x0, 6.0, 14.0, 0.0, true);
        l.preview_parts[0].polygon = rect(x0 + 1.0, 2.0, x0 + 5.0, 11.0);
        l
    };
    let (a, b) = (set_back("a", 0.0), set_back("b", 6.0));
    let d = domain(&[&a, &b], [0.0, -1.0]).expect("a joined domain");
    assert!(d.holes.is_empty());
    assert_eq!(bounds(&d.polygon), [0.0, 2.0, 12.0, 11.0], "{:?}", d.polygon);
    let area = (0..d.polygon.len())
        .map(|i| {
            let (p, q) = (d.polygon[i], d.polygon[(i + 1) % d.polygon.len()]);
            p[0] * q[1] - q[0] * p[1]
        })
        .sum::<f64>()
        .abs()
        / 2.0;
    assert!((area - 108.0).abs() < 1e-6, "{area}");
}

#[test]
fn four_narrow_lots_make_two_merged_houses() {
    let (k, (fr, _)) = (kit(), recipes());
    let lots = row();
    let (f, s) = run(&lots);
    // Every 6 m lot is too narrow alone.
    assert_eq!((s.candidates, s.lots_filled_before), (4, 0));
    assert_eq!((s.groups, s.lots_merged, s.lots_filled_after, s.instances_added), (2, 4, 4, 2));
    assert!(s.still_empty_by_reason.is_empty(), "{:?}", s.still_empty_by_reason);
    assert_eq!(f.lots.len(), 2);
    assert!(f.empty.is_empty());
    let tol = fr.inside_tolerance_m.value;
    let mut hulls = Vec::new();
    for (g, members) in [("lot-0", ["lot-0", "lot-1"]), ("lot-2", ["lot-2", "lot-3"])] {
        let pl = placed(&f.lots, g).unwrap();
        assert_eq!(pl.status, LotStatus::Merged);
        assert_eq!(pl.merged_from, members);
        assert_eq!(pl.pad_z_m, Some(0.0));
        let x0 = if g == "lot-0" { 0.0 } else { 12.0 };
        assert_eq!(bounds(&pl.polygon), [x0, 0.0, x0 + 12.0, 10.0]);
        assert_eq!(pl.front, Some([[x0, 0.0], [x0 + 12.0, 0.0]]));
        let on: Vec<_> = f.instances.iter().filter(|i| i.lot.as_deref() == Some(g)).collect();
        assert_eq!(on.len(), 1, "one building per group");
        let inst = on[0];
        // 14 m is wider than the 12 m group.
        assert_eq!(inst.kit, "house_10x8");
        let model = k.model(&inst.kit).unwrap();
        let hull = model.world_hull([inst.pos[0], inst.pos[1]], inst.yaw_deg);
        // Inside the merged domain, and across the members' shared boundary.
        for p in &hull {
            assert!(p[0] >= x0 - tol && p[0] <= x0 + 12.0 + tol && p[1] >= -tol && p[1] <= 10.0 + tol, "{g} {p:?}");
        }
        let b = bounds(&hull);
        assert!(b[0] < x0 + 6.0 && b[2] > x0 + 6.0, "{g} spans both lots: {b:?}");
        // The street door faces the street, on the front edge.
        let door = model.world_doors([inst.pos[0], inst.pos[1]], inst.yaw_deg)[0];
        let off = (door.facing_deg - STREET_NORMAL_DEG).abs();
        assert!(off.min(360.0 - off) < 1e-9, "{g} faces {}", door.facing_deg);
        assert_eq!(inst.door, Some(door.at));
        assert!(door.at[1].abs() < 1e-9, "{g} door at {:?}", door.at);
        hulls.push(b);
    }
    let [a, b] = [hulls[0], hulls[1]];
    assert!(a[2] <= b[0] + tol || b[2] <= a[0] + tol, "{a:?} overlaps {b:?}");
}

#[test]
fn a_lot_behind_the_row_is_never_merged_with_it() {
    let mut lots = row();
    // Behind lot-0, sharing its whole back edge, facing the street to the north (y = 20).
    lots.push(lot_on("lot-behind", 0.0, 6.0, 10.0, 20.0, false));
    let (f, s) = run(&lots);
    assert_eq!(s.candidates, 5);
    assert_eq!(s.groups, 2);
    let behind = placed(&f.lots, "lot-behind").unwrap();
    assert_eq!(behind.status, LotStatus::Empty);
    assert!(f.lots.iter().all(|l| !l.merged_from.iter().any(|m| m == "lot-behind")));
    assert!(f.instances.iter().all(|i| i.lot.as_deref() != Some("lot-behind")));
    assert_eq!(s.still_empty_by_reason.get("no_neighbour"), Some(&1));
}

#[test]
fn a_filled_lot_is_never_absorbed() {
    // lot-0, lot-1 (6 m), lot-2 (12 m: a 10 m house fits it alone), lot-3, lot-4 (6 m).
    let lots = vec![
        narrow("lot-0", 0.0),
        narrow("lot-1", 6.0),
        lot_on("lot-2", 12.0, 12.0, 10.0, 0.0, true),
        narrow("lot-3", 24.0),
        narrow("lot-4", 30.0),
    ];
    let (f, s) = run(&lots);
    assert_eq!((s.lots_filled_before, s.candidates, s.groups, s.lots_filled_after), (1, 4, 2, 5));
    let filled = placed(&f.lots, "lot-2").unwrap();
    assert_eq!(filled.status, LotStatus::Filled);
    assert!(filled.merged_from.is_empty());
    assert_eq!(f.instances.iter().filter(|i| i.lot.as_deref() == Some("lot-2")).count(), 1);
    assert_eq!(placed(&f.lots, "lot-0").unwrap().merged_from, ["lot-0", "lot-1"]);
    assert_eq!(placed(&f.lots, "lot-3").unwrap().merged_from, ["lot-3", "lot-4"]);
    assert_eq!(f.instances.len(), 3);
}

#[test]
fn same_seed_same_bytes() {
    let dirs = ["a", "b"].map(|n| Path::new(env!("CARGO_TARGET_TMPDIR")).join("merge").join(n));
    let header = || Header {
        name: "fixture".into(),
        producer: "settle test".into(),
        recipe: "balmora-x10s".into(),
        seed: SEED.into(),
        kit_blake3: blake3::hash(KIT.as_bytes()).to_hex().to_string(),
        origin_m: [0.0, 0.0],
    };
    for d in &dirs {
        let _ = fs::remove_dir_all(d);
        run(&row()).0.file(header()).write(d).unwrap();
    }
    let bytes = dirs.each_ref().map(|d| fs::read(d.join(PLACEMENTS_FILE)).unwrap());
    assert_eq!(bytes[0], bytes[1]);
}

#[test]
fn without_a_merge_recipe_the_fit_is_unchanged() {
    let (k, (f, _)) = (kit(), recipes());
    let lots = row();
    let (mut fitted, stats) = fit_and_merge(&lots, &k, &f, None, SEED, &flat);
    assert!(stats.is_none());
    let mut alone = settle::fit::fit(&lots, &k, &f, SEED, &flat);
    // Wall time differs between calls.
    (fitted.stats.fit_ms, alone.stats.fit_ms) = (0.0, 0.0);
    assert_eq!(fitted, alone);
    assert!(fitted.lots.iter().all(|l| l.status == LotStatus::Empty));
}
