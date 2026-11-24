const F=require("./fails.json"); const filled=new Set(require("./filledlots.json"));
const bucket=f=>{ if(f.reason!="inside") return f.reason;
  if(f.cls===undefined) return "1 kit_bigger_than_lot";
  if(!f.free){ if(f.wmax<8.0||f.dmax<7.8) return "2 no_rect_for_any_front"; return f.free90?"3a shape_fits_only_turned":"3b shape_no_model"; }
  return "4 anchor_"+f.cls; };
const c={},ex={},rec={}; for(const f of F){const b=bucket(f); f.b=b; c[b]=(c[b]||0)+1; (ex[b]||=[]).length<3&&ex[b].push(f.lot.slice(7,15)+"#"+f.pi); for(const k of ["nu","turn90","n_back"]) if(f.var[k]) rec[b+"|"+k]=(rec[b+"|"+k]||0)+1;}
console.log(c); console.log(ex); console.log(rec);
// lot level: empty lots, best bucket of their parts
const by={}; for(const f of F){ if(filled.has(f.full)) continue; (by[f.full]||=[]).push(f); }
const order=["4 anchor_street","4 anchor_side","4 anchor_back","4 anchor_notch","4 anchor_null","pad","overlap","3a shape_fits_only_turned","3b shape_no_model","2 no_rect_for_any_front","1 kit_bigger_than_lot"];
const lc={}; const lotRec={}; for(const [l,fs] of Object.entries(by)){ const b=fs.map(f=>f.b).sort((x,y)=>order.indexOf(x)-order.indexOf(y))[0]; lc[b]=(lc[b]||0)+1; for(const k of ["nu","turn90"]) if(fs.some(f=>f.var[k])) lotRec[k]=(lotRec[k]||0)+1; }
console.log("empty lots",Object.keys(by).length, lc, lotRec);
const q=(a)=>{a=a.filter(x=>x!=null).sort((x,y)=>x-y);return a.length?[0.1,0.5,0.9].map(p=>a[Math.max(1,Math.ceil(p*a.length))-1].toFixed(2)):[]};
for(const b of Object.keys(c)){const fs=F.filter(f=>f.b==b); console.log(b.padEnd(28),"exc",q(fs.map(f=>f.exc)).join("/"),"ang",q(fs.map(f=>f.angle)).join("/"),"gap",q(fs.map(f=>f.frontGap)).join("/"),"lotW",q(fs.map(f=>f.lotW)).join("/"),"wmax",q(fs.map(f=>f.wmax)).join("/"),"dmax",q(fs.map(f=>f.dmax)).join("/"),"partWxD",q(fs.map(f=>f.partW)).join("/"),q(fs.map(f=>f.partD)).join("/"));}
