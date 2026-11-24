//! Slice PL-2: fitting kit buildings into lots. A fixture row of four 12 x 14 m footprints on a straight street
//! (street south, along +x), the six-box kit of `tests/formats.rs`: hulls inside their footprint, no overlap, doors
//! facing the street, byte-identical output per seed, and every empty reason; then the `lot` and `match` domains.
#![allow(clippy::float_cmp)] // exact assertions are the point
use settle::fit::{EmptyReason, Fitted, Header, fit};
use settle::formats::{KitFile, KitKind, LotStatus, PLACEMENTS_FILE, PlacementsFile};
use settle::lots::{Access, Lot, Part, Xy};
use settle::recipe::{FitDomain, FitRecipe};
use std::fs;
use std::path::Path;

/// The six boxes of `tests/formats.rs`: three buildings (10 x 8, 14 x 10 with a side door, 8 x 8 tower on quarter
/// turns), a wall, an arch, stairs.
const KIT: &str = r#"{
  "format": 1, "name": "boxes", "producer": "test", "units": "m",
  "frame": "local x right, y front (door side), z up; pivot at model origin",
  "models": [
    { "id": "house_10x8", "kind": "building", "tags": ["wealth:low"], "roles": ["dwelling"],
      "footprint": [[-5,-4],[5,-4],[5,4],[-5,4]],
      "obb": { "centre": [0,0], "half": [5,4], "yaw_deg": 0 },
      "doors": [ { "at": [0,4], "facing_deg": 90, "width_m": 1.4, "sill_z_m": 0.2 } ],
      "height_m": 7.5, "floors": 2, "base_offset_m": -0.4, "foundation_depth_m": 0.6, "yaw_step_deg": 0,
      "weight": 1.0, "blake3": "aa" },
    { "id": "house_14x10", "kind": "building", "tags": null, "roles": null,
      "footprint": [[-7,-5],[7,-5],[7,5],[-7,5]],
      "doors": [ { "at": [0,5], "facing_deg": 90, "width_m": 1.6, "sill_z_m": 0.2 },
                 { "at": [7,0], "facing_deg": 0, "width_m": 1.2, "sill_z_m": 0.2 } ],
      "height_m": 9, "floors": null, "base_offset_m": -0.5, "foundation_depth_m": null, "yaw_step_deg": null,
      "weight": 0.5, "blake3": "bb" },
    { "id": "tower_8x8", "kind": "building", "footprint": [[-4,-4],[4,-4],[4,4],[-4,4]],
      "doors": [ { "at": [0,4], "facing_deg": 90, "width_m": 1.2, "sill_z_m": 0.3 } ],
      "height_m": 16, "base_offset_m": -0.3, "yaw_step_deg": 90, "weight": 0.2, "blake3": "cc" },
    { "id": "wall_6", "kind": "wall", "footprint": [[-3,-0.4],[3,-0.4],[3,0.4],[-3,0.4]], "doors": [],
      "height_m": 3, "base_offset_m": -0.2, "weight": 1, "blake3": "dd" },
    { "id": "arch_4", "kind": "arch", "footprint": [[-2,-0.5],[2,-0.5],[2,0.5],[-2,0.5]], "doors": [],
      "height_m": 4, "base_offset_m": -0.2, "weight": 1, "blake3": "ee" },
    { "id": "stairs_3x6", "kind": "stairs", "footprint": [[-1.5,-3],[1.5,-3],[1.5,3],[-1.5,3]], "doors": [],
      "height_m": 2.5, "base_offset_m": 0, "weight": 1, "blake3": "ff" }
  ] }"#;

const SEED: &str = "fit-fixture";
/// The street's outward normal seen from the lots (they stand north of it): -y.
const STREET_NORMAL_DEG: f64 = 270.0;

fn kit() -> KitFile {
    KitFile::from_json(KIT).expect("synthetic kit parses")
}

fn recipe() -> FitRecipe {
    let r = settle::Recipe::parse(include_str!("../../../recipes/balmora-x10s.toml")).unwrap();
    r.fit.expect("balmora-x10s has a [fit] table")
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Xy> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

/// A lot north of the street (y = 0), `w` along it, `d` deep, its one footprint the whole lot.
fn lot(id: &str, x0: f64, w: f64, d: f64) -> Lot {
    let polygon = rect(x0, 0.0, x0 + w, d);
    let front = [[x0, 0.0], [x0 + w, 0.0]];
    Lot {
        id: id.into(),
        use_kind: "plot".into(),
        polygon: polygon.clone(),
        holes: vec![],
        front: Some(front),
        axis: [1.0, 0.0],
        access: Some(Access { kind: "public".into(), segment: front, width_m: 6.0 }),
        role: Some("dwelling.row".into()),
        function: Some("domestic".into()),
        wealth: Some(0.5),
        floors: Some([2, 3]),
        preview_parts: vec![Part { label: "building".into(), polygon, holes: vec![] }],
        ground_z_m: 0.0,
    }
}

/// Four 12 x 14 m lots in a row.
fn row() -> Vec<Lot> {
    (0..4).map(|i| lot(&format!("lot-{i}"), 12.0 * f64::from(i), 12.0, 14.0)).collect()
}

fn flat(_: f64, _: f64) -> f64 {
    0.0
}

fn header() -> Header {
    Header {
        name: "fixture".into(),
        producer: "settle test".into(),
        recipe: "balmora-x10s".into(),
        seed: SEED.into(),
        kit_blake3: blake3::hash(KIT.as_bytes()).to_hex().to_string(),
        origin_m: [0.0, 0.0],
    }
}

fn bounds(h: &[Xy]) -> [f64; 4] {
    h.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, p| {
        [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
    })
}

fn lot_status<'a>(f: &'a Fitted, id: &str) -> &'a settle::formats::PlacedLot {
    f.lots.iter().find(|l| l.id == id).unwrap()
}

