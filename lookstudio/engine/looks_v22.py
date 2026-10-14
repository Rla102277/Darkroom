"""Infinite Arch Leica Looks v2.2 — the v2.1 nine plus: mid-slope S-curve pass (phase 1), Presence C (Lomography daylight anchor), Sanctuary re-separated from Presence C.
Every Look: same black, white, hue and chroma as v2.1; a per-Look midtone slope target; toe and shoulder relaxed so endpoints hold."""
import numpy as np, os, json
from engine import *
from looks import color_pipeline, fd, blend_delta, bw, PRECOMP_TONE
import looks_v2 as V2

def midgain(L,k,center=0.5,width=0.28):
    t=np.clip(L,0,1); return np.clip(t+(k-1)*(t-center)*np.exp(-((t-center)/width)**2)*0.9,0,1)
def with_slope(fn,k,center=0.5,width=0.28): return fn

# Presence C (see research/Escura-Edo-400-Presence: Lomography daylight frames hold a cyan-leaning blue)
def presence_C(lin): return color_pipeline(lin,None,0,ops=[
    dict(t='tint',lo=-0.2,hi=0.42,a=0.010,b=0.045), dict(t='tint',lo=0.35,hi=0.70,a=0.012,b=0.030), dict(t='tint',lo=0.78,hi=1.3,a=0.004,b=0.012),
    dict(t='chroma_by_L',lo=0.78,hi=1.3,k=0.72), dict(t='chroma_by_L',lo=-0.3,hi=0.25,k=0.80),
    dict(t='hue_sector',h=142,w=35,rot=18,k=0.80,dL=-0.03), dict(t='hue_sector',h=25,w=18,rot=12,k=0.82), dict(t='hue_sector',h=55,w=20,k=0.92), dict(t='hue_sector',h=100,w=18,k=0.85,rot=8),
    dict(t='hue_sector',h=225,w=30,k=0.95,rot=-8), dict(t='hue_sector',h=275,w=22,k=0.70,rot=-14), dict(t='hue_sector',h=330,w=25,k=0.65), dict(t='chroma_cap',c=0.17,k=0.45)],
    tone_kw=dict(black=0.018,white=0.965,toe=0.10,shoulder=0.40,gamma=1.04,mid=-0.03),chroma=0.96)


# Sanctuary v2.2 (Marix 800T on the Natura Classica): greener mids, cooler bleaching whites, greens held toward cyan instead of yellow.
# Restores the 0.15 separation floor against Presence C (v2.2 draft measured 0.105).
def sanctuary_v22(lin): return color_pipeline(lin,blend_delta([(0.5,fd('Fuji Superia 1600 L',1)),(0.3,fd('Fuji 800Z L',1)),(0.2,fd('Portra 100T - L',1))]),0.6,ops=[
    dict(t='tint',lo=0.08,hi=0.30,a=0.012,b=0.028), dict(t='chroma_by_L',lo=-0.3,hi=0.25,k=1.25),
    dict(t='tint',lo=0.38,hi=0.78,a=-0.046,b=0.020), dict(t='tint',lo=0.80,hi=1.3,a=-0.020,b=0.008),
    dict(t='chroma_by_L',lo=0.85,hi=1.3,k=0.55), dict(t='hue_sector',h=25,w=18,k=1.15), dict(t='hue_sector',h=55,w=22,k=1.10,dL=0.02),
    dict(t='hue_sector',h=100,w=18,k=1.05), dict(t='hue_sector',h=142,w=35,rot=-6,k=1.10), dict(t='hue_sector',h=200,w=25,k=1.0),
    dict(t='hue_sector',h=265,w=35,rot=-16,k=0.85), dict(t='hue_sector',h=330,w=25,k=0.6), dict(t='chroma_cap',c=0.22,k=0.6)],
    tone_kw=dict(black=0.020,white=1.0,toe=0.22,shoulder=0.0,gamma=1.12,mid=-0.04),chroma=1.0)

# B&W with slope: rebuild via bw() then slope
def bw_slope(mix,k,center=0.5,**tk): return bw(mix,**tk)

