"""v2.3 B&W candidates: toned (Leica's own mono Looks are all toned, see research/Official-Leica-Looks) and Vault rebuilt as a Zone System Look.
Tone magnitudes are calibrated against the captured Leica cubes: Selenium b −0.019 in the mids, Blue b −0.015 to −0.019, Greg Williams b +0.004."""
import numpy as np, os, json, sys
from engine import *
from looks import PRECOMP_TONE
from looks_v22 import mid_slope, solve_slope, Y, x
def bw_toned(mix,tints=(),**tk):
    """mix: linear RGB weights. tints: list of (lo,hi,a,b) OKLab tint by output lightness window."""
    def fn(lin):
        y=lin@np.array(mix); lab=lin2oklab(np.repeat(y[...,None],3,-1)); L=lab[...,0]; L=tone(L,**tk)
        L=L+PRECOMP_TONE*np.sin(np.pi*np.clip(L,0,1))
        a=0*L; b=0*L
        for lo,hi,ta,tb in tints:
            w=lum_window(L,lo,hi,0.12); a=a+ta*w; b=b+tb*w
        return oklab2lin(np.stack([L,a,b],-1))
    return fn
CANDS={
 # Invitation: soft silver scale from the Acros/Ilford rolls. B = warm silver (paper-warm mids, neutral whites).
 'invitation_A':dict(name='IA Invitation',id=1001,slope=1.05,fn=bw_toned((0.32,0.58,0.10),black=0.10,white=0.985,toe=0.0,shoulder=0.15,gamma=0.95,mid=0.02)),
 'invitation_B':dict(name='IA Invitation',id=1001,slope=1.05,fn=bw_toned((0.32,0.58,0.10),tints=[(-0.2,0.35,0.003,0.007),(0.30,0.75,0.005,0.013),(0.72,0.95,0.002,0.005)],black=0.10,white=0.985,toe=0.0,shoulder=0.15,gamma=0.95,mid=0.02)),
 # Witness: red-filter density, hard toe. B = faint cool in shadows and mids (cold-tone paper), neutral whites.
 'witness_A':   dict(name='IA Witness',id=1002,slope=1.25,fn=bw_toned((0.48,0.42,0.10),black=0.004,white=0.985,toe=0.34,shoulder=0.10,gamma=1.12,mid=-0.05)),
 'witness_B':   dict(name='IA Witness',id=1002,slope=1.25,fn=bw_toned((0.48,0.42,0.10),tints=[(-0.2,0.30,-0.003,-0.011),(0.28,0.65,-0.003,-0.008)],black=0.004,white=0.985,toe=0.34,shoulder=0.10,gamma=1.12,mid=-0.05)),
 # Vault: the Ansel / Zone System Look. Full scale Zone 0 to X, texture held in II-III and VII-VIII, sky darkened by an orange-red filter mix,
 # selenium-toned print (Ansel toned in selenium: cool violet-black in the shadows and lower mids, neutral highlights, deeper Dmax).
 'vault_A':     dict(name='IA Vault',id=1003,slope=1.30,fn=bw_toned((0.40,0.58,0.02),black=0.004,white=0.998,toe=0.12,shoulder=0.0,gamma=1.02,mid=0.02,mid_center=0.30)),
 'vault_B':     dict(name='IA Vault',id=1003,slope=1.30,fn=bw_toned((0.52,0.46,0.02),tints=[(-0.2,0.28,0.006,-0.018),(0.25,0.55,0.004,-0.011),(0.55,0.80,0.001,-0.003)],black=0.003,white=0.998,toe=0.10,shoulder=0.04,gamma=1.00,mid=0.03,mid_center=0.32)),
 'vault_C':     dict(name='IA Vault',id=1003,slope=1.35,fn=bw_toned((0.62,0.36,0.02),tints=[(-0.2,0.28,0.006,-0.018),(0.25,0.55,0.004,-0.011),(0.55,0.80,0.001,-0.003)],black=0.003,white=0.998,toe=0.10,shoulder=0.04,gamma=1.00,mid=0.03,mid_center=0.32)),
}
ZONES=np.array([0.0,0.02,0.05,0.10,0.18,0.30,0.45,0.62,0.80,0.92,1.0])  # input sRGB gray placed at Zone 0..X (roughly one stop apart in linear light)
def zone_report(lut):
    g=np.linspace(0,1,17); out=[]
    for z in ZONES:
        i=z*16; i0=int(np.floor(i)); i1=min(i0+1,16); f=i-i0
        v=(1-f)*lut[i0,i0,i0]+f*lut[i1,i1,i1]; out.append(float(v@Y))
    return out
if __name__=='__main__':
    os.makedirs('out23bw',exist_ok=True); rep={}
    for k,L in CANDS.items():
        lut=compile_look(L['fn'],smooth=0.0); s0,n0=mid_slope(lut); lut,kk=solve_slope(lut,L['slope'],float(n0[8]))
        np.save(f'out23bw/{k}.npy',lut); write_cube(f'out23bw/{k}.CUBE',lut,L['name'],L['id'],0)
        m=metrics(lut); n=np.array([lut[i,i,i]@Y for i in range(17)]); d=np.gradient(n,x)
        lab=lin2oklab(s2l(lut[8,8,8][None]))[0]; labs=lin2oklab(s2l(lut[3,3,3][None]))[0]; labh=lin2oklab(s2l(lut[14,14,14][None]))[0]
        z=zone_report(lut)
        rep[k]=dict(m,zones=[round(v,3) for v in z],slope_toe=round(float(d[1:4].mean()),2),slope_mid=round(float(d[6:11].mean()),2),slope_sh=round(float(d[13:16].mean()),2),tint_sh=[round(float(v),3) for v in labs[1:]],tint_mid=[round(float(v),3) for v in lab[1:]],tint_hi=[round(float(v),3) for v in labh[1:]])
        print(f"{k:13s} black {n[0]:.3f} mid {n[8]:.3f} white {n[16]:.3f} | toe {d[1:4].mean():.2f} mid {d[6:11].mean():.2f} sh {d[13:16].mean():.2f} | tint sh{rep[k]['tint_sh']} mid{rep[k]['tint_mid']} hi{rep[k]['tint_hi']} | zones {[round(v,2) for v in z]}")
    json.dump(rep,open('out23bw/metrics.json','w'),indent=1)
