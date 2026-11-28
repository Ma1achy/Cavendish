"""Part 1: the representation ladder for a boundary cell, from `srbench contrib`'s per-cell phases.

    python ladder.py        # data/ladder.json and data/ladder.md

For every shape and pitch: the phase error of each representation (summed over the boundary cells,
against the exact clipped pieces) at each tower; the reference's own floor (the exact pieces summed
by two different kernels, soup and welded polyhedron); fitted orders in h; the pieces' real vertex,
edge and face counts; and, from `srbench time`, FLOPs, bytes and ns per cell per point.
"""

from __future__ import annotations

import json

import numpy as np

import common as C

# The ladder's rungs, in the order of the brief, with what each stores (bytes, f32 unless said).
RUNGS = [
    ("fill", "fill only (mass at the cell centre)", 4),
    ("centroid", "fill + centroid", 16),
    ("second", "+ second moment", 40),
    ("third", "+ third moment", 80),
    ("plic_centroid_normal", "PLIC, normal from the centroid offset", 4 + 8),
    ("plic_mesh_normal", "PLIC, mesh area-weighted normal", 4 + 8),
    ("plic_mof", "PLIC, moment-of-fluid normal (f64)", 4 + 8),
    ("mof_fill32", "PLIC MOF, normal 2×32 bit, offset from the fill", 4 + 8),
    ("mof_fill16", "PLIC MOF, normal 2×16 bit, offset from the fill", 4 + 4),
    ("mof_fill8", "PLIC MOF, normal 2×8 bit, offset from the fill", 4 + 2),
    ("mof_store32", "PLIC MOF, normal and offset stored, 32 bit each", 4 + 12),
    ("mof_store16", "PLIC MOF, normal and offset stored, 16 bit each", 4 + 6),
    ("mof_store8", "PLIC MOF, normal and offset stored, 8 bit each", 4 + 3),
    ("mof_pack32", "PLIC MOF, normal 11+11 bit and offset 10 bit in one word", 4 + 4),
    ("two_plane", "two planes where the surface has two directions", 4 + 16),
    ("mof_residual", "PLIC MOF + residual moments to l = 3", 80 + 8),
    ("mof_fill8_residual", "PLIC MOF (8-bit normal) + residual moments to l = 3", 80 + 2),
    ("two_plane_residual", "two planes + residual moments to l = 3", 80 + 16),
    ("mof_fill8_residual_common", "PLIC MOF (8-bit) + multipole of P − Q about the cell centre, to l = 3", 80 + 2 + 12),
    ("exact", "the exact clipped piece", None),
]


def load(shape: str, level: int) -> dict[str, object]:
    meta = json.loads((C.DATA / f"contrib-{shape}-L{level}.json").read_text())
    d = C.WORK / shape / f"L{level}"
    reps, stats, nt = meta["reps"], meta["stats"], meta.get("towers", 3)
    c = np.fromfile(d / "contrib.f64").reshape(-1, len(reps), nt)
    s = np.fromfile(d / "stats.f64").reshape(-1, len(stats))
    return {"reps": reps, "stats": stats, "c": c, "s": s, "seconds": meta["seconds"]}


def errors(dat: dict[str, object]) -> dict[str, list[float]]:
    reps, c = dat["reps"], dat["c"]
    ex = c[:, reps.index("exact"), :]
    out = {r: np.abs((c[:, j, :] - ex).sum(axis = 0)).tolist() for j, r in enumerate(reps)}
    out["_signal_boundary"] = ex.sum(axis = 0).tolist()
    out["_floor"] = out.pop("exact_welded")
    return out


