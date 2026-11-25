"""Compare the stored cavity runs (results/cav_*.json) with the primary-source references.

No solver runs: it only reads the JSON the earlier `cavity.py` runs wrote. The reference values
were read from the papers themselves (see docs/research/fluids.md section 10):

- de Vahl Davis 1983, Table V (p. 256): Nu and the midplane velocity maxima. His estimated
  accuracy is Table VII (p. 260): 0.1, 0.2, 0.3 and 1 % for Ra 10^3 .. 10^6.
- Hortmann, Peric & Scheuerer 1990, Table IV (p. 202): grid-independent values from grids of up
  to 640 x 640, stated accurate to 0.01 % (abstract). Ra 10^4 .. 10^6 only.

Usage: python -I compare_refs.py   (prints a table; the output is kept in results/compare_refs.txt)
"""

import json
import math
from pathlib import Path

RESULTS = Path(__file__).parent / "results"

# Ra -> (Nu, u_max on the vertical midplane, v_max on the horizontal midplane), alpha/L units.
DE_VAHL_DAVIS = {
    1e3: (1.118, 3.649, 3.697),
    1e4: (2.243, 16.178, 19.617),
    1e5: (4.519, 34.73, 68.59),
    1e6: (8.800, 64.63, 219.36),
}
HORTMANN = {
    1e4: (2.24475, 16.1802, 19.6295),
    1e5: (4.52164, 34.7399, 68.6396),
    1e6: (8.82513, 64.8367, 220.461),
}
GRIDS = (32, 64, 128)


def load(ra: float, n: int) -> dict[str, float]:
    """Read one stored run."""
    return json.loads((RESULTS / f"cav_{ra:.0e}_{n}.json".replace("e+0", "e")).read_text())


def pct(value: float, ref: float) -> float:
    """Signed percentage difference of value from ref."""
    return 100.0 * (value / ref - 1.0)


def main() -> None:
    for ra, (nu_dvd, u_dvd, v_dvd) in DE_VAHL_DAVIS.items():
        runs = {n: load(ra, n) for n in GRIDS}
        nu = {n: runs[n]["Nu_hot"] for n in GRIDS}
        hort = HORTMANN.get(ra)
        ref = hort[0] if hort else nu_dvd
        name = "Hortmann" if hort else "de Vahl Davis"
        print(f"Ra = {ra:.0e}   (reference for the pass test: {name} {ref})")
        for n in GRIDS:
            line = f"  N = {n:3d}  Nu = {nu[n]:.4f}  vs dVD {pct(nu[n], nu_dvd):+6.2f} %"
            if hort:
                line += f"  vs Hortmann {pct(nu[n], hort[0]):+6.2f} %"
            print(line)
        fixed = nu[128] + (nu[128] - nu[64]) / 3.0
        order = math.log((nu[32] - nu[64]) / (nu[64] - nu[128])) / math.log(2.0)
        observed = nu[128] - (nu[64] - nu[128]) / (2.0**order - 1.0)
        for label, value in (("Richardson (64,128), p = 2", fixed), (f"Richardson (32,64,128), p = {order:.2f}", observed)):
            line = f"  {label:36s} {value:.5f}  vs dVD {pct(value, nu_dvd):+7.3f} %"
            if hort:
                line += f"  vs Hortmann {pct(value, hort[0]):+7.3f} %"
            print(line)
        # Grid for 1 % if the error falls as N^-2 from the N = 128 value.
        err = abs(pct(nu[128], ref))
        print(f"  N for 1 % against {name} if error ~ N^-2: {128 * math.sqrt(err / 1.0):.0f}")
        u, v = runs[128]["umax_midline"], runs[128]["vmax_midline"]
        line = f"  N = 128 u_max {u:.3f} v_max {v:.3f}: vs dVD {pct(u, u_dvd):+.2f} / {pct(v, v_dvd):+.2f} %"
        if hort:
            line += f"; vs Hortmann {pct(u, hort[1]):+.2f} / {pct(v, hort[2]):+.2f} %"
        print(line)
        print()


if __name__ == "__main__":
    main()
