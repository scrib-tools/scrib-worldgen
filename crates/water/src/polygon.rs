//! Lake outlines: 4-connected components of Magna Urbis's lake mask, marching squares (on a component's cells, or
//! on a height field's zero contour; saddles split, so diagonal cells stay apart as 4-connectivity has them),
//! Douglas-Peucker simplification.
//!
//! Coordinates here are cell-index space: cell (x, y) has its centre at (x, y), y down (row 0 = north).
use std::collections::HashMap;

/// 4-connected components of the cells where `mask` is non-zero, labelled in row-major order of their first cell.
/// Returns the labels (-1 = not in the mask) and the cells of each component.
/// # Panics
/// More than `i32::MAX` components.
#[must_use]
pub fn components(mask: &[u8], n: usize) -> (Vec<i32>, Vec<Vec<usize>>) {
    let mut label = vec![-1_i32; n * n];
    let mut comps: Vec<Vec<usize>> = Vec::new();
    let mut stack = Vec::new();
    for start in 0..n * n {
        if mask[start] == 0 || label[start] >= 0 {
            continue;
        }
        let id = i32::try_from(comps.len()).expect("component count fits i32");
        let mut cells = Vec::new();
        label[start] = id;
        stack.push(start);
        while let Some(c) = stack.pop() {
            cells.push(c);
            let (x, y) = (c % n, c / n);
            let mut visit = |nb: usize| {
                if mask[nb] != 0 && label[nb] < 0 {
                    label[nb] = id;
                    stack.push(nb);
                }
            };
            if x > 0 {
                visit(c - 1);
            }
            if x + 1 < n {
                visit(c + 1);
            }
            if y > 0 {
                visit(c - n);
            }
            if y + 1 < n {
                visit(c + n);
            }
        }
        cells.sort_unstable();
        comps.push(cells);
    }
    (label, comps)
}

/// Closed rings (cell-index space, last point not repeated) around one component's cells.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn rings(cells: &[usize], n: usize) -> Vec<Vec<[f64; 2]>> {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for &c in cells {
        let (x, y) = (c % n, c / n);
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    // Local field with one empty cell of padding on every side: local (i, j) = cell (x0 + i - 1, y0 + j - 1).
    let (w, h) = (x1 - x0 + 3, y1 - y0 + 3);
    let mut field = vec![1.0; w * h];
    for &c in cells {
        field[(c / n - y0 + 1) * w + (c % n - x0 + 1)] = -1.0;
    }
    iso_rings(&field, w, h)
        .into_iter()
        .map(|ring| ring.into_iter().map(|p| [x0 as f64 + p[0] - 1.0, y0 as f64 + p[1] - 1.0]).collect())
        .collect()
}

/// Closed rings (local index space: sample (i, j) at (i, j), last point not repeated) of the zero contour of a
/// `w` x `h` row-major field, inside where the value is negative. A vertex sits on a grid edge where the field,
/// taken linear along it, crosses zero (so a ±1 field gives edge midpoints). Saddles split, so diagonal inside
/// samples stay apart. The border samples must be outside (positive): every ring then closes.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn iso_rings(field: &[f64], w: usize, h: usize) -> Vec<Vec<[f64; 2]>> {
    // Edge keys: horizontal edge between local (i, j) and (i + 1, j) = 2·(j·w + i); vertical between (i, j) and
    // (i, j + 1) = 2·(j·w + i) + 1.
    let hk = |i: usize, j: usize| 2 * (j * w + i);
    let vk = |i: usize, j: usize| 2 * (j * w + i) + 1;
    let mut adjacent: HashMap<usize, [usize; 2]> = HashMap::new();
    let mut link = |a: usize, b: usize| {
        for (p, q) in [(a, b), (b, a)] {
            let e = adjacent.entry(p).or_insert([usize::MAX; 2]);
            if e[0] == usize::MAX { e[0] = q } else { e[1] = q }
        }
    };
    let inside = |i: usize, j: usize| field[j * w + i] < 0.0;
    for j in 0..h.saturating_sub(1) {
        for i in 0..w.saturating_sub(1) {
            let (tl, tr, br, bl) = (inside(i, j), inside(i + 1, j), inside(i + 1, j + 1), inside(i, j + 1));
            let code = u8::from(tl) << 3 | u8::from(tr) << 2 | u8::from(br) << 1 | u8::from(bl);
            let (t, b, l, r) = (hk(i, j), hk(i, j + 1), vk(i, j), vk(i + 1, j));
            match code {
                1 | 14 => link(l, b),
                2 | 13 => link(b, r),
                3 | 12 => link(l, r),
                4 | 11 => link(t, r),
                5 => {
                    link(t, r);
                    link(l, b);
                }
                6 | 9 => link(t, b),
                7 | 8 => link(t, l),
                10 => {
                    link(t, l);
                    link(b, r);
                }
                _ => {}
            }
        }
    }
    let point = |k: usize| -> [f64; 2] {
        let (cell, vertical) = (k / 2, k % 2 == 1);
        let (i, j) = (cell % w, cell / w);
        let (a, b) = (field[cell], if vertical { field[cell + w] } else { field[cell + 1] });
        // A linked edge has one end inside (negative) and one outside: a - b is never 0.
        let t = (a / (a - b)).clamp(0.0, 1.0);
        if vertical { [i as f64, j as f64 + t] } else { [i as f64 + t, j as f64] }
    };
    let mut keys: Vec<usize> = adjacent.keys().copied().collect();
    keys.sort_unstable();
    let mut seen: HashMap<usize, bool> = HashMap::new();
    let mut out = Vec::new();
    for start in keys {
        if seen.contains_key(&start) {
            continue;
        }
        let mut ring = Vec::new();
        let (mut prev, mut cur) = (usize::MAX, start);
        loop {
            seen.insert(cur, true);
            ring.push(point(cur));
            let [a, b] = adjacent[&cur];
            let next = if a == prev { b } else { a };
            prev = cur;
            cur = next;
            if cur == start || seen.contains_key(&cur) {
                break;
            }
        }
        out.push(ring);
    }
    out
}

