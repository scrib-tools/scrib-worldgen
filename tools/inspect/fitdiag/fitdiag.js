// Port of crates/settle/src/fit.rs `match` domain + diagnostics. node fitdiag.js
const fs = require('fs');
const crypto = require('crypto');
const DIR = 'D:/Gamedev/scrib-worldgen/target/c5b-match/medina';
const SITE = 'C:/cargo-target/scrib/worldgen/sites/balmora-x10hs-b2w-c1';
const KIT = 'C:/cargo-target/scrib/worldgen/kit/hlaalu-bs/kit.json';
const lots = JSON.parse(fs.readFileSync(DIR + '/lots.json')).lots;
const kit = JSON.parse(fs.readFileSync(KIT));
const meta = JSON.parse(fs.readFileSync(SITE + '/site.json'));
const hb = fs.readFileSync(SITE + '/heights.f32');
const H = new Float32Array(hb.buffer, hb.byteOffset, hb.length / 4);
const R = { slide_step: 1, slide_max: 9, setback: 0, tol: 0.05, otol: 0.05, probe: 1, gs: 1, rise: 1, margin: 0.1, back_tol: 45 };

const dot = (a, b) => a[0] * b[0] + a[1] * b[1];
const sub = (a, b) => [a[0] - b[0], a[1] - b[1]];
const unit = v => { const l = Math.hypot(v[0], v[1]); return l > 0 && isFinite(l) ? [v[0] / l, v[1] / l] : null; };
const cw = v => [v[1], -v[0]];
const angleDeg = v => { const a = Math.atan2(v[1], v[0]) * 180 / Math.PI; return ((a % 360) + 360) % 360; };
const mod = (a, m) => ((a % m) + m) % m;
const sarea = r => { let s = 0; for (let i = 0; i < r.length; i++) { const a = r[i], b = r[(i + 1) % r.length]; s += a[0] * b[1] - b[0] * a[1]; } return s / 2; };
const centroid = r => { const A = sarea(r); if (Math.abs(A) > 0) { let cx = 0, cy = 0; for (let i = 0; i < r.length; i++) { const p = r[i], q = r[(i + 1) % r.length]; const k = p[0] * q[1] - q[0] * p[1]; cx += (p[0] + q[0]) * k; cy += (p[1] + q[1]) * k; } return [cx / (6 * A), cy / (6 * A)]; } return [r.reduce((s, p) => s + p[0], 0) / r.length, r.reduce((s, p) => s + p[1], 0) / r.length]; };
const crossO = (o, a, b) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
function hull(points) {
  const p = points.slice().sort((a, b) => a[0] - b[0] || a[1] - b[1]).filter((q, i, s) => i == 0 || q[0] != s[i - 1][0] || q[1] != s[i - 1][1]);
  if (p.length < 3) return p;
  const h = [];
  const chain = (q, start) => { while (h.length >= start + 2 && crossO(h[h.length - 2], h[h.length - 1], q) <= 0) h.pop(); h.push(q); };
  for (const q of p) chain(q, 0);
  const lower = h.length;
  for (let i = p.length - 2; i >= 0; i--) chain(p[i], lower - 1);
  h.pop(); return h;
}
const edges = r => r.map((a, i) => [a, r[(i + 1) % r.length]]);
function minBox(points) { // centre + unit edge direction
  const h = hull(points); let best = null;
  for (const [a, b] of edges(h)) { const e = unit(sub(b, a)); if (!e) continue; const f = [-e[1], e[0]];
    const sp = v => h.reduce(([lo, hi], p) => [Math.min(lo, dot(v, p)), Math.max(hi, dot(v, p))], [Infinity, -Infinity]);
    const [[elo, ehi], [flo, fhi]] = [sp(e), sp(f)]; const area = (ehi - elo) * (fhi - flo);
    if (!best || area < best.area) { const ce = (elo + ehi) / 2, cf = (flo + fhi) / 2; best = { area, c: [ce * e[0] + cf * f[0], ce * e[1] + cf * f[1]], e, w: ehi - elo, d: fhi - flo }; } }
  return best;
}
const segDist = (p, a, b) => { const ab = sub(b, a), l2 = dot(ab, ab); const t = l2 > 0 ? Math.min(1, Math.max(0, dot(sub(p, a), ab) / l2)) : 0; return Math.hypot(p[0] - a[0] - ab[0] * t, p[1] - a[1] - ab[1] * t); };
const halfPlanes = h => edges(h).map(([a, b]) => { const n = unit(cw(sub(b, a))); return n ? [n, dot(n, a)] : null; }).filter(Boolean);
function segEnters(planes, shrink, p0, p1) { const d = sub(p1, p0); let t0 = 0, t1 = 1;
  for (const [n, c] of planes) { const num = c - shrink - dot(n, p0), den = dot(n, d);
    if (den == 0) { if (num < 0) return false; } else if (den < 0) t0 = Math.max(t0, num / den); else t1 = Math.min(t1, num / den);
    if (t0 >= t1) return false; } return t0 < t1; }
