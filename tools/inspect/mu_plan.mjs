// Magna Urbis's own plan for one culture run: streets, blocks, lots, MU's building footprints, walls, water.
// Usage: node mu_plan.mjs <run dir (layout.json, lots.json)> <site dir (site.json, water.u8)> <out.html> [out.svg]
import fs from "node:fs";
const [dir, siteDir, out, outSvg] = process.argv.slice(2);
const doc = JSON.parse(fs.readFileSync(`${dir}/layout.json`));
const L = doc.layout, fr = doc.frame;
const lots = JSON.parse(fs.readFileSync(`${dir}/lots.json`)).lots;
const site = JSON.parse(fs.readFileSync(`${siteDir}/site.json`));
const water = fs.readFileSync(`${siteDir}/water.u8`);
const toW = (p) => [fr.west_m + p.x, fr.north_m - p.y];

// View: the town (lots) plus a margin.
let x0 = 1e9, y0 = 1e9, x1 = -1e9, y1 = -1e9;
for (const l of lots) for (const [x, y] of l.polygon) { x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y); }
const m = 120; x0 -= m; y0 -= m; x1 += m; y1 += m;
const W = x1 - x0, H = y1 - y0;
const P = ([x, y]) => `${(x - x0).toFixed(1)},${(y1 - y).toFixed(1)}`;
const pts = (pg) => pg.map(P).join(" ");
const area = (pg) => { let s = 0; for (let i = 0; i < pg.length; i++) { const [a, b] = pg[i], [c, d] = pg[(i + 1) % pg.length]; s += a * d - b * c; } return Math.abs(s) / 2; };

// Water: run-length rects per raster row inside the view (row 0 = north edge at origin_m[1]).
const [nx, ny] = site.dims, c = site.cell_m, [ox, oy] = site.origin_m;
let wsvg = "";
for (let r = 0; r < ny; r++) {
  const y = oy - r * c; if (y < y0 - c || y > y1 + c) continue;
  let s = -1;
  for (let q = 0; q <= nx; q++) {
    const wet = q < nx && water[r * nx + q] > 0;
    if (wet && s < 0) s = q;
    if (!wet && s >= 0) { const xa = ox + s * c, xb = ox + q * c; if (xb > x0 && xa < x1) wsvg += `<rect x="${(xa - x0).toFixed(1)}" y="${(y1 - y).toFixed(1)}" width="${(xb - xa).toFixed(1)}" height="${(c + 0.05).toFixed(2)}"/>`; s = -1; }
  }
}

const lotCls = { plot: "plot", garden: "garden", reserve: "reserve", place: "place", court: "place" };
const lotSvg = lots.map((l) => `<polygon class="${lotCls[l.use] ?? "reserve"}" points="${pts(l.polygon)}"/>`).join("");
const civic = /prayer|minaret|khan|hammam|tomb|mausoleum|tavern|kiosk|keep|tower|wall|mill|wheel|smithy|granary|stable|shop/;
const parts = { building: 0, annex: 0, civic: 0 }, partArea = { building: 0, annex: 0, civic: 0 };
const partSvg = lots.flatMap((l) => l.preview_parts.map((p) => {
  const k = p.label === "building" ? "building" : civic.test(p.label) ? "civic" : "annex";
  parts[k]++; partArea[k] += area(p.polygon);
  return `<polygon class="mu-${k}" points="${pts(p.polygon)}"><title>${p.label} · ${l.role}</title></polygon>`;
})).join("");
const roleCls = (r) => ({ bridge: "s-main", causeway: "s-main", approach: "s-main", "gate-street": "s-main", street: "s-street", lane: "s-lane", "wall-lane": "s-lane", close: "s-close", stairs: "s-stairs" }[r] ?? "s-lane");
const streetSvg = L.streets.map((s) => {
  const w = s.widths.reduce((a, b) => a + b, 0) / Math.max(1, s.widths.length);
  return `<polyline class="${roleCls(s.role)}" stroke-width="${w.toFixed(1)}" points="${s.path.map((p) => P(toW(p))).join(" ")}"><title>${s.role} · ${w.toFixed(1)} m</title></polyline>`;
}).join("");
const wallSvg = (L.enclosures ?? []).map((e) => `<polyline class="wall" points="${e.path.map((p) => P(toW(p))).join(" ")}"/>`).join("");

