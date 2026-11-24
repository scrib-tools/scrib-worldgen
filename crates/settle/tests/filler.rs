//! Slice PL-3b: closing the street wall. A row of 12 m lots on a straight street (street south, along +x), a 10 m
//! house kit, a 3.66 m wall and a 3.66 m arch: the empty lot between two houses is closed by whole wall pieces, a 4 m
//! lane mouth between two runs gets one centred arch, a 12 m one stays open, byte-identical output per seed. Towers
//! (a 6 m by 7 m `tower`): at a run's open ends and where it bends, none beside an arch or into a house, spaced.
#![allow(clippy::float_cmp)] // exact assertions are the point
#![allow(clippy::many_single_char_names)] // k, f, s, w, o as in tests/merge.rs
use settle::filler::{ARCH_ROLE, FillerStats, TOWER_ROLE, WALL_ROLE, fit_merge_fill};
use settle::fit::{Fitted, Header};
use settle::formats::{Instance, KitFile, LotStatus, PLACEMENTS_FILE, rotate};
use settle::lots::{Access, Lot, Part, Xy};
use settle::merge::fit_and_merge;
use settle::recipe::{FillerRecipe, FitRecipe, MergeRecipe};
use std::fs;
use std::path::Path;

/// A 10 x 8 house (door on the front, +y local), a 3.66 m wall and a 3.66 m arch (x along the wall, outer face on
/// local +y at 2.043, 2.257 m deep), a 6 m wide tower (same frame, 7 m deep).
const KIT: &str = r#"{
  "format": 1, "name": "row", "producer": "test", "units": "m",
  "frame": "local x right, y front (door side), z up; pivot at model origin",
  "models": [
    { "id": "house_10x8", "kind": "building", "footprint": [[-5,-4],[5,-4],[5,4],[-5,4]],
      "doors": [ { "at": [0,4], "facing_deg": 90, "width_m": 1.4, "sill_z_m": 0.2 } ],
      "height_m": 7.5, "base_offset_m": -0.4, "weight": 1.0, "blake3": "aa" },
    { "id": "wall_366", "kind": "wall", "footprint": [[-1.83,-0.214],[1.83,-0.214],[1.83,2.043],[-1.83,2.043]],
      "doors": [], "height_m": 7.5, "base_offset_m": -3.66, "weight": 1.0, "blake3": "bb" },
    { "id": "arch_366", "kind": "arch", "footprint": [[-1.83,-0.214],[1.83,-0.214],[1.83,2.043],[-1.83,2.043]],
      "doors": [], "height_m": 7.5, "base_offset_m": -3.66, "weight": 1.0, "blake3": "cc" },
    { "id": "tower_6", "kind": "tower", "footprint": [[-3,-4.957],[3,-4.957],[3,2.043],[-3,2.043]],
      "obb": { "centre": [0,-1.457], "half": [3,3.5], "yaw_deg": 0 },
      "doors": [], "height_m": 9.3, "base_offset_m": -3.66, "ground_z_m": 0.0, "weight": 1.0, "blake3": "dd" }
  ] }"#;

const SEED: &str = "filler-fixture";
const WALL_M: f64 = 3.66;

fn kit() -> KitFile {
    KitFile::from_json(KIT).expect("synthetic kit parses")
}

