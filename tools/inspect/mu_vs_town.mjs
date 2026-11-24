// MU plan (lots + MU's own building footprints) vs what settle placed (kit hulls). Usage: node mu_vs_town.mjs <run dir> <kit.json> <out.html>
import fs from "node:fs";
const [dir, kitPath, out] = process.argv.slice(2);
const lots = JSON.parse(fs.readFileSync(`${dir}/lots.json`)).lots;
const pl = JSON.parse(fs.readFileSync(`${dir}/placements.json`));
const kit = new Map(JSON.parse(fs.readFileSync(kitPath)).models.map((m) => [m.id, m]));
const area = (pg) => { let s = 0; for (let i = 0; i < pg.length; i++) { const [a, b] = pg[i], [c, d] = pg[(i + 1) % pg.length]; s += a * d - b * c; } return Math.abs(s) / 2; };
const hull = (m, pos, yaw) => { const r = (yaw * Math.PI) / 180, c = Math.cos(r), s = Math.sin(r); return m.footprint.map(([x, y]) => [pos[0] + c * x - s * y, pos[1] + s * x + c * y]); };

let minX = 1e9, minY = 1e9, maxX = -1e9, maxY = -1e9;
for (const l of lots) for (const [x, y] of l.polygon) { minX = Math.min(minX, x); maxX = Math.max(maxX, x); minY = Math.min(minY, y); maxY = Math.max(maxY, y); }
const pad = 20; minX -= pad; minY -= pad; maxX += pad; maxY += pad;
const W = maxX - minX, H = maxY - minY;
const pts = (pg) => pg.map(([x, y]) => `${(x - minX).toFixed(1)},${(maxY - y).toFixed(1)}`).join(" ");
const poly = (pg, cls) => `<polygon class="${cls}" points="${pts(pg)}"/>`;

const useCls = { plot: "plot", garden: "garden", reserve: "reserve", place: "place" };
const lotSvg = lots.map((l) => poly(l.polygon, useCls[l.use] ?? "reserve")).join("");
const lotSvgFaint = lots.map((l) => poly(l.polygon, (useCls[l.use] ?? "reserve") + " faint")).join("");
const stat = { muParts: 0, muBuildings: 0, muArea: 0, lotArea: {}, kit: {}, kitArea: {} };
for (const l of lots) stat.lotArea[l.use] = (stat.lotArea[l.use] ?? 0) + area(l.polygon);
const muSvg = lots.flatMap((l) => l.preview_parts.map((p) => { stat.muParts++; if (p.label === "building") stat.muBuildings++; stat.muArea += area(p.polygon); return poly(p.polygon, p.label === "building" ? "mub" : "mux"); })).join("");
const kitSvg = pl.instances.map((i) => {
  const m = kit.get(i.kit); if (!m) return "";
  const k = m.kind; stat.kit[k] = (stat.kit[k] ?? 0) + 1;
  const h = hull(m, i.pos, i.yaw_deg); stat.kitArea[k] = (stat.kitArea[k] ?? 0) + area(h);
  return ["building", "tower", "wall", "arch", "stairs", "bridge"].includes(k) ? poly(h, "k-" + k) : "";
}).join("");
const lotArea = Object.values(stat.lotArea).reduce((a, b) => a + b, 0);
const built = (stat.kitArea.building ?? 0) + (stat.kitArea.tower ?? 0);
const fmt = (v) => Math.round(v).toLocaleString("en");
const pc = (v) => (100 * v).toFixed(0) + " %";
const svg = (body) => `<svg viewBox="0 0 ${W.toFixed(0)} ${H.toFixed(0)}" xmlns="http://www.w3.org/2000/svg">${body}</svg>`;