#[test]
fn row_fits_inside_without_overlap_doors_on_the_street() {
    let (k, r, lots) = (kit(), recipe(), row());
    let f = fit(&lots, &k, &r, SEED, &flat);
    assert_eq!(f.instances.len(), 4, "{:?}", f.empty);
    assert!(f.empty.is_empty());
    assert_eq!((f.stats.tried, f.stats.filled, f.stats.instances, f.stats.candidates), (4, 4, 4, 3));
    let tol = r.inside_tolerance_m.value;
    let mut hulls = Vec::new();
    for inst in &f.instances {
        let lot = lots.iter().find(|l| Some(&l.id) == inst.lot.as_ref()).unwrap();
        assert_eq!(lot_status(&f, &lot.id).status, LotStatus::Filled);
        let model = k.model(&inst.kit).unwrap();
        // Hull inside its (rectangular) footprint.
        let hull = model.world_hull([inst.pos[0], inst.pos[1]], inst.yaw_deg);
        let [px0, py0, px1, py1] = bounds(&lot.preview_parts[0].polygon);
        for p in &hull {
            assert!(p[0] >= px0 - tol && p[0] <= px1 + tol && p[1] >= py0 - tol && p[1] <= py1 + tol, "{} {p:?}", inst.id);
        }
        // The street door faces the street and stands on the front edge.
        let door = model.world_doors([inst.pos[0], inst.pos[1]], inst.yaw_deg)[0];
        let off = (door.facing_deg - STREET_NORMAL_DEG).abs();
        assert!(off.min(360.0 - off) < 1e-9, "{} faces {}", inst.id, door.facing_deg);
        assert_eq!(inst.door, Some(door.at));
        assert!(door.at[1].abs() < 1e-9, "{} door at {:?}", inst.id, door.at);
        // Flat ground: pad 0, pivot at minus the sill.
        assert_eq!(inst.pos[2], -model.doors[0].sill_z_m);
        assert_eq!(lot_status(&f, &lot.id).pad_z_m, Some(0.0));
        hulls.push(bounds(&hull));
    }
    // No two hulls overlap (axis-aligned rectangles at quarter turns).
    for (i, a) in hulls.iter().enumerate() {
        for b in &hulls[i + 1..] {
            let apart = a[2] <= b[0] + tol || b[2] <= a[0] + tol || a[3] <= b[1] + tol || b[3] <= a[1] + tol;
            assert!(apart, "{a:?} overlaps {b:?}");
        }
    }
    // 14 x 10 is wider than the 12 m frontage: never placed.
    assert!(f.instances.iter().all(|i| i.kit != "house_14x10"));
}

#[test]
fn same_seed_same_bytes() {
    let (k, r, lots) = (kit(), recipe(), row());
    let dirs = ["a", "b"].map(|n| Path::new(env!("CARGO_TARGET_TMPDIR")).join("fit").join(n));
    for d in &dirs {
        let _ = fs::remove_dir_all(d);
        fit(&lots, &k, &r, SEED, &flat).file(header()).write(d).unwrap();
    }
    let bytes = dirs.each_ref().map(|d| fs::read(d.join(PLACEMENTS_FILE)).unwrap());
    assert_eq!(bytes[0], bytes[1]);
    // The file reads back as written.
    let back = PlacementsFile::load(&dirs[0]).unwrap();
    assert_eq!(back, fit(&lots, &k, &r, SEED, &flat).file(header()));
    assert_eq!(back.lots.len(), 4);
}

#[test]
fn part_smaller_than_every_model_is_empty_inside() {
    let (k, r) = (kit(), recipe());
    let mut lots = row();
    lots.push(lot("lot-small", 60.0, 5.0, 5.0));
    let f = fit(&lots, &k, &r, SEED, &flat);
    assert_eq!(lot_status(&f, "lot-small").status, LotStatus::Empty);
    assert_eq!(lot_status(&f, "lot-small").pad_z_m, None);
    assert_eq!(f.empty.get("lot-small"), Some(&EmptyReason::Inside));
    assert_eq!(f.stats.empty_by_reason.get("inside"), Some(&1));
    assert_eq!(f.stats.filled, 4);
}

#[test]
fn sloped_part_beyond_the_pad_step_is_rejected() {
    let (k, r) = (kit(), recipe());
    // Lot 2 (x 24..36) rises 1 m per m away from the street: the back of every model is buried.
    let slope = |x: f64, y: f64| if (24.0..=36.0).contains(&x) { y.max(0.0) } else { 0.0 };
    let f = fit(&row(), &k, &r, SEED, &slope);
    assert_eq!(f.empty.get("lot-2"), Some(&EmptyReason::PadStep));
    assert_eq!(lot_status(&f, "lot-2").status, LotStatus::Empty);
    assert_eq!(f.stats.empty_by_reason.get("pad_step"), Some(&1));
    assert_eq!(f.stats.filled, 3);
    assert!(f.instances.iter().all(|i| i.lot.as_deref() != Some("lot-2")));
}

/// Slice PL-2b: one 10 x 8 house carrying the ground vanilla placed it on (1.2 m below its pivot, mesh bottom 4 m
/// below), its door sill 0.77 m above that ground; `foundation` adds an explicit `foundation_depth_m`.
fn ground_kit(foundation: Option<f64>) -> KitFile {
    let depth = foundation.map_or(String::new(), |d| format!(r#""foundation_depth_m": {d},"#));
    KitFile::from_json(&format!(
        r#"{{ "format": 1, "name": "ground", "producer": "test", "units": "m", "frame": "local",
  "models": [ {{ "id": "house_g", "kind": "building", "footprint": [[-5,-4],[5,-4],[5,4],[-5,4]],
    "doors": [ {{ "at": [0,4], "facing_deg": 90, "width_m": 1.4, "sill_z_m": -0.43 }} ],
    "height_m": 9, "base_offset_m": -4, "ground_z_m": -1.2, {depth} "weight": 1, "blake3": "gg" }} ] }}"#
    ))
    .expect("ground kit parses")
}

