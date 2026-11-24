//! End to end on a synthetic guide: a 257² island (dome, a valley to the south coast, a closed basin on the north
//! flank) with one height stamp across the valley, run with the shipped recipe at full resolution.
#![allow(clippy::many_single_char_names, clippy::cast_precision_loss, clippy::float_cmp, clippy::manual_assert_eq)]
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const N: usize = 257;
const SPACING: f64 = 20.0;
const ORIGIN: [f64; 2] = [1000.0, 9000.0];
const SEA: f64 = 5.0;
const FILL: f32 = -100.0;
/// Stamp: 25 x 25 samples at 10 m, weight 1 inside and a linear ramp to 0 over its outer 4 samples.
const STAMP_DIMS: usize = 25;
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

fn height_at(c: usize, r: usize) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let (x, y) = ((c as f64 - 128.0) * SPACING, (128.0 - r as f64) * SPACING);
    let rad = x.hypot(y);
    if rad > 2400.0 {
        return FILL; // no data: the adapter's fill
    }
    let dome = 260.0 * (1.0 - (rad / 2300.0).powi(2));
    // Valley from the summit to the south coast (x = 0, y < 0).
    let valley = if y < 0.0 { 45.0 * (-(x / 160.0).powi(2)).exp() * (1.0 - (-y / 2300.0)).max(0.0).sqrt() } else { 0.0 };
    // Closed basin on the north flank.
    let basin = 110.0 * (-((x - 300.0).powi(2) + (y - 1000.0).powi(2)) / (2.0 * 300.0_f64.powi(2))).exp();
    let ripple = 3.0 * (x / 170.0).sin() * (y / 230.0).cos();
    #[allow(clippy::cast_possible_truncation)]
    let h = (SEA + dome - valley - basin + ripple) as f32;
    h
}

/// Stamp origin: across the valley, 1 km south of the summit.
fn stamp_origin() -> [f64; 2] {
    let half = (STAMP_DIMS - 1) as f64 * STAMP_SPACING / 2.0;
    let c = pos(128.0, 128.0 + 1000.0 / SPACING);
    [c[0] - half, c[1] + half]
}

fn stamp_weight_sample(c: usize, r: usize) -> f32 {
    let edge = c.min(r).min(STAMP_DIMS - 1 - c).min(STAMP_DIMS - 1 - r);
    #[allow(clippy::cast_precision_loss)]
    let w = (edge as f32 / 4.0).min(1.0);
    w
}