def run() -> None:
    res: dict[str, object] = {}
    for shape in C.SHAPES:
        prep = C.load_json(f"prep-{shape}.json")
        lv = []
        for k, h in enumerate(C.PITCHES[shape]):
            try:
                dat = load(shape, k)
            except FileNotFoundError:
                continue
            s, names = dat["s"], dat["stats"]
            col = {n: s[:, i] for i, n in enumerate(names)}
            pieces = {key: {"median": float(np.median(col[key])), "p90": float(np.percentile(col[key], 90)),
                            "max": float(col[key].max()), "mean": float(col[key].mean())}
                      for key in ("triangles", "surface_triangles", "welded_vertices", "welded_edges",
                                  "welded_faces", "plic_vertices", "plic_faces")}
            lv.append({"level": k, "h": h, "cells": len(s), "errors": errors(dat), "pieces": pieces,
                       "two_plane_share": float(col["two_plane"].mean()), "concave_share": float(col["concave"].mean()),
                       "flat_cells": int(col["flat"].sum()),
                       "c3_flat_max": float(np.abs(dat["c"][col["flat"] > 0, dat["reps"].index("plic_mesh_normal")]
                                                   - dat["c"][col["flat"] > 0, 0]).max()) if col["flat"].sum() else None,
                       "c3_flat_turned_max": float(np.abs(dat["c"][col["flat"] > 0, dat["reps"].index("plic_mesh_turned")]
                                                          - dat["c"][col["flat"] > 0, 0]).max()) if col["flat"].sum() else None,
                       "mof_resid_over_h_p90": float(np.percentile(col["mof_resid_over_h"], 90)),
                       "store8_mass_err_abs_p90": float(np.percentile(np.abs(col["store8_mass_err"]), 90)),
                       "store16_mass_err_abs_p90": float(np.percentile(np.abs(col["store16_mass_err"]), 90)),
                       "seconds": dat["seconds"]})
        # Orders: slope of log error against log h over the levels, per rep and tower.
        orders = {}
        if len(lv) >= 3:
            hs = np.log([x["h"] for x in lv])
            for r in lv[0]["errors"]:
                if r.startswith("_"):
                    continue
                o = []
                for t in range(len(C.DISTANCES)):
                    e = np.array([x["errors"][r][t] for x in lv])
                    fl = np.array([x["errors"]["_floor"][t] for x in lv])
                    ok = e > 10 * fl
                    o.append(float(np.polyfit(hs[ok], np.log(e[ok]), 1)[0]) if ok.sum() >= 3 else None)
                orders[r] = o
        res[shape] = {"levels": lv, "orders": orders, "mesh_triangles": prep["triangles"]}
    C.save_json("ladder.json", res)
    write_md(res)


def fmt(x: float | None) -> str:
    return "—" if x is None else f"{x:.1e}"


def write_md(res: dict[str, object]) -> None:
    lines = ["# The ladder (generated by ladder.py from data/contrib-*.json)", ""]
    for shape, r in res.items():
        lv = r["levels"]
        if not lv:
            continue
        top = lv[-1]
        lines += [f"## {C.NAME[shape]}, h = {top['h']:.4g} m ({top['cells']} boundary cells)", "",
                  "| representation | " + " | ".join(f"{d} m" for d in C.DISTANCES) + " | order at " + " / ".join(f"{d}" for d in C.DISTANCES) + " |",
                  "|---|" + "---|" * (len(C.DISTANCES) + 1)]
        e = top["errors"]
        for key, label, _ in RUNGS:
            if key not in e:
                continue
            o = r["orders"].get(key, [None] * len(C.DISTANCES))
            lines.append(f"| {label} | " + " | ".join(fmt(x) for x in e[key]) + " | "
                         + " / ".join("—" if x is None else f"{x:.1f}" for x in o) + " |")
        lines.append("| *reference floor (two exact kernels)* | " + " | ".join(fmt(x) for x in e["_floor"]) + " | |")
        lines.append("| *boundary cells' own phase* | " + " | ".join(fmt(abs(x)) for x in e["_signal_boundary"]) + " | |")
        p = top["pieces"]
        lines += ["", "Exact pieces: " + ", ".join(f"{k} median {v['median']:.0f} (90% {v['p90']:.0f}, max {v['max']:.0f})"
                                                  for k, v in p.items()), ""]
    (C.DATA / "ladder.md").write_text("\n".join(lines), encoding = "utf-8")


if __name__ == "__main__":
    run()