/// Street ground (y < 0, where the door probe reads the pad) at 10 m; the lot behind it `drop` m lower.
fn stepped(drop: f64) -> impl Fn(f64, f64) -> f64 {
    move |_, y| if y < 0.0 { 10.0 } else { 10.0 - drop }
}

#[test]
fn ground_z_seats_the_pivot_and_sets_the_foundation() {
    let (k, r) = (ground_kit(None), recipe());
    assert_eq!(k.models[0].ground_z_m, Some(-1.2));
    let lots = vec![lot("lot-g", 0.0, 12.0, 14.0)];
    // Flat ground at 10 m: pivot = pad - ground_z_m, not pad - sill.
    let f = fit(&lots, &k, &r, SEED, &|_, _| 10.0);
    assert_eq!(f.instances.len(), 1, "{:?}", f.empty);
    assert_eq!(f.instances[0].pos[2], 11.2);
    assert_eq!(lot_status(&f, "lot-g").pad_z_m, Some(10.0));
    // Ground under the hull 2.5 m below the pad: inside the 2.8 m foundation (ground_z_m - base_offset_m).
    let f = fit(&lots, &k, &r, SEED, &stepped(2.5));
    assert_eq!(f.instances.len(), 1, "{:?}", f.empty);
    assert_eq!(f.instances[0].pos[2], 11.2);
    // 3.0 m below: the foundation would show.
    let f = fit(&lots, &k, &r, SEED, &stepped(3.0));
    assert!(f.instances.is_empty());
    assert_eq!(f.empty.get("lot-g"), Some(&EmptyReason::PadStep));
    assert_eq!(f.stats.empty_by_reason.get("pad_step"), Some(&1));
    // An explicit foundation_depth_m wins over the fallback.
    let f = fit(&lots, &ground_kit(Some(2.0)), &r, SEED, &stepped(2.5));
    assert_eq!(f.empty.get("lot-g"), Some(&EmptyReason::PadStep));
    // The older kits (no ground_z_m) keep the sill rule: tested by the row fixture above.
    assert!(kit().models.iter().all(|m| m.ground_z_m.is_none()));
}

#[test]
fn footprint_under_a_placed_hull_is_empty_overlap() {
    let (k, r) = (kit(), recipe());
    let mut lots = row();
    // Same ground as lot-0, sorted after it.
    lots.push(lot("lot-0b", 0.0, 12.0, 14.0));
    let f = fit(&lots, &k, &r, SEED, &flat);
    assert_eq!(f.empty.get("lot-0b"), Some(&EmptyReason::Overlap));
    assert_eq!(f.stats.filled, 4);
}

#[test]
fn lot_without_footprint_is_not_tried() {
    let (k, r) = (kit(), recipe());
    let mut lots = row();
    let mut bare = lot("lot-bare", 60.0, 12.0, 14.0);
    bare.preview_parts.clear();
    lots.push(bare);
    let f = fit(&lots, &k, &r, SEED, &flat);
    assert_eq!((f.stats.lots, f.stats.tried, f.stats.no_footprint), (5, 4, 1));
    assert_eq!(lot_status(&f, "lot-bare").status, LotStatus::Empty);
    assert!(!f.empty.contains_key("lot-bare"));
}

#[test]
fn oblique_street_doors_face_its_normal() {
    let (k, r) = (kit(), recipe());
    let turn = 30.0_f64;
    let rot = |p: Xy, deg: f64| settle::formats::rotate(p, deg);
    let mut lots = row();
    for l in &mut lots {
        l.polygon = l.polygon.iter().map(|&p| rot(p, turn)).collect();
        l.preview_parts[0].polygon = l.polygon.clone();
        l.front = l.front.map(|[a, b]| [rot(a, turn), rot(b, turn)]);
        l.access.as_mut().unwrap().segment = l.front.unwrap();
        l.axis = rot(l.axis, turn);
    }
    let f = fit(&lots, &k, &r, SEED, &flat);
    assert_eq!(f.instances.len(), 4, "{:?}", f.empty);
    let tol = r.inside_tolerance_m.value;
    for (inst, lot) in f.instances.iter().zip(row()) {
        let model = k.model(&inst.kit).unwrap();
        let door = model.world_doors([inst.pos[0], inst.pos[1]], inst.yaw_deg)[0];
        let off = (door.facing_deg - (STREET_NORMAL_DEG + turn)).rem_euclid(360.0);
        assert!(off.min(360.0 - off) < 1e-9, "{} faces {}", inst.id, door.facing_deg);
        // Back in the street's frame, the hull sits inside the unturned footprint.
        let [px0, py0, px1, py1] = bounds(&lot.polygon);
        for p in model.world_hull([inst.pos[0], inst.pos[1]], inst.yaw_deg) {
            let q = rot(p, -turn);
            assert!(q[0] >= px0 - tol && q[0] <= px1 + tol && q[1] >= py0 - tol && q[1] <= py1 + tol, "{} {q:?}", inst.id);
        }
    }
}

fn tunable<T>(value: T) -> settle::recipe::Tunable<T> {
    settle::recipe::Tunable { value, unit: "test".into(), source: "test".into() }
}

/// `recipe()` fitting into whole lots with a 0.5 m setback.
fn lot_recipe() -> FitRecipe {
    let mut r = recipe();
    r.domain = Some(tunable(FitDomain::Lot));
    r.lot_setback_m = Some(tunable(0.5));
    r.validate().unwrap();
    r
}

/// A 20 x 14 m lot whose footprint (9 x 7 m) takes no model of the kit.
fn wide_lot(id: &str, x0: f64) -> Lot {
    let mut l = lot(id, x0, 20.0, 14.0);
    l.preview_parts[0].polygon = rect(x0 + 5.0, 0.0, x0 + 14.0, 7.0);
    l
}