fn recipes() -> (FitRecipe, MergeRecipe, FillerRecipe) {
    let r = settle::Recipe::parse(include_str!("../../../recipes/balmora-x10s.toml")).unwrap();
    let mut filler = r.filler.expect("balmora-x10s has a [filler] table");
    filler.walls.value = vec!["wall_366".into()];
    filler.arches.value = vec!["arch_366".into()];
    (r.fit.expect("a [fit] table"), r.merge.expect("a [merge] table"), filler)
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Xy> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

/// A 12 m by 10 m lot `x0..x0 + 12` north of the street `y = 0`; `house`: its one footprint is the whole lot.
fn lot(id: &str, x0: f64, house: bool) -> Lot {
    let polygon = rect(x0, 0.0, x0 + 12.0, 10.0);
    let front = [[x0, 0.0], [x0 + 12.0, 0.0]];
    Lot {
        id: id.into(),
        use_kind: "plot".into(),
        polygon: polygon.clone(),
        holes: vec![],
        front: Some(front),
        axis: [1.0, 0.0],
        access: Some(Access { kind: "public".into(), segment: front, width_m: 12.0 }),
        role: Some("dwelling.house".into()),
        function: Some("domestic".into()),
        wealth: Some(0.5),
        floors: Some([2, 2]),
        preview_parts: if house { vec![Part { label: "building".into(), polygon, holes: vec![] }] } else { vec![] },
        ground_z_m: 0.0,
    }
}

/// House, empty lot, house.
fn row() -> Vec<Lot> {
    vec![lot("lot-0", 0.0, true), lot("lot-1", 12.0, false), lot("lot-2", 24.0, true)]
}

fn flat(_: f64, _: f64) -> f64 {
    0.0
}

fn run(lots: &[Lot]) -> (Fitted, FillerStats) {
    let (k, (f, m, w)) = (kit(), recipes());
    let (fitted, _, stats) = fit_merge_fill(lots, &k, &f, Some(&m), Some(&w), SEED, &flat).unwrap();
    (fitted, stats.expect("a filler recipe gives filler stats"))
}

/// x span and y span of an instance's world hull.
fn spans(k: &KitFile, i: &Instance) -> ([f64; 2], [f64; 2]) {
    let h = k.model(&i.kit).unwrap().world_hull([i.pos[0], i.pos[1]], i.yaw_deg);
    let span = |a: usize| h.iter().fold([f64::INFINITY, f64::NEG_INFINITY], |[lo, hi], p| [lo.min(p[a]), hi.max(p[a])]);
    (span(0), span(1))
}

fn with_role<'a>(f: &'a Fitted, role: &str) -> Vec<&'a Instance> {
    f.instances.iter().filter(|i| i.role == role).collect()
}

#[test]
fn the_empty_lot_between_two_houses_is_closed_by_whole_walls() {
    let (k, (_, _, w)) = (kit(), recipes());
    let o = w.overlap_max_m.value;
    let (f, s) = run(&row());
    let houses = with_role(&f, "building");
    assert_eq!(houses.len(), 2);
    let walls = with_role(&f, WALL_ROLE);
    // The 14 m between the houses (1 m of each house lot and the 12 m empty lot): four 3.66 m pieces, the 1 m side
    // strips outside the houses take no piece.
    assert_eq!(walls.len(), 4, "{walls:?}");
    assert_eq!((s.walls, s.gaps, s.skipped_by_reason.get("gap_too_short")), (4, 3, Some(&2)));
    let mut cover: Vec<[f64; 2]> = Vec::new();
    for wall in &walls {
        assert_eq!(wall.kit, "wall_366");
        assert_eq!(wall.lot.as_deref(), Some("lot-1"));
        assert_eq!(wall.pos[2], 0.0);
        let (x, y) = spans(&k, wall);
        assert!((x[1] - x[0] - WALL_M).abs() < 1e-9, "whole piece: {x:?}");
        // The outer face on the face line y = 0, the wall inside the lot.
        assert!(y[0].abs() < 1e-9, "outer face at {}", y[0]);
        assert!(y[1] > 2.0);
        for h in &houses {
            let (hx, _) = spans(&k, h);
            let overlap = x[1].min(hx[1]) - x[0].max(hx[0]);
            assert!(overlap <= o + 1e-9, "wall {x:?} over house {hx:?} by {overlap}");
        }
        cover.push(x);
    }
    cover.sort_by(|a, b| a[0].total_cmp(&b[0]));
    // End to end, overlapping by at most `overlap_max_m`.
    for p in cover.windows(2) {
        assert!(p[1][0] <= p[0][1] + 1e-9 && p[0][1] - p[1][0] <= o + 1e-9, "{p:?}");
    }
    assert!(cover[0][0] <= 12.0 && cover[3][1] >= 24.0, "lot-1's face [12, 24] closed: {cover:?}");
    // The walls run from house to house.
    assert!((cover[0][0] - 11.0).abs() < 1e-9 && (cover[3][1] - 25.0).abs() < 1e-9, "{cover:?}");
    assert_eq!(f.lots.iter().find(|l| l.id == "lot-1").unwrap().status, LotStatus::Filler);
    assert_eq!(f.lots.iter().filter(|l| l.status == LotStatus::Filled).count(), 2);
    assert_eq!(s.lots_filler, 1);
    assert!((s.faces_m - 36.0).abs() < 1e-9 && (s.buildings_m - 20.0).abs() < 1e-9, "{s:?}");
    assert!((s.walls_m - 14.0).abs() < 1e-9 && (s.open_m - 2.0).abs() < 1e-9, "{s:?}");
    assert!((s.coverage - 34.0 / 36.0).abs() < 1e-9 && (s.coverage_before - 20.0 / 36.0).abs() < 1e-9);
    assert_eq!(s.arches, 0);
}

