"""The gradiometer's transfer function, ideal and in the near field (C2, C3).

    python transfer.py          # transfer.json, transfer.pdf, transfer.png

1. |H(f)|/H(0) of the arms' separation (uniform time-varying gradient -> phase): numerical, from
   flight.py's arms, against sinc²(πfT) (Cheinet et al. 2008 eq. 9; acceleration transfer
   H_a = k_eff H/ω² = k_eff T² sinc²(ωT/2)). Symmetric launch without floor (the textbook case)
   and the engine's (u₀ = 3.86 m/s, floor).
2. The near field: a point mass at horizontal distance d, level with the lower launch point (z = 0)
   or midway up the tower (z = 5 m), whose mass is modulated as e^{iωt}. Its flight phase relative
   to the read frozen at the π pulse, R(f) = Σ s w ΔV e^{iω(τ−T)} / Σ s w ΔV, is the transfer
   function the instrument really applies to that source; 1 − R is the error of the instantaneous
   approximation at frequency f.
"""

from __future__ import annotations

import json
from dataclasses import replace
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

import flight as F
import style as S

HERE = Path(__file__).resolve().parent
FREQS = np.geomspace(1e-3, 30.0, 1200)
DISTANCES = [1.0, 1.5, 2.0, 3.0, 5.0, 10.0, 20.0, 50.0]
HEIGHTS = [0.0, 5.0]


def sinc2(f: np.ndarray, t: float) -> np.ndarray:
    return np.sinc(f * t) ** 2  # numpy's sinc(x) = sin(πx)/(πx)


def near_response(ins: F.Instrument, d: float, z: float, freqs: np.ndarray) -> np.ndarray:
    """R(f) (complex) for a modulated point mass at (d, 0, z) from the tower's base."""
    tau, w, z_lo, z_up = F.nodes(ins)
    src = np.array([d, 0.0, z])
    acc = np.zeros(len(tau))
    for height, s in ((0.0, 1.0), (ins.separation, -1.0)):
        r_u = np.hypot(d, height + z_up - z)
        r_l = np.hypot(d, height + z_lo - z)
        acc += s * w * (-1.0 / r_u + 1.0 / r_l)
    del src
    return (np.exp(2j * np.pi * np.outer(freqs, tau - ins.half)) @ acc) / acc.sum()


def crossing(freqs: np.ndarray, err: np.ndarray, level: float) -> float | None:
    """The lowest frequency at which `err` first reaches `level` (log-interpolated)."""
    above = np.nonzero(err >= level)[0]
    if len(above) == 0:
        return None
    i = int(above[0])
    if i == 0:
        return float(freqs[0])
    f0, f1, e0, e1 = freqs[i - 1], freqs[i], err[i - 1], err[i]
    x = (np.log(level) - np.log(e0)) / (np.log(e1) - np.log(e0))
    return float(np.exp(np.log(f0) + x * (np.log(f1) - np.log(f0))))