/// Every hull of `f` inside its lot `polygon` (axis-aligned) shrunk by `setback` on the sides and back, door side
/// on the street (y = 0) facing it.
fn assert_in_shrunk_lot(f: &Fitted, k: &KitFile, lot: &Lot, setback: f64) {
    let inst = f.instances.iter().find(|i| i.lot.as_deref() == Some(lot.id.as_str())).expect("a building");
    let model = k.model(&inst.kit).unwrap();
    let [x0, _, x1, y1] = bounds(&lot.polygon);
    let hb = bounds(&model.world_hull([inst.pos[0], inst.pos[1]], inst.yaw_deg));
    assert!(hb[0] >= x0 + setback - 1e-9 && hb[2] <= x1 - setback + 1e-9 && hb[3] <= y1 - setback + 1e-9, "{hb:?}");
    assert!(hb[1] >= -1e-9, "{hb:?} in the street");
    let door = model.world_doors([inst.pos[0], inst.pos[1]], inst.yaw_deg)[0];
    let off = (door.facing_deg - STREET_NORMAL_DEG).abs();
    assert!(off.min(360.0 - off) < 1e-9, "{} faces {}", inst.id, door.facing_deg);
}

#[test]
fn lot_domain_fills_a_lot_whose_footprint_is_too_small() {
    let (k, r, lr) = (kit(), recipe(), lot_recipe());
    let lots = vec![wide_lot("lot-w", 0.0)];
    let by_footprint = fit(&lots, &k, &r, SEED, &flat);
    assert_eq!(by_footprint.empty.get("lot-w"), Some(&EmptyReason::Inside));
    let by_lot = fit(&lots, &k, &lr, SEED, &flat);
    assert_eq!(lot_status(&by_lot, "lot-w").status, LotStatus::Filled, "{:?}", by_lot.empty);
    assert_in_shrunk_lot(&by_lot, &k, &lots[0], 0.5);
    // The door side stands on the street line (fit.setback_m = 0); the largest model wins the 20 m frontage.
    let inst = &by_lot.instances[0];
    let door = k.model(&inst.kit).unwrap().world_doors([inst.pos[0], inst.pos[1]], inst.yaw_deg)[0];
    assert!(door.at[1].abs() < 1e-9, "door at {:?}", door.at);
    assert_eq!(inst.kit, "house_14x10");
    assert_eq!((by_lot.stats.lot_domain, by_lot.stats.filled_without_footprint), (true, 0));
}

#[test]
fn lot_domain_tries_street_lots_without_footprint() {
    let (k, r, lr) = (kit(), recipe(), lot_recipe());
    let mut bare = lot("lot-bare", 0.0, 12.0, 14.0);
    bare.preview_parts.clear();
    let mut back = lot("lot-back", 20.0, 12.0, 14.0);
    back.preview_parts.clear();
    back.access = None;
    let lots = vec![bare, back];
    assert_eq!(fit(&lots, &k, &r, SEED, &flat).stats.tried, 0);
    let f = fit(&lots, &k, &lr, SEED, &flat);
    assert_eq!((f.stats.tried, f.stats.tried_without_footprint, f.stats.filled_without_footprint, f.stats.no_footprint), (1, 1, 1, 1));
    assert_eq!(lot_status(&f, "lot-bare").status, LotStatus::Filled);
    assert_in_shrunk_lot(&f, &k, &lots[0], 0.5);
    assert_eq!(lot_status(&f, "lot-back").status, LotStatus::Empty);
}

#[test]
fn lot_domain_sets_a_house_back_behind_a_skewed_front() {
    let (k, lr) = (kit(), lot_recipe());
    // The front kinks 0.6 m back past x = 10; the access (and the street normal) stays on the straight part.
    let mut l = wide_lot("lot-kink", 0.0);
    l.polygon = vec![[0.0, 0.0], [10.0, 0.0], [20.0, 0.6], [20.0, 14.0], [0.0, 14.0]];
    let f = fit(std::slice::from_ref(&l), &k, &lr, SEED, &flat);
    assert_eq!(lot_status(&f, "lot-kink").status, LotStatus::Filled, "{:?}", f.empty);
    let inst = &f.instances[0];
    let model = k.model(&inst.kit).unwrap();
    let hull = model.world_hull([inst.pos[0], inst.pos[1]], inst.yaw_deg);
    let [hx0, hy0, hx1, hy1] = bounds(&hull);
    assert!(hx0 >= 0.5 - 1e-9 && hx1 <= 19.5 + 1e-9 && hy1 <= 13.5 + 1e-9, "{hull:?}");
    // Behind the kinked front everywhere, flush with it at the hull's corner furthest along it.
    let front_y = |x: f64| if x <= 10.0 { 0.0 } else { 0.06 * (x - 10.0) };
    assert!(hull.iter().all(|p| p[1] >= front_y(p[0]) - 1e-9), "{hull:?}");
    assert!((hy0 - front_y(hx1)).abs() < 1e-9, "front {hy0} vs {}", front_y(hx1));
}

#[test]
fn explicit_footprint_domain_is_the_default_bytes() {
    let (k, lots) = (kit(), row());
    let mut r = recipe();
    let dir = |n: &str| Path::new(env!("CARGO_TARGET_TMPDIR")).join("fit-domain").join(n);
    let _ = fs::remove_dir_all(dir("default"));
    fit(&lots, &k, &r, SEED, &flat).file(header()).write(&dir("default")).unwrap();
    r.domain = Some(tunable(FitDomain::Footprint));
    let _ = fs::remove_dir_all(dir("footprint"));
    fit(&lots, &k, &r, SEED, &flat).file(header()).write(&dir("footprint")).unwrap();
    let read = |n: &str| fs::read(dir(n).join(PLACEMENTS_FILE)).unwrap();
    assert_eq!(read("default"), read("footprint"));
    assert!(!fit(&lots, &k, &r, SEED, &flat).stats.lot_domain);
}

