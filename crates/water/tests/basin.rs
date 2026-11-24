//! A dry closed basin crossed by a river: a 257² slope to the south coast, a valley down its middle and a closed
//! basin in the valley, run with the shipped recipe at full resolution and Magna Urbis keeping no lake. The course
//! crosses the basin at its spill level, so the basin becomes a lake at that level and the course is split at it;
//! with a stamp in the basin (off the course) the course ends at the basin's rim and no lake is made.
#![allow(clippy::many_single_char_names, clippy::cast_precision_loss, clippy::float_cmp)]
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const N: usize = 257;
const SPACING: f64 = 20.0;
const ORIGIN: [f64; 2] = [1000.0, 9000.0];
const SEA: f64 = 5.0;
/// Basin centre (guide sample) and its depth (m).
const BASIN: [f64; 2] = [128.0, 128.0];
const BASIN_DEPTH: f64 = 50.0;
/// Stamp: 5 x 5 samples at 10 m, weight 1.
const STAMP_DIMS: usize = 5;
const STAMP_SPACING: f64 = 10.0;

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn pos(c: f64, r: f64) -> [f64; 2] {
    [ORIGIN[0] + c * SPACING, ORIGIN[1] - r * SPACING]
}

/// A slope falling 200 m to a straight south coast (row 230), valley walls rising 80 m either side of the middle
/// column and on at 3 % (no flats: every course runs to the valley), and a Gaussian hollow in the valley (closed: its downstream side rises faster than the slope falls).
fn height_at(c: usize, r: usize) -> f32 {
    let (x, t) = ((c as f64 - BASIN[0]) * SPACING, (r as f64 - BASIN[1]) * SPACING);
    let plane = 200.0 * (1.0 - r as f64 / 230.0);
    let walls = 80.0 * (1.0 - (-(x / 350.0).powi(2)).exp()) + 0.03 * x.abs();
    let basin = BASIN_DEPTH * (-(x * x / (2.0 * 250.0_f64.powi(2)) + t * t / (2.0 * 200.0_f64.powi(2)))).exp();
    #[allow(clippy::cast_possible_truncation)]
    let h = (SEA + plane + walls - basin) as f32;
    h
}

/// Writes the guide; a stamp of weight 1 centred on guide point `stamp` when given.
fn write_guide(dir: &Path, stamp: Option<[f64; 2]>) {
    let heights: Vec<f32> = (0..N * N).map(|i| height_at(i % N, i / N)).collect();
    let meta = serde_json::json!({
        "format": 1, "dims": [N, N], "sample_spacing_m": SPACING, "sea_level_m": SEA, "fill_m": -100.0,
        "origin_m": ORIGIN, "adapter": "test-basin",
    });
    fs::write(dir.join("meta.json"), serde_json::to_string(&meta).unwrap()).unwrap();
    fs::write(dir.join("heights.f32"), water::guide::f32_bytes(&heights)).unwrap();
    if let Some(p) = stamp {
        let half = (STAMP_DIMS - 1) as f64 * STAMP_SPACING / 2.0;
        let mut data = vec![100.0_f32; STAMP_DIMS * STAMP_DIMS];
        data.extend(std::iter::repeat_n(1.0_f32, STAMP_DIMS * STAMP_DIMS));
        let stamps = serde_json::json!({"format": 1, "stamps": [{
            "name": "town", "origin_m": [p[0] - half, p[1] + half], "dims": [STAMP_DIMS, STAMP_DIMS],
            "spacing_m": STAMP_SPACING, "offset": 0,
        }]});
        fs::write(dir.join("stamps.json"), serde_json::to_string(&stamps).unwrap()).unwrap();
        fs::write(dir.join("stamps.f32"), water::guide::f32_bytes(&data)).unwrap();
    }
}