/// Exact bilinear stamp weight at a guide point (0 outside).
fn stamp_weight(p: [f64; 2]) -> f64 {
    let o = stamp_origin();
    let (u, v) = ((p[0] - o[0]) / STAMP_SPACING, (o[1] - p[1]) / STAMP_SPACING);
    let top = (STAMP_DIMS - 1) as f64;
    if !(0.0..=top).contains(&u) || !(0.0..=top).contains(&v) {
        return 0.0;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (c, r) = ((u.floor() as usize).min(STAMP_DIMS - 2), (v.floor() as usize).min(STAMP_DIMS - 2));
    let (fu, fv) = (u - c as f64, v - r as f64);
    let w = |cc: usize, rr: usize| f64::from(stamp_weight_sample(cc, rr));
    (w(c, r) * (1.0 - fu) + w(c + 1, r) * fu) * (1.0 - fv) + (w(c, r + 1) * (1.0 - fu) + w(c + 1, r + 1) * fu) * fv
}

fn write_guide(dir: &Path) -> Vec<f32> {
    let heights: Vec<f32> = (0..N * N).map(|i| height_at(i % N, i / N)).collect();
    let meta = serde_json::json!({
        "format": 1, "dims": [N, N], "sample_spacing_m": SPACING, "sea_level_m": SEA, "fill_m": FILL,
        "origin_m": ORIGIN, "adapter": "test-island", "heights_blake3": "input",
    });
    fs::write(dir.join("meta.json"), serde_json::to_string(&meta).unwrap()).unwrap();
    fs::write(dir.join("heights.f32"), water::guide::f32_bytes(&heights)).unwrap();
    let mut stamp: Vec<f32> = Vec::new();
    for r in 0..STAMP_DIMS {
        for c in 0..STAMP_DIMS {
            stamp.push(height_at(128 + c / 2, 178 + r / 2));
        }
    }
    for r in 0..STAMP_DIMS {
        for c in 0..STAMP_DIMS {
            stamp.push(stamp_weight_sample(c, r));
        }
    }
    let stamps = serde_json::json!({"format": 1, "stamps": [{
        "name": "town", "origin_m": stamp_origin(), "dims": [STAMP_DIMS, STAMP_DIMS], "spacing_m": STAMP_SPACING,
        "offset": 0,
    }]});
    fs::write(dir.join("stamps.json"), serde_json::to_string(&stamps).unwrap()).unwrap();
    fs::write(dir.join("stamps.f32"), water::guide::f32_bytes(&stamp)).unwrap();
    fs::create_dir_all(dir.join("mods/sub")).unwrap();
    fs::write(dir.join("mods/sub/a.txt"), "kept").unwrap();
    heights
}

fn recipe() -> (water::Recipe, String) {
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../recipes/water-x10s.toml");
    let mut recipe = water::Recipe::parse(&fs::read_to_string(shipped).unwrap()).unwrap();
    recipe.downsample.value = 1;
    // The V2 formula for this 5.1 km square: max(0.003 * 5.14^2, 0.02) km².
    recipe.settings.get_mut("riverArea").unwrap().value = toml::Value::Float(0.08);
    recipe.settings.get_mut("minRiverLength").unwrap().value = toml::Value::Float(150.0);
    // Keep every suitable closed basin as a lake, so the fixture's basin is one.
    recipe.conditioning.get_mut("lakes").unwrap().value = toml::Value::Float(2.0);
    recipe.validate().unwrap();
    let text = toml::to_string(&recipe).unwrap();
    (recipe, text)
}

fn hash_dir(dir: &Path) -> String {
    let mut h = blake3::Hasher::new();
    for name in ["heights.f32", "water.json", "stamps.json", "stamps.f32", "mods/sub/a.txt"] {
        h.update(&fs::read(dir.join(name)).unwrap());
    }
    let mut meta: Value = serde_json::from_slice(&fs::read(dir.join("meta.json")).unwrap()).unwrap();
    meta["water_from"]["guide"] = Value::Null;
    h.update(meta.to_string().as_bytes());
    h.finalize().to_hex().to_string()
}

/// A sample of the input at or below sea within two spacings of guide point `p`.
fn near_sea(input: &[f32], p: [f64; 2]) -> bool {
    let (cu, ru) = ((p[0] - ORIGIN[0]) / SPACING, (ORIGIN[1] - p[1]) / SPACING);
    #[allow(clippy::cast_possible_truncation)]
    let (c0, r0) = (cu.round() as i64, ru.round() as i64);
    (-2..=2).any(|dr| {
        (-2..=2).any(|dc| {
            let (c, r) = (c0 + dc, r0 + dr);
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let ok = (0..N as i64).contains(&c) && (0..N as i64).contains(&r) && f64::from(input[r as usize * N + c as usize]) <= SEA;
            ok
        })
    })
}

/// Bilinear ground of the guide grid `h` at guide point `p`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn ground(h: &[f32], p: [f64; 2]) -> f64 {
    let top = (N - 1) as f64;
    let (cu, ru) = (((p[0] - ORIGIN[0]) / SPACING).clamp(0.0, top), ((ORIGIN[1] - p[1]) / SPACING).clamp(0.0, top));
    let (c, r) = ((cu as usize).min(N - 2), (ru as usize).min(N - 2));
    let (u, v) = (cu - c as f64, ru - r as f64);
    let z = |cc: usize, rr: usize| f64::from(h[rr * N + cc]);
    (z(c, r) * (1.0 - u) + z(c + 1, r) * u) * (1.0 - v) + (z(c, r + 1) * (1.0 - u) + z(c + 1, r + 1) * u) * v
}

fn pts(ring: &Value) -> Vec<[f64; 2]> {
    ring.as_array().unwrap().iter().map(|v| [v[0].as_f64().unwrap(), v[1].as_f64().unwrap()]).collect()
}

/// On a lake: inside its outline and outside its holes.
fn on_lake(p: [f64; 2], lakes: &[Value]) -> bool {
    lakes.iter().any(|l| {
        inside(p, l["outline"].as_array().unwrap()) && !l["holes"].as_array().unwrap().iter().any(|h| inside(p, h.as_array().unwrap()))
    })
}

