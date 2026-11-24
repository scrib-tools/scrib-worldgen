const j=JSON.parse(require("fs").readFileSync(process.argv[2],"utf8"));
const W=7.8,L=10.2;let n=0,ok=0;const shorts=[],areas=[];const byRole={};
function obb(p){let best=null;for(let i=0;i<p.length;i++){const a=p[i],b=p[(i+1)%p.length];let dx=b[0]-a[0],dy=b[1]-a[1];const len=Math.hypot(dx,dy);if(len<1e-6)continue;dx/=len;dy/=len;let u0=1e18,u1=-1e18,v0=1e18,v1=-1e18;for(const q of p){const u=q[0]*dx+q[1]*dy,v=-q[0]*dy+q[1]*dx;u0=Math.min(u0,u);u1=Math.max(u1,u);v0=Math.min(v0,v);v1=Math.max(v1,v);}const w=u1-u0,h=v1-v0;if(!best||w*h<best[0]*best[1])best=[w,h];}return best;}
function area(p){let s=0;for(let i=0;i<p.length;i++){const a=p[i],b=p[(i+1)%p.length];s+=a[0]*b[1]-b[0]*a[1];}return Math.abs(s)/2;}
for(const l of j.lots)for(const pp of (l.preview_parts||[])){const b=obb(pp.polygon);if(!b)continue;n++;const s=Math.min(...b),g=Math.max(...b);shorts.push(s);areas.push(area(pp.polygon));const r=(l.role||"?").split(".").slice(0,2).join(".");byRole[r]=byRole[r]||[0,0];byRole[r][0]++;if(s>=W&&g>=L){ok++;byRole[r][1]++;}}
shorts.sort((a,b)=>a-b);areas.sort((a,b)=>a-b);const q=(a,f)=>a[Math.floor(f*(a.length-1))].toFixed(1);
console.log(`footprints ${n}, hold ${W}x${L}: ${ok} (${(100*ok/n).toFixed(0)}%); short p10 ${q(shorts,.1)} p50 ${q(shorts,.5)} p90 ${q(shorts,.9)}; area p10 ${q(areas,.1)} p50 ${q(areas,.5)} p90 ${q(areas,.9)}; lots ${j.lots.length}, with part ${j.lots.filter(l=>(l.preview_parts||[]).length).length}`);
if(process.argv[3])console.log(JSON.stringify(byRole));
