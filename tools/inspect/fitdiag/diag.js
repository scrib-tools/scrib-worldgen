// Diagnostics of the failing match parts. node diag.js
const D = require('./fitdiag.js');
const { pass, models, target, tryCand, inside, dot, sub, cw, unit, rotate, toWorld, angleDeg, mod, R, minBox, edges, bbox, boxesMeet, penetration } = D;
const fails = []; const base = pass({}, fails);
const placed = base.placed; const F = { overlaps: h => { const b = bbox(h); return placed.some(([pb, ph]) => boxesMeet(b, pb, R.otol) && penetration(h, ph) > R.otol); } };
const out = []; const cnt = {}; const inc = (k, v = 1) => cnt[k] = (cnt[k] || 0) + v;
const doorsBase = c => { const ds = c.m.doors; if (!ds.length) return [null]; const f = ds[0]; const back = ds.slice(1).find(d => { const off = mod(d.facing_deg - f.facing_deg - 180, 360); return Math.min(off, 360 - off) <= R.back_tol; }); return back ? [f, back] : [f]; };
// (c) free position in the street frame (raster of the lot, 0.25 m, then exact inside test).
function freeFit(t, opts) { const G = 0.25; const P = t.lot.polygon.map(p => [dot(t.u, p), dot(t.n, p)]); const us = P.map(p => p[0]), ns = P.map(p => p[1]);
  const u0 = Math.min(...us), n0 = Math.min(...ns), W = Math.ceil((Math.max(...us) - u0) / G) + 1, Hh = Math.ceil((Math.max(...ns) - n0) / G) + 1;
  const S = new Int32Array((W + 1) * (Hh + 1)); for (let j = 0; j < Hh; j++) for (let i = 0; i < W; i++) { const uu = u0 + (i + 0.5) * G, nn = n0 + (j + 0.5) * G; const w = [uu * t.u[0] + nn * t.n[0], uu * t.u[1] + nn * t.n[1]];
    const v = D.contains(t.shape, w) ? 0 : 1; S[(j + 1) * (W + 1) + i + 1] = v + S[j * (W + 1) + i + 1] + S[(j + 1) * (W + 1) + i] - S[j * (W + 1) + i]; }
  const blocked = (i0, j0, i1, j1) => S[j1 * (W + 1) + i1] - S[j0 * (W + 1) + i1] - S[j1 * (W + 1) + i0] + S[j0 * (W + 1) + i0];
  // largest inscribed rectangle area (cells) for the report
  let bestRect = 0, wmax = 0, dmax = 0; const hgt = new Int32Array(W); for (let j = 0; j < Hh; j++) { for (let i = 0; i < W; i++) hgt[i] = blocked(i, j, i + 1, j + 1) ? 0 : hgt[i] + 1;
    const st = []; for (let i = 0; i <= W; i++) { const h = i < W ? hgt[i] : 0; let start = i; while (st.length && st[st.length - 1][1] >= h) { const [s0, h0] = st.pop(); bestRect = Math.max(bestRect, h0 * (i - s0) * G * G); if (h0 * G >= 7.8) wmax = Math.max(wmax, (i - s0) * G); if ((i - s0) * G >= 8.0) dmax = Math.max(dmax, h0 * G); start = s0; } st.push([start, h]); } }
  const fits = [];
  for (const c of models) { const turns = []; for (const d of (opts.doors == 'all' ? c.m.doors.length ? c.m.doors : [null] : doorsBase(c))) { turns.push([d, 0]); if (opts.turn90) turns.push([d, 90], [d, -90]); }
    for (const [d, yo] of turns) { const yaw = mod(angleDeg(t.n) - (d ? d.facing_deg : 90) + yo, 360); const loc = c.hull.map(p => rotate(p, yaw)); const lu = loc.map(p => dot(t.u, p)), ln = loc.map(p => dot(t.n, p));
      const wu = Math.max(...lu) - Math.min(...lu), dn = Math.max(...ln) - Math.min(...ln); const wi = Math.ceil(wu / G), dj = Math.ceil(dn / G); let tries = 0, ok = null;
      for (let j = Hh - dj; j >= 0 && !ok && tries < 30; j--) for (let i = 0; i + wi <= W && !ok && tries < 30; i++) { if (blocked(i, j, i + wi, j + dj)) continue; tries++;
        const su = u0 + i * G - Math.min(...lu) + (wi * G - wu) / 2, sn = n0 + j * G - Math.min(...ln) + (dj * G - dn) / 2; const pos = [su * t.u[0] + sn * t.n[0], su * t.u[1] + sn * t.n[1]]; const h = c.hull.map(p => toWorld(p, pos, yaw));
        if (inside(t, h)) ok = { h, model: c.m.id, frontGap: Math.max(...P.map(p => p[1])) - (sn + Math.max(...ln)) }; }
      if (ok) { fits.push(ok); break; } } }
  return { bestRect, wmax, dmax, fits }; }
