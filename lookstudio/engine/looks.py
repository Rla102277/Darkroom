import numpy as np, os, json
from engine import *
ROOT=os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
M9=os.path.join(ROOT,'sources','dcp','M9','M9 Digital Camera ')
STD=os.path.join(ROOT,'sources','dcp','Adobe-Standard','M9_Digital_Camera_Adobe_Standard.dcp')
def fd(name,illum=2): return film_delta(M9+name+'.dcp',STD,illum)
def blend_delta(parts):
    dh=sum(w*d[0] for w,d in parts); ds=np.exp(sum(w*np.log(np.maximum(d[1],1e-3)) for w,d in parts)); dv=np.exp(sum(w*np.log(np.maximum(d[2],1e-3)) for w,d in parts)); return dh,ds,dv

# provisional camera pre-compensation (from the controlled shelf set): camera lifts neutrals ~+0.03 and
# compresses high-chroma response; push tone down slightly and chroma up at the top end.
PRECOMP_TONE=-0.025
def precomp(lab):
    L,C,h=lab2lch(lab); L=L+PRECOMP_TONE*np.sin(np.pi*np.clip(L,0,1)); return lch2lab(L,C,h)

def color_pipeline(lin,film=None,k=1.0,ops=(),tone_kw={},chroma=1.0,pre=True):
    x=apply_film(lin,film,k) if film is not None else lin
    lab=lin2oklab(x)
    lab=shape(lab,list(ops)+[dict(t='chroma',k=chroma)])
    L,C,h=lab2lch(lab); L=tone(L,**tone_kw); lab=lch2lab(L,C,h)
    if pre: lab=precomp(lab)
    L,C,h=lab2lch(lab); C=C*np.clip((L-0.06)/0.16,0,1); lab=lch2lab(L,C,h)
    return oklab2lin(lab)

LOOKS={}
# ---------- PRESENCE : Escura Edo 400, from the 18 scans ----------
LOOKS['presence']=dict(id=1004,name='IA Presence',fn=lambda lin: color_pipeline(lin,None,0,ops=[
    dict(t='tint',lo=-0.2,hi=0.42,a=0.010,b=0.045),        # shadows: blue crushed -> olive/amber (scans: R-B +.06, G lifted, B low)
    dict(t='tint',lo=0.35,hi=0.72,a=0.018,b=0.055),        # mids warm ochre (R-B +.14)
    dict(t='tint',lo=0.78,hi=1.3,a=-0.006,b=-0.008),       # highlights cool/clean
    dict(t='chroma_by_L',lo=0.78,hi=1.3,k=0.45),           # highlight desat (sat .11 at L.8, .03 at L.9)
    dict(t='chroma_by_L',lo=-0.3,hi=0.25,k=0.80),
    dict(t='hue_sector',h=142,w=35,rot=18,k=0.80,dL=-0.03),# greens -> cyan-green, muted, darker (forest)
    dict(t='hue_sector',h=25,w=18,rot=12,k=0.82),          # reds -> salmon/orange
    dict(t='hue_sector',h=55,w=20,k=0.92),                 # torii orange kept but not neon
    dict(t='hue_sector',h=100,w=18,k=0.85,rot=8),          # yellow-green
    dict(t='hue_sector',h=265,w=35,k=0.55,rot=-15),        # blues: rare on this film, pulled toward teal and muted
    dict(t='hue_sector',h=330,w=25,k=0.65),                # magenta suppressed
    dict(t='chroma_cap',c=0.16,k=0.45)],
    tone_kw=dict(black=0.018,white=0.985,toe=0.10,shoulder=0.22,gamma=1.06,mid=-0.02),chroma=0.92))

