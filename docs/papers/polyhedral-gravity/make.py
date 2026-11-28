"""Regenerate every number, table and figure of the paper, then the PDF.

    python make.py all                 # everything except the slow reruns below
    python make.py conv                # Cavendish reads the benchmark (oracle/tests/benchmark.rs; minutes)
    python make.py level               # the research benchmark, tower beside each body (~10 min)
    python make.py realread-rerun      # rerun the real-read notes' analysis and figure steps
    python make.py bench               # the leading-read benchmark (~40 min, below-normal priority)
    python make.py esa <venv-python>   # ESA fixtures (GPL tool, own venv), then crosscheck + mp
    python make.py shapes-study        # rerun the shapes study's own scripts (hours; downloads)
    python make.py <target> ...        # tables | farfield | nearedge | forty | crosscheck | mp | omitted |
                                       # scenes | render | diagrams | shapes | realread | figures |
                                       # engine | pdf

TASK-S5-18 moved the kernel's figures (far field, near edge), the 40-digit comparison, the
engine's table and the benchmark (Table "phase error at h = 1/64 m", Figure "against pitch", the
static numbers) onto Cavendish's own tests, which write data/cavendish/ (README.md lists each
command); the research scripts stay as the independent cross-check (Appendix B). The rows README.md
still marks provisional come from the research scripts. Every phase is the flight-integral read
(docs/research/paper-real-read.md) unless the paper says "leading read".
"""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parents[2]
BENCH = REPO / "docs" / "research" / "exact-skin-and-partial-fill"
DATA = HERE / "data"
FIG = HERE / "figures"
TEX = HERE / "generated"
BODIES = ("cube_aligned", "cube", "icosahedron", "plate", "sphere")


def low_priority() -> dict[str, object]:
    """Run children below normal priority (the owner uses this PC)."""
    if os.name == "nt":
        return {"creationflags": 0x00004000}  # BELOW_NORMAL_PRIORITY_CLASS
    return {"preexec_fn": lambda: os.nice(10)}


def run(args: list[str], out: pathlib.Path | None = None, cwd: pathlib.Path | None = None) -> str:
    res = subprocess.run(args, capture_output=True, text=True, encoding="utf-8", cwd=cwd,
                         check=True, **low_priority())  # type: ignore[arg-type]
    if out is not None:
        out.write_text(res.stdout, encoding="utf-8")
    return res.stdout


# ------------------------------------------------------------------ data

def bench() -> None:
    for variant in ("", "edges"):
        for name in BODIES:
            suffix = "-edges" if variant else ""
            args = [sys.executable, str(BENCH / "skin_bench.py"), "run", name, "8,16,32,64", "4"]
            run(args + ([variant] if variant else []), DATA / f"{name}{suffix}.json")
            print("bench", name, variant or "triple")


def tables() -> None:
    """Draft 3's leading-read benchmark summaries (data/summary*.md), which §5 cites for the
    triple-product numerator's effect on the 10 m rows; Table 4 itself is `level` + `realread`."""
    for suffix in ("", "-edges"):
        paths = [str(DATA / f"{b}{suffix}.json") for b in BODIES]
        run([sys.executable, str(BENCH / "tables.py"), "summary", *paths],
            DATA / f"summary{suffix}.md")
        run([sys.executable, str(BENCH / "tables.py"), *paths], DATA / f"tables{suffix}.md")


CAV = DATA / "cavendish"  # what Cavendish's own tests write for the paper (TASK-S5-18)


def cargo(args: list[str]) -> str:
    """A Cavendish test run for the paper's data, below normal priority, from the repository root."""
    cargo_bin = shutil.which("cargo") or str(pathlib.Path.home() / ".cargo" / "bin" / "cargo")
    return run([cargo_bin, *args], cwd=REPO)


def farfield() -> None:
    """Cavendish's far-field diagnostic (cav-core, polyhedral/tests.rs) -> data/cavendish/farfield.txt."""
    cargo(["test", "-p", "cav-core", "--lib", "far_field_by_distance", "--", "--ignored"])


def nearedge() -> None:
    """Cavendish's near-edge diagnostic (cav-core, polyhedral/tests.rs) -> data/cavendish/nearedge.txt."""
    cargo(["test", "-p", "cav-core", "--lib", "near_edge_by_distance", "--", "--ignored"])


