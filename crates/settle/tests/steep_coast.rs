//! ST-3d: a game-like site (steep rolling hills, a sea coast, a narrow winding river through the centre, a field of
//! puddle-sized swamp pools between them), the failure seen on vanilla Balmora: at its own relief no quarter street
//! can climb Magna Urbis's grades and the town collapses to one reserve lot. With the recipe's `relief_scale` (heights
//! planned at real-world grades) and `min_water_area_m2` (pools plan as dry ground) both banks get streets and lots.
// Fixture raster maths: sizes are small constants.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
use magna_urbis_core::urban::Point;
use settle::grid::Frame;
use settle::site::{SiteMeta, SiteRaster};
use settle::{Recipe, prepare, run_culture};
use std::collections::BTreeMap;

const CELL_M: f64 = 2.5;
const SIDE_CELLS: usize = 960;
const ORIGIN_M: [f64; 2] = [10_000.0, 20_000.0];
const WATER_LEVEL_M: f64 = 10.0;
const BED_DEPTH_M: f64 = 1.5;
/// Hills: `HILL_BASE_M + HILL_M * (sin(dx/40) sin(dy/34) + 1)` above the water level, slopes up to ~0.6 like
/// Balmora's ground (median 0.35-0.5).
const HILL_BASE_M: f64 = 12.0;
const HILL_M: f64 = 22.0;
/// Sea west of this offset from the centre (m).
const COAST_DX_M: f64 = -700.0;
const RIVER_HALF_WIDTH_M: f64 = 6.0;
/// Pools of `POOL_M` square every `POOL_PITCH_M` between the coast and the river.
const POOLS_DX_M: (f64, f64) = (-650.0, -80.0);
const POOL_M: f64 = 8.0;
const POOL_PITCH_M: f64 = 40.0;
const RELIEF_FIX: f64 = 0.25;
const CULTURES: [&str; 3] = ["european-organic", "medina", "korean"];

fn centre() -> [f64; 2] {
    let half = SIDE_CELLS as f64 * CELL_M / 2.0;
    [ORIGIN_M[0] + half, ORIGIN_M[1] - half]
}

