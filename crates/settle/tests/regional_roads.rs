//! `settle --roads`: a town laid along the guide's regional roads (`settle::regional`, port of Magna Urbis's
//! `regionalStreets`, `rust/bridge/urbanRoads.ts:6`). The river fixture (as `river.rs`: gentle valley, one straight
//! ~30 m river east-west through the middle) with a hand-written `roads.json`: one road north-south across the river
//! with a bridge crossing, running off the crop both ways, and one road far off the crop. The layout carries the road
//! as regional streets and its bridge on the road's line, and the town's own streets join it. Without a roads file
//! nothing changes (the other site tests pin today's output).
// Fixture raster maths: sizes are small constants.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
// Widths are copied, not computed: exact.
#![allow(clippy::float_cmp)]
use magna_urbis_core::urban::{Layout, Point, Street};
use settle::regional::{self, RoadSegment};
use settle::site::{SiteMeta, SiteRaster};
use settle::{Recipe, RegionalRoads, prepare, prepare_with_roads};
use std::collections::BTreeMap;
use std::path::Path;

const CELL_M: f64 = 5.0;
const SIDE_CELLS: usize = 480;
const ORIGIN_M: [f64; 2] = [10_000.0, 20_000.0];
const RIVER_HALF_WIDTH_M: f64 = 15.0;
const WATER_LEVEL_M: f64 = 10.0;
const BANK_M: f32 = 12.0;
const SLOPE: f32 = 0.02;
const BED_DEPTH_M: f32 = 1.5;
const CULTURE: &str = "european-organic";
/// The road's width (m): a `main` road, Magna Urbis rank 1.
const ROAD_WIDTH_M: f64 = 6.0;
/// How far (m) a street end may stand from the regional road and still join it: Magna Urbis snaps junctions to the
/// road's centreline; well under a street width.
const JOIN_TOLERANCE_M: f64 = 0.5;
/// How far (m) a bridge may stand off the road's line: Magna Urbis fits the deck on the given path.
const LINE_TOLERANCE_M: f64 = 0.5;

fn centre() -> [f64; 2] {
    let half = SIDE_CELLS as f64 * CELL_M / 2.0;
    [ORIGIN_M[0] + half, ORIGIN_M[1] - half]
}

fn river_site() -> SiteRaster {
    let [_, cy] = centre();
    let mut heights = Vec::new();
    let mut water = Vec::new();
    for row in 0..SIDE_CELLS {
        let y = ORIGIN_M[1] - (row as f64 + 0.5) * CELL_M;
        let off = (y - cy).abs();
        for _ in 0..SIDE_CELLS {
            let wet = off < RIVER_HALF_WIDTH_M;
            water.push(u8::from(wet));
            heights.push(if wet { WATER_LEVEL_M as f32 - BED_DEPTH_M } else { BANK_M + SLOPE * off as f32 });
        }
    }
    SiteRaster {
        meta: SiteMeta {
            format: 1,
            name: "river roads fixture".into(),
            producer: "settle tests".into(),
            origin_m: ORIGIN_M,
            cell_m: CELL_M,
            dims: [SIDE_CELLS, SIDE_CELLS],
            water_level_m: WATER_LEVEL_M,
            centre_m: centre(),
            source: "synthetic: slope + straight river".into(),
            claim: None,
            blake3: BTreeMap::new(),
        },
        heights,
        water,
        roads: None,
    }
}

/// A `roads.json` (format 1, guide frame, extra fields as `worldgen-roads` writes them): road 0 north-south through
/// the centre, 3 km long, bridged over the river's banks; road 1 far east of the crop.
fn roads_json() -> String {
    let [cx, cy] = centre();
    let bank = RIVER_HALF_WIDTH_M + 3.0;
    serde_json::json!({
        "format": 1,
        "tool": "settle tests",
        "segments": [
            {
                "id": 0, "class": "main", "surface": "dirt", "width_m": ROAD_WIDTH_M, "a": 0, "b": 1,
                "points": [[cx, cy + 1500.0], [cx, cy + 400.0], [cx, cy - 400.0], [cx, cy - 1500.0]],
                "crossings": [{"kind": "bridge", "points": [[cx, cy + bank], [cx, cy - bank]]}],
                "length_m": 3000.0, "grade": 0.02
            },
            {
                "id": 1, "class": "track", "surface": "dirt", "width_m": 3.0, "a": 1, "b": 2,
                "points": [[cx + 9000.0, cy], [cx + 9500.0, cy]],
                "crossings": [], "length_m": 500.0, "grade": 0.0
            }
        ],
        "junctions": []
    })
    .to_string()
}

fn recipe() -> Recipe {
    let mut recipe = Recipe::parse(include_str!("../../../recipes/example.toml")).unwrap();
    recipe.cultures.value = vec![CULTURE.into()];
    recipe
}

