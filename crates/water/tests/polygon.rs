//! Lake outlines: components and marching-squares rings on small masks.
use water::polygon::{components, iso_rings, push_out, rings, signed_area, simplify_ring};
use water::run::fit_levels;

fn mask(rows: &[&str]) -> (Vec<u8>, usize) {
    let n = rows.len();
    (rows.iter().flat_map(|r| r.bytes().map(|b| u8::from(b == b'#'))).collect(), n)
}

#[test]
fn ring_with_a_hole_and_split_diagonals() {
    let (m, n) = mask(&[".....", ".###.", ".#.#.", ".###.", "....#"]);
    let (_, comps) = components(&m, n);
    // The ring of eight cells, and the lone corner cell (diagonal to it: 4-connectivity keeps it apart).
    assert_eq!(comps.len(), 2);
    let rs = rings(&comps[0], n);
    assert_eq!(rs.len(), 2, "outer ring and the hole");
    let mut areas: Vec<f64> = rs.iter().map(|r| signed_area(r).abs()).collect();
    areas.sort_by(f64::total_cmp);
    // Marching squares at 0.5 between cell centres: the outer ring is a 3x3 square with chamfered corners (9 - 4/8),
    // the hole a diamond of area 0.5.
    assert!((areas[1] - 8.5).abs() < 1e-9, "{areas:?}");
    assert!((areas[0] - 0.5).abs() < 1e-9, "{areas:?}");
    let lone = rings(&comps[1], n);
    assert_eq!(lone.len(), 1);
    assert!((signed_area(&lone[0]).abs() - 0.5).abs() < 1e-9);
}

#[test]
fn simplify_keeps_corners() {
    let mut ring = Vec::new();
    for k in 0..10 {
        ring.push([f64::from(k), 0.0]);
    }
    for k in 0..10 {
        ring.push([10.0, f64::from(k)]);
    }
    for k in 0..10 {
        ring.push([10.0 - f64::from(k), 10.0]);
    }
    for k in 0..10 {
        ring.push([0.0, 10.0 - f64::from(k)]);
    }
    let s = simplify_ring(&ring, 0.1);
    assert_eq!(s.len(), 4);
    assert!((signed_area(&s).abs() - 100.0).abs() < 1e-9);
}

#[test]
fn iso_contour_interpolates() {
    // 4 x 3 field, one inside sample at (1, 1) of value -1 between outside samples of 1 and 3: the contour crosses
    // at 1/2 towards a 1 and at 1/4 towards a 3.
    let field = [1.0, 1.0, 1.0, 1.0, 1.0, -1.0, 3.0, 1.0, 1.0, 1.0, 1.0, 1.0];
    let rs = iso_rings(&field, 4, 3);
    assert_eq!(rs.len(), 1);
    let xs: Vec<f64> = rs[0].iter().filter(|p| (p[1] - 1.0).abs() < 1e-12).map(|p| p[0]).collect();
    let (lo, hi) = (xs.iter().copied().fold(f64::INFINITY, f64::min), xs.iter().copied().fold(f64::NEG_INFINITY, f64::max));
    assert!((lo - 0.5).abs() < 1e-12 && (hi - 1.25).abs() < 1e-12, "{xs:?}");
}

#[test]
fn push_out_moves_outwards_and_stops() {
    // Counter-clockwise unit square (y up): every corner moves 1 along its diagonal, away from the centre.
    let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let out = push_out(&square, 1.0, 0.25, |_, _| false);
    let h = std::f64::consts::FRAC_1_SQRT_2;
    for (p, q) in square.iter().zip(&out) {
        assert!(((q[0] - 0.5).abs() - (0.5 + h)).abs() < 1e-12 && ((q[1] - 0.5).abs() - (0.5 + h)).abs() < 1e-12, "{q:?} from {p:?}");
    }
    // A wall at x = 1.3: the right corners stop at the last step before it, the left ones go the whole way.
    let walled = push_out(&square, 1.0, 0.25, |_, to| to[0] > 1.3);
    assert!((walled[1][0] - (1.0 + 0.25 * h)).abs() < 1e-12, "{:?}", walled[1]);
    assert!((walled[0][0] + h).abs() < 1e-12);
}

#[test]
fn river_levels_fill_pool_and_never_rise() {
    // Falling ground down to under the sea; bed depth 1, fill 0.6, sea 0.
    let ground = [50.0, 40.0, 24.0, 20.0, 13.0, 0.0, -2.0];
    let depth = [1.0; 7];
    let none = [None; 7];
    let (l, pooled) = fit_levels(&ground, &depth, &none, 0.6, 0.0);
    // Each point at its ground + 0.6; the last, under the sea, at the sea.
    let want = [50.6, 40.6, 24.6, 20.6, 13.6, 0.6, 0.0];
    for k in 0..7 {
        assert!((l[k] - want[k]).abs() < 1e-9, "{l:?}");
    }
    assert_eq!(pooled, 0);
    // A bed that rises downstream by more than fill + depth (pit at 1, bump at 2): the pit pools to bump - depth.
    let (l, pooled) = fit_levels(&[30.0, 20.0, 25.0, 10.0], &[1.0; 4], &[None; 4], 0.6, 0.0);
    assert!((l[1] - 24.0).abs() < 1e-9 && (l[2] - 24.0).abs() < 1e-9, "{l:?}");
    assert!(l.windows(2).all(|w| w[1] <= w[0]));
    assert_eq!(pooled, 1);
    // On a lake the surface is the lake's, and the outlet below it never rises over it.
    let (l, _) = fit_levels(&[30.0, 5.0, 5.0, 21.0, 12.0], &[1.0; 5], &[None, Some(20.0), Some(20.0), None, None], 0.6, 0.0);
    assert!((l[0] - 30.6).abs() < 1e-9 && (l[1] - 20.0).abs() < 1e-9 && (l[2] - 20.0).abs() < 1e-9, "{l:?}");
    assert!((l[3] - 20.0).abs() < 1e-9 && (l[4] - 12.6).abs() < 1e-9, "{l:?}");
}
