import numpy as np, struct, glob, os
from matplotlib.colors import rgb_to_hsv, hsv_to_rgb
from scipy.ndimage import gaussian_filter, map_coordinates

N=17
g=np.linspace(0,1,N)
R,G,B=np.meshgrid(g,g,g,indexing='ij')
GRID=np.stack([R,G,B],-1)  # [r,g,b,3]

def s2l(x): x=np.clip(x,0,1); return np.where(x<=0.04045,x/12.92,((x+0.055)/1.055)**2.4)
def l2s(x): x=np.clip(x,0,1); return np.where(x<=0.0031308,12.92*x,1.055*x**(1/2.4)-0.055)

# ---------- OKLab ----------
M1=np.array([[0.4122214708,0.5363325363,0.0514459929],[0.2119034982,0.6806995451,0.1073969566],[0.0883024619,0.2817188376,0.6299787005]])
M2=np.array([[0.2104542553,0.7936177850,-0.0040720468],[1.9779984951,-2.4285922050,0.4505937099],[0.0259040371,0.7827717662,-0.8086757660]])
def lin2oklab(rgb):
    lms=rgb@M1.T; lms=np.cbrt(np.maximum(lms,0)); return lms@M2.T
M1i=np.linalg.inv(M1); M2i=np.linalg.inv(M2)
def oklab2lin(lab):
    lms=lab@M2i.T; lms=lms**3; return lms@M1i.T
def lab2lch(lab):
    L=lab[...,0]; C=np.hypot(lab[...,1],lab[...,2]); h=np.degrees(np.arctan2(lab[...,2],lab[...,1]))%360; return L,C,h
def lch2lab(L,C,h):
    r=np.radians(h); return np.stack([L,C*np.cos(r),C*np.sin(r)],-1)

# ---------- DCP HueSat ----------
def dcp_huesat(path,illum=2):
    d=open(path,'rb').read(); bo='<' if d[:2]==b'II' else '>'
    off=struct.unpack(bo+'I',d[4:8])[0]; n=struct.unpack(bo+'H',d[off:off+2])[0]; tags={}
    for i in range(n):
        t,ty,cnt,val=struct.unpack(bo+'HHII',d[off+2+i*12:off+14+i*12]); tags[t]=(ty,cnt,val)
    dims=struct.unpack(bo+'3I',d[tags[0xC6F9][2]:tags[0xC6F9][2]+12])
    tag=0xC6FB if illum==2 and 0xC6FB in tags else 0xC6FA
    ty,cnt,val=tags[tag]; arr=np.frombuffer(d[val:val+cnt*4],dtype=np.dtype(bo+'f4')).reshape(dims[0],dims[1],dims[2],3)
    return arr[:,:,0,:].astype(np.float64)  # [hue][sat] -> (dh, ds, dv)
def film_delta(film,base,illum=2):
    f=dcp_huesat(film,illum); b=dcp_huesat(base,illum)
    dh=f[...,0]-b[...,0]; ds=f[...,1]/np.maximum(b[...,1],1e-3); dv=f[...,2]/np.maximum(b[...,2],1e-3)
    return dh,ds,dv
def apply_film(lin,delta,k=1.0):
    dh,ds,dv=delta; hsv=rgb_to_hsv(np.clip(lin,0,1))
    hi=hsv[...,0]*dh.shape[0]; si=hsv[...,1]*(dh.shape[1]-1)
    coords=[hi.ravel(),si.ravel()]
    def samp(a):
        # wrap hue axis by padding
        ap=np.concatenate([a,a[:1]],0); return map_coordinates(ap,coords,order=1,mode='nearest').reshape(hi.shape)
    H=samp(dh); S=samp(ds); V=samp(dv)
    hsv[...,0]=(hsv[...,0]+k*H/360.0)%1.0
    hsv[...,1]=np.clip(hsv[...,1]*S**k,0,1)
    hsv[...,2]=np.clip(hsv[...,2]*V**k,0,1)
    return hsv_to_rgb(hsv)

# ---------- OKLCh shaping ----------
def hue_window(h,center,width):
    d=(h-center+180)%360-180; return np.exp(-0.5*(d/width)**2)