/// Signed area (shoelace; positive = counter-clockwise in the frame the points are in).
#[must_use]
pub fn signed_area(ring: &[[f64; 2]]) -> f64 {
    let m = ring.len();
    let mut a = 0.0;
    for k in 0..m {
        let (p, q) = (ring[k], ring[(k + 1) % m]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    0.5 * a
}

fn seg_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

fn dp(points: &[[f64; 2]], tol: f64, keep: &mut [bool], lo: usize, hi: usize) {
    if hi <= lo + 1 {
        return;
    }
    let (mut far, mut at) = (0.0, lo);
    for k in lo + 1..hi {
        let d = seg_distance(points[k], points[lo], points[hi]);
        if d > far {
            (far, at) = (d, k);
        }
    }
    if far > tol {
        keep[at] = true;
        dp(points, tol, keep, lo, at);
        dp(points, tol, keep, at, hi);
    }
}

/// Douglas-Peucker on a closed ring: split at the first point and the point farthest from it.
#[must_use]
pub fn simplify_ring(ring: &[[f64; 2]], tol: f64) -> Vec<[f64; 2]> {
    let m = ring.len();
    if m < 4 || tol <= 0.0 {
        return ring.to_vec();
    }
    let far = (1..m)
        .max_by(|&a, &b| {
            let d = |k: usize| (ring[k][0] - ring[0][0]).hypot(ring[k][1] - ring[0][1]);
            d(a).total_cmp(&d(b)).then(b.cmp(&a))
        })
        .unwrap_or(m / 2);
    let mut closed = ring.to_vec();
    closed.push(ring[0]);
    let mut keep = vec![false; m + 1];
    (keep[0], keep[far], keep[m]) = (true, true, true);
    dp(&closed, tol, &mut keep, 0, far);
    dp(&closed, tol, &mut keep, far, m);
    let out: Vec<_> = (0..m).filter(|&k| keep[k]).map(|k| ring[k]).collect();
    if out.len() < 3 { ring.to_vec() } else { out }
}

/// Each vertex of a closed ring with its inside on the left (outer rings counter-clockwise, holes clockwise, y up)
/// moved `dist` outwards, along the mean of the outward normals of its two edges, in steps of at most `step`; a
/// vertex stays at its last position before a step that `blocked(from, to)` refuses.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn push_out(ring: &[[f64; 2]], dist: f64, step: f64, blocked: impl Fn([f64; 2], [f64; 2]) -> bool) -> Vec<[f64; 2]> {
    let m = ring.len();
    if m < 3 || dist <= 0.0 || step <= 0.0 {
        return ring.to_vec();
    }
    let normal = |a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dy);
        if l > 0.0 { [dy / l, -dx / l] } else { [0.0, 0.0] }
    };
    let steps = (dist / step).ceil().max(1.0) as usize;
    (0..m)
        .map(|k| {
            let p = ring[k];
            let (n1, n2) = (normal(ring[(k + m - 1) % m], p), normal(p, ring[(k + 1) % m]));
            let (sx, sy) = (n1[0] + n2[0], n1[1] + n2[1]);
            let l = sx.hypot(sy);
            if l < 1e-9 {
                return p;
            }
            let (ux, uy) = (sx / l, sy / l);
            let mut at = p;
            for i in 1..=steps {
                let t = dist * i as f64 / steps as f64;
                let next = [p[0] + ux * t, p[1] + uy * t];
                if blocked(at, next) {
                    break;
                }
                at = next;
            }
            at
        })
        .collect()
}