fn distance_to_path(path: &[Point], p: Point) -> f64 {
    path.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy).max(1e-18)).clamp(0.0, 1.0);
            p.distance(Point { x: a.x + dx * t, y: a.y + dy * t })
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn the_port_splits_a_road_at_its_crossing() {
    let p = |x, y| Point { x, y };
    let segment = RoadSegment {
        class: "highway".into(),
        width: 8.0,
        path: vec![p(0.0, 0.0), p(100.0, 0.0), p(200.0, 0.0)],
        crossings: vec![("ford".into(), vec![p(80.0, 2.0), p(120.0, -2.0)]), ("bridge".into(), vec![p(5.0, 0.0)])],
    };
    let streets = regional::regional_streets(&[segment]);
    let roles: Vec<&str> = streets.iter().map(|s| s.role.as_str()).collect();
    assert_eq!(roles, ["regional", "ford", "regional"], "a one-point crossing is dropped, the ford splits the road");
    assert_eq!(streets[1].path, vec![p(80.0, 0.0), p(100.0, 0.0), p(120.0, 0.0)]);
    assert_eq!(streets[0].path, vec![p(0.0, 0.0), p(80.0, 0.0)]);
    assert_eq!(streets[2].path, vec![p(120.0, 0.0), p(200.0, 0.0)]);
    assert!(streets.iter().all(|s| s.rank == 0 && s.widths.len() == s.path.len() && s.widths.iter().all(|&w| w == 8.0)));
}

#[test]
fn the_town_grows_along_the_regional_road_and_its_bridge() {
    let mut site = river_site();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("river-roads-site");
    site.write(&dir).unwrap();
    let recipe = recipe();
    let roads = RegionalRoads::parse(&roads_json()).unwrap();

    // Without a roads file: no regional road on this recipe (no [crossing]), as before.
    let plain = prepare(&site, &recipe).unwrap();
    assert!(plain.roads.is_empty() && plain.regional.is_none());

    // The prepared roads: road 0 clipped to the crop, split at its bridge; road 1 is off the crop.
    let prepared = prepare_with_roads(&site, &recipe, Some(&roads)).unwrap();
    let frame = prepared.grid.frame;
    let road_x = frame.to_local(centre()[0], centre()[1]).x;
    let by_role = regional::by_role(&prepared.roads);
    assert_eq!(by_role, BTreeMap::from([("bridge".to_string(), 1), ("regional".to_string(), 2)]));
    let cell = frame.width_m / frame.n as f64;
    for s in &prepared.roads {
        assert!(s.rank == 1 && s.widths.iter().all(|&w| w == ROAD_WIDTH_M));
        for q in &s.path {
            assert!((q.x - road_x).abs() < 1e-9, "off the road's line: {q:?}");
            assert!(q.y >= cell - 1e-9 && q.y <= frame.width_m - cell + 1e-9, "off the crop: {q:?}");
        }
    }

    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("river-roads-out");
    let stats = settle::settle_with(&dir, &recipe, &out, None, Some(&roads)).unwrap();
    assert_eq!(stats[0]["regional_roads_by_role"]["bridge"], 1);
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join(CULTURE).join("layout.json")).unwrap()).unwrap();
    assert_eq!(written["regional_roads"]["blake3"], roads.blake3.as_str());
    let layout: Layout = serde_json::from_value(written["layout"].clone()).unwrap();

    let role = |r: &'static str| layout.streets.iter().filter(move |s| s.role == r);
    let regional: Vec<&Street> = role("regional").collect();
    assert!(!regional.is_empty(), "no regional street in the layout");
    let on_line = |s: &Street| s.path.iter().all(|q| (q.x - road_x).abs() < LINE_TOLERANCE_M);
    let bridges: Vec<&Street> = role("bridge").collect();
    let bridges_on_line = bridges.iter().filter(|s| on_line(s)).count();
    eprintln!(
        "streets {}, regional {}, bridges {} ({} on the road's line)",
        layout.streets.len(),
        regional.len(),
        bridges.len(),
        bridges_on_line
    );
    assert!(bridges_on_line >= 1, "no bridge on the road's line: {bridges:?}");

    // The town's own streets join the regional road: some street end lies on it.
    let network = ["regional", "approach", "bridge", "ford", "causeway", "outbound"];
    let joined = layout
        .streets
        .iter()
        .filter(|s| !network.contains(&s.role.as_str()))
        .flat_map(|s| [s.path[0], s.path[s.path.len() - 1]])
        .filter(|&q| regional.iter().any(|r| distance_to_path(&r.path, q) < JOIN_TOLERANCE_M))
        .count();
    eprintln!("town street ends on the regional road: {joined}");
    assert!(joined > 0, "no town street joins the regional road");
}
