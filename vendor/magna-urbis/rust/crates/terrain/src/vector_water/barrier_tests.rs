//! The barrier mask handed down the water chain (`condition_and_water_masked`): an empty mask changes nothing, and the
//! routed water goes round a barrier laid across a valley floor.
use super::planform_tests::{v2_conditioning, v2_settings};
use super::*;

const N: usize = 256;
const WIDTH: f64 = 10_000.0;

/// The unconfined valley of `braid_threads_are_long_islands`: sea for `x < 4`, the floor along `y = 128`.
fn valley() -> (Vec<f32>, Settings, Climate) {
    let cell = WIDTH / N as f64;
    let height: Vec<f32> = (0..N * N)
        .map(|c| {
            let (x, y) = (c % N, c / N);
            let across = (y as f64 - N as f64 / 2.0).abs() * cell;
            (if x < 4 { -5.0 } else { 0.01 * x as f64 * cell + 0.002 * across + 0.1 * (across - 1500.0).max(0.0) }) as f32
        })
        .collect();
    let settings = Settings { river_area: 0.5, anabranching: false, avulsions: false, deltas: false, headwaters: 0.0, ..v2_settings(WIDTH) };
    (height, settings, Climate { temperature: 10.0, precipitation: 1200.0 })
}

/// `x` in `150..158`, `y` in `118..=138`: 168 cells across the valley floor.
fn rectangle() -> Vec<u8> {
    let mut barrier = vec![0u8; N * N];
    for y in 118..=138 {
        for x in 150..158 {
            barrier[y * N + x] = 1;
        }
    }
    barrier
}

fn masked(barrier: Option<&[u8]>) -> (ConditioningResult, Water) {
    let (height, settings, climate) = valley();
    let (result, water, _) = condition_and_water_masked(
        &height,
        N,
        WIDTH,
        &v2_conditioning("none"),
        &settings,
        climate,
        "barrier",
        false,
        barrier,
        &|| 0.0,
    );
    (result, water)
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn assert_same(label: &str, (ra, wa): &(ConditioningResult, Water), (rb, wb): &(ConditioningResult, Water)) {
    assert_eq!(bits(&ra.height), bits(&rb.height), "{label}: height");
    assert_eq!(bits(&ra.delta), bits(&rb.delta), "{label}: delta");
    let (ga, gb) = (&wa.hydro.grids, &wb.hydro.grids);
    assert_eq!(bits(&ga.filled), bits(&gb.filled), "{label}: filled");
    assert_eq!(bits(&ga.accumulation), bits(&gb.accumulation), "{label}: accumulation");
    assert_eq!(ga.receiver, gb.receiver, "{label}: receiver");
    assert_eq!(ga.order, gb.order, "{label}: order");
    assert_eq!(ga.basin, gb.basin, "{label}: basin");
    assert_eq!(ga.sea, gb.sea, "{label}: sea");
    assert_eq!(ga.outlets, gb.outlets, "{label}: outlets");
    assert_eq!(wa.lake, wb.lake, "{label}: lake");
    assert_eq!(wa.lake_level.as_deref().map(bits), wb.lake_level.as_deref().map(bits), "{label}: lake_level");
    assert_eq!(wa.lake_kind, wb.lake_kind, "{label}: lake_kind");
    assert_eq!(wa.lakes, wb.lakes, "{label}: lakes");
    assert_eq!(wa.river_cells, wb.river_cells, "{label}: river_cells");
    assert_eq!(
        serde_json::to_string(&wa.rivers).unwrap(),
        serde_json::to_string(&wb.rivers).unwrap(),
        "{label}: rivers"
    );
}

#[test]
fn no_barrier_is_the_same_bytes() {
    let (height, settings, climate) = valley();
    let reference = condition_and_water(&height, N, WIDTH, &v2_conditioning("none"), &settings, climate, "barrier", false);
    assert_same("None", &masked(None), &reference);
    assert_same("all zero", &masked(Some(&vec![0u8; N * N])), &reference);
}

#[test]
fn flow_goes_round_a_barrier() {
    let barrier = rectangle();
    let (_, water) = masked(Some(&barrier));
    let g = &water.hydro.grids;
    for c in 0..N * N {
        if barrier[c] != 0 {
            assert_eq!(g.receiver[c], -1, "barrier cell {c} has a receiver");
            assert_eq!(g.accumulation[c], 0.0, "barrier cell {c} accumulates");
            assert_eq!(water.lake[c], 0, "barrier cell {c} is a lake");
        }
        let r = g.receiver[c];
        assert!(r < 0 || barrier[r as usize] == 0, "cell {c} drains into barrier cell {r}");
    }
    let mut at = 128 * N + 200;
    let mut steps = 0;
    while g.receiver[at] >= 0 {
        assert_eq!(barrier[at], 0, "the walk crosses barrier cell {at}");
        at = g.receiver[at] as usize;
        steps += 1;
        assert!(steps < N * N, "the walk from (200, 128) does not end");
    }
    assert_eq!(barrier[at], 0, "the walk ends on barrier cell {at}");
    assert!(
        g.sea[at] != 0,
        "the walk from (200, 128) ends at ({}, {}) off the sea: lake {}, height {}",
        at % N,
        at / N,
        water.lake[at],
        water.hydro.height[at]
    );

    let mouth = |w: &Water| (0..N).map(|y| w.hydro.grids.accumulation[y * N + 4]).fold(0.0_f32, f32::max);
    let (_, open) = masked(None);
    eprintln!("barrier: largest accumulation on x == 4: {} with the barrier, {} without", mouth(&water), mouth(&open));
    let cell = WIDTH / N as f64;
    let crossing = water.rivers.as_ref().map_or(0, |network| {
        [&network.rivers, &network.lake_links, &network.oxbows, &network.braids, &network.distributaries]
            .iter()
            .flat_map(|lines| lines.iter())
            .flat_map(|line| line.points.chunks_exact(2))
            .filter(|p| {
                let (x, y) = ((p[0] / cell).floor(), (p[1] / cell).floor());
                x >= 0.0 && y >= 0.0 && (x as usize) < N && (y as usize) < N && barrier[y as usize * N + x as usize] != 0
            })
            .count()
    });
    eprintln!("barrier: {crossing} points of the traced lines fall in a barrier cell (any: {})", crossing > 0);
}