#[test]
fn a_4_m_lane_mouth_gets_one_centred_arch_a_12_m_one_none() {
    let k = kit();
    // Run [0, 12], a 4 m lane, run [16, 28], a 12 m street, run [40, 52]: three runs (no shared boundary).
    let lots = vec![lot("a", 0.0, true), lot("b", 16.0, true), lot("c", 40.0, true)];
    let (f, s) = run(&lots);
    assert_eq!((s.runs, s.mouths), (3, 2));
    let arches = with_role(&f, ARCH_ROLE);
    assert_eq!(arches.len(), 1, "{arches:?}");
    let arch = arches[0];
    assert_eq!((arch.kit.as_str(), arch.lot.as_deref()), ("arch_366", None));
    let (x, y) = spans(&k, arch);
    assert!((f64::midpoint(x[0], x[1]) - 14.0).abs() < 1e-9, "centred in the mouth: {x:?}");
    assert!(y[0].abs() < 1e-9, "outer face on the street line: {y:?}");
    assert_eq!(s.skipped_by_reason.get("mouth_too_wide"), Some(&1));
    assert!((s.arches_m - WALL_M).abs() < 1e-9 && (s.frontage_m - 40.0).abs() < 1e-9, "{s:?}");
}

#[test]
fn same_seed_same_bytes() {
    let dirs = ["a", "b"].map(|n| Path::new(env!("CARGO_TARGET_TMPDIR")).join("filler").join(n));
    let header = || Header {
        name: "fixture".into(),
        producer: "settle test".into(),
        recipe: "balmora-x10s".into(),
        seed: SEED.into(),
        kit_blake3: blake3::hash(KIT.as_bytes()).to_hex().to_string(),
        origin_m: [0.0, 0.0],
    };
    let mut lots = row();
    lots.extend([lot("lot-3", 40.0, true), lot("lot-4", 56.0, false)]);
    for d in &dirs {
        let _ = fs::remove_dir_all(d);
        run(&lots).0.file(header()).write(d).unwrap();
    }
    let bytes = dirs.each_ref().map(|d| fs::read(d.join(PLACEMENTS_FILE)).unwrap());
    assert_eq!(bytes[0], bytes[1]);
}

#[test]
fn without_a_filler_recipe_nothing_changes() {
    let (k, (f, m, _)) = (kit(), recipes());
    let lots = row();
    let (mut fitted, merged, filled) = fit_merge_fill(&lots, &k, &f, Some(&m), None, SEED, &flat).unwrap();
    assert!(filled.is_none() && merged.is_some());
    let (mut alone, _) = fit_and_merge(&lots, &k, &f, Some(&m), SEED, &flat);
    // Wall time differs between calls.
    (fitted.stats.fit_ms, alone.stats.fit_ms) = (0.0, 0.0);
    assert_eq!(fitted, alone);
    assert!(fitted.instances.iter().all(|i| i.role == "building"));
}

/// `placements.json` of house, empty, house, a 4 m lane (arch), house, a 12 m street, house and an empty lot, as the
/// filler wrote it before towers existed (slice PL-3b, 2026-11-24).
const PRE_TOWER_BLAKE3: &str = "1e889d0f89f1da42aff04742423fa7a95633498a9fab4462c4854f15565fe2a4";

#[test]
fn without_towers_the_placements_are_the_pre_tower_bytes() {
    let mut lots = row();
    lots.extend([lot("lot-3", 40.0, true), lot("lot-4", 64.0, true), lot("lot-5", 76.0, false)]);
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("filler").join("pre-tower");
    let _ = fs::remove_dir_all(&d);
    let header = Header {
        name: "fixture".into(),
        producer: "settle test".into(),
        recipe: "balmora-x10s".into(),
        seed: SEED.into(),
        kit_blake3: blake3::hash(KIT.as_bytes()).to_hex().to_string(),
        origin_m: [0.0, 0.0],
    };
    run(&lots).0.file(header).write(&d).unwrap();
    let hash = blake3::hash(&fs::read(d.join(PLACEMENTS_FILE)).unwrap()).to_hex().to_string();
    assert_eq!(hash, PRE_TOWER_BLAKE3);
}

/// Tower spacing of the fixture: a bit over its tower's 6 m width (the town recipe's value was this before it was
/// measured on the real town).
const FIXTURE_TOWER_SPACING_M: f64 = 8.0;