const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Balmora 2.0: Magna Urbis plan vs placed town</title>
<style>
:root{--bg:#f7f5f0;--fg:#222;--mut:#666;--plot:#f1d7b0;--garden:#cfe3bf;--reserve:#ddd;--mub:#7a3b16;--mux:#b86a2c;--kb:#c0392b;--kt:#6c3483;--kw:#555}
@media (prefers-color-scheme:dark){:root{--bg:#1b1b1d;--fg:#eee;--mut:#aaa;--plot:#5a4630;--garden:#34482b;--reserve:#3a3a3a;--mub:#f0a060;--mux:#c9874a;--kb:#ff6b5a;--kt:#c39bd3;--kw:#bbb}}
body{background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,sans-serif;margin:24px auto;max-width:1500px;padding:0 16px}
h1{font-size:22px;margin:0 0 4px}p{max-width:900px}.mut{color:var(--mut)}
.row{display:grid;grid-template-columns:1fr 1fr;gap:16px}figure{margin:0}figcaption{font-weight:600;margin-bottom:6px}
svg{width:100%;height:auto;border:1px solid #8884;background:var(--bg)}
.plot{fill:var(--plot);stroke:#0003;stroke-width:.4}.garden{fill:var(--garden);stroke:#0003;stroke-width:.4}.reserve,.place{fill:var(--reserve);stroke:#0003;stroke-width:.4}
.faint{opacity:.45}.mub{fill:var(--mub)}.mux{fill:var(--mux)}
.k-building{fill:var(--kb)}.k-tower{fill:var(--kt)}.k-wall,.k-arch{fill:var(--kw)}.k-stairs,.k-bridge{fill:var(--kw);opacity:.6}
table{border-collapse:collapse;margin:12px 0}td,th{padding:3px 12px;text-align:right;border-bottom:1px solid #8883}td:first-child,th:first-child{text-align:left}
.key span{display:inline-block;width:12px;height:12px;margin:0 4px 0 14px;vertical-align:-1px}
</style></head><body>
<h1>Balmora 2.0: what Magna Urbis planned vs what we placed</h1>
<p class="mut">Run <code>${dir.replace(/\\/g, "/")}</code> (culture ${pl.recipe?.culture ?? "medina"}), kit <code>${kitPath.replace(/\\/g, "/")}</code>. North up, ${fmt(W)} m by ${fmt(H)} m.</p>
<div class="row">
<figure><figcaption>Magna Urbis's plan: lots and its own building footprints</figcaption>${svg(lotSvg + muSvg)}
<div class="key"><span style="background:var(--plot)"></span>plot <span style="background:var(--garden)"></span>garden <span style="background:var(--reserve)"></span>reserve/place <span style="background:var(--mub)"></span>MU building <span style="background:var(--mux)"></span>MU annex/other</div></figure>
<figure><figcaption>What settle placed: kit hulls (Hlaalu Balmora+Suran)</figcaption>${svg(lotSvgFaint + kitSvg)}
<div class="key"><span style="background:var(--kb)"></span>house <span style="background:var(--kt)"></span>tower <span style="background:var(--kw)"></span>wall/arch</div></figure>
</div>
<table><tr><th></th><th>count</th><th>area m²</th><th>share of all lots</th></tr>
<tr><td>All lots</td><td>${lots.length}</td><td>${fmt(lotArea)}</td><td>100 %</td></tr>
${Object.entries(stat.lotArea).map(([k, v]) => `<tr><td>&nbsp;&nbsp;${k}</td><td>${lots.filter((l) => l.use === k).length}</td><td>${fmt(v)}</td><td>${pc(v / lotArea)}</td></tr>`).join("")}
<tr><td>MU building footprints (all parts)</td><td>${stat.muParts} (${stat.muBuildings} "building")</td><td>${fmt(stat.muArea)}</td><td>${pc(stat.muArea / lotArea)}</td></tr>
<tr><td>Placed kit houses + towers</td><td>${(stat.kit.building ?? 0) + (stat.kit.tower ?? 0)}</td><td>${fmt(built)}</td><td>${pc(built / lotArea)}</td></tr>
<tr><td>Placed walls/arches</td><td>${(stat.kit.wall ?? 0) + (stat.kit.arch ?? 0)}</td><td>${fmt((stat.kitArea.wall ?? 0) + (stat.kitArea.arch ?? 0))}</td><td></td></tr>
</table>
<p class="mut">Placed instances by kit kind: ${Object.entries(stat.kit).map(([k, v]) => `${k} ${v}`).join(", ")}.</p>
</body></html>`;
fs.writeFileSync(out, html);
console.log(JSON.stringify({ lots: lots.length, lotArea: fmt(lotArea), byUse: Object.fromEntries(Object.entries(stat.lotArea).map(([k, v]) => [k, fmt(v)])), muParts: stat.muParts, muBuildings: stat.muBuildings, muArea: fmt(stat.muArea), kit: stat.kit, kitArea: Object.fromEntries(Object.entries(stat.kitArea).map(([k, v]) => [k, fmt(v)])) }));