def lum_window(L,lo,hi,soft=0.12):
    return 1/(1+np.exp(-(L-lo)/soft))*(1-1/(1+np.exp(-(L-hi)/soft)))
def shape(lab,ops):
    L,C,h=lab2lch(lab)
    a,b=lab[...,1].copy(),lab[...,2].copy()
    for op in ops:
        t=op['t']
        if t=='chroma':      C=C*op['k']
        elif t=='hue_sector':
            w=hue_window(h,op['h'],op.get('w',25))
            if 'rot' in op: h=(h+op['rot']*w)%360
            if 'k' in op:   C=C*(1+(op['k']-1)*w)
            if 'dL' in op:  L=L+op['dL']*w*np.clip(C/0.08,0,1)
        elif t=='tint':
            w=lum_window(L,op['lo'],op['hi'],op.get('soft',0.1))
            lab2=lch2lab(L,C,h); lab2[...,1]+=op['a']*w; lab2[...,2]+=op['b']*w; L,C,h=lab2lch(lab2)
        elif t=='chroma_by_L':
            w=lum_window(L,op['lo'],op['hi'],op.get('soft',0.1)); C=C*(1+(op['k']-1)*w)
        elif t=='chroma_cap':
            C=np.where(C>op['c'],op['c']+(C-op['c'])*op['k'],C)
    return lch2lab(L,C,h)

def tone(L,black=0.004,white=0.99,toe=0.0,shoulder=0.0,gamma=1.0,mid=0.0,mid_center=0.5):
    # L is OKLab lightness 0..1 (perceptual)
    t=np.clip(L,0,1)**gamma
    t=t*(1-toe*np.exp(-t/0.10))
    t=t-shoulder*np.clip(t-0.72,0,1)**2
    t=t+mid*np.exp(-0.5*((t-mid_center)/0.22)**2)
    t=np.clip(t,0,1)
    Lb=lin2oklab(s2l(np.array([black]*3)))[0]; Lw=lin2oklab(s2l(np.array([white]*3)))[0]
    return Lb+(Lw-Lb)*t

def soft_clip(lin):
    return np.clip(lin,0,1)

def compile_look(color_fn, smooth=0.6):
    lin=s2l(GRID); out=color_fn(lin); out=soft_clip(out); srgb=l2s(out)
    if smooth>0:
        sm=np.stack([gaussian_filter(srgb[...,c],smooth,mode='nearest') for c in range(3)],-1)
        # keep endpoints exact
        srgb=sm
        srgb[0,0,0]=l2s(out[0,0,0]); srgb[N-1,N-1,N-1]=l2s(out[N-1,N-1,N-1])
    return np.clip(srgb,0,1)

def write_cube(path,lut,title,lid,base=0,creator='The Infinite Arch'):
    rows=[]
    for b in range(N):
        for gg in range(N):
            for r in range(N):
                v=lut[r,gg,b]; rows.append(f"{v[0]:.6f} {v[1]:.6f} {v[2]:.6f}")
    hdr=f'#Created by: {creator}\n#Unique Leica Look ID: {lid}\n#Based Filmstyle Mode: {"Monochrome" if base else "Standard"}\n\nTITLE "{title}"\n\n#LUT size\nLUT_3D_SIZE 17\n\n#data domain\nDOMAIN_MIN 0.0 0.0 0.0\nDOMAIN_MAX 1.0 1.0 1.0\n\n#LUT data points\n'
    open(path,'w').write(hdr+'\n'.join(rows)+'\n')

def apply_lut(lut,img):
    c=np.clip(img,0,1)*(N-1); coords=[c[...,0].ravel(),c[...,1].ravel(),c[...,2].ravel()]
    return np.stack([map_coordinates(lut[...,ch],coords,order=1) for ch in range(3)],-1).reshape(img.shape)

def metrics(lut):
    neut=np.array([lut[i,i,i] for i in range(N)]); Y=neut@np.array([.2126,.7152,.0722])
    d2=max(np.abs(np.diff(lut,2,axis=ax)).max() for ax in range(3))
    return dict(black=[round(float(x),4) for x in neut[0]],white=[round(float(x),4) for x in neut[-1]],neutralY=[round(float(y),3) for y in Y[::2]],monotone=bool(np.all(np.diff(Y)>0)),max_second_difference=round(float(d2),4))
