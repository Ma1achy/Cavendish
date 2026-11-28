"""Quadrature check for torus.py body: the frozen (d) error at its peak, engine rule against a finer one.

    python torus_qcheck.py       # torus-qcheck.json
"""
import json, sys, numpy as np
from dataclasses import replace
sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
import torus as Tm, flight as F
L = Tm.L
b = json.load(open('torus-body.json', encoding='utf-8'))
x = np.array(b['x']); i = int(np.argmax(np.abs(b['d']['frozen']))); xm = float(x[i])
mesh = L.torus_mesh(); vox = L.voxelise(mesh, Tm.H, origin=Tm.H * L.GENERIC - np.array([0.0, Tm.Y0, 0.0]))
mass = vox.frac * Tm.H**3

c = np.array([xm, Tm.Y0, 0.0])
def pot(pts, ts):
    q = pts - c
    u = Tm._u_points(mass, vox.centroid, q) - Tm._u_mesh(vox.piece_tris, q) + Tm._u_quadrupole(vox.second, vox.centroid, q)
    return -L.RHO * u
out = []
for o, p in [(6, 0.1), (12, 0.025)]:
    ins = replace(F.AION10, order=o, panel=p)
    out.append({'order': o, 'panel_s': p, 'x_m': xm, 'frozen_d_error_rad': F.frozen_phase(pot, ins, np.zeros(3), 0.0), 'json_rad': b['d']['frozen'][i]}); print(out[-1])
open('torus-qcheck.json', 'w').write(json.dumps(out, indent=1))