function penetration(a, b) { let depth = Infinity; for (const h of [a, b]) for (const [n] of halfPlanes(h)) {
  const sp = hh => hh.reduce(([lo, hi], p) => [Math.min(lo, dot(n, p)), Math.max(hi, dot(n, p))], [Infinity, -Infinity]);
  const [[alo, ahi], [blo, bhi]] = [sp(a), sp(b)]; depth = Math.min(depth, Math.min(ahi, bhi) - Math.max(alo, blo)); if (depth <= 0) return depth; } return depth; }
const bbox = h => h.reduce((b, p) => [Math.min(b[0], p[0]), Math.min(b[1], p[1]), Math.max(b[2], p[0]), Math.max(b[3], p[1])], [Infinity, Infinity, -Infinity, -Infinity]);
const boxesMeet = (a, b, pad) => a[0] < b[2] - pad && b[0] < a[2] - pad && a[1] < b[3] - pad && b[1] < a[3] - pad;
const rotate = (v, yaw) => { const q = mod(yaw, 360); let s, c; if (q == 0) [s, c] = [0, 1]; else if (q == 90) [s, c] = [1, 0]; else if (q == 180) [s, c] = [0, -1]; else if (q == 270) [s, c] = [-1, 0]; else { s = Math.sin(q * Math.PI / 180); c = Math.cos(q * Math.PI / 180); } return [v[0] * c - v[1] * s, v[0] * s + v[1] * c]; };
const toWorld = (l, pos, yaw) => { const r = rotate(l, yaw); return [pos[0] + r[0], pos[1] + r[1]]; };
const pip = (ring, q) => { let ins = false; for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) { const a = ring[i], b = ring[j]; if ((a[1] > q[1]) != (b[1] > q[1]) && q[0] < (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) + a[0]) ins = !ins; } return ins; };
const contains = (shape, q) => pip(shape.outer, q) && !shape.holes.some(h => pip(h, q));
function ground(x, y) { const [cols, rows] = meta.dims; let col = (x - meta.origin_m[0]) / meta.cell_m - 0.5, row = (meta.origin_m[1] - y) / meta.cell_m - 0.5;
  col = Math.min(Math.max(col, 0), cols - 1); row = Math.min(Math.max(row, 0), rows - 1); const c0 = Math.floor(col), r0 = Math.floor(row), c1 = Math.min(c0 + 1, cols - 1), r1 = Math.min(r0 + 1, rows - 1);
  const tc = col - c0, tr = row - r0, at = (c, r) => H[r * cols + c]; return (at(c0, r0) * (1 - tc) + at(c1, r0) * tc) * (1 - tr) + (at(c0, r1) * (1 - tc) + at(c1, r1) * tc) * tr; }
// Ground samples under a hull (corners + grid inside).
function groundRange(h) { let lo = Infinity, hi = -Infinity; const add = p => { const z = ground(p[0], p[1]); lo = Math.min(lo, z); hi = Math.max(hi, z); };
  h.forEach(add); const pl = halfPlanes(h); const [x0, y0, x1, y1] = bbox(h); const nx = Math.floor((x1 - x0) / R.gs), ny = Math.floor((y1 - y0) / R.gs);
  for (let j = 0; j <= ny; j++) for (let i = 0; i <= nx; i++) { const p = [x0 + i * R.gs, y0 + j * R.gs]; if (!pl.some(([n, c]) => dot(n, p) > c)) add(p); } return [lo, hi]; }