/// The water geometry fits the carved heights: every river point (off lakes) between its bed and its bank tops,
/// every lake outline vertex on ground at or over the lake's level, except where a course meets the lake (within
/// three cells of a river point at or under the lake's level: its carved bed cuts the shore under the level, the
/// lake closes across the channel and the river carries on). Returns the lake vertices found below their level.
#[allow(clippy::cast_precision_loss)]
fn check_fit(file: &Value, heights: &[f32], recipe: &water::Recipe) -> usize {
    // Magna Urbis's cell (m).
    let cell = f64::from(recipe.downsample.value) * SPACING;
    let share = recipe.river_fill.value;
    let lakes = file["lakes"].as_array().unwrap();
    let rivers: Vec<Vec<[f64; 4]>> = file["rivers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            r["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    let a: Vec<f64> = v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
                    [a[0], a[1], a[2], a[3]]
                })
                .collect()
        })
        .collect();
    let (mut checked, mut banked, mut pooled, mut worst) = (0, 0, 0, f64::NEG_INFINITY);
    for (id, p) in rivers.iter().enumerate() {
        for k in 0..p.len() {
            let (q, w, level) = ([p[k][0], p[k][1]], p[k][2], p[k][3]);
            assert!(k == 0 || level <= p[k - 1][3], "river {id} level rises at {k}");
            if on_lake(q, lakes) || level <= SEA + 1e-9 {
                continue;
            }
            checked += 1;
            let g = ground(heights, q);
            let depth = magna_urbis_terrain::vector_water::bed_depth(w);
            assert!(level >= g - depth - 1e-6, "river {id} point {k} under its bed: level {level}, ground {g}");
            if level > g + share * depth + 1e-6 {
                pooled += 1;
            }
            // Bank tops, both sides, at the end of Magna Urbis's flat bed and banks (vector_water/incision.rs:139).
            // Where the lower one stands over the bed (the course runs in a channel, not across a slope or a beach)
            // the water lies under it, within 2 m.
            let (a, b) = (p[k.saturating_sub(1)], p[(k + 1).min(p.len() - 1)]);
            let (tx, ty) = (b[0] - a[0], b[1] - a[1]);
            let l = tx.hypot(ty);
            let reach = (0.5 * w).max(0.75 * cell) + w.max(1.5 * cell);
            let (nx, ny) = (-ty / l * reach, tx / l * reach);
            let bank = ground(heights, [q[0] + nx, q[1] + ny]).min(ground(heights, [q[0] - nx, q[1] - ny]));
            if bank >= g - depth {
                banked += 1;
                worst = worst.max(level - bank);
                assert!(level <= bank + 2.0, "river {id} point {k} over its banks: level {level}, bank {bank}");
            }
        }
    }
    println!("fit: {checked} river points off lakes, {pooled} pooled, {banked} in a channel: level - lower bank top <= {worst:.2} m");
    assert!(checked > 100 && banked > checked / 2);
    let mut below = 0;
    for lake in lakes {
        let level = lake["level_m"].as_f64().unwrap();
        let inlet = |v: [f64; 2]| {
            rivers.iter().flatten().any(|q| q[3] <= level + 1e-6 && (q[0] - v[0]).hypot(q[1] - v[1]) < 3.0 * cell)
        };
        for ring in std::iter::once(&lake["outline"]).chain(lake["holes"].as_array().unwrap()) {
            for v in pts(ring) {
                let g = ground(heights, v);
                if g < level - 0.01 {
                    below += 1;
                    assert!(inlet(v), "lake {} vertex {v:?} on ground {g} under its level {level}", lake["id"]);
                }
            }
        }
    }
    below
}

fn inside(p: [f64; 2], ring: &[Value]) -> bool {
    let pt = |v: &Value| [v[0].as_f64().unwrap(), v[1].as_f64().unwrap()];
    let mut odd = false;
    for k in 0..ring.len() {
        let (a, b) = (pt(&ring[k]), pt(&ring[(k + 1) % ring.len()]));
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            odd = !odd;
        }
    }
    odd
}

