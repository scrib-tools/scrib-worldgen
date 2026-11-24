// Does each regional road (as settle passes it) meet the town's streets? node joins.mjs <layout.json> <roads.json>
// Per road on the crop: min distance from the densified road (2 m) to any town street, and the metres of road
// within 8 m of a town street (shared line = MU kept/joined it). Also lists layout streets with role regional/approach rank<=2 near a road end.
import fs from 'node:fs';
const [lp, rp] = process.argv.slice(2);
const J = JSON.parse(fs.readFileSync(lp)), R = JSON.parse(fs.readFileSync(rp)), F = J.frame, L = J.layout;
const toLocal = ([x, y]) => [x - F.west_m, F.north_m - y];
const segs = []; for (const s of L.streets) for (let i = 1; i < s.path.length; i++) segs.push([[s.path[i - 1].x, s.path[i - 1].y], [s.path[i].x, s.path[i].y], s]);
const sd = (p, a, b) => { const dx = b[0] - a[0], dy = b[1] - a[1], l = dx * dx + dy * dy || 1, t = Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l)); return Math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy); };
const near = (p) => { let b = Infinity, st = null; for (const [a, c, s] of segs) { const d = sd(p, a, c); if (d < b) { b = d; st = s; } } return [b, st]; };
const c = [L.center.x, L.center.y];
for (const s of R.segments) {
  const pts = s.points.map(toLocal).filter(p => p[0] >= 5 && p[0] <= F.width_m - 5 && p[1] >= 5 && p[1] <= F.width_m - 5);
  if (pts.length < 2) continue;
  let min = [Infinity], shared = 0, roles = {};
  for (let i = 1; i < pts.length; i++) { const a = pts[i - 1], b = pts[i], len = Math.hypot(b[0] - a[0], b[1] - a[1]), k = Math.ceil(len / 2);
    for (let j = 0; j <= k; j++) { const p = [a[0] + (b[0] - a[0]) * j / k, a[1] + (b[1] - a[1]) * j / k], r = near(p); if (r[0] < min[0]) min = [r[0], r[1], p]; if (r[0] < 8) { shared += len / k; roles[r[1].role + '/' + r[1].rank] = 1; } } }
  console.log(`seg ${s.id} ${s.class}: min gap ${min[0].toFixed(1)} m to ${min[1].role}/${min[1].rank} at d(c) ${Math.hypot(min[2][0] - c[0], min[2][1] - c[1]).toFixed(0)} m; ${shared.toFixed(0)} m within 8 m of town streets [${Object.keys(roles).join(' ')}]`);
}
const keep = L.streets.filter(s => s.role === 'regional' || s.role === 'outbound'); console.log('layout regional/outbound streets:', keep.map(s => s.role + ':' + s.path.length + 'pts').join(' ') || 'none');