/// The fixture recipes with balmora2-x10s-b2w's tower tunables and the fixture tower; a run may bend 60 degrees at
/// one plot (`merge.max_angle_deg` 70) so the bend below is one run.
fn tower_recipes(spacing_m: Option<f64>) -> (FitRecipe, MergeRecipe, FillerRecipe) {
    let (f, mut m, mut w) = recipes();
    let b2w = settle::Recipe::parse(include_str!("../../../recipes/balmora2-x10s-b2w.toml")).unwrap().filler.unwrap();
    let mut towers = b2w.towers.expect("balmora2-x10s-b2w lists towers");
    towers.value = vec!["tower_6".into()];
    w.towers = Some(towers);
    w.tower_turn_deg = b2w.tower_turn_deg;
    w.tower_min_spacing_m = b2w.tower_min_spacing_m;
    // The fixture is a few plots across: it keeps its own spacing unless a test asks for one (the recipe's 80 m is
    // tuned for a whole town and would leave the fixture one tower).
    if let Some(t) = w.tower_min_spacing_m.as_mut() {
        t.value = spacing_m.unwrap_or(FIXTURE_TOWER_SPACING_M);
    }
    m.max_angle_deg.value = 70.0;
    (f, m, w)
}

fn run_with(lots: &[Lot], (f, m, w): &(FitRecipe, MergeRecipe, FillerRecipe)) -> (Fitted, FillerStats) {
    let (fitted, _, stats) = fit_merge_fill(lots, &kit(), f, Some(m), Some(w), SEED, &flat).unwrap();
    (fitted, stats.unwrap())
}

/// An empty lot with its street face `front` (the street on the side away from the polygon's centroid).
fn plot(id: &str, polygon: Vec<Xy>, front: [Xy; 2]) -> Lot {
    let mut l = lot(id, 0.0, false);
    let d = [front[1][0] - front[0][0], front[1][1] - front[0][1]];
    let len = d[0].hypot(d[1]);
    (l.polygon, l.front, l.axis) = (polygon, Some(front), [d[0] / len, d[1] / len]);
    l.access = Some(Access { kind: "public".into(), segment: front, width_m: len });
    l
}

const S3: f64 = 0.866_025_403_784_438_6; // sin 60, cos 30

/// The bend: two empty 12 m plots on the street `y = 0` (x -24..0), then two more along `d = (cos -60, sin -60)` from
/// the origin, the street turning 60 degrees at the origin; the corner plots meet on the bisector.
fn bend() -> Vec<Lot> {
    let (d, m) = ([0.5, -S3], [S3, 0.5]);
    let at = |s: f64, k: f64| [d[0] * s + m[0] * k, d[1] * s + m[1] * k];
    let mitre = [10.0 / S3 / 2.0, 10.0];
    vec![
        plot("a0", rect(-24.0, 0.0, -12.0, 10.0), [[-24.0, 0.0], [-12.0, 0.0]]),
        plot("a1", vec![[-12.0, 0.0], [0.0, 0.0], mitre, [-12.0, 10.0]], [[-12.0, 0.0], [0.0, 0.0]]),
        plot("b0", vec![[0.0, 0.0], at(12.0, 0.0), at(12.0, 10.0), mitre], [[0.0, 0.0], at(12.0, 0.0)]),
        plot("b1", vec![at(12.0, 0.0), at(24.0, 0.0), at(24.0, 10.0), at(12.0, 10.0)], [at(12.0, 0.0), at(24.0, 0.0)]),
    ]
}

/// Middle of a tower's outer face (its corner point) and the direction its local +y faces.
fn corner_of(i: &Instance) -> (Xy, Xy) {
    let r = rotate([0.0, 2.043], i.yaw_deg);
    ([i.pos[0] + r[0], i.pos[1] + r[1]], rotate([0.0, 1.0], i.yaw_deg))
}

fn near(a: Xy, b: Xy) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6
}

/// No tower reaches into a house (their x/y boxes, both axis-aligned here, overlap by at most `overlap_max_m`).
fn clear_of_houses(k: &KitFile, f: &Fitted, o: f64) {
    for t in with_role(f, TOWER_ROLE) {
        let (tx, ty) = spans(k, t);
        for h in with_role(f, "building") {
            let (hx, hy) = spans(k, h);
            let (ox, oy) = (tx[1].min(hx[1]) - tx[0].max(hx[0]), ty[1].min(hy[1]) - ty[0].max(hy[0]));
            assert!(ox <= o + 1e-9 || oy <= o + 1e-9, "tower {t:?} into house {h:?}");
        }
    }
}

