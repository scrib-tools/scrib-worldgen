//! `streets.json` (`settle::streets`, slice S1a): a town's Magna Urbis streets as a road network v1 file in the guide
//! frame. Fixture: a 20 m site raster (1 m cells, north-west corner at world (100, 200)) rising 0.1 m per m eastward,
//! with a one-cell river at column 13; three streets in Magna Urbis's local frame (x east, y south): a gate-street
//! (rank 1) and a street (rank 2) sharing an end, the street crossing the river, and a close (rank 4) whose end stands
//! 0.2 m off the shared one. The class/surface tables are the real recipe's (`recipes/balmora2-x10s-b2w.toml`).
// Fixture raster maths: sizes are small constants.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
// Points are moved, not computed (the frame shift is exact on these values): exact.
#![allow(clippy::float_cmp)]
use magna_urbis_core::urban::{Point, Street};
use serde::Deserialize;
use settle::Recipe;
use settle::grid::Frame;
use settle::recipe::StreetsRecipe;
use settle::site::{SiteMeta, SiteRaster};
use settle::streets::{self, StreetsFile};
use std::collections::BTreeMap;

const SIDE: usize = 20;
const ORIGIN_M: [f64; 2] = [100.0, 200.0];
const RIVER_COL: usize = 13;
const RISE_PER_M: f64 = 0.1;

fn site() -> SiteRaster {
    let heights = (0..SIDE * SIDE).map(|i| (RISE_PER_M * (i % SIDE) as f64) as f32).collect();
    let water = (0..SIDE * SIDE).map(|i| u8::from(i % SIDE == RIVER_COL)).collect();
    let meta = SiteMeta {
        format: 1,
        name: "fixture".into(),
        producer: "test".into(),
        origin_m: ORIGIN_M,
        cell_m: 1.0,
        dims: [SIDE, SIDE],
        water_level_m: 0.0,
        centre_m: [110.0, 190.0],
        source: "test".into(),
        claim: None,
        blake3: BTreeMap::new(),
    };
    SiteRaster { meta, heights, water, roads: None }
}

fn frame() -> Frame {
    Frame { west_m: ORIGIN_M[0], north_m: ORIGIN_M[1], width_m: SIDE as f64, n: SIDE }
}

fn street(points: &[(f64, f64)], rank: usize, role: &str) -> Street {
    let path: Vec<Point> = points.iter().map(|&(x, y)| Point { x, y }).collect();
    Street { widths: vec![4.0; path.len()], path, rank, role: role.into() }
}

fn spec() -> StreetsRecipe {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../recipes/balmora2-x10s-b2w.toml");
    let recipe = Recipe::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    recipe.streets.expect("the recipe has a [streets] table")
}

fn fixture() -> StreetsFile {
    let town = [
        street(&[(2.0, 5.0), (8.0, 5.0)], 1, "gate-street"),
        street(&[(8.0, 5.0), (12.0, 5.0), (18.0, 5.0)], 2, "street"),
        street(&[(8.2, 5.0), (8.2, 12.0)], 4, "close"),
    ];
    let regional = [street(&[(0.0, 5.0), (20.0, 5.0)], 1, "regional")];
    streets::streets("fixture", &town, &frame(), &site(), &regional, &spec())
}