/// The river's centreline offset east of the centre (m) at a world-y offset `dy` from it.
fn river_dx(dy: f64) -> f64 {
    30.0 * (dy / 150.0).sin()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Water {
    Sea,
    River,
    Pool,
}

fn water(dx: f64, dy: f64) -> Option<Water> {
    if dx < COAST_DX_M {
        Some(Water::Sea)
    } else if (dx - river_dx(dy)).abs() < RIVER_HALF_WIDTH_M {
        Some(Water::River)
    } else if POOLS_DX_M.0 < dx
        && dx < POOLS_DX_M.1
        && (dx - POOLS_DX_M.0).rem_euclid(POOL_PITCH_M) < POOL_M
        && (dy + 5_000.0).rem_euclid(POOL_PITCH_M) < POOL_M
    {
        Some(Water::Pool)
    } else {
        None
    }
}

fn steep_site() -> SiteRaster {
    let [cx, cy] = centre();
    let mut heights = Vec::with_capacity(SIDE_CELLS * SIDE_CELLS);
    let mut wet = Vec::with_capacity(SIDE_CELLS * SIDE_CELLS);
    for row in 0..SIDE_CELLS {
        let dy = ORIGIN_M[1] - (row as f64 + 0.5) * CELL_M - cy;
        for col in 0..SIDE_CELLS {
            let dx = ORIGIN_M[0] + (col as f64 + 0.5) * CELL_M - cx;
            let w = water(dx, dy).is_some();
            let hill = HILL_BASE_M + HILL_M * ((dx / 40.0).sin() * (dy / 34.0).sin() + 1.0) + 0.02 * dx.abs();
            heights.push(if w { WATER_LEVEL_M - BED_DEPTH_M } else { WATER_LEVEL_M + hill } as f32);
            wet.push(u8::from(w));
        }
    }
    SiteRaster {
        meta: SiteMeta {
            format: 1,
            name: "steep coast fixture".into(),
            producer: "settle tests".into(),
            origin_m: ORIGIN_M,
            cell_m: CELL_M,
            dims: [SIDE_CELLS, SIDE_CELLS],
            water_level_m: WATER_LEVEL_M,
            centre_m: centre(),
            source: "synthetic: steep hills + coast + narrow river + swamp pools".into(),
            claim: None,
            blake3: BTreeMap::new(),
        },
        heights,
        water: wet,
        roads: None,
    }
}

fn recipe(relief_scale: f64, min_water_area_m2: f64) -> Recipe {
    let mut recipe = Recipe::parse(include_str!("../../../recipes/example.toml")).unwrap();
    recipe.relief_scale.value = relief_scale;
    recipe.min_water_area_m2.value = min_water_area_m2;
    recipe.validate().unwrap();
    recipe
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Bank {
    West,
    East,
    /// Touches or crosses the river band.
    Wet,
}

/// Which bank a polygon or polyline (Magna Urbis's local frame) lies on, inside the reach where the river runs.
fn bank(frame: &Frame, points: &[Point]) -> Bank {
    let [cx, cy] = centre();
    let side = |p: &Point| {
        let [x, y] = frame.to_world(*p);
        let off = x - cx - river_dx(y - cy);
        if off <= -RIVER_HALF_WIDTH_M {
            Bank::West
        } else if off >= RIVER_HALF_WIDTH_M {
            Bank::East
        } else {
            Bank::Wet
        }
    };
    let first = points.first().map_or(Bank::Wet, side);
    if points.iter().all(|p| side(p) == first) { first } else { Bank::Wet }
}

#[test]
fn steep_coast_town_builds_both_banks_with_relief_scale_and_pool_floor() {
    let site = steep_site();
    let [cx, cy] = centre();

    // The cause: at the game's own relief no quarter street routes, whatever the water.
    let unfixed = recipe(1.0, 0.0);
    let prepared = prepare(&site, &unfixed).unwrap();
    let run = run_culture(&prepared, &unfixed, CULTURES[0]).unwrap();
    eprintln!("relief 1.0: {} lots, {} streets", run.partition.parcels.len(), run.layout.streets.len());
    let frame = prepared.grid.frame;
    let banks: Vec<Bank> = run.partition.parcels.iter().map(|l| bank(&frame, &l.shape.outer)).collect();
    assert!(
        !(banks.contains(&Bank::West) && banks.contains(&Bank::East)),
        "the fixture no longer reproduces the steep-site collapse ({} lots): did Magna Urbis change?",
        run.partition.parcels.len()
    );

    // The fix in our input: heights about the water level x RELIEF_FIX, pools dry, river and sea kept.
    let fixed = recipe(RELIEF_FIX, Recipe::parse(include_str!("../../../recipes/example.toml")).unwrap().min_water_area_m2.value);
    let prepared = prepare(&site, &fixed).unwrap();
    let grid = &prepared.grid;
    let frame = grid.frame;
    let code = fixed.water_code().unwrap();
    // The grid cell holding world offset (dx, dy) from the centre, and the world point it samples.
    let sample = |dx: f64, dy: f64| {
        let p = frame.to_local(cx + dx, cy + dy);
        let s = fixed.raster_spacing_m.value;
        let (col, row) = ((p.x / s) as usize, (p.y / s) as usize);
        let at = frame.to_world(Point { x: (col as f64 + 0.5) * s, y: (row as f64 + 0.5) * s });
        (row * frame.n + col, at)
    };
    let wet_in_site = |[x, y]: [f64; 2]| site.cell_index(x, y).is_some_and(|i| site.water[i] != 0);
    let (pool, at) = sample(POOLS_DX_M.0 + POOL_PITCH_M * 5.0 + POOL_M / 2.0, POOL_M / 2.0);
    assert_eq!(water(at[0] - cx, at[1] - cy), Some(Water::Pool));
    assert!(wet_in_site(at), "the site has a pool there");
    assert_eq!(grid.water[pool], 0, "a pool plans as dry ground");
    let (river, at) = sample(river_dx(0.0), 0.0);
    assert!(wet_in_site(at));
    assert_eq!(grid.water[river], code, "the river stays water");
    let (sea, at) = sample(COAST_DX_M - 100.0, 0.0);
    assert!(wet_in_site(at));
    assert_eq!(grid.water[sea], code, "the sea stays water");
    let (hill, [x, y]) = sample(120.0, 85.0);
    let want = (WATER_LEVEL_M + (f64::from(site.height_at(x, y)) - WATER_LEVEL_M) * RELIEF_FIX) as f32;
    assert!((grid.heights[hill] - want).abs() < 1e-4, "height {} planned, want {want}", grid.heights[hill]);

    for culture in CULTURES {
        let run = run_culture(&prepared, &fixed, culture).unwrap();
        let lots = &run.partition.parcels;
        eprintln!("{culture} relief {RELIEF_FIX}: {} lots, {} streets", lots.len(), run.layout.streets.len());
        let wet: Vec<_> = lots.iter().filter(|l| bank(&frame, &l.shape.outer) == Bank::Wet).map(|l| l.id.clone()).collect();
        assert!(wet.is_empty(), "{culture}: lots touching the river: {wet:?}");
        let count = |b: Bank| lots.iter().filter(|l| bank(&frame, &l.shape.outer) == b).count();
        let streets = |b: Bank| run.layout.streets.iter().filter(|s| bank(&frame, &s.path) == b).count();
        let (lw, le, sw, se) = (count(Bank::West), count(Bank::East), streets(Bank::West), streets(Bank::East));
        assert!(lw > 0 && le > 0, "{culture}: lots west {lw}, east {le}");
        assert!(sw > 0 && se > 0, "{culture}: streets west {sw}, east {se}");
    }
}