// Candidates. opts.doors: 'base' | 'all'; opts.turn90: also turns with the door 90 deg off the street.
const models = kit.models.filter(m => m.kind == 'building').map(m => { const h = hull(m.footprint); return { m, hull: h, area: sarea(h) }; }).filter(c => c.hull.length >= 3 && c.area > 0);
function streetDoors(c, opts) { const ds = c.m.doors; if (!ds.length) return [null];
  if (opts.doors == 'all') { const out = []; for (const d of ds) if (!out.some(o => Math.abs(mod(o.facing_deg - d.facing_deg + 180, 360) - 180) < 1)) out.push(d); return out; }
  const f = ds[0]; const back = ds.slice(1).find(d => { const off = mod(d.facing_deg - f.facing_deg - 180, 360); return Math.min(off, 360 - off) <= R.back_tol; }); return back ? [f, back] : [f]; }
function streetNormal(lot) { const c = centroid(lot.polygon); const fs = s => { if (!s) return null; const [a, b] = s; const n = unit(cw(sub(b, a))); if (!n) return null; const mid = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]; return dot(n, sub(mid, c)) >= 0 ? n : [-n[0], -n[1]]; };
  return (lot.access && fs(lot.access.segment)) || fs(lot.front) || (unit(lot.axis) && cw(unit(lot.axis))); }
function target(lot, part, n) { const u = cw(n); const holes = lot.holes || []; const shape = { outer: lot.polygon, holes };
  const along = v => lot.polygon.map(p => dot(v, p)); const box = minBox(part.polygon); if (!box) return null;
  const sign = sarea(lot.polygon) >= 0 ? 1 : -1;
  return { lot, part, shape, rings: [lot.polygon, ...holes], area: Math.abs(sarea(lot.polygon)) - holes.reduce((s, h) => s + Math.abs(sarea(h)), 0), n, u,
    perimeter: [lot.polygon, ...holes].flatMap(edges).reduce((s, [a, b]) => s + Math.hypot(b[0] - a[0], b[1] - a[1]), 0),
    n_max: Math.max(...part.polygon.map(p => dot(n, p))), u_range: [Math.min(...along(u)), Math.max(...along(u))], anchor_u: dot(u, box.c), box, sign }; }
function inside(t, h) { const near = p => t.rings.some(r => edges(r).some(([a, b]) => segDist(p, a, b) <= R.tol));
  if (!h.every(p => contains(t.shape, p) || near(p))) return false; const pl = halfPlanes(h);
  return !t.rings.some(r => edges(r).some(([a, b]) => segEnters(pl, R.tol, a, b))); }
// How far a hull leaves the lot: worst outside vertex distance or lot-vertex notch depth, and the edge class crossed.
function excursion(t, h) { let worst = 0, cls = null; const sign = t.sign;
  for (const p of h) { if (contains(t.shape, p)) continue; let best = Infinity, be = null; for (const r of t.rings) for (const [a, b] of edges(r)) { const d = segDist(p, a, b); if (d < best) { best = d; be = [a, b, r]; } }
    if (best > worst) { worst = best; const [a, b, r] = be; if (r !== t.lot.polygon) cls = 'hole'; else { const o0 = cw(sub(b, a)), o = unit([o0[0] * sign, o0[1] * sign]); const on = dot(o, t.n), ou = Math.abs(dot(o, t.u)); cls = on > ou ? 'street' : -on > ou ? 'back' : 'side'; } } }
  const pl = halfPlanes(h); for (const r of t.rings) for (const q of r) { const dep = Math.min(...pl.map(([n, c]) => c - dot(n, q))); if (dep > worst) { worst = dep; cls = 'notch'; } }
  return [worst, cls]; }
function padTest(h, probe, cm) { const pz = ground(probe[0], probe[1]); const below = cm.foundation_depth_m ?? (cm.ground_z_m - cm.base_offset_m); const [glo, ghi] = groundRange(h);
  return { ok: glo >= pz - below && ghi <= pz + R.rise, pz, below, glo, ghi, drop: pz - glo, riseBy: ghi - pz }; }