#[test]
fn streets_share_junctions_bridge_the_river_and_take_the_table() {
    let file = fixture();
    assert_eq!(file.segments.len(), 3);
    let [gate, main, close] = [&file.segments[0], &file.segments[1], &file.segments[2]];
    // One shared junction for the three ends at local (8, 5); the close's 0.2 m-off end merged onto it.
    assert_eq!(gate.b, main.a);
    assert_eq!(close.a, main.a);
    assert_eq!(file.junctions.len(), 4);
    let shared = &file.junctions[main.a as usize];
    assert_eq!(shared.kind, "junction");
    assert_eq!(close.points[0], shared.pos);
    // Guide frame: local (x, y south) -> (100 + x, 200 - y).
    assert_eq!(gate.points, vec![[102.0, 195.0], [108.0, 195.0]]);
    assert_eq!(shared.pos, [108.0, 195.0]);
    // Table: rank 1 main + flagstone, rank 2 local + cobble, role close trail + dirt.
    assert_eq!((gate.class.as_str(), gate.surface.as_str()), ("main", "flagstone"));
    assert_eq!((main.class.as_str(), main.surface.as_str()), ("local", "cobble"));
    assert_eq!((close.class.as_str(), close.surface.as_str()), ("trail", "dirt"));
    // The river (world x 113..114) is bridged, padded, on the street's line; the others cross no water.
    assert!(gate.crossings.is_empty() && close.crossings.is_empty());
    assert_eq!(main.crossings.len(), 1);
    let bridge = &main.crossings[0];
    assert_eq!(bridge.kind, "bridge");
    let (first, last) = (bridge.points[0], bridge.points[bridge.points.len() - 1]);
    assert!(first[0] <= 113.0 - 1.0 && first[0] > 108.0, "bridge starts at {first:?}");
    assert!(last[0] >= 114.0 + 1.0 && last[0] < 118.0, "bridge ends at {last:?}");
    assert!(bridge.points.iter().all(|p| (p[1] - 195.0).abs() < 1e-9));
    // Length along the path; grade on the site heights (0.1 rise per m east), the bridge span left out.
    assert!((main.length_m - 10.0).abs() < 1e-9);
    assert!((gate.grade - RISE_PER_M).abs() < 1e-4, "gate grade {}", gate.grade);
    assert!(close.grade < 1e-4, "close runs north-south, flat: {}", close.grade);
    // Along the regional road: the gate-street and the street; the close leaves it.
    assert!(gate.regional && main.regional && !close.regional);
    assert!((gate.width_m - 4.0).abs() < 1e-9);
}

// The reader's shape (Scrib `superfargoth/crates/sf_worldgen/src/roads.rs` `FileV1`, `Segment`, `Crossing`,
// `Junction`), copied, not linked.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
enum RoadClass {
    Highway,
    Main,
    Local,
    Track,
    Trail,
    #[serde(other)]
    Other,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
enum CrossingKind {
    Bridge,
    Ford,
    Causeway,
    Access,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct ReaderCrossing {
    kind: CrossingKind,
    points: Vec<[f64; 2]>,
}

#[derive(Deserialize)]
struct ReaderSegment {
    id: u32,
    class: RoadClass,
    #[serde(default)]
    surface: String,
    width_m: f64,
    a: u32,
    b: u32,
    points: Vec<[f64; 2]>,
    #[serde(default)]
    crossings: Vec<ReaderCrossing>,
    #[serde(default)]
    length_m: f64,
    #[serde(default)]
    grade: f64,
}

#[derive(Deserialize)]
struct ReaderJunction {
    id: u32,
    pos: [f64; 2],
    #[serde(default)]
    kind: String,
}

#[derive(Deserialize)]
struct ReaderFile {
    format: u32,
    #[serde(default)]
    segments: Vec<ReaderSegment>,
    #[serde(default)]
    junctions: Vec<ReaderJunction>,
}

#[test]
fn streets_json_reads_as_road_network_v1() {
    let file = fixture();
    let text = serde_json::to_string(&file).unwrap();
    let read: ReaderFile = serde_json::from_str(&text).unwrap();
    assert_eq!(read.format, 1);
    assert_eq!(read.segments.len(), file.segments.len());
    assert_eq!(read.junctions.len(), file.junctions.len());
    let classes: Vec<&RoadClass> = read.segments.iter().map(|s| &s.class).collect();
    assert_eq!(classes, [&RoadClass::Main, &RoadClass::Local, &RoadClass::Trail]);
    for (r, s) in read.segments.iter().zip(&file.segments) {
        assert_eq!((r.id, r.a, r.b, &r.points, r.surface.as_str()), (s.id, s.a, s.b, &s.points, s.surface.as_str()));
        assert!(r.points.len() >= 2 && r.width_m > 0.0);
        assert!((r.length_m - s.length_m).abs() < 1e-12 && (r.grade - s.grade).abs() < 1e-12);
    }
    assert_eq!(read.segments[1].crossings.len(), 1);
    assert_eq!(read.segments[1].crossings[0].kind, CrossingKind::Bridge);
    assert!(read.segments[1].crossings[0].points.len() >= 2);
    let ids: Vec<u32> = read.junctions.iter().map(|j| j.id).collect();
    assert_eq!(ids, (0..read.junctions.len() as u32).collect::<Vec<_>>());
    assert!(read.junctions.iter().all(|j| j.pos[0].is_finite() && !j.kind.is_empty()));
    // The writer's own shape reads it back too (floats to serde_json's text precision).
    let back: StreetsFile = serde_json::from_str(&text).unwrap();
    assert_eq!(back.segments.iter().map(|s| &s.role).collect::<Vec<_>>(), file.segments.iter().map(|s| &s.role).collect::<Vec<_>>());
}
