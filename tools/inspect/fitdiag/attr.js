const D=require("./fitdiag.js"); const {pass}=D; const F=require("./fails.json");
const bucket=f=>{ if(f.reason!="inside") return f.reason; if(f.cls===undefined) return "1 kit_bigger_than_lot"; if(!f.free){ if(f.wmax<8.0||f.dmax<7.8) return "2 no_rect_for_any_front"; return f.free90?"3a shape_fits_only_turned":"3b shape_no_model"; } return "4 anchor_"+f.cls; };
const NB=[0,-0.5,-1,-1.5,-2,-3,-4,-5,-6];
const V={nback:{stages:[{},{nslide:NB}]}, part15:{stages:[{},{nslide:NB},{frame:"part",max_angle:15,nslide:NB}]}, rise2:{stages:[{},{rise:2}]}, all:{stages:[{},{nslide:NB},{frame:"part",max_angle:15,nslide:NB},{nslide:NB,rise:2},{frame:"part",max_angle:15,nslide:NB,rise:2}]}, turn90_ub:{doors:"all",turn90:true}};
const res={}; for(const [k,o] of Object.entries(V)) res[k]=pass(o).fp;
const tab={}; for(const f of F){const b=bucket(f); tab[b]||={n:0}; tab[b].n++; for(const k in V) if(res[k].has(f.full+"#"+f.pi)) tab[b][k]=(tab[b][k]||0)+1;}
for(const [b,v] of Object.entries(tab).sort()) console.log(b.padEnd(28), JSON.stringify(v));
// filled before but lost in "all"
const b0=pass({}).fp; for(const k in V) console.log(k,"lost",[...b0].filter(x=>!res[k].has(x)).length);