fn recipe() -> (water::Recipe, String) {
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../recipes/water-x10s.toml");
    let mut recipe = water::Recipe::parse(&fs::read_to_string(shipped).unwrap()).unwrap();
    recipe.downsample.value = 1;
    recipe.settings.get_mut("riverArea").unwrap().value = toml::Value::Float(0.08);
    recipe.settings.get_mut("minRiverLength").unwrap().value = toml::Value::Float(150.0);
    // Magna Urbis keeps no lake: no closed depression kept by the conditioning, and the water budget's smallest
    // lake larger than the basin, which then spills (not terminal) and holds no drawn water: the flood crosses it
    // to its spill (`vector_water/mod.rs:227-230`).
    recipe.conditioning.get_mut("lakes").unwrap().value = toml::Value::Float(0.0);
    recipe.settings.get_mut("lakeArea").unwrap().value = toml::Value::Float(500.0);
    recipe.validate().unwrap();
    let text = toml::to_string(&recipe).unwrap();
    (recipe, text)
}

/// Bilinear ground of the guide grid `h` at guide point `p`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn ground(h: &[f32], p: [f64; 2]) -> f64 {
    let top = (N - 1) as f64;
    let (cu, ru) = (((p[0] - ORIGIN[0]) / SPACING).clamp(0.0, top), ((ORIGIN[1] - p[1]) / SPACING).clamp(0.0, top));
    let (c, r) = ((cu as usize).min(N - 2), (ru as usize).min(N - 2));
    let (u, v) = (cu - c as f64, ru - r as f64);
    let z = |cc: usize, rr: usize| f64::from(h[rr * N + cc]);
    (z(c, r) * (1.0 - u) + z(c + 1, r) * u) * (1.0 - v) + (z(c, r + 1) * (1.0 - u) + z(c + 1, r + 1) * u) * v
}

fn inside(p: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut odd = false;
    for k in 0..ring.len() {
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            odd = !odd;
        }
    }
    odd
}

fn ring(v: &Value) -> Vec<[f64; 2]> {
    v.as_array().unwrap().iter().map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]).collect()
}

fn rivers(file: &Value) -> Vec<Vec<[f64; 4]>> {
    file["rivers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            r["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| [0, 1, 2, 3].map(|i| v[i].as_f64().unwrap()))
                .collect()
        })
        .collect()
}

struct Run {
    report: water::Report,
    file: Value,
    heights: Vec<f32>,
    json: Vec<u8>,
}

fn run(name: &str, stamp: Option<[f64; 2]>) -> Run {
    let guide = scratch(&format!("basin-guide-{name}"));
    write_guide(&guide, stamp);
    let (recipe, text) = recipe();
    let out = scratch(&format!("basin-out-{name}"));
    let report = water::run(&guide, &recipe, &text, &out).unwrap();
    println!("{name}: {}", report.summary());
    let bytes = fs::read(out.join("heights.f32")).unwrap();
    let heights = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
    let json = fs::read(out.join("water.json")).unwrap();
    Run { report, file: serde_json::from_slice(&json).unwrap(), heights, json }
}

/// Every river: levels non-increasing; every point off the lakes between its bed and a bed depth over the ground.
fn check_rivers(r: &Run, lakes: &[Vec<[f64; 2]>]) {
    for (id, p) in rivers(&r.file).iter().enumerate() {
        for k in 0..p.len() {
            assert!(k == 0 || p[k][3] <= p[k - 1][3], "river {id} level rises at {k}");
            let q = [p[k][0], p[k][1]];
            if lakes.iter().any(|l| inside(q, l)) || p[k][3] <= SEA + 1e-9 {
                continue;
            }
            let (g, depth) = (ground(&r.heights, q), magna_urbis_terrain::vector_water::bed_depth(p[k][2]));
            assert!(p[k][3] <= g + depth + 1e-6, "river {id} point {k} floats: level {}, ground {g}", p[k][3]);
            assert!(p[k][3] >= g - depth - 1e-6, "river {id} point {k} under its bed: level {}, ground {g}", p[k][3]);
        }
    }
    assert_eq!(r.report.stats.points_floating, 0);
}

