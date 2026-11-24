// MU's ensure_regional_access contact test (urban/network/exits.rs:392) on a final layout: largest street component
// (by length, streets joined at shared vertices) vs the regional roads settle passed in; contact = a vertex within 3 m.
// node contact.mjs <layout.json> <roads.json>
import fs from 'node:fs';
const [lp, rp] = process.argv.slice(2);
const J = JSON.parse(fs.readFileSync(lp)), R = JSON.parse(fs.readFileSync(rp)), F = J.frame, S = J.layout.streets;
const key = (p) => `${Math.round(p.x * 100)},${Math.round(p.y * 100)}`;
const par = new Map(); const find = (k) => { while (par.get(k) !== k) { par.set(k, par.get(par.get(k))); k = par.get(k); } return k; };
const uni = (a, b) => { a = find(a); b = find(b); if (a !== b) par.set(a, b); };
for (const s of S) for (const p of s.path) if (!par.has(key(p))) par.set(key(p), key(p));
for (const s of S) for (let i = 1; i < s.path.length; i++) uni(key(s.path[i - 1]), key(s.path[i]));
const len = new Map(); for (const s of S) { let l = 0; for (let i = 1; i < s.path.length; i++) l += Math.hypot(s.path[i].x - s.path[i - 1].x, s.path[i].y - s.path[i - 1].y); const r = find(key(s.path[0])); len.set(r, (len.get(r) || 0) + l); }
const comps = [...len.entries()].sort((a, b) => b[1] - a[1]);
const big = comps[0][0];
const toLocal = ([x, y]) => [x - F.west_m, F.north_m - y];
const sd = (p, a, b) => { const dx = b[0] - a[0], dy = b[1] - a[1], l = dx * dx + dy * dy || 1, t = Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l)); return Math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy); };
const roadSegs = []; for (const s of R.segments) { const pts = s.points.map(toLocal); for (let i = 1; i < pts.length; i++) roadSegs.push([pts[i - 1], pts[i], s.id]); }
console.log(`components ${comps.length}; largest ${comps[0][1].toFixed(0)} m, next ${comps.slice(1, 4).map(c => c[1].toFixed(0)).join(' ')}`);
const hits = [];
for (const s of S) { if (find(key(s.path[0])) !== big) continue; for (const p of s.path) for (const [a, b, id] of roadSegs) { const d = sd([p.x, p.y], a, b); if (d < 3) hits.push(`${s.role}/${s.rank} seg ${id} ${d.toFixed(1)} m`); } }
console.log('contacts of the largest component (<3 m):', hits.length ? [...new Set(hits)].slice(0, 8).join('; ') : 'none');