def main() -> None:
    res: dict[str, object] = {}
    ideal = replace(F.AION10, floor = False).symmetric()
    num = F.separation_transfer(ideal, FREQS)
    ref = sinc2(FREQS, ideal.half)
    low = FREQS <= 10.0
    c2 = float(np.max(np.abs(num[low] - ref[low])))
    ctrl = float(np.max(np.abs(num[low] - sinc2(FREQS[low], 1.01 * ideal.half))))
    res["C2"] = {"max_abs_diff_f_le_10Hz": c2, "control_T_plus_1pct": ctrl, "pass": c2 <= 1e-9 and ctrl > 1e-9}
    print(f"C2: |numerical − sinc²| ≤ {c2:.1e} for f ≤ 10 Hz; control (T+1%) {ctrl:.1e}")
    engine = F.separation_transfer(F.AION10, FREQS)
    tau, w, z_lo, z_up = F.nodes(replace(F.AION10, order = 16, panel = 0.005))
    area = float(np.sum(w * (z_up - z_lo)))
    res["engine_arms"] = {"separation_area_m_s": area, "ideal_area_vT2": F.AION10.kick * F.AION10.half**2,
                          "ratio": area / (F.AION10.kick * F.AION10.half**2),
                          "max_abs_diff_vs_sinc2_f_le_10Hz": float(np.max(np.abs(engine[low] - ref[low])))}
    print("engine arms: separation area / vT² =", res["engine_arms"]["ratio"])
    # Where sinc² falls: 1 %, 10 %, half, first null.
    marks = {lvl: crossing(FREQS, 1.0 - ref, lvl) for lvl in (0.01, 0.1, 0.5)}
    res["sinc2_falls"] = {"1pct_Hz": marks[0.01], "10pct_Hz": marks[0.1], "half_Hz": marks[0.5],
                          "first_null_Hz": 1.0 / ideal.half, "at_10Hz": float(sinc2(np.array([10.0]), ideal.half)[0]),
                          "envelope_at_10Hz_1_over_(pi f T)^2": float(1.0 / (np.pi * 10.0 * ideal.half) ** 2)}
    print("sinc² falls 1 % at", marks[0.01], "Hz, 10 % at", marks[0.1], "Hz, half at", marks[0.5])
    near: dict[str, object] = {}
    c3 = None
    for z in HEIGHTS:
        for d in DISTANCES:
            r = near_response(F.AION10, d, z, FREQS)
            err = np.abs(r - 1.0)
            key = f"d={d:g},z={z:g}"
            near[key] = {"d": d, "z": z, "abs_R": np.abs(r).tolist(), "err": err.tolist(),
                         "f_1pct_Hz": crossing(FREQS, err, 0.01), "f_10pct_Hz": crossing(FREQS, err, 0.1),
                         "f_half_Hz": crossing(np.asarray(FREQS), np.abs(1 - np.abs(r)), 0.5)}
            if d == 50.0 and z == 0.0:
                sel = FREQS <= 2.0
                c3 = float(np.max(np.abs(np.abs(r[sel]) - ref[sel]) / ref[sel].clip(1e-30)))
            print(f"z={z:g} d={d:5.1f}: error 1 % at {near[key]['f_1pct_Hz']:.3g} Hz, 10 % at "
                  f"{near[key]['f_10pct_Hz']:.3g} Hz")
    res["near"] = near
    res["C3"] = {"d50_max_rel_diff_vs_sinc2_f_le_2Hz": c3, "pass": c3 is not None and c3 <= 0.01}
    print(f"C3: d = 50 m, f ≤ 2 Hz, max relative |R| − sinc² = {c3:.2e}")
    res["freqs_Hz"] = FREQS.tolist()
    res["sinc2"] = ref.tolist()
    res["numerical_ideal"] = num.tolist()
    res["numerical_engine"] = engine.tolist()
    (HERE / "transfer.json").write_text(json.dumps(res))
    plot(res)


def plot(res: dict[str, object]) -> None:
    S.apply()
    f = np.array(res["freqs_Hz"])
    fig, (a, b) = plt.subplots(1, 2, figsize = (6.8, 2.6))
    a.loglog(f, res["sinc2"], color = S.BLACK, lw = 1.0, label = r"sinc$^2(\pi f T)$")
    a.loglog(f, res["numerical_engine"], color = S.ORANGE, lw = 1.0, ls = "--",
             label = "engine arms ($u_0$ = 3.86 m/s, floor)")
    a.loglog(f, 1.0 / (np.pi * f * 0.73) ** 2, color = S.INK2, lw = 0.6, ls = ":", label = r"$1/(\pi fT)^2$")
    for lvl, lab in ((0.99, "1 %"), (0.9, "10 %")):
        a.axhline(lvl, color = S.GRID, lw = 0.6)
    a.set_ylim(1e-5, 2)
    a.set_xlabel("frequency (Hz)")
    a.set_ylabel(r"$|H(f)|/H(0)$")
    a.set_title("(a) uniform gradient: the textbook transfer", fontsize = 8)
    a.legend(fontsize = 6.5, loc = "lower left")
    near = res["near"]
    for k, d in enumerate([1.5, 2.0, 5.0, 10.0, 50.0]):
        r = near[f"d={d:g},z=0"]
        b.loglog(f, np.maximum(r["err"], 1e-12), color = S.SERIES[k], lw = 1.0, label = f"d = {d:g} m")
    b.loglog(f, np.abs(1.0 - np.array(res["sinc2"])), color = S.BLACK, lw = 0.8, ls = "--", label = r"$1-$sinc$^2$")
    for lvl in (0.01, 0.1):
        b.axhline(lvl, color = S.INK2, lw = 0.6, ls = ":")
    b.set_xlim(1e-2, 30)
    b.set_ylim(1e-6, 3)
    b.set_xlabel("frequency (Hz)")
    b.set_ylabel(r"$|R(f)-1|$, instantaneous error")
    b.set_title("(b) point source at distance d, level with the lower launch", fontsize = 8)
    b.legend(fontsize = 6.5, loc = "lower right", ncol = 2)
    fig.tight_layout()
    fig.savefig(HERE / "transfer.pdf")
    fig.savefig(HERE / "transfer.png", dpi = 200)


if __name__ == "__main__":
    main()