#[test]
fn island_end_to_end() {
    let guide = scratch("island-guide");
    let input = write_guide(&guide);
    let (recipe, text) = recipe();
    let out = scratch("island-out");
    let report = water::run(&guide, &recipe, &text, &out).unwrap();
    println!("{}", report.summary());

    // Heights: finite, input + delta, and the chain changed something.
    let bytes = fs::read(out.join("heights.f32")).unwrap();
    let heights: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
    assert_eq!(heights.len(), N * N);
    for i in 0..N * N {
        assert!(heights[i].is_finite());
        assert_eq!(heights[i].to_bits(), (input[i] + report.delta[i]).to_bits(), "sample {i}");
    }
    assert!(report.stats.max_abs_delta_m > 0.0);
    assert_eq!(fs::read_to_string(out.join("mods/sub/a.txt")).unwrap(), "kept");
    let meta: Value = serde_json::from_slice(&fs::read(out.join("meta.json")).unwrap()).unwrap();
    assert_eq!(meta["adapter"], "test-island");
    assert_eq!(meta["water_from"]["magna_urbis_rev"], water::run::MAGNA_URBIS_REV);
    assert_eq!(meta["heights_blake3"], blake3::hash(&bytes).to_hex().as_str());
    assert_eq!(meta["water_from"]["input_heights_blake3"], "input");

    // water.json parses; rivers: levels non-increasing, one reaches the sea, none inside the stamp.
    let file: Value = serde_json::from_slice(&fs::read(out.join("water.json")).unwrap()).unwrap();
    assert_eq!(file["format"], 1);
    let rivers = file["rivers"].as_array().unwrap();
    assert!(!rivers.is_empty());
    let mut to_sea = 0;
    for river in rivers {
        let pts = river["points"].as_array().unwrap();
        assert!(pts.len() >= 2);
        let mut last = f64::INFINITY;
        for p in pts {
            let v: Vec<f64> = p.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
            assert!(v.iter().all(|x| x.is_finite()) && v[2] > 0.0);
            assert!(v[3] <= last, "river {} level rises: {} > {last}", river["id"], v[3]);
            last = v[3];
            assert!(stamp_weight([v[0], v[1]]) == 0.0, "river {} inside the stamp at {v:?}", river["id"]);
        }
        let end = pts.last().unwrap();
        if river["kind"] == "river" && near_sea(&input, [end[0].as_f64().unwrap(), end[1].as_f64().unwrap()]) {
            to_sea += 1;
        }
    }
    assert!(to_sea >= 1, "no river reaches the sea");
    assert!(report.stats.rivers_clipped >= 1, "the valley river should be cut at the stamp");

    // Lakes: outline vertices off the stamp, no stamp sample of weight > 0 inside a lake.
    let o = stamp_origin();
    for lake in file["lakes"].as_array().unwrap() {
        let ring = lake["outline"].as_array().unwrap();
        assert!(ring.len() >= 3);
        for p in ring {
            assert!(stamp_weight([p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]) == 0.0);
        }
        for r in 0..STAMP_DIMS {
            for c in 0..STAMP_DIMS {
                #[allow(clippy::cast_precision_loss)]
                let p = [o[0] + c as f64 * STAMP_SPACING, o[1] - r as f64 * STAMP_SPACING];
                assert!(stamp_weight_sample(c, r) == 0.0 || !inside(p, ring));
            }
        }
    }
    // The frame: the lake sits in the basin (300 m east, 1000 m north of the summit).
    let basin = pos(128.0 + 300.0 / SPACING, 128.0 - 1000.0 / SPACING);
    let near_basin = file["lakes"].as_array().unwrap().iter().any(|lake| {
        let ring = lake["outline"].as_array().unwrap();
        #[allow(clippy::cast_precision_loss)]
        let k = ring.len() as f64;
        let cx = ring.iter().map(|p| p[0].as_f64().unwrap()).sum::<f64>() / k;
        let cy = ring.iter().map(|p| p[1].as_f64().unwrap()).sum::<f64>() / k;
        (cx - basin[0]).hypot(cy - basin[1]) < 300.0
    });
    assert!(near_basin, "no lake in the basin");
    let below = check_fit(&file, &heights, &recipe);
    assert_eq!(report.stats.lake_vertices_below, below);

    // Deterministic: a second run writes the same files.
    let again = scratch("island-out-2");
    water::run(&guide, &recipe, &text, &again).unwrap();
    assert_eq!(hash_dir(&out), hash_dir(&again));
}

/// The downsampled path: the chain at half resolution, its change brought back on land only.
#[test]
fn island_downsampled() {
    let guide = scratch("island-guide-d2");
    let input = write_guide(&guide);
    let (mut recipe, _) = recipe();
    recipe.downsample.value = 2;
    let text = toml::to_string(&recipe).unwrap();
    let out = scratch("island-out-d2");
    let report = water::run(&guide, &recipe, &text, &out).unwrap();
    println!("{}", report.summary());
    assert_eq!((report.stats.grid_n, report.stats.padded_n), (129, 258));
    let bytes = fs::read(out.join("heights.f32")).unwrap();
    let heights: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
    for i in 0..N * N {
        assert!(heights[i].is_finite());
        if f64::from(input[i]) <= SEA {
            assert_eq!(heights[i].to_bits(), input[i].to_bits(), "sea sample {i} changed");
        }
    }
    assert!(report.stats.max_abs_delta_m > 0.0);
    let file: Value = serde_json::from_slice(&fs::read(out.join("water.json")).unwrap()).unwrap();
    let below = check_fit(&file, &heights, &recipe);
    assert_eq!(report.stats.lake_vertices_below, below);
    let rivers = file["rivers"].as_array().unwrap();
    assert!(!rivers.is_empty());
    for river in rivers {
        for p in river["points"].as_array().unwrap() {
            assert!(stamp_weight([p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]) == 0.0);
        }
    }
}
