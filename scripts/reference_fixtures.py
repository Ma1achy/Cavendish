#!/usr/bin/env python3
# pyright: strict
"""Run George's reference scripts with no window and save their arrays as fixtures.

    <clone>/1D_oscillator        ──>  <out>/1d_oscillator/      phase series, periodogram, arm tables
    <clone>/100m_oscillators     ──>  <out>/100m_oscillators/   phase series (GGN only), arm tables
    <clone>/fig2_10m             ──>  <out>/fig2_10m/           panels b, c, d; ULDM; noise; periodograms
    <clone>/fig2_uldm_folding    ──>  <out>/fig2_uldm_folding/  the same, on the 100 m instrument
    <clone>/fig2_folding_map.py  ──>  <out>/fig2_folding_map/   the fold_frequency triangle wave

The scripts (github.com/Thranduil02/atom-interferometry) have no command line and their lengths are
module constants, so each script's source is executed with those constants replaced, every match of
them, with ``__name__`` set to ``"ref"`` so that ``__main__`` blocks do not run, with ``plt.show`` and
``Figure.savefig`` turned into no-ops, with what the script prints thrown away, and after
``np.random.seed(0)``. Nothing of the scripts' text is in this file or in what it writes.

Every phase series is saved twice:

    published   the script exactly as published: each arm table holds the arm at (j + 1)·dt and is
                paired with the source at j·dt.
    same_time   the same pipeline with only that one-step offset corrected: each arm table is the
                arm at j·dt (the launch height prepended, the last entry dropped). The trapezoid,
                the window, the clamp at the launch height and the constants are as published.

The clone is untrusted input: run this from outside it, with ``-I``, from a throwaway environment that
has numpy and matplotlib and nothing else.

Usage:
    python -I scripts/reference_fixtures.py <clone-dir> <out-dir>

Needs Python 3.10, numpy 2 and matplotlib.
"""

from __future__ import annotations

import contextlib
import io
import json
import re
import sys
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import cast

import matplotlib

matplotlib.use("Agg")  # no window, whatever the script asks for

import matplotlib.figure
import matplotlib.pyplot as plt
import numpy as np
from numpy.typing import NDArray

Array     = NDArray[np.float64]
Namespace = dict[str, object]

UPSTREAM  = "https://github.com/Thranduil02/atom-interferometry"
COMMIT    = "d7d360ad92837bec929b85091b2bdd9af81d285c"
SEED      = 0
MAX_SHOTS = 20_000   # no run here needs more; a script about to make more is stopped before it starts
ARMS      = ("lower1", "upper1", "lower2", "upper2")
PANELS    = ("b", "c", "d")


# ======================================== RUNNING A SCRIPT ========================================


@dataclass(frozen = True)
class Case:
    """One script and how it is run.

    Attributes:
        script    (str):            The script's file name in the clone.
        directory (str):            The fixture directory's name.
        overrides (dict[str, str]): Module constants to replace, as Python source.
        cycle     (float):          The script's cycle time, seconds: with ``T_total`` it gives the shots.
    """

    script:    str
    directory: str
    overrides: dict[str, str]
    cycle:     float


def _silence() -> None:
    """Makes ``plt.show`` and ``Figure.savefig`` do nothing: the scripts save to a Desktop path."""
    setattr(plt, "show", lambda *a, **k: None)  # pyright: ignore[reportUnknownLambdaType]
    setattr(matplotlib.figure.Figure, "savefig", lambda self, *a, **k: None)  # pyright: ignore[reportUnknownLambdaType]


def _substituted(source: str, overrides: Mapping[str, str]) -> str:
    """The script's source with every assignment to each overridden constant replaced.

    Every match is replaced, not the first: one script has a docstring line that begins like its
    ``T_total`` assignment, and replacing only that would leave the real constant at a year of
    shots. An override that matches nothing is an error.

    Args:
        source    (str):               The script's text.
        overrides (Mapping[str, str]): Constant names and their new values, as Python source.

    Returns:
        str: The text to execute.
    """
    for name, value in overrides.items():
        pattern   = re.compile(rf"^(\s*){re.escape(name)}(\s*)=(?!=)[^\n;]*", re.M)
        source, n = pattern.subn(
            lambda m, name = name, value = value: f"{m.group(1)}{name}{m.group(2)}= {value} ", source
        )
        if n < 1:
            raise SystemExit(f"override `{name}` matched nothing: the script has changed")
    return source