#[test]
fn a_bent_run_gets_a_tower_at_the_bend_and_at_both_open_ends() {
    let (k, r) = (kit(), tower_recipes(None));
    let (f, s) = run_with(&bend(), &r);
    assert_eq!(s.runs, 1, "the bend is one run: {s:?}");
    let towers = with_role(&f, TOWER_ROLE);
    assert_eq!((s.towers, towers.len()), (3, 3), "{towers:?} {s:?}");
    assert!(!s.skipped_by_reason.keys().any(|k| k.starts_with("tower")), "{s:?}");
    let spots: Vec<(Xy, Xy)> = towers.iter().map(|t| corner_of(t)).collect();
    let bisector = [-0.5, -S3];
    for (at, facing) in [([-24.0, 0.0], [0.0, -1.0]), ([12.0, -24.0 * S3], [-S3, -0.5]), ([0.0, 0.0], bisector)] {
        let hit = spots.iter().find(|(p, _)| near(*p, at)).unwrap_or_else(|| panic!("no tower at {at:?}: {spots:?}"));
        assert!(near(hit.1, facing), "tower at {at:?} faces {:?}, not {facing:?}", hit.1);
    }
    assert!(towers.iter().all(|t| t.lot.is_none() && t.kit == "tower_6" && t.pos[2] == 0.0));
    // Walls stop against the towers: on the straight leg (outer face on y = 0), no wall runs more than
    // `overlap_max_m` into the end tower's [-27, -21] or the bend tower's x >= -3.
    let o = r.2.overlap_max_m.value;
    let leg: Vec<[f64; 2]> =
        with_role(&f, WALL_ROLE).iter().map(|w| spans(&k, w)).filter(|(_, y)| y[0].abs() < 1e-9).map(|(x, _)| x).collect();
    assert!(!leg.is_empty());
    for x in &leg {
        assert!(x[0] >= -21.0 - o - 1e-9 && x[1] <= -3.0 + o + 1e-9, "wall {x:?} runs into a tower");
    }
    assert!(s.walls > 0 && s.towers_m > 0.0 && s.walls_m + s.towers_m <= s.faces_m + 1e-9, "{s:?}");
}

#[test]
fn towers_keep_their_spacing() {
    // Ends first (gate towers, then corners): the bend tower stands 24 m from the first end.
    let (f, s) = run_with(&bend(), &tower_recipes(Some(30.0)));
    assert_eq!(s.towers, 2, "{s:?}");
    assert_eq!(s.skipped_by_reason.get("tower_spacing"), Some(&1));
    let spots: Vec<Xy> = with_role(&f, TOWER_ROLE).iter().map(|t| corner_of(t).0).collect();
    assert!(spots.iter().all(|p| !near(*p, [0.0, 0.0])), "{spots:?}");
}

#[test]
fn run_ends_beside_an_arch_get_no_tower_open_street_corners_do() {
    let (k, r) = (kit(), tower_recipes(None));
    // Run [0, 12], a 4 m lane (arch), run [16, 28], a 12 m street, run [40, 52]; all plots empty.
    let lots = vec![lot("a", 0.0, false), lot("b", 16.0, false), lot("c", 40.0, false)];
    let (f, s) = run_with(&lots, &r);
    assert_eq!((s.runs, s.arches), (3, 1), "{s:?}");
    let mut xs: Vec<f64> = with_role(&f, TOWER_ROLE)
        .iter()
        .map(|t| {
            let (p, facing) = corner_of(t);
            assert!(p[1].abs() < 1e-9 && near(facing, [0.0, -1.0]), "{t:?}");
            p[0]
        })
        .collect();
    xs.sort_by(f64::total_cmp);
    assert_eq!(xs, [0.0, 28.0, 40.0, 52.0]);
    clear_of_houses(&k, &f, r.2.overlap_max_m.value);
}

#[test]
fn no_tower_stands_in_a_house() {
    let (k, r) = (kit(), tower_recipes(None));
    // House, empty, house: both run ends lie on a house lot.
    let (f, s) = run_with(&row(), &r);
    assert_eq!((s.towers, s.skipped_by_reason.get("tower_overlap")), (0, Some(&2)), "{s:?}");
    assert_eq!(s.walls, 4);
    clear_of_houses(&k, &f, r.2.overlap_max_m.value);
}
