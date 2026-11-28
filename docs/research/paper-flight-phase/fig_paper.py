"""The proposed replacement figure for the paper's moving-body results (paper style).

    python fig_paper.py          # flight-phase.pdf, flight-phase.png (reads map.json, walkpast-spot.json,
                                 # torus-world.json, transfer.json)

(a) where the instantaneous read is within 1 % and 10 % (point-mass passes, temporal error only);
(b) Spot walking past at 1.4 m/s, 2 m: the real phase (flight) against the paper's leading read;
(c) the torus's body-frame (c) and (d) errors over the pass, leading against flight;
(d) the world-fixed torus lines: leading against flight, with the textbook sinc² envelope.
"""

from __future__ import annotations

import json
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

import style as S

HERE = Path(__file__).resolve().parent


def main() -> None:
    S.apply()
    fig, ((a, b), (tp, c)) = plt.subplots(2, 2, figsize = (6.8, 4.8))
    m = json.loads((HERE / "map.json").read_text(encoding = "utf-8"))
    v, d = np.array(m["speeds_m_s"]), np.array(m["distances_m"])
    for z, ls in (("0", "-"), ("5", "--")):
        err = np.array(m["z"][z]["temporal_max_over_peak"])
        cs = a.contour(v, d, err, levels = [0.01, 0.1], colors = [S.ORANGE, S.VERMILION], linestyles = ls,
                       linewidths = 1.1)
        if z == "0":
            a.clabel(cs, fmt = {0.01: "1 %", 0.1: "10 %"}, fontsize = 6.5)
    for (vv, dd, lab) in ((1.4, 2.0, "walker"), (2.5, 10.0, "2.5 m/s"), (10.0, 10.0, "car")):
        a.plot([vv], [dd], marker = "o", color = S.BLUE, ms = 3)
        a.annotate(lab, (vv, dd), textcoords = "offset points", xytext = (3, 3), fontsize = 6.5, color = S.BLUE)
    a.set_xscale("log")
    a.set_yscale("log")
    a.set_xlabel("speed (m/s)")
    a.set_ylabel("closest approach (m)")
    a.set_title("(a) instant read: 1 %, 10 % error", fontsize = 8)
    spot = HERE / "walkpast-spot.json"
    if spot.exists():
        r = json.loads(spot.read_text(encoding = "utf-8"))
        t = np.array(r["t"])
        for k, pose in enumerate(["upright", "turned 90° about z", "on its side (90° about x)"]):
            p = r["poses"][pose]
            b.plot(t, p["flight"], color = S.SERIES[k], lw = 1.1, label = pose.split(" (")[0])
            b.plot(t, p["leading"], color = S.SERIES[k], lw = 0.8, ls = ":")
        b.set_xlabel("time of the π pulse (s)")
        b.set_ylabel("phase (rad)")
        b.set_title("(b) Spot walking past, 2 m", fontsize = 8)
        b.legend(fontsize = 6, loc = "lower left")
    world = HERE / "torus-world.json"
    if world.exists():
        w = json.loads(world.read_text(encoding = "utf-8"))
        h = w["h"]
        for j, (speed, r) in enumerate(w["speeds"].items()):
            for k in r["flight"]:
                f = int(k) * float(speed) / h
                c.loglog([f], [max(r["flight"][k])], marker = S.MARKERS[j], color = S.BLUE, ls = "none",
                         label = f"flight, {speed} m/s" if k == "1" else None)
                c.loglog([f], [max(r["leading"][k])], marker = S.MARKERS[j], color = S.ORANGE, ls = "none",
                         mfc = "none", label = "leading (any speed)" if (k == "1" and j == 0) else None)
        ff = np.geomspace(0.05, 40, 600)
        lead1 = max(w["speeds"]["1"]["leading"]["1"])
        c.loglog(ff, lead1 * np.sinc(ff * 0.73) ** 2, color = S.INK2, lw = 0.6, ls = ":", label = r"$\times$sinc$^2(\pi fT)$")
        c.set_xlabel("line frequency kv/h (Hz)")
        c.set_ylabel("line amplitude (rad)")
        c.legend(fontsize = 5.5, loc = "lower left")
    body = HERE / "torus-body.json"
    if body.exists():
        r = json.loads(body.read_text(encoding = "utf-8"))
        x = np.array(r["x"])
        for kind, colour in (("c", S.GREEN), ("d", S.VERMILION)):
            for read, ls in (("leading", ":"), ("flight", "-")):
                tp.semilogy(x, np.maximum(np.abs(r[kind][read]), 1e-14), color = colour, ls = ls, lw = 1.0,
                           label = f"({kind}) {read}")
        tp.set_xlabel("torus centre along the track (m), 1 m/s")
        tp.set_ylabel("|phase error| (rad)")
        tp.set_title("(c) torus, body-frame voxels, h = 0.1 m", fontsize = 8)
        tp.legend(fontsize = 6, ncol = 2, loc = "lower center")
    c.set_title("(d) torus lines, world-fixed voxels, (c)", fontsize = 8)
    fig.tight_layout()
    fig.savefig(HERE / "flight-phase.pdf")
    fig.savefig(HERE / "flight-phase.png", dpi = 200)


if __name__ == "__main__":
    main()
