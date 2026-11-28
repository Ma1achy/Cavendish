"""C1: flight.py against Cavendish itself (`cav run`) on two moving point-mass scenes.

    cav run scenes/walk.ron --out <dir>/walk ; cav run scenes/fast.ron --out <dir>/fast
    python crosscheck.py <dir>          # crosscheck.json

Criterion (CRITERIA.md, C1): |flight.py − cav| ≤ 1e-10 rad at every readout; control: the frozen
read differs from cav by ≥ 1e-6 rad somewhere in each scene.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

import flight as F

HERE = Path(__file__).resolve().parent

SCENES = {  # (mass kg, from m, to m, start s, speed m/s): as written in scenes/*.ron
    "walk": (80.0, (2.0, -8.0, 1.0), (2.0, 8.0, 1.0), 0.5, 1.4),
    "fast": (1000.0, (5.0, -40.0, 7.0), (5.0, 40.0, 7.0), 2.0, 10.0),
}


def pass_path(frm: tuple[float, ...], to: tuple[float, ...], start: float, speed: float):
    a, b = np.array(frm), np.array(to)
    length = float(np.linalg.norm(b - a))

    def path(t: F.F64) -> F.F64:
        s = np.clip((np.asarray(t) - start) * speed, 0.0, length)
        return a[None, :] + (b - a)[None, :] * (s / length)[:, None]
    return path, (start, start + length / speed)


def main(bundles: Path) -> None:
    out: dict[str, object] = {}
    ins = F.Instrument(order = 12, panel = 0.02)
    tower = np.zeros(3)
    ok = True
    for name, (mass, frm, to, start, speed) in SCENES.items():
        d = bundles / name
        times = np.load(d / "detector_time.npy").ravel()
        cav = np.load(d / "signal.npy").ravel()
        keep = np.isfinite(times)
        path, breaks = pass_path(frm, to, start, speed)
        pot = F.point_potential(mass, path)
        ours, frozen = [], []
        for t in times[keep]:
            ours.append(F.flight_phase(pot, ins, tower, t - 2 * ins.half, breaks))
            frozen.append(F.frozen_phase(pot, ins, tower, t - 2 * ins.half))
        diff = np.abs(np.array(ours) - cav[keep])
        ctrl = np.abs(np.array(frozen) - cav[keep])
        passed = bool(diff.max() <= 1e-10 and ctrl.max() >= 1e-6)
        ok &= passed
        out[name] = {"readouts_s": times[keep].tolist(), "cav_rad": cav[keep].tolist(), "flight_py_rad": ours,
                     "frozen_py_rad": frozen, "max_abs_diff_rad": float(diff.max()),
                     "control_max_frozen_vs_cav_rad": float(ctrl.max()), "pass": passed}
        print(f"{name}: max |py − cav| = {diff.max():.2e} rad; control {ctrl.max():.2e} rad; "
              f"peak |Φ| {np.abs(cav).max():.3f} rad -> {'PASS' if passed else 'FAIL'}")
    out["engine_revision"] = json.loads((bundles / "walk" / "manifest.json").read_text(encoding = "utf-8"))[
        "numerics"]["engine"]["revision"]
    out["pass"] = ok
    (HERE / "crosscheck.json").write_text(json.dumps(out, indent = 1))


if __name__ == "__main__":
    main(Path(sys.argv[1]))
