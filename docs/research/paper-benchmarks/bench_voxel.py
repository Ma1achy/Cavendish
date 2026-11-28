"""Criterion B1: a voxelised body summed cube by cube in four codes, against 40 digits.

    KERNEL_BENCH=<path to kernel_bench.exe> .venv-bench/Scripts/python bench_voxel.py

Each body is voxelised to whole cubes (centre inside, winding number) at h = 1/16 m on a lattice
offset by a fixed random sub-cell shift. The same cube set is then summed by:
  - the study's cube voxel sum: the closed form over each cube's 12 triangles (`skin_bench`);
  - Harmonica's prism sum (Nagy et al. 2000 via choclo), U and g_z;
  - Cavendish's cube kernel `cube_potential` (prism near, halves, series far) and Nagy's
    `prism_potential` alone (U only: Cavendish's cube kernel is a potential kernel);
  - the reference: Nagy's corner sum per cube at 40 digits.
The read is the gradiometer phase k_eff T² Δg_z·Gρ (ρ = 2700 kg/m³), lower point at 1.5, 10 and
100 m from the centre in three directions, upper point 5 m above it. Writes `voxel.json`.
"""

from __future__ import annotations

import json

import mpmath as mp
import numpy as np

import common as c

H = 1.0 / 16.0
LOWER = (1.5, 10.0, 100.0)
SCALE = c.PHASE_PER_DG * c.sb.G * c.sb.RHO  # rad per (m of g at density 1, no G)


def main() -> None:
    rng = np.random.default_rng(7)
    offset = rng.uniform(0.0, H, size = 3)
    every = c.bodies()
    lower = np.array([d * u for d in LOWER for u in c.DIRECTIONS])
    pts = np.concatenate([lower, lower + np.array([0.0, 0.0, 5.0])])
    m = len(lower)
    out: dict[str, object] = {"h": H, "offset": offset.tolist(), "rad_per_unit_g": SCALE, "bodies": {}}
    for name in ("cube", "icosahedron", "stellated"):
        centres = c.voxelise(every[name], H, offset)
        boxes = c.cube_boxes(centres, H)
        ref_u, ref_gz = [], []
        for p in pts:
            su, sg = mp.mpf(0), mp.mpf(0)
            for b in boxes:
                u, gz = c.mp_box(b[[0, 2, 4]], b[[1, 3, 5]], p)
                su += u
                sg += gz
            ref_u.append(su)
            ref_gz.append(sg)
        u_py, g_py = c.ours_py(c.sb.cube_tris(centres, H), pts)
        u_hm, g_hm = c.harmonica_boxes(boxes, pts)
        u_ck, u_pr = c.cavendish_cubes(centres, H, pts)
        ref_phase = [float((ref_gz[m + k] - ref_gz[k]) * SCALE) for k in range(m)]
        rows: dict[str, object] = {"cubes": len(centres), "R_lower": np.linalg.norm(lower, axis = 1).tolist(),
                                   "phase_ref_rad": ref_phase}
        for code, u, gz in (("study_cube_sum", u_py, g_py[:, 2]), ("harmonica", u_hm, g_hm[:, 2]),
                            ("cavendish_cube_kernel", u_ck, None), ("cavendish_prism", u_pr, None)):
            eu = [float(abs(mp.mpf(float(u[k])) / ref_u[k] - 1)) for k in range(len(pts))]
            row: dict[str, object] = {"U_rel": eu}
            if gz is not None:
                row["phase_err_rad"] = [float(abs((mp.mpf(float(gz[m + k])) - mp.mpf(float(gz[k])))
                                                  - (ref_gz[m + k] - ref_gz[k])) * SCALE) for k in range(m)]
            rows[code] = row
        out["bodies"][name] = rows  # type: ignore[index]
        print(f"\n{name}: {len(centres)} cubes of 1/16 m")
        for d in LOWER:
            idx = [k for k in range(m) if abs(np.linalg.norm(lower[k]) - d) < 1e-9]
            line = f"  lower {d:6.1f} m  phase {max(abs(ref_phase[k]) for k in idx):.3e} rad"
            for code in ("study_cube_sum", "harmonica", "cavendish_cube_kernel", "cavendish_prism"):
                r = rows[code]
                assert isinstance(r, dict)
                line += f" | {code} U {max(r['U_rel'][k] for k in idx):.0e}"
                if "phase_err_rad" in r:
                    line += f" ph {max(r['phase_err_rad'][k] for k in idx):.0e}"
            print(line)
    (c.HERE / "voxel.json").write_text(json.dumps(out, indent = 1), encoding = "utf-8")


if __name__ == "__main__":
    main()