# ---------- THRESHOLD : blue hour / fog / transition ----------
LOOKS['threshold']=dict(id=1005,name='IA Threshold',fn=lambda lin: color_pipeline(lin,blend_delta([(0.6,fd('Fuji Superia 400 L')),(0.4,fd('Kodak Portra 800 L',1))]),0.7,ops=[
    dict(t='tint',lo=-0.2,hi=0.45,a=-0.012,b=-0.050),      # cool cyan-blue shadows
    dict(t='tint',lo=0.40,hi=0.75,a=-0.008,b=-0.022),
    dict(t='tint',lo=0.80,hi=1.3,a=0.006,b=0.018),         # practical lights / last warmth in highlights
    dict(t='hue_sector',h=265,w=40,rot=-12,k=0.85),        # blue -> cyan-blue
    dict(t='hue_sector',h=200,w=25,k=0.95),
    dict(t='hue_sector',h=55,w=25,k=1.05),                 # sodium/tungsten oranges survive
    dict(t='hue_sector',h=142,w=35,k=0.6,rot=15),          # greens drained toward cyan
    dict(t='hue_sector',h=25,w=18,k=0.75),
    dict(t='hue_sector',h=330,w=25,k=0.6),
    dict(t='chroma_cap',c=0.14,k=0.5)],
    tone_kw=dict(black=0.024,white=0.975,toe=0.02,shoulder=0.25,gamma=0.98,mid=-0.02),chroma=0.60))

# ---------- AMERICAN NEGATIVE : Aurea / Nostalgic Neg, printed color negative ----------
LOOKS['americannegative']=dict(id=1006,name='IA American Negative',fn=lambda lin: color_pipeline(lin,blend_delta([(0.55,fd('Kodak Portra 400 L')),(0.30,fd('Kodak Gold 200 - L')),(0.15,fd('Kodak Ektar 100 - L'))]),0.9,ops=[
    dict(t='tint',lo=-0.2,hi=0.40,a=0.012,b=0.020),        # rich warm-brown shadows
    dict(t='tint',lo=0.60,hi=1.3,a=0.006,b=0.028),         # amber highlights
    dict(t='hue_sector',h=142,w=35,rot=-22,k=0.78,dL=-0.02),# greens -> olive
    dict(t='hue_sector',h=265,w=35,rot=-10,k=0.72),        # blues softened toward cyan
    dict(t='hue_sector',h=45,w=25,k=1.0),                  # skin/orange preserved
    dict(t='hue_sector',h=25,w=18,k=0.9),
    dict(t='hue_sector',h=100,w=20,k=1.05,rot=-8),         # yellows toward gold
    dict(t='chroma_cap',c=0.17,k=0.55)],
    tone_kw=dict(black=0.012,white=0.965,toe=0.05,shoulder=0.30,gamma=0.98,mid=-0.01),chroma=0.85))

# ---------- KIN : 400H, airy daylight people ----------
LOOKS['400h']=dict(id=1007,name='IA Kin',fn=lambda lin: color_pipeline(lin,blend_delta([(0.7,fd('Fuji 400H L')),(0.3,fd('400H+1 - L'))]),1.0,ops=[
    dict(t='tint',lo=-0.2,hi=0.35,a=0.004,b=-0.006),
    dict(t='tint',lo=0.70,hi=1.3,a=0.008,b=0.010),
    dict(t='hue_sector',h=142,w=35,rot=20,k=0.85,dL=0.04), # greens -> mint, lighter
    dict(t='hue_sector',h=265,w=35,k=0.75,dL=0.03,rot=-10),# pastel blues, kept blue not lavender
    dict(t='hue_sector',h=345,w=18,k=1.15,dL=0.02),        # pinks lifted (narrow, keeps sky out)
    dict(t='hue_sector',h=45,w=20,rot=3,k=0.95),           # skin warm, clean
    dict(t='hue_sector',h=25,w=18,k=0.85,rot=6),
    dict(t='chroma_by_L',lo=0.8,hi=1.3,k=0.8),
    dict(t='chroma_cap',c=0.16,k=0.5)],
    tone_kw=dict(black=0.006,white=0.995,toe=0.0,shoulder=0.30,gamma=0.90,mid=0.035),chroma=0.90))

