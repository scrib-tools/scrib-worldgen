// Shore bands: per distance from water, terrain (height above the water level, slope) and MU's use of the land.
// Usage: node shore.mjs <site dir> <run dir>...
import fs from "node:fs";
const [siteDir, ...runs] = process.argv.slice(2);
const site = JSON.parse(fs.readFileSync(`${siteDir}/site.json`));
const [nx, ny] = site.dims, c = site.cell_m, [ox, oy] = site.origin_m;
const hb = fs.readFileSync(`${siteDir}/heights.f32`); const H = new Float32Array(hb.buffer, hb.byteOffset, nx * ny);
const wat = fs.readFileSync(`${siteDir}/water.u8`);
// Distance to water (cells), two-pass chamfer.
const D = new Float32Array(nx * ny).fill(1e9);
for (let i = 0; i < nx * ny; i++) if (wat[i]) D[i] = 0;
const s2 = Math.SQRT2;
for (let r = 0; r < ny; r++) for (let q = 0; q < nx; q++) { const i = r * nx + q; let d = D[i];
  if (q) d = Math.min(d, D[i - 1] + 1); if (r) { d = Math.min(d, D[i - nx] + 1); if (q) d = Math.min(d, D[i - nx - 1] + s2); if (q < nx - 1) d = Math.min(d, D[i - nx + 1] + s2); } D[i] = d; }
for (let r = ny - 1; r >= 0; r--) for (let q = nx - 1; q >= 0; q--) { const i = r * nx + q; let d = D[i];
  if (q < nx - 1) d = Math.min(d, D[i + 1] + 1); if (r < ny - 1) { d = Math.min(d, D[i + nx] + 1); if (q < nx - 1) d = Math.min(d, D[i + nx + 1] + s2); if (q) d = Math.min(d, D[i + nx - 1] + s2); } D[i] = d; }
// Local water level: height of the nearest water cell is costly; use the min water height within the town box instead.
const cellOf = (x, y) => [Math.floor((x - ox) / c), Math.floor((oy - y) / c)];
const slope = (q, r) => { if (q < 1 || r < 1 || q >= nx - 1 || r >= ny - 1) return 0; const gx = (H[r * nx + q + 1] - H[r * nx + q - 1]) / (2 * c), gy = (H[(r + 1) * nx + q] - H[(r - 1) * nx + q]) / (2 * c); return Math.hypot(gx, gy); };
const bands = [0, 10, 20, 40, 80, 160, 320];
const band = (dm) => { for (let b = bands.length - 1; b >= 0; b--) if (dm >= bands[b]) return b; return 0; };
// Point in polygon raster of a set of polygons -> Uint8 mask with a value per class.
function raster(polys, mask, val) {
  for (const pg of polys) {
    let x0 = 1e9, x1 = -1e9, y0 = 1e9, y1 = -1e9; for (const [x, y] of pg) { x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y); }
    const [qa, rb] = cellOf(x0, y0), [qb, ra] = cellOf(x1, y1);
    for (let r = Math.max(0, ra); r <= Math.min(ny - 1, rb); r++) { const y = oy - (r + 0.5) * c;
      for (let q = Math.max(0, qa); q <= Math.min(nx - 1, qb); q++) { const x = ox + (q + 0.5) * c; let ins = false;
        for (let i = 0, j = pg.length - 1; i < pg.length; j = i++) { const [xi, yi] = pg[i], [xj, yj] = pg[j]; if ((yi > y) !== (yj > y) && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) ins = !ins; }
        if (ins) mask[r * nx + q] = val; } } }
}
for (const run of runs) {
  const lots = JSON.parse(fs.readFileSync(`${run}/lots.json`)).lots;
  const use = new Uint8Array(nx * ny), bld = new Uint8Array(nx * ny);
  const code = { plot: 1, garden: 2, reserve: 3, place: 4, court: 4 };
  for (const l of lots) raster([l.polygon], use, code[l.use] ?? 3);
  raster(lots.flatMap((l) => l.preview_parts.map((p) => p.polygon)), bld, 1);
  // Town box = lots bbox + 150 m; water level = 5th percentile of water heights inside.
  let bx0 = 1e9, bx1 = -1e9, by0 = 1e9, by1 = -1e9; for (const l of lots) for (const [x, y] of l.polygon) { bx0 = Math.min(bx0, x); bx1 = Math.max(bx1, x); by0 = Math.min(by0, y); by1 = Math.max(by1, y); }
  const [qa, rb] = cellOf(bx0 - 150, by0 - 150), [qb, ra] = cellOf(bx1 + 150, by1 + 150);
  const wh = []; for (let r = ra; r <= rb; r++) for (let q = qa; q <= qb; q++) if (wat[r * nx + q]) wh.push(H[r * nx + q]); wh.sort((a, b) => a - b);
  const wl = wh[Math.floor(wh.length * 0.5)];
  const acc = bands.map(() => ({ n: 0, steep: 0, hi: [], street: 0, plot: 0, garden: 0, reserve: 0, place: 0, bld: 0 }));
  for (let r = ra; r <= rb; r++) for (let q = qa; q <= qb; q++) { const i = r * nx + q; if (wat[i]) continue; const dm = D[i] * c; if (dm > 320) continue;
    const a = acc[band(dm)]; a.n++; if (slope(q, r) > 0.12) a.steep++; a.hi.push(H[i] - wl);
    const u = use[i]; if (u === 1) a.plot++; else if (u === 2) a.garden++; else if (u === 3) a.reserve++; else if (u === 4) a.place++; if (bld[i]) a.bld++; }
  console.log(`\n${run.split(/[\\/]/).slice(-2).join("/")}  (water level median ${wl.toFixed(1)} m, town box)`);
  console.log("band_m     land_ha  steep>12%  h_above_water p10/p50/p90   plot  garden reserve place  outside  MU_built");
  acc.forEach((a, b) => { if (!a.n) return; a.hi.sort((x, y) => x - y); const p = (k) => a.hi[Math.floor(a.hi.length * k)].toFixed(0); const pc = (v) => ((100 * v) / a.n).toFixed(0).padStart(5) + "%";
    const out = a.n - a.plot - a.garden - a.reserve - a.place;
    console.log(`${String(bands[b]).padStart(3)}-${String(bands[b + 1] ?? "").padEnd(4)} ${(a.n * c * c / 1e4).toFixed(1).padStart(8)} ${pc(a.steep)}      ${p(0.1).padStart(5)}/${p(0.5).padStart(4)}/${p(0.9).padStart(4)}      ${pc(a.plot)}${pc(a.garden)}${pc(a.reserve)}${pc(a.place)} ${pc(out)}  ${pc(a.bld)}`); });
}