// One turn: opts.nslide = list of n offsets (m) to try per u position (baseline [0]); opts.yawOff = extra yaw (deg).
function tryTurn(F, t, c, door, opts, diag) {
  const facing = door ? door.facing_deg : 90; const yaw = mod(angleDeg(t.n) - facing + (opts.yawOff || 0), 360);
  const local = c.hull.map(p => rotate(p, yaw)); const sp = v => local.reduce(([lo, hi], p) => [Math.min(lo, dot(v, p)), Math.max(hi, dot(v, p))], [Infinity, -Infinity]);
  const [hulo, huhi] = sp(t.u); const [, hnhi] = sp(t.n); const lo = t.u_range[0] - hulo, hi = t.u_range[1] - huhi;
  if (hi < lo - 2 * R.tol) return { err: 0, narrow: true };
  const depth0 = t.n_max - R.setback - hnhi; const mid = t.anchor_u - (hulo + huhi) / 2;
  const slides = [mid]; const smax = opts.slide_max || R.slide_max;
  for (let k = 1; ; k++) { const plus = mid + k * R.slide_step, minus = mid - k * R.slide_step; const pok = plus <= hi, mok = minus >= lo; if (slides.length >= smax || !(pok || mok)) break; if (pok) slides.push(plus); if (mok && slides.length < smax) slides.push(minus); }
  let furthest = 0;
  for (const dn of (opts.nslide || [0])) for (const s of slides) {
    const depth = depth0 + dn; const pos = [s * t.u[0] + depth * t.n[0], s * t.u[1] + depth * t.n[1]]; const h = c.hull.map(p => toWorld(p, pos, yaw));
    if (!inside(t, h)) { if (diag) { const [e, cls] = excursion(t, h); if (!diag.best || e < diag.best.e) diag.best = { e, cls, model: c.m.id, dn, ds: s - mid }; } continue; }
    furthest = Math.max(furthest, 1); if (F.overlaps(h)) continue; furthest = 2;
    let probe = null; if (door) { const at = toWorld(door.at, pos, yaw), f = (door.facing_deg + yaw) * Math.PI / 180; probe = [at[0] + Math.cos(f) * R.probe, at[1] + Math.sin(f) * R.probe]; } else probe = centroid(h);
    const pt = padTest(h, probe, c.m);
    if (!pt.ok) { if (diag) { const v = Math.max(pt.drop - pt.below, pt.riseBy - R.rise); if (!diag.pad || v < diag.pad.v) diag.pad = { v, ...pt, model: c.m.id, probeVsCentroid: pt.pz - ground(...centroid(h)) }; }
      if (opts.pad == 'level') { const lo2 = pt.ghi - R.rise, hi2 = pt.glo + pt.below; if (lo2 <= hi2) { const pz = Math.min(Math.max(pt.pz, lo2), hi2); if (Math.abs(pz - pt.pz) <= opts.step_max) return { ok: true, hull: h, step: pz - pt.pz, dn }; } }
      continue; }
    return { ok: true, hull: h, dn };
  }
  return { err: furthest };
}
function tryCand(F, t, c, opts, diag) { if (c.area > t.area + R.tol * t.perimeter) return { err: 0, big: true }; let furthest = 0, narrowAll = true;
  const turns = []; for (const d of streetDoors(c, opts)) { turns.push([d, 0]); if (opts.turn90) { turns.push([d, 90]); turns.push([d, -90]); } }
  for (const [d, yo] of turns) { const r = tryTurn(F, t, c, d, { ...opts, yawOff: yo }, diag); if (r.ok) return r; if (!r.narrow) narrowAll = false; furthest = Math.max(furthest, r.err); }
  return { err: furthest, narrow: narrowAll }; }
// Intersection area of convex CCW hull `h` with polygon `poly` (Sutherland-Hodgman clip of poly by h).
function clipArea(poly, h) { let out = poly.slice(); if (sarea(out) < 0) out.reverse();
  for (const [a, b] of edges(h)) { const inp = out; out = []; if (!inp.length) break; const side = p => crossO(a, b, p) >= 0;
    for (let i = 0; i < inp.length; i++) { const p = inp[i], q = inp[(i + 1) % inp.length]; const ip = side(p), iq = side(q);
      if (ip) out.push(p); if (ip != iq) { const d1 = crossO(a, b, p), d2 = crossO(a, b, q), tt = d1 / (d1 - d2); out.push([p[0] + (q[0] - p[0]) * tt, p[1] + (q[1] - p[1]) * tt]); } } }
  return out.length >= 3 ? Math.abs(sarea(out)) : 0; }