// ---------------------------------------------------------------------------------------------------------------
// `match` domain (slice C5b): one kit model per Magna Urbis building footprint, inside its lot, best IoU.

/// `recipe()` matching footprints; `margin` = score margin of the seeded draw.
fn match_recipe(margin: f64) -> FitRecipe {
    let mut r = recipe();
    r.domain = Some(tunable(FitDomain::Match));
    r.score_margin.value = margin;
    r.validate().unwrap();
    r
}

fn part(label: &str, polygon: Vec<Xy>) -> Part {
    Part { label: label.into(), polygon, holes: vec![] }
}

/// Every instance's hull inside its lot's (axis-aligned) bounds and no two hulls interpenetrating.
fn assert_in_lots_apart(f: &Fitted, k: &KitFile, lots: &[Lot]) {
    let hulls: Vec<[f64; 4]> = f
        .instances
        .iter()
        .map(|i| {
            let lot = lots.iter().find(|l| Some(l.id.as_str()) == i.lot.as_deref()).unwrap();
            let [x0, y0, x1, y1] = bounds(&lot.polygon);
            let hb = bounds(&k.model(&i.kit).unwrap().world_hull([i.pos[0], i.pos[1]], i.yaw_deg));
            assert!(hb[0] >= x0 - 1e-9 && hb[1] >= y0 - 1e-9 && hb[2] <= x1 + 1e-9 && hb[3] <= y1 + 1e-9, "{} {hb:?} outside", i.id);
            hb
        })
        .collect();
    for (i, a) in hulls.iter().enumerate() {
        for b in &hulls[i + 1..] {
            assert!(a[2] <= b[0] + 1e-9 || b[2] <= a[0] + 1e-9 || a[3] <= b[1] + 1e-9 || b[3] <= a[1] + 1e-9, "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn match_gives_an_l_footprint_the_best_iou_model_door_to_the_street() {
    let k = kit();
    // A 9.6 x 8 m body with a 4 x 3 m back wing (no model lies inside it) in a 20 x 16 m lot; a tomb is left alone.
    let mut l = lot("lot-l", 0.0, 20.0, 16.0);
    let el = vec![[5.0, 0.0], [14.6, 0.0], [14.6, 8.0], [9.0, 8.0], [9.0, 11.0], [5.0, 11.0]];
    l.preview_parts = vec![part("building", el), part("tomb", rect(15.0, 12.0, 19.0, 15.0))];
    let lots = vec![l];
    assert_eq!(fit(&lots, &k, &recipe(), SEED, &flat).instances[0].kit, "tower_8x8", "footprint domain: only the tower is inside");
    let f = fit(&lots, &k, &match_recipe(0.0), SEED, &flat);
    // Centred on the box centre x = 9.8, door side on the street: house_10x8 76.8 / 92 beats tower 64 / 88.8 and
    // house_14x10 84.8 / 144.
    assert_eq!(f.instances.len(), 1);
    let inst = &f.instances[0];
    assert_eq!(inst.kit, "house_10x8");
    let model = k.model(&inst.kit).unwrap();
    let hb = bounds(&model.world_hull([inst.pos[0], inst.pos[1]], inst.yaw_deg));
    assert!((hb[0] - 4.8).abs() < 1e-9 && (hb[2] - 14.8).abs() < 1e-9 && hb[1].abs() < 1e-9 && (hb[3] - 8.0).abs() < 1e-9, "{hb:?}");
    let door = model.world_doors([inst.pos[0], inst.pos[1]], inst.yaw_deg)[0];
    let off = (door.facing_deg - STREET_NORMAL_DEG).abs();
    assert!(off.min(360.0 - off) < 1e-9 && door.at[1].abs() < 1e-9, "door {door:?}");
    let s = &f.stats;
    assert!(s.match_domain);
    assert_eq!((s.building_parts, s.parts_tried, s.parts_filled, s.tried, s.filled), (1, 1, 1, 1, 1));
    assert_eq!(s.parts_skipped_by_label.get("tomb"), Some(&1));
    let iou = s.match_iou.unwrap()[1];
    assert!((iou - 76.8 / 92.0).abs() < 1e-6, "IoU {iou}");
    assert_in_lots_apart(&f, &k, &lots);
}

#[test]
fn match_never_places_a_hull_outside_its_lot_or_over_a_placed_one() {
    let mut k = kit();
    k.models.retain(|m| m.id != "tower_8x8");
    // Footprint A (13 x 10) takes house_14x10, 0.5 m over its right edge; B (9 x 8) has only house_10x8 left, which
    // overlaps A's hull centred, overlaps it more slid left and leaves the lot (x <= 23.6) slid right.
    let mut l = lot("lot-ab", 0.0, 23.6, 12.0);
    l.preview_parts = vec![part("building", rect(1.0, 0.0, 14.0, 10.0)), part("building", rect(14.0, 0.0, 23.0, 8.0))];
    // A plot without building footprints and a non-plot lot are left alone.
    let mut court = lot("lot-court", 30.0, 12.0, 14.0);
    court.preview_parts[0].label = "court".into();
    let mut yard = lot("lot-yard", 50.0, 12.0, 14.0);
    yard.use_kind = "compound".into();
    let lots = vec![l, court, yard];
    let f = fit(&lots, &k, &match_recipe(0.0), SEED, &flat);
    assert_eq!(f.instances.iter().map(|i| i.kit.as_str()).collect::<Vec<_>>(), ["house_14x10"]);
    let s = &f.stats;
    assert_eq!((s.building_parts, s.parts_tried, s.parts_filled, s.tried, s.filled, s.no_footprint), (2, 2, 1, 1, 1, 2));
    assert_eq!(s.parts_empty_by_reason.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(), [("overlap", 1)]);
    assert_eq!(s.parts_skipped_by_label.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(), [("building on compound", 1), ("court", 1)]);
    assert_eq!(lot_status(&f, "lot-ab").status, LotStatus::Filled);
    assert_in_lots_apart(&f, &k, &lots);
}

#[test]
fn match_same_seed_same_choice() {
    let k = kit();
    // 9 x 9 footprints in 12 x 12 lots: house_10x8 (72 / 89) and tower_8x8 (64 / 81) are within the 0.1 margin.
    let lots: Vec<Lot> = (0..6)
        .map(|i| {
            let x0 = 12.0 * f64::from(i);
            let mut l = lot(&format!("lot-{i}"), x0, 12.0, 12.0);
            l.preview_parts[0].polygon = rect(x0 + 1.5, 0.0, x0 + 10.5, 9.0);
            l
        })
        .collect();
    let r = match_recipe(0.1);
    let (a, b) = (fit(&lots, &k, &r, SEED, &flat), fit(&lots, &k, &r, SEED, &flat));
    assert_eq!((&a.lots, &a.instances, &a.empty), (&b.lots, &b.instances, &b.empty));
    assert_eq!(a.stats.parts_filled, 6);
    assert_in_lots_apart(&a, &k, &lots);
}

// `match` fallback stages (slice C5e): back-slide, then the footprint's own box frame, only for unmatched footprints.

/// `match_recipe(0.0)` with both fallback stages: back-slide to 6 m, part frame within 15 degrees.
fn fallback_recipe(back: bool, frame: bool) -> FitRecipe {
    let mut r = match_recipe(0.0);
    r.back_slide_max_m = back.then(|| tunable(6.0));
    r.part_frame_max_deg = frame.then(|| tunable(15.0));
    r.validate().unwrap();
    r
}

const SKEW_DEG: f64 = 6.0;

/// A 12 x 16 m lot at `x0` whose street edge rises 6 degrees off the access (street along +x at y = 0, normal -y);
/// its building footprint's front lies on that edge.
fn skewed_lot(id: &str, x0: f64) -> Lot {
    let t = SKEW_DEG.to_radians().tan();
    let mut l = lot(id, x0, 12.0, 16.0);
    l.polygon = vec![[x0, 0.0], [x0 + 12.0, 12.0 * t], [x0 + 12.0, 16.0], [x0, 16.0]];
    l.preview_parts = vec![part("building", vec![[x0 + 1.0, t], [x0 + 11.0, 11.0 * t], [x0 + 11.0, 9.0], [x0 + 1.0, 9.0]])];
    l
}

fn turned(p: Xy, deg: f64) -> Xy {
    let (s, c) = deg.to_radians().sin_cos();
    [c * p[0] - s * p[1], s * p[0] + c * p[1]]
}

const TURN_DEG: f64 = 10.0;

/// An 11 x 10 m lot at `x0` turned 10 degrees about its front-left corner, the access still along +x (normal -y); its
/// footprint (10 x 9 m, on the lot's front) is turned with it.
fn turned_lot(id: &str, x0: f64) -> Lot {
    let at = |r: Vec<Xy>| r.into_iter().map(|p| turned(p, TURN_DEG)).map(|p| [p[0] + x0, p[1]]).collect::<Vec<_>>();
    let mut l = lot(id, x0, 11.0, 10.0);
    l.polygon = at(rect(0.0, 0.0, 11.0, 10.0));
    l.preview_parts = vec![part("building", at(rect(0.5, 0.0, 10.5, 9.0)))];
    l
}

#[test]
fn match_back_slide_fills_a_lot_whose_street_edge_is_6_degrees_off() {
    let k = kit();
    let lots = vec![skewed_lot("lot-skew", 0.0)];
    let base = fit(&lots, &k, &match_recipe(0.0), SEED, &flat);
    assert_eq!(base.empty.get("lot-skew"), Some(&EmptyReason::Inside), "the front corner pokes into the street");
    let f = fit(&lots, &k, &fallback_recipe(true, true), SEED, &flat);
    assert_eq!(lot_status(&f, "lot-skew").status, LotStatus::Filled, "{:?}", f.empty);
    assert_eq!(f.stats.parts_filled_by_stage.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(), [("back_slide", 1)]);
    let inst = &f.instances[0];
    let model = k.model(&inst.kit).unwrap();
    let hull = model.world_hull([inst.pos[0], inst.pos[1]], inst.yaw_deg);
    // Inside the lot (behind its oblique street edge), door to the street, set back a whole number of 1 m steps.
    let t = SKEW_DEG.to_radians().tan();
    let tol = 0.05;
    assert!(hull.iter().all(|p| p[0] >= -1e-9 && p[0] <= 12.0 + 1e-9 && p[1] <= 16.0 + 1e-9 && p[1] >= p[0] * t - tol), "{hull:?}");
    let door = model.world_doors([inst.pos[0], inst.pos[1]], inst.yaw_deg)[0];
    let off = (door.facing_deg - STREET_NORMAL_DEG).abs();
    assert!(off.min(360.0 - off) < 1e-9, "door faces {}", door.facing_deg);
    let back = bounds(&hull)[1] - t;
    assert!((back - back.round()).abs() < 1e-9 && (1.0..=6.0).contains(&back.round()), "set back {back} m");
}

#[test]
fn match_part_frame_fills_a_footprint_10_degrees_off_the_street() {
    let mut k = kit();
    // The tower turns in quarter turns only; without it no street-frame model fits the turned lot at any depth.
    k.models.retain(|m| m.id != "tower_8x8");
    let lots = vec![turned_lot("lot-turn", 0.0)];
    for r in [match_recipe(0.0), fallback_recipe(true, false)] {
        assert_eq!(fit(&lots, &k, &r, SEED, &flat).empty.get("lot-turn"), Some(&EmptyReason::Inside));
    }
    let f = fit(&lots, &k, &fallback_recipe(true, true), SEED, &flat);
    assert_eq!(lot_status(&f, "lot-turn").status, LotStatus::Filled, "{:?}", f.empty);
    assert_eq!(f.stats.parts_filled_by_stage.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(), [("part_frame", 1)]);
    let inst = &f.instances[0];
    assert_eq!(inst.kit, "house_10x8");
    let model = k.model(&inst.kit).unwrap();
    // In the lot's own frame: centred on the footprint, door side on the lot's front, facing the footprint's front.
    let hull: Vec<Xy> = model.world_hull([inst.pos[0], inst.pos[1]], inst.yaw_deg).into_iter().map(|p| turned(p, -TURN_DEG)).collect();
    let hb = bounds(&hull);
    assert!((hb[0] - 0.5).abs() < 1e-9 && (hb[2] - 10.5).abs() < 1e-9 && hb[1].abs() < 1e-9 && (hb[3] - 8.0).abs() < 1e-9, "{hb:?}");
    let door = model.world_doors([inst.pos[0], inst.pos[1]], inst.yaw_deg)[0];
    assert!((door.facing_deg - (STREET_NORMAL_DEG + TURN_DEG)).abs() < 1e-9, "door faces {}", door.facing_deg);
    let iou = f.stats.match_iou.unwrap()[1];
    assert!((iou - 80.0 / 90.0).abs() < 1e-6, "IoU {iou}");
}

#[test]
fn match_fallback_never_moves_a_part_matched_at_its_front() {
    let k = small_kit();
    // The `match_same_seed_same_choice` row, the two-footprint lot of `match_never_places_...` shifted to x = 100, the
    // two fallback lots, and a skewed lot whose shallow front footprint only a back-slid hull would take, over the spot
    // its back footprint matches at its front (fallbacks run after the lot's front matches: the back one keeps it).
    let mut lots: Vec<Lot> = (0..6)
        .map(|i| {
            let x0 = 12.0 * f64::from(i);
            let mut l = lot(&format!("lot-{i}"), x0, 12.0, 12.0);
            l.preview_parts[0].polygon = rect(x0 + 1.5, 0.0, x0 + 10.5, 9.0);
            l
        })
        .collect();
    let mut ab = lot("lot-ab", 100.0, 23.6, 12.0);
    ab.preview_parts = vec![part("building", rect(101.0, 0.0, 114.0, 10.0)), part("building", rect(114.0, 0.0, 123.0, 8.0))];
    let mut pair = skewed_lot("lot-pair", 200.0);
    let t = SKEW_DEG.to_radians().tan();
    pair.preview_parts = vec![
        part("building", vec![[201.0, t], [211.0, 11.0 * t], [211.0, 5.0], [201.0, 5.0]]),
        part("building", rect(201.0, 6.0, 211.0, 14.0)),
    ];
    // And (slice C5f) a lot on the slope that only the `sink_back` stage fills.
    // And (the `small` stage) a lot too small for every building that holds the kit's small tower.
    lots.extend([ab, skewed_lot("lot-skew", 140.0), turned_lot("lot-turn", 170.0), pair, slope_lot("lot-slope", SLOPE_X0), small_lot("lot-small", -50.0)]);
    let base = fit(&lots, &k, &match_recipe(0.0), SEED, &slope_behind_door);
    assert!(base.instances.iter().any(|i| i.lot.as_deref() == Some("lot-pair")), "the back footprint matches at its front");
    let f = fit(&lots, &k, &fallback_recipe(true, true), SEED, &slope_behind_door);
    let sunk = fit(&lots, &k, &sink_recipe(), SEED, &slope_behind_door);
    let small = fit(&lots, &k, &small_recipe(), SEED, &slope_behind_door);
    let key = |i: &settle::formats::Instance| (i.lot.clone(), i.kit.clone(), i.pos, i.yaw_deg, i.door);
    for (before, later) in [(&base, &f), (&f, &sunk), (&sunk, &small)] {
        let after: Vec<_> = later.instances.iter().map(key).collect();
        for i in &before.instances {
            assert!(after.contains(&key(i)), "{} on {:?} moved", i.kit, i.lot);
        }
        assert!(later.stats.parts_filled > before.stats.parts_filled, "a later stage fills more");
        assert_in_lots_apart(later, &k, &lots);
    }
    assert_eq!(f.stats.parts_filled_by_stage.get("front"), Some(&base.stats.parts_filled));
    // The sink stage leaves the earlier stages' counts alone and adds the sloped lot.
    let earlier: Vec<_> = sunk.stats.parts_filled_by_stage.iter().filter(|(s, _)| **s != "sink_back").map(|(s, n)| (*s, *n)).collect();
    assert_eq!(earlier, f.stats.parts_filled_by_stage.iter().map(|(s, n)| (*s, *n)).collect::<Vec<_>>());
    assert_eq!(sunk.stats.parts_filled_by_stage.get("sink_back"), Some(&1));
    assert!(sunk.instances.iter().any(|i| i.lot.as_deref() == Some("lot-slope")));
    // The `small` stage likewise adds only the small lot, with the tower.
    let earlier: Vec<_> = small.stats.parts_filled_by_stage.iter().filter(|(s, _)| **s != "small").map(|(s, n)| (*s, *n)).collect();
    assert_eq!(earlier, sunk.stats.parts_filled_by_stage.iter().map(|(s, n)| (*s, *n)).collect::<Vec<_>>());
    assert_eq!(small.stats.parts_filled_by_stage.get("small"), Some(&1));
    assert!(small.instances.iter().any(|i| i.lot.as_deref() == Some("lot-small") && i.kit == "bal_5x6"));
}

// `match` last stage `sink_back` (slice C5f): the back wall sinks into a slope, the door stays at street level.

/// The sloped lots start here; west of it the ground is flat.
const SLOPE_X0: f64 = 300.0;
/// Rise of the ground behind the street (m per m): 1.6 m at the back of an 8 m deep house.
const SLOPE: f64 = 0.2;

/// Flat ground (0 m) west of `SLOPE_X0`; east of it the street is at 0 m and the ground rises `SLOPE` behind it.
fn slope_behind_door(x: f64, y: f64) -> f64 {
    if x >= SLOPE_X0 { SLOPE * y.max(0.0) } else { 0.0 }
}

/// A 12 x 12 m lot at `x0` with a 9 x 9 m building footprint on its front (the `match_same_seed_same_choice` lot).
fn slope_lot(id: &str, x0: f64) -> Lot {
    let mut l = lot(id, x0, 12.0, 12.0);
    l.preview_parts[0].polygon = rect(x0 + 1.5, 0.0, x0 + 10.5, 9.0);
    l
}

/// `fallback_recipe(true, true)` with the `sink_back` stage at 2 m.
fn sink_recipe() -> FitRecipe {
    let mut r = fallback_recipe(true, true);
    r.sink_back_max_m = Some(tunable(2.0));
    r.validate().unwrap();
    r
}

#[test]
fn match_sink_back_fills_a_lot_rising_1_6_m_behind_the_door() {
    let k = kit();
    let lots = vec![slope_lot("lot-slope", SLOPE_X0)];
    let base = fit(&lots, &k, &fallback_recipe(true, true), SEED, &slope_behind_door);
    assert_eq!(base.empty.get("lot-slope"), Some(&EmptyReason::PadStep), "1.6 m is above the 1 m pad rise");
    let f = fit(&lots, &k, &sink_recipe(), SEED, &slope_behind_door);
    assert_eq!(lot_status(&f, "lot-slope").status, LotStatus::Filled, "{:?}", f.empty);
    assert_eq!(f.stats.parts_filled_by_stage.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(), [("sink_back", 1)]);
    let inst = &f.instances[0];
    let model = k.model(&inst.kit).unwrap();
    // At the front (door side on the street edge), door to the street.
    let hull = model.world_hull([inst.pos[0], inst.pos[1]], inst.yaw_deg);
    assert!(bounds(&hull)[1].abs() < 1e-9, "{hull:?}");
    // The door pad rule's height: the street ground in front of the door (0 m), pivot at minus the sill; not raised.
    assert_eq!(lot_status(&f, "lot-slope").pad_z_m, Some(0.0));
    assert_eq!(inst.pos[2], -model.doors[0].sill_z_m);
    // The back wall is buried by the slope's 1.6 m.
    let [p50, max] = f.stats.sink_back_rise_m.unwrap();
    assert!((p50 - 1.6).abs() < 1e-9 && (max - 1.6).abs() < 1e-9, "{p50} {max}");
}

// `match` final stage `small` (owner 2026-11-24: towers where no house fits): `small_kinds` models in the lots no
// building fits.

/// The fixture kit plus a 5 x 6 m model of kind `tower` (the smallest building is 8 x 8 m).
fn small_kit() -> KitFile {
    let tower = r#"{ "id": "bal_5x6", "kind": "tower", "footprint": [[-2.5,-3],[2.5,-3],[2.5,3],[-2.5,3]],
      "doors": [ { "at": [0,3], "facing_deg": 90, "width_m": 1.2, "sill_z_m": 0.1 } ],
      "height_m": 12, "base_offset_m": -0.3, "weight": 1, "blake3": "ab" },"#;
    KitFile::from_json(&KIT.replacen(r#""models": ["#, &format!("\"models\": [{tower}"), 1)).expect("small kit parses")
}

/// A 7 x 8 m lot at `x0`, its one footprint the whole lot: no building fits, the 5 x 6 m tower does.
fn small_lot(id: &str, x0: f64) -> Lot {
    lot(id, x0, 7.0, 8.0)
}

/// `sink_recipe()` with the `small` stage on kind `tower`.
fn small_recipe() -> FitRecipe {
    let mut r = sink_recipe();
    r.small_kinds = Some(tunable(vec![KitKind::Tower]));
    r.validate().unwrap();
    r
}

#[test]
fn match_small_stage_puts_a_tower_where_no_building_fits() {
    let k = small_kit();
    let lots = vec![small_lot("lot-small", 0.0)];
    let base = fit(&lots, &k, &sink_recipe(), SEED, &flat);
    assert_eq!(base.empty.get("lot-small"), Some(&EmptyReason::Inside), "every building is wider than the lot");
    assert!(base.instances.is_empty());
    let f = fit(&lots, &k, &small_recipe(), SEED, &flat);
    assert_eq!(lot_status(&f, "lot-small").status, LotStatus::Filled, "{:?}", f.empty);
    assert_eq!(f.stats.parts_filled_by_stage.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(), [("small", 1)]);
    assert_eq!(f.stats.small_candidates, 1);
    let inst = &f.instances[0];
    assert_eq!(inst.kit, "bal_5x6");
    let model = k.model(&inst.kit).unwrap();
    // Inside the lot, its door side on the street edge, door to the street.
    assert_in_lots_apart(&f, &k, &lots);
    assert!(bounds(&model.world_hull([inst.pos[0], inst.pos[1]], inst.yaw_deg))[1].abs() < 1e-9);
    let door = model.world_doors([inst.pos[0], inst.pos[1]], inst.yaw_deg)[0];
    let off = (door.facing_deg - STREET_NORMAL_DEG).abs();
    assert!(off.min(360.0 - off) < 1e-9, "door faces {}", door.facing_deg);
    // `building` is refused as a small kind (it is the earlier stages' candidates).
    let mut bad = small_recipe();
    bad.small_kinds = Some(tunable(vec![KitKind::Building]));
    assert!(bad.validate().is_err());
}
