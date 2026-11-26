//! A synthetic guide (256² hills, 20 m, sea along the west edge, a north-south river from `water.json`, a small lake)
//! and five fixed sites, two on each bank and one centred in the lake by its east shore: the lake site routed from a
//! landing on land, every site reached, every road ending on a site's edge (no road point inside a site), the river
//! crossed on a bridge or ford, no road point in the sea, coordinates back in the guide frame, the same file twice.
// Fixture indices and metres: small bounded values.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::naive_bytecount)]
use magna_urbis_core::settlements::roads::PROFILES;
use magna_urbis_core::settlements::roads::profile::profile_for;
use roads::raster::{LAKE, RIVER, SEA};
use roads::run::PROFILE_CULTURE;
use roads::{Recipe, SitesFile};
use std::path::{Path, PathBuf};

const N: usize = 256;
const SPACING_M: f64 = 20.0;
/// Guide frame of sample (0, 0): off the origin so a frame slip shows.
const ORIGIN_M: [f64; 2] = [10_000.0, 50_000.0];
/// Columns of sea along the west edge.
const SEA_COLS: usize = 12;
/// River centreline x (m east of the origin) and its width.
const RIVER_X_M: f64 = 2600.0;
const RIVER_WIDTH_M: f64 = 30.0;

const RECIPE: &str = r#"
seed = { value = "roads-test", unit = "seed string", source = "test" }
raster_n = { value = 256, unit = "cells", source = "test: the guide's own side" }
pad_depth_m = { value = 20.0, unit = "m", source = "test" }
grid = { value = 512, unit = "cells", source = "Magna Urbis default (roads/mod.rs:75)" }
density = { value = 0.5, unit = "0-1", source = "Magna Urbis default" }
level = { value = "rural", unit = "level id", source = "Magna Urbis default" }
sharing = { value = 0.7, unit = "0-1", source = "Magna Urbis default" }
crossing_cost = { value = 1.0, unit = "x", source = "Magna Urbis default" }
landing_search_m = { value = 400.0, unit = "m", source = "test: the lake's half width" }
profiles = { value = { dunmer = "medieval", imperial = "roman" }, unit = "culture -> profile id", source = "test" }
"#;

fn height(col: usize, row: usize) -> f32 {
    if col < SEA_COLS {
        return -8.0;
    }
    let (x, y) = (col as f64 * SPACING_M, row as f64 * SPACING_M);
    let hills = 18.0 * (x / 610.0).sin() * (y / 730.0).cos() + 10.0 * (x / 290.0 + y / 410.0).sin();
    (12.0 + 0.012 * (x - SEA_COLS as f64 * SPACING_M) + hills).max(2.0) as f32
}

fn write_guide(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let meta = serde_json::json!({
        "format": 1, "dims": [N, N], "sample_spacing_m": SPACING_M, "sea_level_m": 0.0, "origin_m": ORIGIN_M,
        "producer": "roads test",
    });
    std::fs::write(dir.join("meta.json"), meta.to_string()).unwrap();
    let mut bytes = Vec::with_capacity(N * N * 4);
    for row in 0..N {
        for col in 0..N {
            bytes.extend_from_slice(&height(col, row).to_le_bytes());
        }
    }
    std::fs::write(dir.join("heights.f32"), bytes).unwrap();
    let [ox, oy] = ORIGIN_M;
    // A gently meandering river from the north edge to the south edge, level 1 m below the ground.
    let points: Vec<[f64; 4]> = (0..=16)
        .map(|k| {
            let y = oy + 200.0 - f64::from(k) * (N as f64 * SPACING_M + 400.0) / 16.0;
            let x = ox + RIVER_X_M + 120.0 * (f64::from(k) * 0.7).sin();
            [x, y, RIVER_WIDTH_M, 8.0]
        })
        .collect();
    let water = serde_json::json!({
        "format": 1,
        "lakes": [{ "id": 0, "level_m": 30.0, "area_m2": 0.0,
            "outline": [[ox + 3400.0, oy - 2300.0], [ox + 3800.0, oy - 2300.0], [ox + 3800.0, oy - 2700.0], [ox + 3400.0, oy - 2700.0]],
            "holes": [] }],
        "rivers": [{ "id": 7, "kind": "river", "outlet": "edge", "points": points }],
    });
    std::fs::write(dir.join("water.json"), water.to_string()).unwrap();
}