const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W.toFixed(0)} ${H.toFixed(0)}" width="${(W * 1.4).toFixed(0)}" height="${(H * 1.4).toFixed(0)}">
<style>
.bg{fill:#e9e4d6}.water rect{fill:#7aaad6}
.plot{fill:#f0d3ab;stroke:#a88a64;stroke-width:.35}.garden{fill:#c9dfb3;stroke:#8aa874;stroke-width:.35}
.reserve{fill:#d8d6d0;stroke:#999;stroke-width:.35}.place{fill:#efe9dc;stroke:#aaa;stroke-width:.35}
.mu-building{fill:#7a3b16}.mu-annex{fill:#b8763c}.mu-civic{fill:#5b3a8c}
polyline{fill:none;stroke-linecap:round;stroke-linejoin:round}
.s-main{stroke:#6d6252}.s-street{stroke:#8a7e6c}.s-lane{stroke:#a39885}.s-close{stroke:#b9ae9a}.s-stairs{stroke:#9c8f7a;stroke-dasharray:1.5 1}
.wall{stroke:#3a3a3a;stroke-width:2.5}
</style>
<rect class="bg" width="100%" height="100%"/><g class="water">${wsvg}</g>${lotSvg}${streetSvg}${partSvg}${wallSvg}
<g transform="translate(20,${(H - 20).toFixed(0)})"><rect width="100" height="4" fill="#222"/><text y="-6" font-size="14" font-family="sans-serif">100 m</text></g>
</svg>`;
if (outSvg) fs.writeFileSync(outSvg, svg);

const lotArea = lots.reduce((s, l) => s + area(l.polygon), 0);
const byUse = {}; for (const l of lots) byUse[l.use] = (byUse[l.use] ?? 0) + area(l.polygon);
const fmt = (v) => Math.round(v).toLocaleString("en");
const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Balmora 2.0: Magna Urbis's ${doc.culture} plan</title>
<style>body{margin:0;background:#f7f5f0;color:#222;font:14px/1.5 system-ui,sans-serif}header{padding:12px 20px}h1{font-size:20px;margin:0}
.mut{color:#666}.key span{display:inline-block;width:12px;height:12px;margin:0 4px 0 14px;vertical-align:-1px}
.map{overflow:auto;border-top:1px solid #ccc;height:calc(100vh - 120px);cursor:grab}.map svg{display:block}</style></head><body>
<header><h1>Magna Urbis's plan, culture ${doc.culture}: exactly what it generated</h1>
<div class="mut">Seed ${doc.seed}, population ${doc.population}, site radius ${Math.round(doc.site_radius_m)} m. MU rev ${doc.magna_urbis_rev.slice(0, 8)}. North up. ${L.streets.length} streets, ${L.blocks.length} blocks, ${lots.length} lots (plot ${fmt(byUse.plot ?? 0)} m², garden ${fmt(byUse.garden ?? 0)} m²), MU footprints: ${parts.building} houses (${fmt(partArea.building)} m²), ${parts.annex} annexes (${fmt(partArea.annex)} m²), ${parts.civic} civic/craft parts (${fmt(partArea.civic)} m²). Hover a shape for its label. Scroll to pan.</div>
<div class="key"><span style="background:#f0d3ab"></span>plot <span style="background:#c9dfb3"></span>garden <span style="background:#d8d6d0"></span>reserve <span style="background:#efe9dc"></span>place/court <span style="background:#7a3b16"></span>MU house <span style="background:#b8763c"></span>MU annex <span style="background:#5b3a8c"></span>MU civic/craft <span style="background:#6d6252"></span>streets (drawn at their width) <span style="background:#3a3a3a"></span>town wall <span style="background:#7aaad6"></span>water</div></header>
<div class="map">${svg}</div></body></html>`;
fs.writeFileSync(out, html);
console.log(JSON.stringify({ view_m: [Math.round(W), Math.round(H)], lots: lots.length, lotArea: fmt(lotArea), parts, partArea: Object.fromEntries(Object.entries(partArea).map(([k, v]) => [k, fmt(v)])) }));
