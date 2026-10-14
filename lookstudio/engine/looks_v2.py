import numpy as np, os, json
from engine import *
from looks import color_pipeline, fd, blend_delta, bw, PRECOMP_TONE, LOOKS as V1
LOOKS={}
# keepers
LOOKS['presence']=V1['presence']
LOOKS['witness']=V1['witness']
# Kin tweak: a touch more density and contrast, pinks calmer, skin warmer, still airy
LOOKS['400h']=dict(id=1007,name='IA Kin',fn=lambda lin: color_pipeline(lin,blend_delta([(0.7,fd('Fuji 400H L')),(0.3,fd('400H+1 - L'))]),1.0,ops=[
    dict(t='tint',lo=-0.2,hi=0.35,a=0.004,b=-0.006),
    dict(t='tint',lo=0.70,hi=1.3,a=0.006,b=0.008),
    dict(t='hue_sector',h=142,w=35,rot=20,k=0.85,dL=0.03),
    dict(t='hue_sector',h=265,w=35,k=0.78,dL=0.02,rot=-10),
    dict(t='hue_sector',h=345,w=16,k=1.05,dL=0.01),
    dict(t='hue_sector',h=45,w=20,rot=4,k=1.0),
    dict(t='hue_sector',h=25,w=18,k=0.88,rot=6),
    dict(t='chroma_by_L',lo=0.8,hi=1.3,k=0.85),
    dict(t='chroma_cap',c=0.16,k=0.5)],
    tone_kw=dict(black=0.006,white=0.993,toe=0.02,shoulder=0.26,gamma=0.93,mid=0.025),chroma=0.92))
# Natura 800T and American Negative 50D from v1.6
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)),'natura_amneg_defs.py')).read())
LOOKS['natura']=dict(id=1008,name='IA Natura',fn=natura_800t)
LOOKS['americannegative']=dict(id=1006,name='IA American Negative',fn=amneg_50d)
# Zone -> Ansel: Zone System gray mix (R+40 O+25 Y+10 G-20 A-10 B-60), full-range curve
LOOKS['zone']=dict(id=1003,name='IA Zone',fn=bw((0.40,0.58,0.02),black=0.004,white=0.998,toe=0.12,shoulder=0.0,gamma=1.02,mid=0.02,mid_center=0.30))
# Cape Light (Meyerowitz): 8x10 Vericolor at dusk. Pastel, luminous, soft blacks, pinks and pale blues
LOOKS['capelight']=dict(id=1001,name='IA Cape Light',fn=lambda lin: color_pipeline(lin,blend_delta([(0.6,fd('Kodak Portra 160 VC L')),(0.2,fd('Kodak Gold 100 - L')),(0.2,fd('Kodak E100G - L'))]),0.6,ops=[
    dict(t='tint',lo=0.10,hi=0.40,a=0.004,b=-0.022),       # cool dusk shadows
    dict(t='tint',lo=0.62,hi=1.3,a=0.012,b=0.020),         # warm luminous highlights
    dict(t='hue_sector',h=330,w=28,k=1.15,dL=0.03),        # pinks lifted
    dict(t='hue_sector',h=265,w=35,k=0.80,dL=0.05,rot=-6), # pale blues, lighter
    dict(t='hue_sector',h=45,w=25,k=0.95,dL=0.02),
    dict(t='hue_sector',h=142,w=35,k=0.80,rot=-10,dL=0.02),# greens softened, warm
    dict(t='hue_sector',h=25,w=18,k=0.90),
    dict(t='chroma_by_L',lo=0.85,hi=1.3,k=0.75),
    dict(t='chroma_cap',c=0.15,k=0.5)],
    tone_kw=dict(black=0.028,white=0.995,toe=0.0,shoulder=0.18,gamma=0.90,mid=0.03),chroma=0.85))
# Fortia: Fortia SP + Velvia 50. Crimson, magenta-blue, dense greens, slide contrast
LOOKS['fortia']=dict(id=1005,name='IA Fortia',fn=lambda lin: color_pipeline(lin,blend_delta([(0.6,fd('Fuji Fortia SP - L')),(0.4,fd('Fuji Velvia 50 - L'))]),1.0,ops=[
    dict(t='hue_sector',h=25,w=18,rot=-10,k=1.12,dL=-0.04),
    dict(t='hue_sector',h=265,w=35,rot=14,k=1.10,dL=-0.05),
    dict(t='hue_sector',h=142,w=30,k=1.05,dL=-0.08),
    dict(t='hue_sector',h=100,w=18,k=1.08),
    dict(t='hue_sector',h=55,w=20,k=1.12,rot=-4),
    dict(t='hue_sector',h=330,w=25,k=1.05),
    dict(t='tint',lo=-0.2,hi=0.35,a=0.006,b=-0.012),
    dict(t='chroma_cap',c=0.26,k=0.7)],
    tone_kw=dict(black=0.004,white=1.0,toe=0.35,shoulder=0.04,gamma=1.14,mid=-0.02),chroma=1.12))
# Ultra: Agfa Ultra 100 + Ektar 100 + Portra 400 VC. Saturated NEGATIVE: warm, lifted mids, soft shoulder
# Afterglow: anchored to the photographer's own ADOX/CineStill rolls (152 scans, X-700 + A2E). Warm sunny negative, lifted blacks, cool-green shadows.
afterglow=lambda lin: color_pipeline(lin,blend_delta([(0.4,fd('Ultra 100 - L')),(0.3,fd('Kodak Ektar 100 - L')),(0.3,fd('Kodak Portra 400 VC L'))]),0.8,ops=[
    dict(t='tint',lo=0.06,hi=0.28,a=-0.010,b=-0.018), dict(t='tint',lo=0.30,hi=0.72,a=0.016,b=0.056), dict(t='tint',lo=0.82,hi=1.3,a=0.003,b=0.004),
    dict(t='hue_sector',h=25,w=18,rot=5,k=1.15), dict(t='hue_sector',h=55,w=22,k=1.12,dL=0.03), dict(t='hue_sector',h=100,w=20,k=1.12,dL=0.02),
    dict(t='hue_sector',h=142,w=32,rot=-12,k=1.08), dict(t='hue_sector',h=265,w=35,rot=-6,k=1.05,dL=0.02), dict(t='hue_sector',h=330,w=25,k=0.9),
    dict(t='chroma_by_L',lo=0.85,hi=1.3,k=0.6), dict(t='chroma_cap',c=0.24,k=0.7)],
    tone_kw=dict(black=0.050,white=0.98,toe=0.0,shoulder=0.28,gamma=0.88,mid=0.04),chroma=1.10)
LOOKS['ultra']=dict(id=1009,name='IA Afterglow',fn=afterglow)
ORDER=['capelight','witness','zone','presence','fortia','americannegative','400h','natura','ultra']
if __name__=='__main__':
    os.makedirs('out2',exist_ok=True); rep={}
    for k in ORDER:
        L=LOOKS[k]; lut=compile_look(L['fn'],smooth=0.0 if k in('witness','zone') else 0.55)
        if k in('witness','zone'): lut=np.repeat(lut.mean(-1)[...,None],3,-1)
        np.save(f'out2/{k}.npy',lut); write_cube(f'out2/{k}.CUBE',lut,L['name'],L['id'],0); rep[k]=metrics(lut); print(k,L['id'],rep[k]['black'],rep[k]['white'],rep[k]['max_second_difference'])
    json.dump(rep,open('out2/metrics.json','w'),indent=1)