fn sites() -> SitesFile {
    let [ox, oy] = ORIGIN_M;
    let json = serde_json::json!({ "format": 1, "sites": [
        { "key": "west-a", "name": "West A", "population": 2000, "culture": "imperial", "centre_m": [ox + 1200.0, oy - 1300.0], "radius_m": 0.0 },
        { "key": "west-b", "name": "West B", "population": 600, "culture": "dunmer", "centre_m": [ox + 1500.0, oy - 3900.0], "radius_m": 0.0 },
        { "key": "east-a", "name": "East A", "population": 1500, "culture": "dunmer", "centre_m": [ox + 4300.0, oy - 1200.0], "radius_m": 0.0 },
        { "key": "east-b", "name": "East B", "population": 300, "culture": "dunmer", "centre_m": [ox + 4100.0, oy - 4100.0], "radius_m": 250.0 },
        { "key": "lake", "name": "Lake", "population": 200, "culture": "dunmer", "centre_m": [ox + 3700.0, oy - 2500.0], "radius_m": 0.0 },
    ]});
    serde_json::from_value(json).unwrap()
}

fn guide_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    write_guide(&dir);
    dir
}

#[test]
fn every_profile_has_a_culture_that_resolves_to_it() {
    for p in &PROFILES {
        let (_, culture) = PROFILE_CULTURE.iter().find(|(id, _)| *id == p.id).unwrap_or_else(|| panic!("no culture for {}", p.id));
        assert_eq!(PROFILES[profile_for(culture)].id, p.id, "{culture}");
    }
}

