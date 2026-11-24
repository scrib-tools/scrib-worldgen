//! ST-3a: a synthetic site (gentle valley slopes, one straight ~30 m river east-west across the middle, its bed below
//! the water level) through the library: streets and lots on both banks, no lot in the water, one seed = one layout.
// Fixture raster maths: sizes are small constants.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
use magna_urbis_core::urban::Point;
use settle::site::{SiteMeta, SiteRaster};
use settle::{Recipe, prepare, run_culture};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CELL_M: f64 = 5.0;
const SIDE_CELLS: usize = 480;
const ORIGIN_M: [f64; 2] = [10_000.0, 20_000.0];
const RIVER_HALF_WIDTH_M: f64 = 15.0;
const WATER_LEVEL_M: f64 = 10.0;
const BANK_M: f32 = 12.0;
const SLOPE: f32 = 0.02;
const BED_DEPTH_M: f32 = 1.5;
const CULTURE: &str = "european-organic";

fn centre() -> [f64; 2] {
    let half = SIDE_CELLS as f64 * CELL_M / 2.0;
    [ORIGIN_M[0] + half, ORIGIN_M[1] - half]
}

/// The fixture: a valley rising 2 % away from a straight river through `centre()`.
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
            name: "river fixture".into(),
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

/// Written where the CLI can reach it after the test (`CARGO_TARGET_TMPDIR`, on the build drive).
fn fixture_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("river-site");
    river_site().write(&dir).unwrap();
    dir
}

fn recipe() -> Recipe {
    let mut recipe = Recipe::parse(include_str!("../../../recipes/example.toml")).unwrap();
    recipe.cultures.value = vec![CULTURE.into()];
    recipe
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Bank {
    North,
    South,
    /// Touches or crosses the river band.
    Wet,
}

/// Which bank a polygon or polyline lies on in Magna Urbis's local frame (y south). Exact for a straight
/// east-west band: a connected shape with every vertex on one side lies wholly on that side.
fn bank(points: &[Point], band: (f64, f64)) -> Bank {
    if points.iter().all(|p| p.y <= band.0) {
        Bank::North
    } else if points.iter().all(|p| p.y >= band.1) {
        Bank::South
    } else {
        Bank::Wet
    }
}

#[test]
fn river_town_builds_both_banks_stays_dry_and_is_deterministic() {
    let dir = fixture_dir();
    let site = SiteRaster::load(&dir).unwrap();
    assert_eq!(site.heights.len(), SIDE_CELLS * SIDE_CELLS);
    let recipe = recipe();
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("river-out");
    let stats = settle::settle(&dir, &recipe, &out).unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&stats).unwrap());
    let written = std::fs::read_to_string(out.join(CULTURE).join("layout.json")).unwrap();
    assert!(out.join(CULTURE).join("sheet.png").exists() && out.join(CULTURE).join("stats.json").exists());

    let prepared = prepare(&site, &recipe).unwrap();
    let run = run_culture(&prepared, &recipe, CULTURE).unwrap();
    // The river band in the local frame (y south from the crop's north edge).
    let frame = prepared.grid.frame;
    let band_mid = frame.north_m - centre()[1];
    let band = (band_mid - RIVER_HALF_WIDTH_M, band_mid + RIVER_HALF_WIDTH_M);

    let lots = &run.partition.parcels;
    let wet: Vec<_> = lots.iter().filter(|l| bank(&l.shape.outer, band) == Bank::Wet).map(|l| l.id.clone()).collect();
    assert!(wet.is_empty(), "lots touching the river: {wet:?}");
    let lot_banks: Vec<Bank> = lots.iter().map(|l| bank(&l.shape.outer, band)).collect();
    assert!(lot_banks.contains(&Bank::North) && lot_banks.contains(&Bank::South), "lots on one bank only: {} lots", lots.len());
    let street_banks: Vec<Bank> = run.layout.streets.iter().map(|s| bank(&s.path, band)).collect();
    assert!(street_banks.contains(&Bank::North) && street_banks.contains(&Bank::South), "streets on one bank only");

    let again = settle::run::layout_json(&site, &recipe, &prepared, &run).unwrap();
    assert!(written == again, "two runs with seed {} differ", recipe.seed.value);
}
