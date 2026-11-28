"""The spatial error of the instant gradient read, static point source, out to 300 m.

    python spatial.py            # spatial.json

For a point mass at rest at horizontal distance d (level with the lower launch, z = 0, or at
mid-tower, z = 5 m), the relative error of the paper's launch-point read and of the read at the
arms' effective height, against the static flight integral (the frozen read). No time enters.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

import flight as F

HERE = Path(__file__).resolve().parent


def main() -> None:
    ins = F.AION10
    z_eff = F.effective_height(ins)
    dists = np.geomspace(1.0, 300.0, 60)
    out: dict[str, object] = {"d_m": dists.tolist(), "z_eff_m": z_eff}
    for z in (0.0, 5.0):
        rel, rel_eff = [], []
        for d in dists:
            path = F.straight(np.array([d, 0.0, z]), np.zeros(3))
            fr = F.frozen_phase(F.point_potential(1.0, path), ins, np.zeros(3), 0.0)
            le = F.leading_phase(F.point_field(1.0, path), ins, np.zeros(3), ins.half)
            lf = F.leading_phase(F.point_field(1.0, path), ins, np.zeros(3), ins.half, z_eff)
            rel.append(abs(le - fr) / abs(fr))
            rel_eff.append(abs(lf - fr) / abs(fr))
        out[f"z={z:g}"] = {"launch_point": rel, "effective_height": rel_eff}
        for lvl in (0.01, 0.1):
            for name, r in (("launch", rel), ("eff", rel_eff)):
                r = np.array(r)
                bad = np.nonzero(r > lvl)[0]
                beyond = float(dists[bad[-1] + 1]) if len(bad) and bad[-1] + 1 < len(dists) else (
                    float(dists[0]) if not len(bad) else None)
                out[f"z={z:g}"][f"{name}_within_{lvl:g}_beyond_m"] = beyond
                print(f"z={z:g} {name}: within {lvl:.0%} beyond {beyond} m")
    (HERE / "spatial.json").write_text(json.dumps(out, indent = 1))


if __name__ == "__main__":
    main()
