import fs from "node:fs";
const dir = process.argv[2]; const s = JSON.parse(fs.readFileSync(`${dir}/site.json`));
const [nx, ny] = s.dims, c = s.cell_m, [ox, oy] = s.origin_m, [cx, cy] = s.centre_m;
const hb = fs.readFileSync(`${dir}/heights.f32`); const H = new Float32Array(hb.buffer, hb.byteOffset, nx * ny); const W = fs.readFileSync(`${dir}/water.u8`);
const slope = (i) => { const q = i % nx, r = (i / nx) | 0; if (q < 1 || r < 1 || q >= nx - 1 || r >= ny - 1) return 9; return Math.hypot((H[i + 1] - H[i - 1]) / (2 * c), (H[i + nx] - H[i - nx]) / (2 * c)); };
// Flat components (slope < g, dry), 8-connected; report the biggest within R of the centre, and how far they reach.
for (const g of [0.05, 0.12]) {
  const lab = new Int32Array(nx * ny).fill(-1); const comps = [];
  for (let i = 0; i < nx * ny; i++) { if (lab[i] >= 0 || W[i] || slope(i) >= g) continue; const id = comps.length; let n = 0, sx = 0, sy = 0, near = 0, minD = 1e9; const st = [i]; lab[i] = id;
    while (st.length) { const j = st.pop(); n++; const q = j % nx, r = (j / nx) | 0; const x = ox + q * c, y = oy - r * c; sx += x; sy += y; const d = Math.hypot(x - cx, y - cy); if (d < 900) near++; minD = Math.min(minD, d);
      for (let dr = -1; dr <= 1; dr++) for (let dq = -1; dq <= 1; dq++) { const k = j + dr * nx + dq; if (k < 0 || k >= nx * ny || lab[k] >= 0 || W[k] || slope(k) >= g) continue; lab[k] = id; st.push(k); } }
    comps.push({ ha: n * c * c / 1e4, nearHa: near * c * c / 1e4, minD, cx: sx / n - cx, cy: sy / n - cy }); }
  const big = comps.filter((k) => k.minD < 900 && k.ha > 1).sort((a, b) => b.ha - a.ha).slice(0, 6);
  const tot = comps.filter((k) => k.minD < 900).reduce((a, k) => a + k.nearHa, 0);
  console.log(`slope < ${g}: flat land within 900 m of the centre ${tot.toFixed(0)} ha; biggest pieces (ha total / ha within 900 m, centroid offset m):`);
  for (const k of big) console.log(`   ${k.ha.toFixed(0).padStart(5)} / ${k.nearHa.toFixed(0).padStart(4)}   at ${k.cx.toFixed(0)}, ${k.cy.toFixed(0)}`);
}
