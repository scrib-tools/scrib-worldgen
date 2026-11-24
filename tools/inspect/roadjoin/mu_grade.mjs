// MU-faithful check of the 3 regional streets settle passes in: settle's 5 m crop grid (point samples of the site,
// bilinear), MU's half-cell steepest grade (routing.rs inspect_segment), max_street_grade 0.12, and the approach window
// of macroplan.rs:1282 (nearest road point to the centre within 0.7 r .. 2.5 r).
// node mu_grade.mjs <layout.json> <roads.json> <site dir> [radius_m]
import fs from 'node:fs';
const [layoutPath, roadsPath, siteDir, rArg] = process.argv.slice(2);
const L = JSON.parse(fs.readFileSync(layoutPath)), R = JSON.parse(fs.readFileSync(roadsPath));
const site = JSON.parse(fs.readFileSync(siteDir + '/site.json'));
const SH = new Float32Array(fs.readFileSync(siteDir + '/heights.f32').buffer.slice(0));
const [sn] = site.dims, scm = site.cell_m, [ox, oy] = site.origin_m;
const F = L.frame, n = F.n, sp = F.width_m / n;
const siteH = (x, y) => { const c = (x - ox) / scm, r = (oy - y) / scm, c0 = Math.floor(c), r0 = Math.floor(r), fc = c - c0, fr = r - r0;
  const g = (rr, cc) => SH[Math.min(sn - 1, Math.max(0, rr)) * sn + Math.min(sn - 1, Math.max(0, cc))];
  return (g(r0, c0) * (1 - fc) + g(r0, c0 + 1) * fc) * (1 - fr) + (g(r0 + 1, c0) * (1 - fc) + g(r0 + 1, c0 + 1) * fc) * fr; };
const G = new Float64Array(n * n);
for (let r = 0; r < n; r++) for (let c = 0; c < n; c++) G[r * n + c] = siteH(F.west_m + (c + 0.5) * sp, F.north_m - (r + 0.5) * sp);
const h = ([x, y]) => { const c = x / sp - 0.5, r = y / sp - 0.5, c0 = Math.floor(c), r0 = Math.floor(r), fc = c - c0, fr = r - r0;
  const g = (rr, cc) => G[Math.min(n - 1, Math.max(0, rr)) * n + Math.min(n - 1, Math.max(0, cc))];
  return (g(r0, c0) * (1 - fc) + g(r0, c0 + 1) * fc) * (1 - fr) + (g(r0 + 1, c0) * (1 - fc) + g(r0 + 1, c0 + 1) * fc) * fr; };
const segG = (a, b) => { const len = Math.hypot(b[0] - a[0], b[1] - a[1]), k = Math.max(1, Math.ceil(len / (sp / 2))); let m = 0;
  for (let i = 0; i < k; i++) { const p = [a[0] + (b[0] - a[0]) * i / k, a[1] + (b[1] - a[1]) * i / k], q = [a[0] + (b[0] - a[0]) * (i + 1) / k, a[1] + (b[1] - a[1]) * (i + 1) / k];
    m = Math.max(m, Math.abs(h(q) - h(p)) / (len / k)); } return m; };
const toLocal = ([x, y]) => [x - F.west_m, F.north_m - y];
const clipSq = (pts, lo, hi) => pts.filter(p => p[0] >= lo && p[0] <= hi && p[1] >= lo && p[1] <= hi); // vertex-level approx of regional.rs clip
const c = [L.layout.center.x, L.layout.center.y], rad = Number(rArg || L.site_radius_m);
let nearest = [Infinity];
const W0 = F.west_m, E0 = W0 + F.width_m, S0 = F.north_m - F.width_m, N0 = F.north_m;
for (const s of R.segments) {
  const xs = s.points.map(p => p[0]), ys = s.points.map(p => p[1]);
  if (!(Math.max(...xs) >= W0 && Math.min(...xs) <= E0 && Math.max(...ys) >= S0 && Math.min(...ys) <= N0)) continue;
  const pts = clipSq(s.points.map(toLocal), sp, F.width_m - sp);
  const grades = []; for (let i = 1; i < pts.length; i++) grades.push(segG(pts[i - 1], pts[i]));
  // radial profile: per vertex distance to centre, height, grade of the leg after it
  const prof = pts.map((p, i) => `${Math.hypot(p[0] - c[0], p[1] - c[1]).toFixed(0)}m:${h(p).toFixed(0)}h${i < grades.length ? ':' + grades[i].toFixed(2) : ''}`);
  console.log(`seg ${s.id}: ${pts.length} vertices on crop, legs over 0.12: ${grades.filter(g => g > 0.12).length}/${grades.length}, steepest ${Math.max(...grades).toFixed(2)}`);
  console.log('   d(c):height:grade ' + prof.join(' '));
  for (const p of pts) { const d = Math.hypot(p[0] - c[0], p[1] - c[1]); if (d < nearest[0]) nearest = [d, s.id]; }
}
console.log(`centre h ${h(c).toFixed(1)}; nearest road vertex to centre ${nearest[0].toFixed(0)} m (seg ${nearest[1]}); approach window ${(0.7 * rad).toFixed(0)}..${(2.5 * rad).toFixed(0)} m (r ${rad.toFixed(0)}) -> ${nearest[0] > 0.7 * rad && nearest[0] < 2.5 * rad ? 'approach routed' : 'NO approach'}`);