#[test]
fn island_roads_reach_every_site_and_cross_the_river() {
    let recipe = Recipe::parse(RECIPE).unwrap();
    let dir = guide_dir("roads-island");
    let sites = sites();
    let run = roads::route(&dir, &sites, &recipe).unwrap();
    println!("{}", run.summary);
    let (file, raster) = (&run.file, &run.raster);

    // The fixture's water made it into the raster.
    let count = |code: u8| raster.water.iter().filter(|&&w| w == code).count();
    assert!(count(SEA) > 0 && count(RIVER) > 0 && count(LAKE) > 0, "sea {} river {} lake {}", count(SEA), count(RIVER), count(LAKE));

    assert!(file.unreachable.is_empty(), "unreachable: {:?}", file.unreachable);
    assert!(run.summary.to_string().contains("unreachable 0,") || run.summary.to_string().ends_with("unreachable 0"), "{}", run.summary);
    assert!(!file.segments.is_empty());

    // The lake site is wet and routed from a dry landing; the others from their centres.
    assert_eq!(file.sites.len(), sites.sites.len());
    for (out, s) in file.sites.iter().zip(&sites.sites) {
        assert_eq!(out.key, s.key);
        assert_eq!(raster.water_at(s.centre_m) == LAKE, out.landing.is_some(), "{}: landing {:?}", s.key, out.landing);
    }
    let lake = file.sites.iter().find(|s| s.key == "lake").unwrap();
    let landing = lake.landing.unwrap();
    assert_eq!(raster.water_at(landing), 0, "landing {landing:?} is wet");
    assert!(landing[0] >= ORIGIN_M[0] + 3800.0, "landing {landing:?} not on the east shore");

    // Junction ids and degrees match the segments.
    for (id, j) in file.junctions.iter().enumerate() {
        assert_eq!(j.id, id);
        let ends = file.segments.iter().map(|g| usize::from(g.a == id) + usize::from(g.b == id)).sum::<usize>();
        assert_eq!(j.degree, ends, "junction {id}");
        assert!(ends > 0, "junction {id} ends no segment");
    }
    // Every road stops at a site's edge: no point inside any site's disc (around its centre and its landing), and
    // each site's junctions sit on its edge, at the ends of the segments that reach it.
    let discs = |s: &roads::run::SiteOut| std::iter::once(s.centre_m).chain(s.landing).collect::<Vec<_>>();
    let tol = 1e-6;
    for g in &file.segments {
        assert!(g.points.len() >= 2, "segment {}", g.id);
        for s in &file.sites {
            for c in discs(s) {
                for p in &g.points {
                    let d = (p[0] - c[0]).hypot(p[1] - c[1]);
                    assert!(d >= s.radius_m - tol, "segment {} point {p:?} {d:.1} m inside {} (radius {:.1})", g.id, s.key, s.radius_m);
                }
            }
        }
    }
    for s in &file.sites {
        let js: Vec<_> = file.junctions.iter().filter(|j| j.sites.contains(&s.key)).collect();
        assert!(!js.is_empty(), "{}: no junction", s.key);
        for j in js {
            assert_eq!(j.kind, "site");
            let off = discs(s).iter().map(|c| ((j.pos[0] - c[0]).hypot(j.pos[1] - c[1]) - s.radius_m).abs()).fold(f64::INFINITY, f64::min);
            assert!(off <= file.routing_cell_m, "{}: junction {} {off:.1} m off its edge", s.key, j.id);
            for g in file.segments.iter().filter(|g| g.a == j.id || g.b == j.id) {
                let (end, trimmed) = if g.a == j.id { (g.points[0], g.trimmed_m[0]) } else { (*g.points.last().unwrap(), g.trimmed_m[1]) };
                let d = (end[0] - j.pos[0]).hypot(end[1] - j.pos[1]);
                assert!(d <= tol, "{}: segment {} ends {d:.1} m from its junction", s.key, g.id);
                assert!(trimmed > 0.0, "{}: segment {} reaches the edge untrimmed", s.key, g.id);
            }
        }
    }
    let crossings: Vec<&str> = file.segments.iter().flat_map(|s| &s.crossings).map(|c| c.kind.as_str()).collect();
    assert!(crossings.iter().any(|k| *k == "bridge" || *k == "ford"), "no bridge or ford: {crossings:?}");
    for s in &file.segments {
        for &p in &s.points {
            assert_ne!(raster.water_at(p), SEA, "segment {} point {p:?} in the sea", s.id);
        }
    }

    // Deterministic: the same file twice.
    let again = roads::route(&dir, &sites, &recipe).unwrap();
    assert_eq!(roads::run::to_json(file).unwrap(), roads::run::to_json(&again.file).unwrap());
}

