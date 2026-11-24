// Copy a roads sites file with one site moved/resized: node sites_variant.mjs <in> <out> <key> <x> <y> <radius_m>
import fs from 'node:fs';
const [inp, out, key, x, y, r] = process.argv.slice(2);
const j = JSON.parse(fs.readFileSync(inp));
const s = j.sites.find((s) => s.key === key);
s.centre_m = [Number(x), Number(y)]; s.radius_m = Number(r);
fs.writeFileSync(out, JSON.stringify(j, null, 1));
console.log(JSON.stringify(s));