#[test]
fn basin_crossed_by_a_river_becomes_a_lake() {
    let a = run("lake", None);
    let s = &a.report.stats;
    assert_eq!(s.basin_lakes, 1, "one basin filled");
    let lakes = a.file["lakes"].as_array().unwrap();
    assert_eq!(lakes.len(), 1, "Magna Urbis keeps no lake; the basin adds one");
    let lake = &lakes[0];
    assert_eq!(lake["kind"], "open");
    let level = lake["level_m"].as_f64().unwrap();
    let outline = ring(&lake["outline"]);
    // In the basin, and a real one: its floor well under the level.
    let centre = pos(BASIN[0], BASIN[1]);
    assert!(inside(centre, &outline), "the lake holds the basin centre");
    assert!(ground(&a.heights, centre) < level - 10.0);
    // At its spill: the shore stands at or over the level, except where the courses meet it.
    let all = rivers(&a.file);
    let below = outline.iter().filter(|&&v| ground(&a.heights, v) < level - 0.01).count();
    assert!(below * 10 <= outline.len(), "{below} of {} shore vertices under the level", outline.len());
    // Split at the lake: the course flowing in ends on it and the one flowing out starts on it, at its level.
    let from = usize::try_from(lake["from_course"].as_u64().expect("from_course")).unwrap();
    let last = all[from].last().unwrap();
    assert!(inside([last[0], last[1]], &outline) && last[3] <= level + 1e-9, "river {from} ends on the lake");
    let out = all.iter().position(|p| inside([p[0][0], p[0][1]], &outline) && (p[0][3] - level).abs() < 1e-9);
    assert!(out.is_some(), "a course leaves the lake at its level");
    // No course runs on over the lake water: inside the outline, a point is on the bank (pushed-out band) or at
    // most at the lake's level.
    for p in &all {
        for q in &p[1..p.len() - 1] {
            let g = ground(&a.heights, [q[0], q[1]]);
            assert!(!inside([q[0], q[1]], &outline) || g >= level || q[3] <= level + 1e-9, "a course crosses the lake at {q:?}");
        }
    }
    check_rivers(&a, std::slice::from_ref(&outline));

    // Deterministic.
    let again = run("lake-2", None);
    assert_eq!(a.json, again.json);

    // The same basin with a stamp in it, away from every course: no lake, the course ends at the basin's rim.
    // Away from every course, and from the straight crossing between the course's in and out pieces (Magna
    // Urbis's route across the basin, dropped from the output).
    let (start, end) = ([last[0], last[1]], [all[out.unwrap()][0][0], all[out.unwrap()][0][1]]);
    let crossing: Vec<[f64; 2]> =
        (0..=20).map(|i| f64::from(i) / 20.0).map(|t| [start[0] + (end[0] - start[0]) * t, start[1] + (end[1] - start[1]) * t]).collect();
    let far = |p: [f64; 2]| {
        all.iter().flatten().map(|v| [v[0], v[1]]).chain(crossing.iter().copied()).map(|v| (v[0] - p[0]).hypot(v[1] - p[1])).fold(f64::INFINITY, f64::min)
    };
    let spot = (0..N * N)
        .map(|i| pos((i % N) as f64, (i / N) as f64))
        .filter(|&p| inside(p, &outline) && ground(&a.heights, p) < level - 2.0)
        .max_by(|&p, &q| far(p).total_cmp(&far(q)))
        .unwrap();
    println!("stamp at {spot:?}, {:.0} m from the courses", far(spot));
    let b = run("stamp", Some(spot));
    let s = &b.report.stats;
    assert_eq!(s.basin_lakes, 0);
    assert!(b.file["lakes"].as_array().unwrap().is_empty());
    assert!(s.courses_ended_stamp >= 1, "the course ends at the stamped basin");
    // The course that filled the basin stops at its rim, upstream of it: on ground at or over the level.
    let mu = a.file["rivers"][from]["mu_id"].clone();
    let course: Vec<&Value> = b.file["rivers"].as_array().unwrap().iter().filter(|r| r["mu_id"] == mu).collect();
    assert_eq!(course.len(), 1, "one piece, upstream of the basin");
    assert_eq!(course[0]["clipped"], true);
    assert!(course[0]["joins"].is_null(), "an ended course joins nothing");
    let pts = course[0]["points"].as_array().unwrap();
    let end = [pts.last().unwrap()[0].as_f64().unwrap(), pts.last().unwrap()[1].as_f64().unwrap()];
    assert!(ground(&b.heights, end) >= level, "the course ends on the rim, not in the basin");
    assert!(pts.len() < all[from].len(), "it ends before the basin");
    check_rivers(&b, &[]);
}