def _load(clone: Path, case: Case) -> Namespace:
    """Executes a script with its constants replaced and returns its module namespace.

    The number of shots the overrides imply is checked before anything runs, and again afterwards
    against what the script made of them.

    Args:
        clone (Path): The directory of the clone.
        case  (Case): Which script, and with what constants.

    Returns:
        Namespace: The script's globals after it ran.
    """
    total = float(case.overrides["T_total"])
    shots = int(total / case.cycle)
    if not 0 < shots <= MAX_SHOTS:
        raise SystemExit(f"{case.script}: {shots} shots is outside 1 to {MAX_SHOTS}")
    path   = clone / case.script
    source = _substituted(path.read_text(encoding = "utf-8"), case.overrides)
    np.random.seed(SEED)  # the scripts draw noise from the legacy global stream
    namespace: Namespace = {"__name__": "ref"}
    with contextlib.redirect_stdout(io.StringIO()):
        exec(compile(source, str(path), "exec"), namespace)  # noqa: S102 - the point of this harness
    plt.close("all")
    if float(cast(float, namespace["T_total"])) != total or int(cast(int, namespace["N_cycles"])) != shots:
        raise SystemExit(f"{case.script}: the overrides did not take: T_total = {namespace['T_total']}")
    return namespace


def _array(value: object) -> Array:
    """A copy of ``value`` as contiguous float64."""
    return np.ascontiguousarray(np.asarray(value, dtype = np.float64)).copy()


def _number(namespace: Namespace, name: str) -> float:
    return float(cast(float, namespace[name]))


def _constants(namespace: Namespace, names: tuple[str, ...]) -> dict[str, float]:
    """The named module constants as plain floats, for ``meta.json``."""
    return {name: _number(namespace, name) for name in names}


def _plain(value: Mapping[str, object]) -> dict[str, object]:
    """An oscillator dictionary with only what JSON can hold (numbers, strings, booleans)."""
    return {k: v for k, v in value.items() if isinstance(v, (int, float, str, bool))}


def _same_time(table: Array, launch: float) -> Array:
    """An arm table moved back one step: the arm at j·dt where the script's holds it at (j + 1)·dt."""
    return np.concatenate([[launch], table[:-1]])


# ======================================== WRITING A CASE ========================================


def _write(out: Path, case: Case, arrays: Mapping[str, Array], meta: dict[str, object]) -> None:
    """Writes one case's arrays as float64 ``.npy`` files and its ``meta.json``.

    Args:
        out    (Path):                The fixtures' root.
        case   (Case):                The case.
        arrays (Mapping[str, Array]): File stem to array.
        meta   (dict[str, object]):   What else to record; the common fields are added here.
    """
    directory = out / case.directory
    directory.mkdir(parents = True, exist_ok = True)
    for stale in directory.glob("*.npy"):
        stale.unlink()
    for name, array in arrays.items():
        np.save(directory / f"{name}.npy", np.ascontiguousarray(array, dtype = np.float64), allow_pickle = False)
    record: dict[str, object] = {
        "script":     case.script,
        "upstream":   UPSTREAM,
        "commit":     COMMIT,
        "overrides":  case.overrides,
        "seed":       SEED,
        "numpy":      np.__version__,
        "matplotlib": matplotlib.__version__,
        "python":     sys.version.split()[0],
        **meta,
        "arrays":     {name: list(array.shape) for name, array in arrays.items()},
    }
    (directory / "meta.json").write_text(json.dumps(record, indent = 2) + "\n", encoding = "utf-8")
    size = sum(f.stat().st_size for f in directory.iterdir())
    print(f"{case.directory:<20} {len(arrays):>3} arrays  {size / 1024:8.1f} kB")


def _offset(published: Array, same_time: Array) -> dict[str, float]:
    """The size of a series and of the one-step offset in it, for ``meta.json`` and the provenance."""
    peak = float(np.max(np.abs(published)))
    return {"peak_abs_phase": peak, "max_offset_over_peak": float(np.max(np.abs(published - same_time))) / peak}


# ======================================== THE CASES ========================================

_INSTRUMENT = ("m_atom", "hbar", "G", "g", "n", "T", "dt")


