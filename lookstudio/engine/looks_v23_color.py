"""v2.3 color: true S-curves. Target shape per Look: toe <= 0.80 x mid, shoulder <= 0.75 x mid, mid slope raised.
Reference: Leica Brass toe 0.92 / mid 1.26 / shoulder 0.61. v2.2 Threshold, Kin and Afterglow had toe >= mid (flat)."""
import numpy as np, os, json, sys
from engine import *
from looks import color_pipeline, fd, blend_delta
import looks_v2 as V2, looks_v22 as V22
from looks_v22 import mid_slope, apply_S, Y, x
# S targets: (mid slope, toe ratio, shoulder ratio)
S={'presence':(1.22,0.72,0.66),'threshold':(1.16,0.80,0.58),'ember':(1.42,0.60,0.70),'sanctuary':(1.25,0.68,0.78),'kin':(1.24,0.72,0.66),'afterglow':(1.22,0.72,0.58)}
def toe_shoulder(lut,c_toe,c_sh,black,white,pivot=0.5):
    """Pivot-power S in display luma: below the pivot t -> p*(t/p)^(1+c_toe), above 1-(1-p)*((1-t)/(1-p))^(1+c_sh).
    Slope goes to zero at both ends, endpoints pinned to the Look's own black and white; the mid gain is solved afterwards."""
    y=lut@Y; t=np.clip((y-black)/max(white-black,1e-3),0,1); p=float(np.clip(pivot,0.15,0.85))
    lo=p*np.power(np.clip(t/p,0,1),1+c_toe); hi=1-(1-p)*np.power(np.clip((1-t)/(1-p),0,1),1+c_sh)
    t2=np.where(t<p,lo,hi); g=black+(white-black)*t2
    ratio=np.where(y>1e-4,g/np.maximum(y,1e-4),1.0); return np.clip(lut*ratio[...,None],0,1)
def slopes(lut):
    n=np.array([lut[i,i,i]@Y for i in range(17)]); d=np.gradient(n,x); return float(d[1:4].mean()),float(d[6:11].mean()),float(d[13:16].mean()),n
def shape_S(lut,mid_t,toe_r,sh_r):
    n0=np.array([lut[i,i,i]@Y for i in range(17)]); black,white=float(n0[0]),float(n0[16]); center=float(n0[8]); pivot=(center-black)/max(white-black,1e-3)
    c_toe=c_sh=0.0
    for it in range(24):
        cur=toe_shoulder(lut,c_toe,c_sh,black,white,pivot)
        lo,hi=0.6,2.6
        for _ in range(30):
            k=(lo+hi)/2; s=slopes(apply_S(cur,k,center))[1]
            if s<mid_t: lo=k
            else: hi=k
        out=apply_S(cur,(lo+hi)/2,center); t,m,sh,_=slopes(out)
        if t>toe_r*m+0.02: c_toe=min(c_toe+0.05,1.5)
        elif t<toe_r*m-0.04 and c_toe>0: c_toe=max(c_toe-0.03,0)
        if sh>sh_r*m+0.02: c_sh=min(c_sh+0.05,1.5)
        elif sh<sh_r*m-0.04 and c_sh>0: c_sh=max(c_sh-0.04,0)
    return out,(c_toe,c_sh,(lo+hi)/2)
# Ember-K: a Kodachrome-leaning Ember (McCurry's film). No Kodachrome DCP exists; stand-in is Ektachrome 64 + E100VS + Ektar 25 with the Kodachrome signature applied by hand: red-orange bias, deep clean blues, restrained greens, cool neutral shadows, no magenta bloom.
def ember_K(lin): return color_pipeline(lin,blend_delta([(0.4,fd('Ektachrome 64 - L')),(0.3,fd('Kodak E100VS - L')),(0.3,fd('Ektar 25 - L'))]),1.0,ops=[
    dict(t='hue_sector',h=25,w=18,rot=-6,k=1.14,dL=-0.03), dict(t='hue_sector',h=55,w=20,k=1.06,rot=-6),
    dict(t='hue_sector',h=142,w=30,k=0.88,dL=-0.06,rot=-8), dict(t='hue_sector',h=100,w=18,k=0.92),
    dict(t='hue_sector',h=265,w=35,rot=4,k=1.08,dL=-0.05), dict(t='hue_sector',h=330,w=25,k=0.92),
    dict(t='tint',lo=-0.2,hi=0.35,a=-0.002,b=-0.010), dict(t='chroma_cap',c=0.26,k=0.7)],
    tone_kw=dict(black=0.004,white=1.0,toe=0.35,shoulder=0.04,gamma=1.14,mid=-0.02),chroma=1.10)
from engine import shape, lin2oklab, oklab2lin
def post(fn,ops):
    def g(lin):
        out=fn(lin); lab=lin2oklab(out); return np.clip(oklab2lin(shape(lab,ops)),0,1)
    return g
# v2.3 separation nudge: Threshold pinks and pale blues a touch more themselves, Kin greens a touch more 400H.
threshold_v23=post(V2.LOOKS['capelight']['fn'],[dict(t='hue_sector',h=330,w=25,k=1.10),dict(t='hue_sector',h=225,w=25,k=1.06,rot=4)])
kin_v23=post(V2.LOOKS['400h']['fn'],[dict(t='hue_sector',h=142,w=30,rot=6,k=1.05),dict(t='hue_sector',h=225,w=25,rot=-6)])
BASE={'presence':V22.presence_C,'threshold':threshold_v23,'ember':V2.LOOKS['fortia']['fn'],'sanctuary':V22.sanctuary_v22,'kin':kin_v23,'afterglow':V2.LOOKS['ultra']['fn']}
IDS={'presence':(1004,'IA Presence'),'threshold':(1005,'IA Threshold'),'ember':(1006,'IA Ember'),'sanctuary':(1007,'IA Sanctuary'),'kin':(1008,'IA Kin'),'afterglow':(1009,'IA Afterglow')}
if __name__=='__main__':
    os.makedirs('out23c',exist_ok=True); rep={}
    todo=list(BASE.items())
    todo.append(('ember_K',ember_K))
    for k,fn in todo:
        key=k.replace('_K','')
        lut=compile_look(fn,smooth=0.55); lut,par=shape_S(lut,*S[key]); t,m,sh,n=slopes(lut); mm=metrics(lut)
        lid,name=IDS[key]; np.save(f'out23c/{k}.npy',lut); write_cube(f'out23c/{k}.CUBE',lut,name,lid,0)
        rep[k]=dict(mm,slope_toe=round(t,2),slope_mid=round(m,2),slope_sh=round(sh,2),params=[round(v,3) for v in par])
        print(f"{k:10s} black {n[0]:.3f} mid {n[8]:.3f} white {n[16]:.3f} | toe {t:.2f} mid {m:.2f} sh {sh:.2f} (target {S[key]}) | 2nd {mm['max_second_difference']:.3f} mono {mm['monotone']} | c_toe,c_sh,k {rep[k]['params']}")
    json.dump(rep,open('out23c/metrics.json','w'),indent=1)