ORACLE = ["test", "--manifest-path", "oracle/Cargo.toml", "--target-dir", "target/oracle"]


def forty() -> None:
    """Cavendish against 40 digits (oracle/tests/polyhedral.rs) -> data/cavendish/forty-digits.json."""
    print(cargo([*ORACLE, "--test", "polyhedral", "forty_digits_for_the_paper", "--", "--ignored",
                 "--nocapture"]))


def conv() -> None:
    """The benchmark read by Cavendish (oracle/tests/benchmark.rs) -> data/cavendish/conv.json."""
    print(cargo([*ORACLE, "--test", "benchmark", "the_benchmark_for_the_paper", "--", "--ignored",
                 "--nocapture"]))


def engine() -> None:
    """Table "Cavendish's polyhedral kernel": the kernel's validation cases run alone, each writing its
    record to data/cavendish/engine/ as `cargo xtask validate` has it do, then scripts/engine_table.py."""
    records = CAV / "engine"
    shutil.rmtree(records, ignore_errors=True)
    records.mkdir(parents=True)
    os.environ["CAV_VALIDATION_DIR"] = str(records)
    try:
        cargo(["test", "-p", "cav-core", "--lib", "polyhedral::tests::validation_"])
    finally:
        del os.environ["CAV_VALIDATION_DIR"]
    print(run([sys.executable, str(HERE / "scripts" / "engine_table.py")]))


def esa(venv_python: str) -> None:
    run([venv_python, str(HERE / "scripts" / "esa_fixtures.py")])
    crosscheck()
    mp()


def crosscheck() -> None:
    print(run([sys.executable, str(HERE / "scripts" / "crosscheck.py")]))


def mp() -> None:
    run([sys.executable, str(HERE / "scripts" / "mp_reference.py")], DATA / "mp_reference.txt")


def omitted() -> None:
    """Sizes of the effects the first-order, instantaneous-kick model leaves out (paper §2, S3)."""
    run([sys.executable, str(HERE / "scripts" / "omitted.py")], DATA / "omitted.txt")


def scenes() -> None:
    run([sys.executable, str(HERE / "scripts" / "make_scenes.py")])


def render() -> None:
    """Cavendish draws the benchmark scene (the only figure Cavendish makes today)."""
    cav = shutil.which("cav") or str(REPO / "target" / "release" / "cav.exe")
    FIG.mkdir(exist_ok=True)
    run([cav, "render", str(HERE / "scenes" / "cube-1.5m.ron"), "--out",
         str(FIG / "scene-cube-1.5m.png"), "--view", "iso", "--style", "paper",
         "--size", "1200x900"])


# ------------------------------------------------------------------ figures

SERIES = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#6250d6", "#e34948"]
INK, INK2, GRID = "#0b0b0b", "#52514e", "#d9d8d4"
MARKERS = ["o", "s", "^", "D", "v", "P", "X"]


def style() -> None:
    import matplotlib as mpl

    mpl.rcParams.update({
        "font.family": "serif", "font.size": 8, "axes.edgecolor": INK2, "axes.labelcolor": INK,
        "xtick.color": INK2, "ytick.color": INK2, "axes.grid": True, "grid.color": GRID,
        "grid.linewidth": 0.5, "axes.spines.top": False, "axes.spines.right": False,
        "lines.linewidth": 1.4, "lines.markersize": 4, "legend.frameon": False,
        "savefig.bbox": "tight", "pdf.fonttype": 42,
    })


def parse_table(path: pathlib.Path) -> list[list[str]]:
    lines = path.read_text(encoding="utf-8").splitlines()
    return [ln.split() for ln in lines[1:] if ln.strip()]