def one_d_oscillator(clone: Path, out: Path) -> None:
    """``1D_oscillator``: AION-10, a 1 kg vertical oscillator; 1000 shots.

    The script is module-level code with no function for the phase, so the same-time series is
    recomputed here from the script's own ``potential_difference``, constants and cycle arm tables.
    A control proves the recomputation: with the unshifted tables it must give back the script's
    own series.
    """
    case = Case("1D_oscillator", "1d_oscillator", {"T_total": "2000"}, 2.0)
    ns   = _load(clone, case)
    potential_difference = cast(Callable[[Array, Array, float, Array, float], Array], ns["potential_difference"])
    dt, cycles = _number(ns, "dt"), int(cast(int, ns["N_cycles"]))
    window     = int(cast(int, ns["L"]))
    length     = int(cast(int, ns["N_tot"]))
    height     = _array(ns["h_t"])
    index      = np.asarray(ns["sub_idx"], dtype = np.int64)
    scale      = _number(ns, "m_atom") / _number(ns, "hbar")
    distance, mass = _number(ns, "D_0"), _number(ns, "M")
    tables     = {arm: _array(ns[f"{arm}_cycle"]) for arm in ARMS}

    def series(arms: Mapping[str, Array]) -> Array:
        """The script's sliding trapezoid over 2T, subsampled once a cycle, for these arm tables."""
        tiled = {arm: np.tile(table, cycles)[:length] for arm, table in arms.items()}
        integrals: list[Array] = []
        for ifo in ("1", "2"):
            difference = potential_difference(tiled[f"lower{ifo}"], tiled[f"upper{ifo}"], distance, height, mass)
            running    = np.concatenate(([0.0], np.cumsum(0.5 * (difference[1:] + difference[:-1]) * dt)))
            integrals.append(running[window:] - running[:-window])
        return (scale * integrals[0] - scale * integrals[1])[index]

    published = _array(ns["subsample_phase_diff"])
    control   = float(np.max(np.abs(series(tables) - published)) / np.max(np.abs(published)))
    if control > 1e-15:
        raise SystemExit(f"1D_oscillator: the recomputation differs from the script by {control:.3e} of the peak")
    launches  = {"lower1": 0.0, "upper1": 0.0, "lower2": 5.0, "upper2": 5.0}
    same_time = series({arm: _same_time(table, launches[arm]) for arm, table in tables.items()})
    arrays: dict[str, Array] = {
        "t":               _array(ns["subsample_times"]),
        "phase_published": published,
        "phase_same_time": same_time,
        "f":               _array(ns["f_k_filtered"]),
        "S_published":     _array(ns["S_phasediff_filtered"]),
        **tables,
    }
    _write(out, case, arrays, {
        "instrument":  {**_constants(ns, _INSTRUMENT), "u0": _number(ns, "u_initial"),
                        "v_kick": _number(ns, "velocity_boost"), "cycle": _number(ns, "cycle_time"), "ifo_heights": [0.0, 5.0]},
        "oscillator":  {"M": mass, "D": distance, "h0": _number(ns, "h_0"), "amp": _number(ns, "A"), "f": _number(ns, "f")},
        "window_samples": window,
        "recomputation_control_relative": control,
        "periodogram": "the script's own DFT: no mean subtraction, DC kept, bins up to and including Nyquist",
        **_offset(published, same_time),
    })


def hundred_metre_oscillators(clone: Path, out: Path) -> None:
    """``100m_oscillators``: AICE, oscillator A alone (``phases_multi``: no ULDM, no noise); 4000 shots."""
    case = Case("100m_oscillators", "100m_oscillators", {"T_total": "2e4"}, 5.0)
    ns   = _load(clone, case)
    compute   = cast(Callable[[object], Array], ns["compute_phases"])
    published = _array(ns["phases_multi"])
    tables    = {arm: _array(ns[arm]) for arm in ARMS}
    same_time = _with_same_time_arms(ns, lambda: _array(compute(ns["active_oscillators"])))
    active    = cast(list[dict[str, object]], ns["active_oscillators"])
    _write(out, case, {"t": _array(ns["t_meas"]), "phase_published": published, "phase_same_time": same_time, **tables}, {
        "instrument":  {**_constants(ns, (*_INSTRUMENT, "u0", "v_kick", "T_cyc", "delta_r")), "ifo_heights": [0.0, _number(ns, "delta_r")]},
        "oscillators": [_plain(o) for o in active],
        **_offset(published, same_time),
    })


def _with_same_time_arms(ns: Namespace, compute: Callable[[], Array]) -> Array:
    """Runs ``compute`` with the namespace's four arm tables moved back one step, then restores them."""
    kept     = {arm: ns[arm] for arm in ARMS}
    launches = {"lower1": 0.0, "upper1": 0.0, "lower2": _number(ns, "delta_r"), "upper2": _number(ns, "delta_r")}
    try:
        for arm in ARMS:
            ns[arm] = _same_time(_array(kept[arm]), launches[arm])
        with contextlib.redirect_stdout(io.StringIO()):
            return compute()
    finally:
        ns.update(kept)