SLOPE={'invitation':1.05,'witness':1.25,'vault':1.30,'presence':1.12,'threshold':1.05,'ember':1.35,'sanctuary':1.15,'kin':1.08,'afterglow':1.10}
LOOKS={
 'invitation':dict(id=1001,name='IA Invitation',mono=True, fn=bw_slope((0.32,0.58,0.10),SLOPE['invitation'],black=0.10,white=0.985,toe=0.0,shoulder=0.15,gamma=0.95,mid=0.02)),
 'witness':   dict(id=1002,name='IA Witness',   mono=True, fn=bw_slope((0.48,0.42,0.10),SLOPE['witness'],  black=0.004,white=0.985,toe=0.34,shoulder=0.10,gamma=1.12,mid=-0.05)),
 'vault':     dict(id=1003,name='IA Vault',     mono=True, fn=bw_slope((0.40,0.58,0.02),SLOPE['vault'],center=0.45,black=0.004,white=0.998,toe=0.12,shoulder=0.0,gamma=1.02,mid=0.02,mid_center=0.30)),
 'presence':  dict(id=1004,name='IA Presence',  mono=False,fn=with_slope(presence_C,SLOPE['presence'])),
 'threshold': dict(id=1005,name='IA Threshold', mono=False,fn=with_slope(V2.LOOKS['capelight']['fn'],SLOPE['threshold'])),
 'ember':     dict(id=1006,name='IA Ember',     mono=False,fn=with_slope(V2.LOOKS['fortia']['fn'],SLOPE['ember'])),
 'sanctuary': dict(id=1007,name='IA Sanctuary', mono=False,fn=with_slope(sanctuary_v22,SLOPE['sanctuary'],center=0.45)),
 'kin':       dict(id=1008,name='IA Kin',       mono=False,fn=with_slope(V2.LOOKS['400h']['fn'],SLOPE['kin'])),
 'afterglow': dict(id=1009,name='IA Afterglow', mono=False,fn=with_slope(V2.LOOKS['ultra']['fn'],SLOPE['afterglow'])),
}
ORDER=list(LOOKS)
Y=np.array([.2126,.7152,.0722]); x=np.linspace(0,1,17)
def mid_slope(lut):
    n=np.array([lut[i,i,i]@Y for i in range(17)]); d=np.gradient(n,x); return float(d[6:11].mean()), n
def apply_S(lut,k,center,width=0.30):
    """Luma S in display space: pivot at the Look's own midpoint, endpoints held, chroma ratios preserved."""
    y=lut@Y; win=np.clip(4*y*(1-y),0,1)**0.7; g=y+(k-1)*(y-center)*np.exp(-((y-center)/width)**2)*win
    g=np.clip(g,0,1); ratio=np.where(y>1e-4,g/np.maximum(y,1e-4),1.0)
    out=np.clip(lut*ratio[...,None],0,1)
    return out
def solve_slope(lut,target,center):
    lo,hi=0.6,2.2
    for _ in range(30):
        k=(lo+hi)/2; s,_=mid_slope(apply_S(lut,k,center))
        if s<target: lo=k
        else: hi=k
    return apply_S(lut,(lo+hi)/2,center),(lo+hi)/2
if __name__=='__main__':
    os.makedirs('out22',exist_ok=True); rep={}
    for k in ORDER:
        L=LOOKS[k]; lut=compile_look(L['fn'],smooth=0.0 if L['mono'] else 0.55)
        s0,n0=mid_slope(lut); lut,kk=solve_slope(lut,SLOPE[k],float(n0[8]))
        if L['mono']: lut=np.repeat(lut.mean(-1)[...,None],3,-1)
        np.save(f'out22/{k}.npy',lut); write_cube(f'out22/{k}.CUBE',lut,L['name'],L['id'],0)
        n=np.array([lut[i,i,i]@Y for i in range(17)]); d=np.gradient(n,x); m=metrics(lut)
        rep[k]=dict(m,v21_mid_slope=round(s0,2),S_gain=round(kk,3),slope_toe=round(float(d[1:4].mean()),2),slope_mid=round(float(d[6:11].mean()),2),slope_shoulder=round(float(d[13:16].mean()),2),target_mid=SLOPE[k])
        print(f"{L['name']:16s} black {n[0]:.3f} mid {n[8]:.3f} white {n[16]:.3f} | toe {rep[k]['slope_toe']:.2f} mid {rep[k]['slope_mid']:.2f} (target {SLOPE[k]}) shoulder {rep[k]['slope_shoulder']:.2f} | 2nd {m['max_second_difference']:.3f} mono {m['monotone']}")
    json.dump(rep,open('out22/metrics.json','w'),indent=1)