# ---------- NATURA : Superia 1600 / 800Z night ----------
LOOKS['natura']=dict(id=1008,name='IA Natura',fn=lambda lin: color_pipeline(lin,blend_delta([(0.40,fd('Fuji Superia 1600 L')),(0.25,fd('Fuji Superia 1600 L',1)),(0.20,fd('Fuji 800Z L')),(0.15,fd('Fuji Superia 800 L',1))]),1.0,ops=[
    dict(t='tint',lo=-0.2,hi=0.35,a=-0.010,b=-0.030),      # cool ambient shadows (cyan-green)
    dict(t='tint',lo=0.30,hi=0.60,a=-0.004,b=-0.010),
    dict(t='tint',lo=0.72,hi=1.3,a=0.008,b=0.022),         # warm practicals hold color in highlights
    dict(t='hue_sector',h=55,w=25,k=1.10,rot=-4,dL=0.02),  # orange/yellow signage: brighter, not just more saturated
    dict(t='hue_sector',h=100,w=18,k=1.10),
    dict(t='hue_sector',h=265,w=35,rot=-14,k=0.88),        # blue toward cyan, restrained
    dict(t='hue_sector',h=142,w=30,rot=15,k=0.9),          # greens -> cyan-green ambient
    dict(t='hue_sector',h=330,w=25,k=0.7),
    dict(t='hue_sector',h=25,w=18,k=1.05),
    dict(t='chroma_by_L',lo=0.85,hi=1.3,k=1.0),
    dict(t='chroma_cap',c=0.20,k=0.6)],
    tone_kw=dict(black=0.004,white=0.985,toe=0.30,shoulder=0.12,gamma=1.08,mid=-0.03),chroma=1.0))

# ---------- EMBER : Velvia ----------
LOOKS['ember']=dict(id=1009,name='IA Ember',fn=lambda lin: color_pipeline(lin,blend_delta([(0.7,fd('Fuji Velvia 50 - L')),(0.3,fd('Fuji Fortia SP - L'))]),1.0,ops=[
    dict(t='hue_sector',h=25,w=18,rot=-9,k=1.12,dL=-0.03), # reds -> crimson: hue and density, camera compresses chroma past ~0.7
    dict(t='hue_sector',h=265,w=35,rot=12,k=1.08,dL=-0.04),# blues lean magenta, deeper
    dict(t='hue_sector',h=142,w=30,k=1.05,dL=-0.07),       # greens dense: darker rather than more saturated
    dict(t='hue_sector',h=100,w=18,k=1.10),                # yellows clean
    dict(t='hue_sector',h=55,w=20,k=1.15),
    dict(t='tint',lo=-0.2,hi=0.35,a=0.004,b=-0.010),
    dict(t='chroma_cap',c=0.26,k=0.7)],
    tone_kw=dict(black=0.004,white=1.0,toe=0.32,shoulder=0.05,gamma=1.10,mid=-0.02),chroma=1.10))

# ---------- B&W (Standard base, mix in linear light) ----------
def bw(mix,**tk):
    def fn(lin):
        y=lin@np.array(mix); lab=lin2oklab(np.repeat(y[...,None],3,-1)); L=lab[...,0]; L=tone(L,**tk)
        L=L+PRECOMP_TONE*np.sin(np.pi*np.clip(L,0,1))
        return oklab2lin(np.stack([L,0*L,0*L],-1))
    return fn
LOOKS['invitation']=dict(id=1001,name='IA Invitation',fn=bw((0.32,0.58,0.10),black=0.004,white=0.975,toe=0.08,shoulder=0.28,gamma=0.98,mid=0.03))
LOOKS['witness']   =dict(id=1002,name='IA Witness',  fn=bw((0.48,0.42,0.10),black=0.004,white=0.985,toe=0.34,shoulder=0.10,gamma=1.12,mid=-0.05))
LOOKS['zone']      =dict(id=1003,name='IA Zone',     fn=bw((0.30,0.65,0.05),black=0.004,white=0.995,toe=0.03,shoulder=0.0,gamma=0.88,mid=0.06,mid_center=0.35))

if __name__=='__main__':
    os.makedirs('out',exist_ok=True); rep={}
    for k,L in LOOKS.items():
        lut=compile_look(L['fn'],smooth=0.55 if k not in('invitation','witness','zone') else 0.0)
        np.save(f'out/{k}.npy',lut); write_cube(f'out/{k}.CUBE',lut,L['name'],L['id'],0)
        rep[k]=metrics(lut); print(k,rep[k])
    json.dump(rep,open('out/metrics.json','w'),indent=1)
