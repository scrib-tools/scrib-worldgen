//! Milestone 2 file formats (slice PL-1): `kit.json` parse + world helpers, `placements.json` and `grade.json`
//! round trips, version and hash refusals, byte-identical output.
#![allow(clippy::float_cmp)] // exact assertions are the point
use settle::formats::{
    Door, Feature, FeatureKind, GRADE_FILE, Garden, GardenKind, GardenSource, GradeBreak, GRADE_FORMAT, GRADE_FRAME, GRADE_HEIGHTS_FILE, GRADE_WEIGHTS_FILE,
    GROUND_FILE, GROUND_LEGEND, Grade, GradeMeta, Instance, KitFile, KitKind, LotStatus, PLACEMENTS_FILE,
    PLACEMENTS_FORMAT, PlacedLot, PlacementsFile, PlannedStreet, WorldDoor,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Six boxes: two houses (one with two doors), a tower, a wall, an arch, stairs. Optional fields absent, null, or set.
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

fn kit() -> KitFile {
    KitFile::from_json(KIT).expect("synthetic kit parses")
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("formats").join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn kit_parses_with_optional_fields_absent_or_null() {
    let k = kit();
    assert_eq!(k.models.len(), 6);
    let kinds: Vec<KitKind> = k.models.iter().map(|m| m.kind).collect();
    assert_eq!(
        kinds,
        [KitKind::Building, KitKind::Building, KitKind::Building, KitKind::Wall, KitKind::Arch, KitKind::Stairs]
    );
    let h = k.model("house_10x8").unwrap();
    assert_eq!(h.floors, Some(2));
    assert_eq!(h.roles.as_deref(), Some(&["dwelling".to_string()][..]));
    assert_eq!(h.obb.as_ref().unwrap().half, [5.0, 4.0]);
    let big = k.model("house_14x10").unwrap();
    assert_eq!((big.tags.as_ref(), big.roles.as_ref(), big.floors, big.yaw_step_deg), (None, None, None, None));
    assert_eq!(big.doors.len(), 2);
    assert_eq!(k.model("tower_8x8").unwrap().yaw_step_deg, Some(90.0));
    assert!(k.model("stairs_3x6").unwrap().doors.is_empty());
    assert!(k.model("nope").is_none());
}

#[test]
fn world_hull_yaw_0_and_90_exact() {
    let k = kit();
    let h = k.model("house_10x8").unwrap();
    assert_eq!(h.world_hull([100.0, 200.0], 0.0), vec![[95.0, 196.0], [105.0, 196.0], [105.0, 204.0], [95.0, 204.0]]);
    // R(90): (x, y) -> (-y, x).
    assert_eq!(h.world_hull([100.0, 200.0], 90.0), vec![[104.0, 195.0], [104.0, 205.0], [96.0, 205.0], [96.0, 195.0]]);
    // Quarter turns stay exact whatever the winding.
    assert_eq!(h.world_hull([0.0, 0.0], -270.0), h.world_hull([0.0, 0.0], 90.0));
    assert_eq!(h.world_hull([0.0, 0.0], 180.0), vec![[5.0, 4.0], [-5.0, 4.0], [-5.0, -4.0], [5.0, -4.0]]);
}

#[test]
fn world_hull_yaw_37() {
    let k = kit();
    let h = k.model("house_10x8").unwrap();
    let (s, c) = 37f64.to_radians().sin_cos();
    let pos = [100.0, 200.0];
    let got = h.world_hull(pos, 37.0);
    for (g, l) in got.iter().zip(&h.footprint) {
        let want = [pos[0] + (l[0] * c - l[1] * s), pos[1] + (l[0] * s + l[1] * c)];
        assert_eq!(*g, want, "local {l:?}");
    }
    // A rigid motion: edges keep their 10 m and 8 m.
    let len = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    assert!((len(got[0], got[1]) - 10.0).abs() < 1e-12);
    assert!((len(got[1], got[2]) - 8.0).abs() < 1e-12);
}

#[test]
fn doors_rotate_with_the_model() {
    let k = kit();
    let big = k.model("house_14x10").unwrap();
    // Front door (0, 5) facing +y and side door (7, 0) facing +x, at yaw 90: front faces -x, side faces +y.
    assert_eq!(
        big.world_doors([10.0, 20.0], 90.0),
        vec![WorldDoor { at: [5.0, 20.0], facing_deg: 180.0 }, WorldDoor { at: [10.0, 27.0], facing_deg: 90.0 }]
    );
    assert_eq!(big.world_doors([10.0, 20.0], 0.0)[0], WorldDoor { at: [10.0, 25.0], facing_deg: 90.0 });
    let d37 = big.world_doors([10.0, 20.0], 37.0);
    let (s, c) = 37f64.to_radians().sin_cos();
    assert_eq!(d37[0], WorldDoor { at: [10.0 - 5.0 * s, 20.0 + 5.0 * c], facing_deg: 127.0 });
    assert_eq!(d37[1], WorldDoor { at: [10.0 + 7.0 * c, 20.0 + 7.0 * s], facing_deg: 37.0 });
    // The facing vector, rotated from local, matches the world facing angle.
    let door = Door { at: [0.0, 0.0], facing_deg: 300.0, width_m: 1.0, sill_z_m: 0.0 };
    assert_eq!(door.to_world([0.0, 0.0], 90.0).facing_deg, 30.0);
}

#[test]
fn kit_refusals() {
    let e = KitFile::from_json(&KIT.replacen("\"format\": 1", "\"format\": 2", 1)).unwrap_err();
    assert!(e.contains("format 2"), "{e}");
    let e = KitFile::from_json(&KIT.replacen("\"kind\": \"arch\"", "\"kind\": \"spaceship\"", 1)).unwrap_err();
    assert!(e.contains("spaceship"), "{e}");
    let e = KitFile::from_json(&KIT.replacen("\"id\": \"arch_4\"", "\"id\": \"wall_6\"", 1)).unwrap_err();
    assert!(e.contains("duplicate"), "{e}");
}

fn placements() -> PlacementsFile {
    PlacementsFile {
        format: PLACEMENTS_FORMAT,
        name: "fixture".into(),
        producer: "settle test".into(),
        recipe: "river".into(),
        seed: "7".into(),
        kit_blake3: blake3::hash(KIT.as_bytes()).to_hex().to_string(),
        origin_m: [1000.0, -2000.5],
        lots: vec![
            PlacedLot {
                id: "p12".into(),
                polygon: vec![[0.0, 0.0], [12.0, 0.0], [12.0, 10.0], [0.0, 10.0]],
                status: LotStatus::Filled,
                role: Some("dwelling".into()),
                pad_z_m: Some(12.25),
                front: Some([[0.0, 0.0], [12.0, 0.0]]),
                merged_from: vec![],
            },
            PlacedLot {
                id: "p13".into(),
                polygon: vec![[12.0, 0.0], [30.0, 0.0], [30.0, 10.0]],
                status: LotStatus::Merged,
                role: None,
                pad_z_m: None,
                front: None,
                merged_from: vec!["p14".into(), "p15".into()],
            },
            PlacedLot {
                id: "p16".into(),
                polygon: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
                status: LotStatus::Empty,
                role: None,
                pad_z_m: None,
                front: None,
                merged_from: vec![],
            },
        ],
        instances: vec![
            Instance {
                id: "i0".into(),
                kit: "house_10x8".into(),
                lot: Some("p12".into()),
                pos: [6.0, 5.0, 12.25],
                yaw_deg: 37.5,
                role: "building".into(),
                door: Some([6.1, 0.3]),
            },
            Instance {
                id: "i1".into(),
                kit: "wall_6".into(),
                lot: None,
                pos: [20.0, 0.4, 11.0],
                yaw_deg: 0.0,
                role: "wall".into(),
                door: None,
            },
        ],
        streets: vec![PlannedStreet {
            id: "s3".into(),
            rank: 1,
            role: "lane".into(),
            polyline: vec![[0.0, -2.0, 12.0], [30.0, -2.0, 11.1]],
            widths_m: vec![4.0, 3.2],
            surface: "street_rank1".into(),
        }],
        features: vec![
            Feature::Grade(GradeBreak {
                kind: FeatureKind::RetainingWall,
                polyline: vec![[0.0, 10.0, 12.25], [12.0, 10.0, 12.25]],
                base_z_m: 9.5,
                top_z_m: 12.25,
            }),
            Feature::Garden(Garden {
                kind: GardenKind::Garden,
                source: GardenSource::Sliver,
                lot: "p7".into(),
                polygon: vec![[0.0, 0.0], [3.0, 0.0], [3.0, 9.0], [0.0, 9.0]],
            }),
        ],
        grade: Some(GRADE_FILE.into()),
    }
}

#[test]
fn placements_round_trip_and_bytes() {
    let p = placements();
    let dir = tmp("placements");
    p.write(&dir).unwrap();
    assert_eq!(PlacementsFile::load(&dir).unwrap(), p);
    let again = tmp("placements-again");
    placements().write(&again).unwrap();
    assert_eq!(fs::read(dir.join(PLACEMENTS_FILE)).unwrap(), fs::read(again.join(PLACEMENTS_FILE)).unwrap());
    // No grade yet: the field is left out and reads back as None.
    let mut no_grade = placements();
    no_grade.grade = None;
    let text = no_grade.to_json();
    assert!(!text.contains("\"grade\""));
    assert_eq!(PlacementsFile::from_json(&text).unwrap(), no_grade);
}

#[test]
fn placements_refusals() {
    let text = placements().to_json();
    let e = PlacementsFile::from_json(&text.replacen("\"format\": 1", "\"format\": 3", 1)).unwrap_err();
    assert!(e.contains("format 3"), "{e}");
    let e = PlacementsFile::from_json(&text.replacen("\"status\": \"filled\"", "\"status\": \"full\"", 1)).unwrap_err();
    assert!(e.contains("full"), "{e}");
    let mut bad = placements();
    bad.streets[0].widths_m.pop();
    let e = PlacementsFile::from_json(&bad.to_json()).unwrap_err();
    assert!(e.contains("1 widths for 2 vertices"), "{e}");
}

fn grade() -> Grade {
    let (w, h) = (5, 3);
    let heights = (0..w * h).map(|i| 10.0 + f32::from(u8::try_from(i).unwrap()) * 0.125).collect();
    let weights = (0..w * h).map(|i| [0.0, 0.25, 0.5, 1.0][i % 4]).collect();
    let ground = (0..w * h).map(|i| u8::try_from(i % GROUND_LEGEND.len()).unwrap()).collect();
    Grade {
        meta: GradeMeta {
            format: GRADE_FORMAT,
            frame: GRADE_FRAME.into(),
            origin_m: [500.5, 800.0],
            spacing_m: 2.0,
            dims: [w, h],
            ground_legend: GROUND_LEGEND.iter().map(|s| (*s).to_string()).collect(),
            blake3: BTreeMap::new(),
            max_cut_m: 3.5,
            max_fill_m: 2.25,
            cut_m3: 120.0,
            fill_m3: 80.5,
            blend_band_m: 12.0,
            source: "river seed 7".into(),
        },
        heights,
        weights,
        ground,
    }
}

#[test]
fn grade_round_trip_and_bytes() {
    let mut g = grade();
    let dir = tmp("grade");
    g.write(&dir).unwrap();
    assert_eq!(g.meta.blake3.len(), 3);
    assert_eq!(Grade::load(&dir).unwrap(), g);
    // Stamp-format alignment: sample (0, 0) is the origin, row 0 is north, columns run east.
    assert_eq!(g.meta.sample_pos(0, 0), [500.5, 800.0]);
    assert_eq!(g.meta.sample_pos(4, 2), [508.5, 796.0]);
    let again = tmp("grade-again");
    grade().write(&again).unwrap();
    for f in [GRADE_FILE, GRADE_HEIGHTS_FILE, GRADE_WEIGHTS_FILE, GROUND_FILE] {
        assert_eq!(fs::read(dir.join(f)).unwrap(), fs::read(again.join(f)).unwrap(), "{f}");
    }
}

#[test]
fn grade_refusals() {
    let dir = tmp("grade-bad");
    grade().write(&dir).unwrap();
    // Raster changed after hashing.
    let mut bytes = fs::read(dir.join(GRADE_HEIGHTS_FILE)).unwrap();
    bytes[0] ^= 1;
    fs::write(dir.join(GRADE_HEIGHTS_FILE), &bytes).unwrap();
    let e = Grade::load(&dir).unwrap_err();
    assert!(e.contains("blake3") && e.contains(GRADE_HEIGHTS_FILE), "{e}");
    // Wrong format.
    let dir = tmp("grade-format");
    grade().write(&dir).unwrap();
    let text = fs::read_to_string(dir.join(GRADE_FILE)).unwrap();
    fs::write(dir.join(GRADE_FILE), text.replacen("\"format\": 1", "\"format\": 2", 1)).unwrap();
    let e = Grade::load(&dir).unwrap_err();
    assert!(e.contains("format 2"), "{e}");
    // Raster sizes that do not match dims are refused on write.
    let mut short = grade();
    short.weights.pop();
    assert!(short.write(&tmp("grade-short")).is_err());
}

#[test]
#[ignore = "needs Scrib's kit export"]
fn scrib_kit_export_parses() {
    let path = Path::new("D:/Gamedev/Bevyrrowind/target/worldgen/kit/balmora/kit.json");
    if !path.exists() {
        println!("no Scrib kit export at {}", path.display());
        return;
    }
    let k = KitFile::load(path).unwrap();
    println!("{}: {} models, {} with doors", path.display(), k.models.len(), k.models.iter().filter(|m| !m.doors.is_empty()).count());
}
