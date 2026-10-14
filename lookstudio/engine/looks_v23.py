"""Infinite Arch Leica Looks v2.3 — definition of truth.
Color: looks_v23_color.py (pivot-power S pass on every Look; Ember = Ember K, the Kodachrome-leaning build).
B&W: looks_v23_bw.py (Invitation B warm silver, Witness A neutral, Vault B orange filter + selenium, Zone scale).
Owner's picks 2026-09-29: K, B, A, B.  `python looks_v23.py` regenerates out23/."""
import numpy as np, os, json, shutil, subprocess, sys
from engine import write_cube, metrics
PICKS={'invitation':('out23bw','invitation_B'),'witness':('out23bw','witness_A'),'vault':('out23bw','vault_B'),
       'presence':('out23c','presence'),'threshold':('out23c','threshold'),'ember':('out23c','ember_K'),
       'sanctuary':('out23c','sanctuary'),'kin':('out23c','kin'),'afterglow':('out23c','afterglow')}
IDS={'invitation':(1001,'IA Invitation',True),'witness':(1002,'IA Witness',True),'vault':(1003,'IA Vault',True),'presence':(1004,'IA Presence',False),'threshold':(1005,'IA Threshold',False),'ember':(1006,'IA Ember',False),'sanctuary':(1007,'IA Sanctuary',False),'kin':(1008,'IA Kin',False),'afterglow':(1009,'IA Afterglow',False)}
if __name__=='__main__':
    here=os.path.dirname(os.path.abspath(__file__))
    for mod in ('looks_v23_bw.py','looks_v23_color.py'): subprocess.check_call([sys.executable,mod],cwd=here)
    os.makedirs('out23',exist_ok=True); rep={}
    for k,(d,src) in PICKS.items():
        lut=np.load(f'{d}/{src}.npy'); lid,name,mono=IDS[k]
        if mono: lut=np.repeat(lut.mean(-1)[...,None],3,-1) if np.abs(lut[...,0]-lut[...,2]).max()<1e-6 else lut  # toned B&W keep their tint
        np.save(f'out23/{k}.npy',lut); write_cube(f'out23/{k}.CUBE',lut,name,lid,0); rep[k]=dict(metrics(lut),source=f'{d}/{src}')
        print(k,rep[k]['source'],'black',rep[k]['black'][0],'white',rep[k]['white'][0],'2nd',rep[k]['max_second_difference'],'mono',rep[k]['monotone'])
    json.dump(rep,open('out23/metrics.json','w'),indent=1)