def figure_two(clone: Path, out: Path, case: Case) -> None:
    """A ``fig2_*`` script: panels b and c (vertical oscillators) and d (a circle), ULDM, noise, periodograms."""
    ns          = _load(clone, case)
    compute     = cast(Callable[..., Array], ns["compute_phases"])
    periodogram = cast(Callable[[Array], tuple[Array, Array]], ns["periodogram"])
    uldm_phase  = cast(Callable[[Array, float, float], Array], ns["uldm_phase_vec"])
    times       = _array(ns["t_meas"])
    shots       = int(cast(int, ns["N_cycles"]))
    arrays: dict[str, Array] = {"t": times}
    oscillators: dict[str, dict[str, object]] = {}
    sizes:       dict[str, dict[str, float]]  = {}
    raw: dict[str, Array] = {}
    for panel in PANELS:
        oscillator = cast(dict[str, object], ns[f"osc_{panel}"])

        def run(oscillator: dict[str, object] = oscillator, circular: bool = panel == "d") -> Array:
            return _array(compute(oscillator, circular = circular))

        with contextlib.redirect_stdout(io.StringIO()):
            raw[panel] = run()
        same_time = _with_same_time_arms(ns, run)
        arrays[f"raw_{panel}_published"] = raw[panel]
        arrays[f"raw_{panel}_same_time"] = same_time
        oscillators[panel] = {**_plain(oscillator), "circular": panel == "d"}
        sizes[panel]       = _offset(raw[panel], same_time)
    uldm = _array(uldm_phase(times, _number(ns, "f_phi"), _number(ns, "m_phi")))
    arrays["uldm"] = uldm
    # The script draws one noise series per panel, in the order a, b, c, d, from one seeded stream.
    np.random.seed(SEED)
    sigma = _number(ns, "shot_noise_asd") / float(np.sqrt(_number(ns, "T_cyc")))
    noise = {panel: _array(np.random.normal(0.0, sigma, shots)) for panel in ("a", *PANELS)}
    for panel, series in noise.items():
        arrays[f"noise_{panel}"] = series
    plotted = {
        "a": uldm + noise["a"],
        "b": raw["b"] + noise["b"],
        "c": raw["c"] + uldm + noise["c"],
        "d": raw["d"] + uldm + noise["d"],
    }
    for panel in PANELS:
        frequencies, clean = periodogram(raw[panel])
        arrays["f"] = _array(frequencies)
        arrays[f"S_clean_{panel}"] = _array(clean)
    for panel, series in plotted.items():
        arrays[f"S_noisy_{panel}"] = _array(periodogram(series)[1])
    for arm in ARMS:
        arrays[arm] = _array(ns[arm])
    frequencies = arrays["f"]
    fundamental = float(cast(float, oscillators["b"]["f"]))
    comb: dict[str, float] = {}
    for k in range(1, 5):  # panel b's comb, µrad/√Hz at the bin nearest each harmonic
        nearest = int(np.argmin(np.abs(frequencies - k * fundamental)))
        comb[f"{k}f0"] = float(np.sqrt(arrays["S_clean_b"][nearest])) * 1e6
    _write(out, case, arrays, {
        "instrument":  {**_constants(ns, (*_INSTRUMENT, "u0", "v_kick", "T_cyc", "delta_r", "f_N")), "ifo_heights": [0.0, _number(ns, "delta_r")]},
        "oscillators": oscillators,
        "uldm":        _constants(ns, ("f_phi", "m_phi", "shot_noise_asd")),
        "panels":      sizes,
        "comb_b_urad_per_rthz": comb,
        "periodogram": "the script's own: (T_cyc/N)·|rfft(y − mean)|², DC dropped",
    })


def folding_map(clone: Path, out: Path) -> None:
    """``fig2_folding_map.py``: its ``fold_frequency`` on a grid from 0 to five times Nyquist."""
    case = Case("fig2_folding_map.py", "fig2_folding_map", {"T_total": "1e3"}, 5.0)
    ns   = _load(clone, case)
    fold = cast(Callable[[float, float], float], ns["fold_frequency"])
    nyquist = _number(ns, "f_N")
    true    = np.linspace(0.0, 5.0 * nyquist, 2001)
    seen    = np.array([fold(float(f), nyquist) for f in true], dtype = np.float64)
    _write(out, case, {"f_true": true, "f_obs": seen}, {"f_N": nyquist})


# ======================================== MAIN ========================================


def main(arguments: list[str]) -> int:
    """Runs every case. ``arguments`` is the clone's directory and the fixtures' directory."""
    if len(arguments) != 2:
        print(__doc__)
        return 2
    clone, out = Path(arguments[0]).resolve(), Path(arguments[1]).resolve()
    if clone in Path(__file__).resolve().parents or clone in Path.cwd().resolve().parents or clone == Path.cwd().resolve():
        raise SystemExit("run this from outside the clone, and keep the harness outside it")
    _silence()
    one_d_oscillator(clone, out)
    hundred_metre_oscillators(clone, out)
    figure_two(clone, out, Case("fig2_10m", "fig2_10m", {"T_total": "2e4"}, 2.0))
    figure_two(clone, out, Case("fig2_uldm_folding", "fig2_uldm_folding", {"T_total": "2e4"}, 5.0))
    folding_map(clone, out)
    total = sum(f.stat().st_size for f in out.rglob("*") if f.is_file() and f.suffix in {".npy", ".json"})
    print(f"{'total':<20} {total / 1e6:.2f} MB in {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