function draw(lot, pi) { const h = crypto.createHash('sha256').update(`settle-example-1\0${lot}\0${pi}`).digest(); return Number(h.readBigUInt64LE(0) >> 11n) / 2 ** 53; }

// Full sequential pass. opts.frame: 'street' | 'part' (n = the part box normal nearest the street normal).
function pass(opts, collect) {
  const placed = []; const F = { overlaps: h => { const b = bbox(h); return placed.some(([pb, ph]) => boxesMeet(b, pb, R.otol) && penetration(h, ph) > R.otol); } };
  const order = lots.filter(l => l.use == 'plot').sort((a, b) => a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
  const res = { parts: 0, filled: 0, err: [0, 0, 0], lotsFilled: 0, lotsTried: 0, steps: [], ious: [] };
  for (const lot of order) { const parts = lot.preview_parts.map((p, i) => [i, p]).filter(([, p]) => p.label == 'building'); if (!parts.length) continue; res.lotsTried++;
    const n0 = streetNormal(lot); if (!n0) continue; let any = false;
    for (const [pi, part] of parts) { res.parts++;
      const goalA = Math.abs(sarea(part.polygon)); const diag = collect ? {} : null; let furthest = 0, fits = [], narrow = true, big = true, t = null, n = n0;
      for (const so of (opts.stages || [opts])) { if (fits.length) break;
        n = n0; if (so.frame == 'part') { const b = minBox(part.polygon); const cs = [b.e, [-b.e[1], b.e[0]], [-b.e[0], -b.e[1]], [b.e[1], -b.e[0]]]; n = cs.reduce((x, y) => dot(y, n0) > dot(x, n0) ? y : x); if (so.max_angle && Math.acos(Math.min(1, dot(n, n0))) * 180 / Math.PI > so.max_angle) n = n0; }
        t = target(lot, part, n); if (!t) break; const r0 = R.rise; if (so.rise) R.rise = so.rise;
        for (const c of models) { const r = tryCand(F, t, c, so, diag); if (r.ok) fits.push([c, clipArea(part.polygon, r.hull), r]); else { furthest = Math.max(furthest, r.err); if (!r.narrow) narrow = false; } if (!r.big) big = false; }
        R.rise = r0; }
      if (!t) { res.err[0]++; continue; }
      if (fits.length) { for (const f of fits) { const inter = f[1]; f[1] = inter / (f[0].area + goalA - inter); }
        const best = Math.max(...fits.map(f => f[1])); const pool = fits.filter(f => f[1] >= best - R.margin); const tot = pool.reduce((s, f) => s + Math.max(0, f[0].m.weight), 0);
        let rr = draw(lot.id, pi) * tot; const pick = pool.find(f => (rr -= Math.max(0, f[0].m.weight)) < 0) || pool[pool.length - 1];
        placed.push([bbox(pick[2].hull), pick[2].hull]); res.filled++; (res.fp ||= new Set()).add(lot.id + "#" + pi); any = true; res.ious.push(pick[1]); if (pick[2].step !== undefined) res.steps.push(pick[2].step); (res.dns ||= []).push(pick[2].dn); (res.angs ||= []).push(Math.acos(Math.min(1, dot(n, n0))) * 180 / Math.PI); }
      else { res.err[furthest]++; if (collect) collect.push({ lot, part, pi, t, reason: furthest, narrow: narrow && !big, diag }); } }
    if (any) { res.lotsFilled++; (res.filledLots ||= new Set()).add(lot.id); } }
  res.placed = placed; return res;
}
module.exports = { pass, models, target, tryCand, tryTurn, inside, excursion, streetNormal, minBox, dot, sub, cw, unit, sarea, edges, contains, toWorld, rotate, angleDeg, mod, R, lots, hull, bbox, boxesMeet, penetration, ground };
if (require.main === module) { const fails = []; const r = pass({}, fails); const q = a => { a = a.slice().sort((x, y) => x - y); return [0.1, 0.5, 0.9].map(p => a[Math.max(1, Math.ceil(p * a.length)) - 1].toFixed(3)); };
  console.log('baseline parts', r.parts, 'filled', r.filled, 'err inside/overlap/pad', r.err.join('/'), 'lots', r.lotsTried, 'filled', r.lotsFilled, 'iou', q(r.ious)); }
