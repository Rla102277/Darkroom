
natura_800t=lambda lin: color_pipeline(lin,blend_delta([(0.5,fd('Fuji Superia 1600 L',1)),(0.3,fd('Fuji 800Z L',1)),(0.2,fd('Portra 100T - L',1))]),0.6,ops=[
    dict(t='tint',lo=0.08,hi=0.30,a=0.012,b=0.028), dict(t='chroma_by_L',lo=-0.3,hi=0.25,k=1.15),
    dict(t='tint',lo=0.38,hi=0.78,a=-0.028,b=0.024), dict(t='tint',lo=0.80,hi=1.3,a=-0.010,b=0.018),
    dict(t='chroma_by_L',lo=0.85,hi=1.3,k=0.55), dict(t='hue_sector',h=25,w=18,k=1.15), dict(t='hue_sector',h=55,w=22,k=1.10,dL=0.02),
    dict(t='hue_sector',h=100,w=18,k=1.05), dict(t='hue_sector',h=142,w=35,rot=12,k=0.95), dict(t='hue_sector',h=200,w=25,k=1.0),
    dict(t='hue_sector',h=265,w=35,rot=-16,k=0.85), dict(t='hue_sector',h=330,w=25,k=0.6), dict(t='chroma_cap',c=0.22,k=0.6)],
    tone_kw=dict(black=0.020,white=1.0,toe=0.22,shoulder=0.0,gamma=1.12,mid=-0.04),chroma=1.0)
amneg_50d=lambda lin: color_pipeline(lin,blend_delta([(0.5,fd('Kodak Portra 160 L')),(0.3,fd('Kodak Ektar 100 - L')),(0.2,fd('Kodak Gold 100 - L'))]),0.7,ops=[
    dict(t='tint',lo=0.08,hi=0.28,a=0.014,b=0.030), dict(t='chroma_by_L',lo=-0.3,hi=0.25,k=1.10),
    dict(t='tint',lo=0.25,hi=0.55,a=0.016,b=0.060), dict(t='tint',lo=0.62,hi=0.90,a=-0.014,b=-0.012),
    dict(t='chroma_by_L',lo=0.85,hi=1.3,k=0.5), dict(t='hue_sector',h=142,w=35,rot=-28,k=1.05,dL=0.0), dict(t='hue_sector',h=100,w=20,k=1.10),
    dict(t='hue_sector',h=265,w=35,rot=-22,k=0.85,dL=0.03), dict(t='hue_sector',h=200,w=25,k=0.95), dict(t='hue_sector',h=25,w=18,k=1.15),
    dict(t='hue_sector',h=55,w=22,k=1.05), dict(t='hue_sector',h=330,w=25,k=0.7), dict(t='chroma_cap',c=0.22,k=0.6)],
    tone_kw=dict(black=0.012,white=0.99,toe=0.10,shoulder=0.12,gamma=1.0,mid=0.0),chroma=1.0)
