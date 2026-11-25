"""Compare the Werner & Scheeres face form (the research benchmark's `polyhedron_field`, with the
h_f·2A_f numerator) against ESA's fixtures (`esa_fixtures.py`). Prints, and writes
`data/crosscheck.json`: per body, the largest relative difference in U and in |g| over the points.

Pass criterion, written before running (research note §5): agreement to 1e-12 relative in U and g
at R/s ≤ 100, for every body including the non-convex one.

    python scripts/crosscheck.py
"""

from __future__ import annotations

import json
import pathlib
import sys

import numpy as np

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[2] / "research" / "exact-skin-and-partial-fill"))
import skin_bench  # noqa: E402

skin_bench.edge_numerator = True


def main() -> None:
    fx = json.loads((HERE.parent / "data" / "esa_fixtures.json").read_text(encoding="utf-8"))
    pts = np.array(fx["points"])
    report: dict[str, object] = {"esa_version": fx["version"], "bodies": {}}
    worst = 0.0
    for name, b in fx["bodies"].items():
        tris = np.array(b["triangles"])
        u, g = skin_bench.polyhedron_field(tris, np.ones((len(tris), 1)), pts)
        ue, ge = np.array(b["U"]), np.array(b["g"])
        du = np.abs(u[0] / ue - 1.0)
        # ESA's sign convention for g is read off the data, not assumed.
        sign = 1.0 if np.sum(g[0] * ge) > 0 else -1.0
        dg = np.linalg.norm(g[0] - sign * ge, axis=1) / np.linalg.norm(ge, axis=1)
        report["bodies"][name] = {  # type: ignore[index]
            "faces": len(tris), "max_rel_U": float(du.max()), "max_rel_g": float(dg.max()),
            "g_sign_esa_vs_ours": sign,
        }
        worst = max(worst, float(du.max()), float(dg.max()))
        print(f"{name:13s} {len(tris):5d} faces  U {du.max():.1e}  g {dg.max():.1e}"
              f"  (ESA g sign {sign:+.0f})")
    report["worst"] = worst
    report["pass_1e-12"] = worst <= 1e-12
    print("pass (1e-12):", worst <= 1e-12)
    (HERE.parent / "data" / "crosscheck.json").write_text(json.dumps(report, indent=1),
                                                          encoding="utf-8")


if __name__ == "__main__":
    main()
