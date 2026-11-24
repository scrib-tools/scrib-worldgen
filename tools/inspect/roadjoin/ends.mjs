// Regional road ends vs Balmora 2.0 town streets (settle local frame: x east from west_m, y south from north_m).
// node ends.mjs <layout.json> <roads.json> <site dir>
import fs from 'node:fs';
const [layoutPath, roadsPath, siteDir] = process.argv.slice(2);
const L = JSON.parse(fs.readFileSync(layoutPath));
const R = JSON.parse(fs.readFileSync(roadsPath));
const site = JSON.parse(fs.readFileSync(siteDir + '/site.json'));
const H = new Float32Array(fs.readFileSync(siteDir + '/heights.f32').buffer.slice(0));
const W = new Uint8Array(fs.readFileSync(siteDir + '/water.u8'));
const [n] = site.dims, cm = site.cell_m, [ox, oy] = site.origin_m;
const F = L.frame;
const toLocal = ([x, y]) => [x - F.west_m, F.north_m - y];
const toGuide = ([u, v]) => [u + F.west_m, F.north_m - v];
const h = (p) => { const [x, y] = toGuide(p); const c = (x - ox) / cm, r = (oy - y) / cm;
  const c0 = Math.floor(c), r0 = Math.floor(r), fc = c - c0, fr = r - r0;
  const g = (rr, cc) => H[Math.min(n - 1, Math.max(0, rr)) * n + Math.min(n - 1, Math.max(0, cc))];
  return (g(r0, c0) * (1 - fc) + g(r0, c0 + 1) * fc) * (1 - fr) + (g(r0 + 1, c0) * (1 - fc) + g(r0 + 1, c0 + 1) * fc) * fr; };
const wet = (p) => { const [x, y] = toGuide(p); const c = Math.round((x - ox) / cm), r = Math.round((oy - y) / cm); return W[r * n + c] || 0; };
const d2 = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1]);
const segDist = (p, a, b) => { const dx = b[0] - a[0], dy = b[1] - a[1], l = dx * dx + dy * dy || 1;
  const t = Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l)); const q = [a[0] + t * dx, a[1] + t * dy]; return [d2(p, q), q]; };
// grade profile of a straight line a->b, sampled every `s` m (as MU's segment_grade: rise over run between samples)
const lineGrade = (a, b, s = 5) => { const len = d2(a, b), k = Math.max(1, Math.ceil(len / s)); let mx = 0, wetN = 0;
  for (let i = 0; i < k; i++) { const p = [a[0] + (b[0] - a[0]) * i / k, a[1] + (b[1] - a[1]) * i / k], q = [a[0] + (b[0] - a[0]) * (i + 1) / k, a[1] + (b[1] - a[1]) * (i + 1) / k];
    mx = Math.max(mx, Math.abs(h(q) - h(p)) / (len / k)); if (wet(p)) wetN++; }
  return { len, max: mx, mean: Math.abs(h(b) - h(a)) / (len || 1), wetN }; };
const pathGrade = (pts, s = 5) => { let mx = 0, over = 0, tot = 0, wetN = 0; for (let i = 1; i < pts.length; i++) { const g = lineGrade(pts[i - 1], pts[i], s); mx = Math.max(mx, g.max); tot += g.len; wetN += g.wetN;
  const k = Math.max(1, Math.ceil(g.len / s)); for (let j = 0; j < k; j++) { const a = pts[i-1], b = pts[i]; const p = [a[0] + (b[0] - a[0]) * j / k, a[1] + (b[1] - a[1]) * j / k], q = [a[0] + (b[0] - a[0]) * (j + 1) / k, a[1] + (b[1] - a[1]) * (j + 1) / k]; if (Math.abs(h(q) - h(p)) / (g.len / k) > 0.12) over += g.len / k; } }
  return { len: tot, max: mx, over012: over, wetN }; };
const c = [L.layout.center.x, L.layout.center.y];
const town = L.layout.streets;
const inside = (p, ring) => { let ins = false; for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) { const a = ring[i], b = ring[j];
  if ((a.y > p[1]) !== (b.y > p[1]) && p[0] < (b.x - a.x) * (p[1] - a.y) / (b.y - a.y) + a.x) ins = !ins; } return ins; };
const inQuarter = (p) => L.layout.quarters.some(q => inside(p, (q.shape || q).outer || []));
console.log('centre', c, 'h', h(c).toFixed(1), 'site_radius', L.site_radius_m.toFixed(0), 'town streets', town.length, 'quarters', L.layout.quarters.length);
const W0 = F.west_m, E0 = F.west_m + F.width_m, S0 = F.north_m - F.width_m, N0 = F.north_m;
for (const s of R.segments) {
  const xs = s.points.map(p => p[0]), ys = s.points.map(p => p[1]);
  if (!(Math.max(...xs) >= W0 && Math.min(...xs) <= E0 && Math.max(...ys) >= S0 && Math.min(...ys) <= N0)) continue;
  const loc = s.points.map(toLocal);
  const inCrop = loc.filter(p => p[0] >= 5 && p[0] <= F.width_m - 5 && p[1] >= 5 && p[1] <= F.width_m - 5);
  // the cut end = the trimmed end
  const cutAtStart = s.trimmed_m[0] > 0; const end = cutAtStart ? loc[0] : loc.at(-1);
  const order = cutAtStart ? loc : [...loc].reverse(); // from the cut end outward
  let best = [Infinity], bestSt = null;
  for (const st of town) for (let i = 1; i < st.path.length; i++) { const r = segDist(end, [st.path[i - 1].x, st.path[i - 1].y], [st.path[i].x, st.path[i].y]); if (r[0] < best[0]) { best = r; bestSt = st; } }
  // last stretch: first 300 m from the cut end, and the in-crop part
  let acc = 0, k = 1; while (k < order.length && acc < 300) { acc += d2(order[k - 1], order[k]); k++; }
  const last = pathGrade(order.slice(0, k));
  const crop = pathGrade(inCrop);
  const gap = lineGrade(end, best[1]);
  console.log(`seg ${s.id} ${s.class} w${s.width_m} trimmed ${s.trimmed_m.map(v => v.toFixed(0))} MU grade ${s.grade.toFixed(3)} | cut end local ${end.map(v => v.toFixed(0))} h ${h(end).toFixed(1)} d(c) ${d2(end, c).toFixed(0)} (${(d2(end, c) / L.site_radius_m).toFixed(2)} r) in quarter ${inQuarter(end)}`);
  console.log(`   in-crop: ${inCrop.length} pts ${crop.len.toFixed(0)} m, max grade ${crop.max.toFixed(3)}, ${crop.over012.toFixed(0)} m over 0.12, wet samples ${crop.wetN}`);
  console.log(`   last ${acc.toFixed(0)} m from cut: max ${last.max.toFixed(3)}, ${last.over012.toFixed(0)} m over 0.12, rise ${(h(order[k - 1]) - h(end)).toFixed(1)} m`);
  console.log(`   nearest town street ${bestSt.role}/${bestSt.rank} at ${best[1].map(v => v.toFixed(0))} h ${h(best[1]).toFixed(1)}: gap ${gap.len.toFixed(0)} m, mean ${gap.mean.toFixed(3)}, max ${gap.max.toFixed(3)}, wet ${gap.wetN}`);
}
