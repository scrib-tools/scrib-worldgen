const D=require("./fitdiag.js"); const {pass,R}=D;
const q=a=>{if(!a.length)return "-";a=a.slice().sort((x,y)=>x-y);return [0.5,0.9,1].map(p=>a[Math.max(1,Math.ceil(p*a.length))-1].toFixed(2)).join("/")};
const NB=[0,-0.5,-1,-1.5,-2,-3,-4,-5,-6], NB2=[0,-0.25,-0.5,-1,-1.5,-2];
const V={ base:{},
 fb_nback:{stages:[{},{nslide:NB}]},
 fb_nback2:{stages:[{},{nslide:NB2}]},
 fb_part15_nback:{stages:[{},{nslide:NB},{frame:"part",max_angle:15,nslide:NB}]},
 fb_rise2:{stages:[{},{rise:2}]},
 fb_all:{stages:[{},{nslide:NB},{frame:"part",max_angle:15,nslide:NB},{nslide:NB,rise:2},{frame:"part",max_angle:15,nslide:NB,rise:2}]},
 fb_all2:{stages:[{},{nslide:NB2},{frame:"part",max_angle:15,nslide:NB2},{nslide:NB2,rise:2},{frame:"part",max_angle:15,nslide:NB2,rise:2}]},
};
const b0=pass({});
for(const [k,o] of Object.entries(V)){ const r=pass(o); const moved=(r.dns||[]).filter(d=>d!=0); const angs=(r.angs||[]).filter(a=>a>0.5);
  console.log(k.padEnd(16),"parts",r.filled,"(+"+(r.filled-b0.filled)+")","err i/o/p",r.err.join("/"),"lots",r.lotsFilled,"(+"+(r.lotsFilled-b0.lotsFilled)+")","back n="+moved.length+" "+q(moved.map(Math.abs)),"turned n="+angs.length+" "+q(angs)); }