function frontAt(t, uq) { let best = -Infinity; for (const [a, b] of edges(t.lot.polygon)) { const ua = dot(t.u, a), ub = dot(t.u, b); if ((ua - uq) * (ub - uq) > 0 || ua == ub) continue; const s = (uq - ua) / (ub - ua); best = Math.max(best, dot(t.n, a) + s * (dot(t.n, b) - dot(t.n, a))); } return best; }
const variants = { doors_all: { doors: 'all' }, turn90: { doors: 'all', turn90: true }, n_back: { nslide: [0, -0.5, -1, -1.5, -2, -3, -4, -5, -6] },
  n_both: { nslide: [0, -0.5, 0.5, -1, 1, -1.5, 1.5, -2, 2, -3, 3, -4, 4, -6, 6] }, u_wide: { slide_max: 41 }, nu: { slide_max: 41, nslide: [0, -0.5, 0.5, -1, 1, -1.5, 1.5, -2, 2, -3, 3, -4, 4, -6, 6] },
  nu_doors: { doors: 'all', slide_max: 41, nslide: [0, -0.5, 0.5, -1, 1, -1.5, 1.5, -2, 2, -3, 3, -4, 4, -6, 6] } };
for (const f of fails) { const t = f.t; const r = { lot: f.lot.id.split('/').pop(), full: f.lot.id, pi: f.pi, reason: ['inside', 'overlap', 'pad'][f.reason], narrow: f.narrow };
  r.angle = (() => { const a = Math.acos(Math.min(1, Math.abs(dot(t.box.e, t.u)))) * 180 / Math.PI; return Math.min(a, 90 - a); })();
  r.frontGap = frontAt(t, t.anchor_u) - t.n_max; r.lotW = t.u_range[1] - t.u_range[0]; r.partW = t.box.w; r.partD = t.box.d;
  if (f.diag.best) { r.exc = f.diag.best.e; r.cls = f.diag.best.cls; r.model = f.diag.best.model; }
  if (f.diag.pad) r.pad = f.diag.pad;
  const ff = freeFit(t, {}); r.rect = ff.bestRect; r.wmax = ff.wmax; r.dmax = ff.dmax; r.free = ff.fits.length > 0; r.freeClear = ff.fits.some(x => !F.overlaps(x.h)); r.freeGap = ff.fits.length ? Math.min(...ff.fits.map(x => x.frontGap)) : null;
  const ff2 = freeFit(t, { doors: 'all', turn90: true }); r.free90 = ff2.fits.length > 0;
  r.var = {}; for (const [k, o] of Object.entries(variants)) r.var[k] = models.some(c => tryCand(F, t, c, o, null).ok);
  out.push(r); }
require("fs").writeFileSync(__dirname + "/fails.json", JSON.stringify(out)); require("fs").writeFileSync(__dirname + "/filledlots.json", JSON.stringify([...base.filledLots]));
console.log('fails', out.length, 'base filled', base.filled);
