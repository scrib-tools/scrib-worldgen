// Site slope map (MU street grade classes) + height contours above the river, with MU lot outlines of a run on top.
// Usage: node slopemap.mjs <site dir> <run dir> <out.svg> [half_m]
import fs from "node:fs";
const [siteDir, run, out, halfArg] = process.argv.slice(2);
const site = JSON.parse(fs.readFileSync(`${siteDir}/site.json`));
const [nx, ny] = site.dims, c = site.cell_m, [ox, oy] = site.origin_m, [cx, cy] = site.centre_m;
const hb = fs.readFileSync(`${siteDir}/heights.f32`); const H = new Float32Array(hb.buffer, hb.byteOffset, nx * ny);
const wat = fs.readFileSync(`${siteDir}/water.u8`);
const half = Number(halfArg ?? 700), step = 3;
const x0 = cx - half, x1 = cx + half, y0 = cy - half, y1 = cy + half;
const q0 = Math.floor((x0 - ox) / c), q1 = Math.floor((x1 - ox) / c), r0 = Math.floor((oy - y1) / c), r1 = Math.floor((oy - y0) / c);
const wh = []; for (let r = r0; r <= r1; r++) for (let q = q0; q <= q1; q++) if (wat[r * nx + q]) wh.push(H[r * nx + q]); wh.sort((a, b) => a - b);
const wl = wh[Math.floor(wh.length / 2)];
let rects = "";
for (let r = r0; r < r1; r += step) for (let q = q0; q < q1; q += step) {
  const i = r * nx + q; const X = ox + q * c - x0, Y = (oy - r * c) - y0; const y = 2 * half - Y;
  let fill;
  if (wat[i]) fill = "#6fa3d6";
  else {
    const gx = (H[i + step] - H[i]) / (step * c), gy = (H[i + step * nx] - H[i]) / (step * c); const s = Math.hypot(gx, gy);
    const h = H[i] - wl; const l = Math.max(0, Math.min(1, h / 150)); // lighter = higher
    const base = s < 0.05 ? [120, 190, 110] : s < 0.12 ? [230, 205, 100] : [205, 95, 80];
    fill = `rgb(${base.map((v) => Math.round(v * (0.75 + 0.35 * l))).map((v) => Math.min(255, v)).join(",")})`;
  }
  rects += `<rect x="${X.toFixed(1)}" y="${y.toFixed(1)}" width="${(step * c + 0.1).toFixed(2)}" height="${(step * c + 0.1).toFixed(2)}" fill="${fill}"/>`;
}
// Contours every 10 m above the river (marching squares lite: mark cells crossing a level).
let dots = "";
for (let r = r0; r < r1; r += 2) for (let q = q0; q < q1; q += 2) { const a = H[r * nx + q] - wl, b = H[r * nx + q + 2] - wl, d = H[(r + 2) * nx + q] - wl;
  for (const lv of [10, 20, 30, 40, 50, 60, 80, 100, 150]) if ((a - lv) * (b - lv) < 0 || (a - lv) * (d - lv) < 0) { const X = ox + q * c - x0, y = 2 * half - ((oy - r * c) - y0); dots += `<rect x="${X.toFixed(1)}" y="${y.toFixed(1)}" width="1.6" height="1.6" fill="#3338"/>`; break; } }
const lots = JSON.parse(fs.readFileSync(`${run}/lots.json`)).lots;
const P = ([x, y]) => `${(x - x0).toFixed(1)},${(y1 - y).toFixed(1)}`;
const lotSvg = lots.map((l) => `<polygon points="${l.polygon.map(P).join(" ")}" fill="none" stroke="#111" stroke-width="0.8"/>`).join("");
const parts = lots.flatMap((l) => l.preview_parts.map((p) => `<polygon points="${p.polygon.map(P).join(" ")}" fill="#4a2410"/>`)).join("");
const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${2 * half} ${2 * half}" width="${2 * half * 1.4}" height="${2 * half * 1.4}">${rects}${dots}${lotSvg}${parts}
<g font-family="sans-serif" font-size="16"><rect x="10" y="10" width="430" height="92" fill="#fffd"/>
<rect x="20" y="20" width="14" height="14" fill="rgb(120,190,110)"/><text x="40" y="32">slope &lt; 5 %</text>
<rect x="20" y="42" width="14" height="14" fill="rgb(230,205,100)"/><text x="40" y="54">5-12 % (MU street grade limit 12 %)</text>
<rect x="20" y="64" width="14" height="14" fill="rgb(205,95,80)"/><text x="40" y="76">&gt; 12 % (stairs or unbuildable)</text>
<text x="20" y="96">dots: every 10 m above the river (river level ${wl.toFixed(0)} m); lighter = higher</text></g>
<g transform="translate(20,${2 * half - 20})"><rect width="100" height="4" fill="#111"/><text y="-6" font-size="14" font-family="sans-serif">100 m</text></g></svg>`;
fs.writeFileSync(out, svg);
console.log("river level", wl.toFixed(1));