/// A site the settlement tool lays out after the roads: its roads run on to its routed point (no cut at its disc) and it
/// counts as reached, while every other site is still cut back to its edge.
#[test]
fn a_laid_out_site_keeps_its_roads_to_its_centre() {
    const KEY: &str = "east-a";
    let recipe = Recipe::parse(RECIPE).unwrap();
    let dir = guide_dir("roads-laid-out");
    let mut sites = sites();
    sites.sites.iter_mut().find(|s| s.key == KEY).unwrap().laid_out = true;
    let run = roads::route(&dir, &sites, &recipe).unwrap();
    println!("{}", run.summary);
    let file = &run.file;
    let cell = file.routing_cell_m;
    let tol = 1e-6;
    let dist = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]);
    let ends_at = |g: &roads::run::SegmentOut, j: usize| {
        let mut v = Vec::new();
        if g.a == j {
            v.push((g.points[0], g.trimmed_m[0]));
        }
        if g.b == j {
            v.push((*g.points.last().unwrap(), g.trimmed_m[1]));
        }
        v
    };

    // (3) Every site reached, the laid-out one included.
    assert!(file.unreachable.is_empty(), "unreachable: {:?}", file.unreachable);

    // (1) The laid-out site: its junctions sit on its routed point, not its edge, and the segments ending there are
    // untrimmed; roads run on inside its disc.
    let town = file.sites.iter().find(|s| s.key == KEY).unwrap();
    assert!(town.laid_out && town.landing.is_none(), "{town:?}");
    let routed = town.landing.unwrap_or(town.centre_m);
    let js: Vec<_> = file.junctions.iter().filter(|j| j.sites.iter().any(|k| k == KEY)).collect();
    assert!(!js.is_empty(), "{KEY}: no junction");
    let mut ends = 0;
    for j in js {
        assert_eq!(j.kind, "site", "{KEY}: junction {}", j.id);
        let off = dist(j.pos, routed);
        assert!(off <= cell, "{KEY}: junction {} {off:.1} m from its routed point (cell {cell:.1})", j.id);
        assert!(town.radius_m - off > cell, "{KEY}: junction {} on its edge", j.id);
        for g in &file.segments {
            for (end, trimmed) in ends_at(g, j.id) {
                assert!(dist(end, j.pos) <= tol, "{KEY}: segment {} ends off its junction", g.id);
                assert!(dist(end, routed) <= cell, "{KEY}: segment {} ends {:.1} m off", g.id, dist(end, routed));
                assert!(trimmed == 0.0, "{KEY}: segment {} trimmed {trimmed:.1} m", g.id);
                ends += 1;
            }
        }
    }
    assert!(ends > 0, "{KEY}: no segment ends at its routed point");
    assert!(
        file.segments.iter().flat_map(|g| &g.points).any(|&p| dist(p, town.centre_m) < town.radius_m),
        "{KEY}: no road inside its disc"
    );

    // (2) Every other site is still cut on its edge: no road point inside its disc, its junctions on its edge at
    // trimmed segment ends.
    for s in file.sites.iter().filter(|s| s.key != KEY) {
        assert!(!s.laid_out, "{}", s.key);
        let discs: Vec<[f64; 2]> = std::iter::once(s.centre_m).chain(s.landing).collect();
        for g in &file.segments {
            for &p in &g.points {
                for &c in &discs {
                    assert!(dist(p, c) >= s.radius_m - tol, "segment {} point {p:?} inside {}", g.id, s.key);
                }
            }
        }
        let js: Vec<_> = file.junctions.iter().filter(|j| j.sites.contains(&s.key)).collect();
        assert!(!js.is_empty(), "{}: no junction", s.key);
        for j in js {
            let off = discs.iter().map(|&c| (dist(j.pos, c) - s.radius_m).abs()).fold(f64::INFINITY, f64::min);
            assert!(off <= cell, "{}: junction {} {off:.1} m off its edge", s.key, j.id);
            for g in &file.segments {
                for (_, trimmed) in ends_at(g, j.id) {
                    assert!(trimmed > 0.0, "{}: segment {} reaches the edge untrimmed", s.key, g.id);
                }
            }
        }
    }

    // The flag is written for the laid-out site only (files without it stay as they were).
    assert_eq!(roads::run::to_json(file).unwrap().matches("\"laid_out\": true").count(), 1);
    assert!(!roads::run::to_json(&roads::route(&dir, &self::sites(), &recipe).unwrap().file).unwrap().contains("laid_out"));
}

#[test]
fn a_culture_without_a_profile_is_refused() {
    let recipe = Recipe::parse(RECIPE).unwrap();
    let mut sites = sites();
    sites.sites[0].culture = "khajiit".into();
    let err = roads::route(&guide_dir("roads-culture"), &sites, &recipe).err().unwrap();
    assert!(err.contains("khajiit"), "{err}");
}

#[test]
fn the_island_recipe_parses() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../recipes/roads-x10s.toml");
    Recipe::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
}