def fig_farfield() -> None:
    import matplotlib.pyplot as plt

    rows = parse_table(CAV / "farfield.txt")
    ratio = [float(r[0]) for r in rows]
    cols = {"triple product $r_1\\cdot(r_2\\times r_3)$": 2, "$h_f\\,2A_f$ numerator": 4,
            "$h_f\\,2A_f$, naive $\\ln$ ratio": 6}
    fig, ax = plt.subplots(figsize=(3.3, 2.5))
    for k, (label, c) in enumerate(cols.items()):
        ys = [float(r[c]) for r in rows]
        ax.loglog(ratio, ys, marker=MARKERS[k], color=SERIES[k], label=label)
    eps = 2.2e-16
    ref = [eps * x**2 for x in ratio]
    ax.loglog(ratio, ref, color=INK2, linestyle="--", linewidth=0.8, label=r"$\varepsilon (R/s)^2$")
    ax.set_xlabel("distance over cube side, $R/s$")
    ax.set_ylabel("relative error of $g$")
    ax.legend(fontsize=6.5, loc="upper left")
    fig.savefig(FIG / "farfield.pdf")
    plt.close(fig)


def fig_nearedge() -> None:
    import matplotlib.pyplot as plt

    rows = parse_table(CAV / "nearedge.txt")
    d = [float(r[0]) for r in rows]
    naive = [float(r[2]) if r[2] != "inf" else float("nan") for r in rows]
    hk = [float(r[3]) or float("nan") for r in rows]  # 0: exact to the bit, not drawn on a log axis
    exact = [float(r[1]) for r in rows]
    where = [float(r[4]) for r in rows]  # |p|, metres: the point's distance from the origin
    fig, ax = plt.subplots(figsize=(3.3, 2.5))
    ax.loglog(d, naive, marker="o", color=SERIES[0], label="direct $a+b-\\ell$")
    ax.loglog(d, hk, marker="s", color=SERIES[1], label="$r_0^2/(r-\\ell_1)+r_0^2/(r+\\ell_2)$")
    # Conditioning of the input: the point is known to eps|p|, so L moves by (eps |p| / d) / L.
    # This is the floor no formula can beat.
    floor = [2.2e-16 * p / x / big_l for x, big_l, p in zip(d, exact, where)]
    ax.loglog(d, floor, color=INK2, linestyle="--", linewidth=0.8,
              label=r"input rounding $\varepsilon|p|/(dL)$")
    ax.set_xlabel("distance from the edge, $d$ (m)")
    ax.set_ylabel("relative error of $L_e$")
    ax.legend(fontsize=6.5, loc="upper right")
    fig.savefig(FIG / "nearedge.pdf")
    plt.close(fig)


def fig_crosscheck() -> None:
    import matplotlib.pyplot as plt

    # This form: Cavendish's Faceted (data/cavendish/forty-digits.json, `make.py forty`); ESA: its
    # tool's values as recorded (data/mp_reference.json). The same six bodies and 18 points.
    ref = json.loads((DATA / "mp_reference.json").read_text(encoding="utf-8"))
    cav = json.loads((CAV / "forty-digits.json").read_text(encoding="utf-8"))["bodies"]
    series: list[tuple[str, str, dict[str, list[tuple[float, float]]]]] = [
        ("this form", "o",
         {n: [(r, g) for r, g in zip(cav[n]["R"], cav[n]["g"]) if r <= 100.0 + 1e-9] for n in ref["bodies"]}),
        ("ESA (Tsoulis 2012)", "s",
         {n: [(r["R"], r["esa_g"]) for r in rows] for n, rows in ref["bodies"].items()}),
    ]
    fig, ax = plt.subplots(figsize=(3.3, 2.5))
    for k, (who, mk, bodies) in enumerate(series):
        for j, rows in enumerate(bodies.values()):
            rs = sorted({r for r, _ in rows})
            worst = [max(e for r, e in rows if abs(r - x) < 1e-9) for x in rs]
            ax.loglog(rs, worst, marker=mk, color=SERIES[k], alpha=0.75, linewidth=0.9,
                      label=who if j == 0 else None)
    ax.set_xlabel("distance from the body's centre (m)")
    ax.set_ylabel("relative error of $g$ (vs 40 digits)")
    ax.legend(fontsize=6.5, loc="upper left")
    fig.savefig(FIG / "crosscheck.pdf")
    plt.close(fig)


def diagrams() -> None:
    """The explanatory diagrams. (scripts/octree_diagrams.py draws the octree figures of draft 3,
    which draft 4 leaves to future work; it is kept for the paper that will carry them.)"""
    print(run([sys.executable, str(HERE / "scripts" / "diagrams.py")]))


REAL = REPO / "docs" / "research" / "paper-real-read"
FLIGHT = REPO / "docs" / "research" / "paper-flight-phase"


def level() -> None:
    """The benchmark with the tower beside each body, real and leading reads (scripts/conv_level.py;
    needs the real-read note's compiled kernels: `cargo build --release` in its kern/)."""
    script = str(HERE / "scripts" / "conv_level.py")
    run([sys.executable, script, "run", ",".join(BODIES), "0,1,2,3"])
    run([sys.executable, script, "table"])


def realread() -> None:
    """Draft 4's real-read tables and figures, from the notes' committed output and data/level/."""
    print(run([sys.executable, str(HERE / "scripts" / "realread.py")]))


def realread_rerun() -> None:
    """Rerun the analysis and figure steps of the real-read and flight-phase notes (their runs,
    hours, are in each note's docstrings), then realread."""
    for step in ("sweep", "conv", "regimes", "budget"):
        run([sys.executable, str(REAL / "analyse_real.py"), step], cwd=REAL)
    run([sys.executable, str(FLIGHT / "fig_paper.py")], cwd=FLIGHT)
    realread()


SHAPES_STUDY = BENCH / "shapes"
# The shapes study's own commands, in order (each one process; they download the meshes, pinned
# by SHA-256, into the study's .cache/, and take hours in all). See the study's note, §3.1-§3.4.
STUDY_RUNS = (
    [["shapes_lib.py"]]
    + [["sweep.py", "run", s] for s in ("sphere", "torus", "teapot", "suzanne", "bunny", "spot",
                                        "dragon", "armadillo", "buddha", "bracket")]
    + [["sweep.py", "facets"], ["sweep.py", "plot"], ["regimes.py", "distance"],
       ["regimes.py", "orientation"], ["regimes.py", "translation"], ["regimes.py", "spin"],
       ["regimes.py", "spinworld"], ["primitives.py"], ["exact.py", "moments"],
       ["exact.py", "distance"], ["exact.py", "passby"], ["exact.py", "plot"],
       ["figures.py", "torus@64", "bunny@64"]]  # 16 and 64 cells across their largest extent (2.8 m, 1 m)
)


def shapes() -> None:
    """The shapes study's figures and tables, drawn from its committed output in place."""
    print(run([sys.executable, str(HERE / "scripts" / "shapes.py")]))


def shapes_study() -> None:
    """Rerun the shapes study itself (hours; downloads), then redraw the paper's figures. The
    full-resolution meshes of exact.py need `fullres.py dump <name> closed` first (study §3.3)."""
    for args in STUDY_RUNS:
        print("shapes study:", " ".join(args), flush=True)
        run([sys.executable, str(SHAPES_STUDY / args[0]), *args[1:]], cwd=SHAPES_STUDY)
    shapes()


def figures() -> None:
    FIG.mkdir(exist_ok=True)
    style()
    fig_farfield()
    fig_nearedge()
    fig_crosscheck()


# ------------------------------------------------------------------ PDF

def pdf() -> None:
    local = pathlib.Path.home() / ".local" / "bin" / "tectonic.exe"
    tectonic = shutil.which("tectonic") or (str(local) if local.exists() else None)
    if tectonic:
        run([tectonic, "paper.tex"], cwd=HERE)
    else:
        for _ in range(3):
            run(["pdflatex", "-interaction=nonstopmode", "paper.tex"], cwd=HERE)
            if _ == 0:
                run(["bibtex", "paper"], cwd=HERE)


TARGETS = {"bench": bench, "tables": tables, "farfield": farfield, "nearedge": nearedge,
           "forty": forty, "conv": conv, "engine": engine,
           "crosscheck": crosscheck, "mp": mp, "omitted": omitted, "scenes": scenes, "render": render,
           "figures": figures, "diagrams": diagrams, "shapes": shapes, "shapes-study": shapes_study,
           "level": level, "realread": realread, "realread-rerun": realread_rerun, "pdf": pdf}


def main() -> None:
    args = sys.argv[1:] or ["all"]
    if args[0] == "esa":
        esa(args[1])
        return
    if args == ["all"]:
        args = ["tables", "farfield", "nearedge", "forty", "crosscheck", "mp", "omitted", "scenes",
                "diagrams", "shapes", "figures", "engine", "realread", "pdf"]
    for t in args:
        print("==", t)
        TARGETS[t]()


if __name__ == "__main__":
    main()
