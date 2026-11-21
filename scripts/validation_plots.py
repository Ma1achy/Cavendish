#!/usr/bin/env python3
# pyright: strict
"""Draw the validation report's plots, and the README's accuracy banner, as SVG.

    docs/validation/report.json  ──>  error-vs-tolerance-{light,dark}.svg   each case's error over its tolerance
    (and its history in git)          convergence-{light,dark}.svg          error against step, expected rate drawn in
                                      history-{light,dark}.svg              every case's margin at every report commit
                                      (--banner) one SVG: the summary line and the worst margin over time

`cargo xtask validate --report` writes the report and then runs this script for the plots; the progress
workflow runs it with `--banner` beside the grid. One axis serves every case whatever its unit: the
**margin**, the measured error as a fraction of the tolerance, on a log scale. A case passes to the left
of 1. Cases that measured zero sit in their own column, since zero has no logarithm. Only a case whose
tolerance is zero is called exact; a zero under a tolerance is a zero, and is labelled so.

The style is the README grid's (scripts/progress_banner.py): the same inks on a transparent ground, in a
light and a dark theme. Measured values are ink dots, the tolerance is the one ochre line, and anything
that is not a pass is also said in words, so nothing is told by colour alone.

Usage:
    python scripts/validation_plots.py --report docs/validation/report.json --out docs/validation
    python scripts/validation_plots.py --report docs/validation/report.json --banner banner.svg --theme dark

Needs Python 3.10 and git only.
"""

from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import cast
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parent.parent

# The README's inks, as scripts/progress_banner.py has them.
THEMES: dict[str, dict[str, str]] = {
    "light": {"ink": "#1d1b19", "ink2": "#3d3a34", "pencil": "#4f4a42", "ochre": "#b5822e"},
    "dark":  {"ink": "#efe9dc", "ink2": "#c8c1b2", "pencil": "#c8c1b2", "ochre": "#d9a84e"},
}
FONTS = "'Helvetica Neue',Helvetica,Arial,'Liberation Sans',Arimo,'Nimbus Sans','DejaVu Sans',sans-serif"

WIDTH       = 1600   # the README's width, so the banner lines up with the grid
LABEL_W     = 560    # the column of case names
NOTE_W      = 300    # the column of values, right of the plot
ROW_H       = 30
EXACT_W     = 90     # the column for cases that measured zero, left of the log axis
MIN_DECADE  = -6     # the log axis never starts above 10^-3 nor below this


# ========================================== THE REPORT ==========================================


@dataclass(frozen = True)
class Case:
    """One row of the report.

    Attributes:
        name      (str):          The case's name, ``subsystem.what``.
        subsystem (str):          The group it is listed under.
        status    (str):          ``pass``, ``fail`` or ``missing``.
        unit      (str):          The unit of ``measured`` and ``tolerance``.
        measured  (float | None): The measured error; ``None`` if missing or not finite.
        tolerance (float | None): The largest error that passes.
        margin    (float | None): ``measured / tolerance``; 0 when zero was measured, ``None`` if unknown or over.
        order     (tuple[float, float, float | None] | None): Expected order, allowed distance, measured order.
        points    (list[tuple[float, float]]): ``(step, error)`` pairs of a convergence case.
    """

    name:      str
    subsystem: str
    status:    str
    unit:      str
    measured:  float | None
    tolerance: float | None
    margin:    float | None
    order:     tuple[float, float, float | None] | None
    points:    list[tuple[float, float]]


@dataclass(frozen = True)
class Snapshot:
    """The report as it stood at one commit.

    Attributes:
        label (str):        What the x axis calls it: the commit's date and short hash, or ``now``.
        cases (list[Case]): Its rows.
    """

    label: str
    cases: list[Case]


def _number(value: object) -> float | None:
    """A JSON value as a finite float, or ``None``."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return float(value) if math.isfinite(value) else None


def _mapping(value: object) -> dict[str, object]:
    """A JSON value as an object; anything else as an empty one."""
    return cast("dict[str, object]", value) if isinstance(value, dict) else {}


def _items(value: object) -> list[object]:
    """A JSON value as an array; anything else as an empty one."""
    return cast("list[object]", value) if isinstance(value, list) else []


def parse(text: str) -> list[Case]:
    """Read a report.json.

    Args:
        text (str): The file's contents.

    Returns:
        list[Case]: The cases, in the file's order. A file that is not a report gives an empty list.
    """
    try:
        data: object = json.loads(text)
    except json.JSONDecodeError:
        return []
    rows: list[Case] = []
    for raw in map(_mapping, _items(_mapping(data).get("cases"))):
        order_raw = _mapping(raw.get("order"))
        expected, within = _number(order_raw.get("expected")), _number(order_raw.get("within"))
        order = None
        if expected is not None and within is not None:
            order = (expected, within, _number(order_raw.get("measured")))
        points: list[tuple[float, float]] = []
        for pair in map(_items, _items(raw.get("points"))):
            step, error = (_number(pair[0]), _number(pair[1])) if len(pair) == 2 else (None, None)
            # Only positive values have a place on a log-log plot.
            if step is not None and error is not None and step > 0 and error > 0:
                points.append((step, error))
        rows.append(Case(
            name      = str(raw.get("name", "")),
            subsystem = str(raw.get("subsystem", "")),
            status    = str(raw.get("status", "missing")),
            unit      = str(raw.get("unit", "")),
            measured  = _number(raw.get("measured")),
            tolerance = _number(raw.get("tolerance")),
            margin    = _number(raw.get("margin")),
            order     = order,
            points    = points,
        ))
    return rows


def _git(*args: str) -> str | None:
    """The output of a git command run in the repository, or ``None`` if it fails."""
    try:
        done = subprocess.run(["git", *args], cwd = ROOT, capture_output = True, text = True, encoding = "utf-8")
    except OSError:
        return None
    return done.stdout if done.returncode == 0 else None


def history(report: Path, current: list[Case]) -> list[Snapshot]:
    """Every version of the report in git, oldest first, then the one on disk if it differs.

    Args:
        report  (Path):       The report.json on disk.
        current (list[Case]): Its parsed rows.

    Returns:
        list[Snapshot]: At least one snapshot. Without git, or before the first commit of the
            report, it is the file on disk alone.
    """
    try:
        relative = report.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return [Snapshot(label = "now", cases = current)]
    snapshots: list[Snapshot] = []
    log = _git("log", "--reverse", "--format=%h %cs", "--", relative) or ""
    for line in log.splitlines():
        sha, _, date = line.partition(" ")
        text = _git("show", f"{sha}:{relative}")
        cases = parse(text) if text else []
        if cases:
            snapshots.append(Snapshot(label = f"{date[5:]} {sha}", cases = cases))
    if not snapshots or snapshots[-1].cases != current:
        snapshots.append(Snapshot(label = "now", cases = current))
    return snapshots


def summary(cases: list[Case]) -> str:
    """One line on the report: how many pass, and the least room to spare."""
    passing = sum(c.status == "pass" for c in cases)
    text = f"{passing} of {len(cases)} cases pass"
    for status, word in (("fail", "fail"), ("missing", "missing")):
        count = sum(c.status == status for c in cases)
        if count:
            text += f", {count} {word}"
    worst = _worst(cases)
    if worst is not None and worst.margin is not None:
        text += f" · worst margin {100 * worst.margin:.0f}% of tolerance ({worst.name})"
    return text


def _worst(cases: list[Case]) -> Case | None:
    """The case that ran with the largest margin, if any ran."""
    ran = [c for c in cases if c.margin is not None]
    return max(ran, key = lambda c: c.margin or 0.0) if ran else None


# =========================================== DRAWING ============================================


def _style(theme: str) -> str:
    """The stylesheet every plot shares."""
    c = THEMES[theme]
    return (
        f".t{{fill:{c['ink']};font-family:{FONTS}}}.m{{fill:{c['ink2']}}}.w{{fill:{c['pencil']}}}"
        f".axis{{stroke:{c['pencil']};stroke-opacity:0.45;stroke-width:1.5;fill:none}}"
        f".grid{{stroke:{c['pencil']};stroke-opacity:0.18;stroke-width:1}}"
        f".dot{{fill:{c['ink']}}}.ring{{fill:none;stroke:{c['ink']};stroke-width:2}}"
        f".line{{stroke:{c['ink']};stroke-width:2;fill:none;stroke-linejoin:round}}"
        f".thin{{stroke:{c['ink']};stroke-opacity:0.35;stroke-width:1.5;fill:none}}"
        f".tol{{stroke:{c['ochre']};stroke-width:2.5;fill:none}}.tolfill{{fill:{c['ochre']}}}"
        f".dash{{stroke:{c['ochre']};stroke-width:2;stroke-dasharray:6 5;fill:none}}"
    )


def _svg(width: int, height: int, theme: str, title: str, body: list[str]) -> str:
    """Wrap drawing instructions in an SVG document with the shared style and an accessible title."""
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}" '
        f'role="img" aria-label="{escape(title)}"><title>{escape(title)}</title><style>{_style(theme)}</style>'
        + "".join(body)
        + "</svg>\n"
    )


def _text(x: float, y: float, text: str, size: int = 18, cls: str = "t", anchor: str = "start", weight: int = 400) -> str:
    """One line of text."""
    return (
        f'<text class="{cls}" x="{x:.1f}" y="{y:.1f}" font-size="{size}" font-weight="{weight}" '
        f'text-anchor="{anchor}">{escape(text)}</text>'
    )


def _percent(margin: float) -> str:
    """A margin as a percentage: whole numbers from 10% up, two figures below."""
    return f"{100 * margin:.0f}%" if margin >= 0.1 else f"{100 * margin:.2g}%"


def _share(case: Case) -> str:
    """How a passing or failing case stands to its tolerance, in words.

    "exact" is kept for cases that had to be: a tolerance of zero, met. A zero measured under a
    tolerance that is not zero is "zero", which says less and is all that is known.
    """
    if case.margin is None:
        return "over"
    if case.margin == 0:
        return "exact" if case.tolerance == 0 else "zero measured"
    return _percent(case.margin)


def _shown(value: float | None) -> str:
    """A number as a label: short, exact for zero."""
    if value is None:
        return "–"
    if value == 0:
        return "0"
    return f"{value:.3g}"


@dataclass(frozen = True)
class MarginAxis:
    """The shared axis: a "zero" column, then margins from ``10**low`` to 10 on a log scale.

    Attributes:
        start (float): Where the axis begins.
        end   (float): Where it ends.
        low   (int):   The exponent of the smallest margin on the log part.
        along (bool):  ``True`` for a horizontal axis; ``False`` for a vertical one, which grows upwards.
    """

    start: float
    end:   float
    low:   int
    along: bool = True

    def at(self, margin: float | None) -> float:
        """The position of a margin. Zero goes to the zero column; an unknown or huge one to the far end."""
        span = self.end - self.start
        gap  = EXACT_W if self.along else 34.0
        sign = 1.0 if span > 0 else -1.0
        if margin == 0:
            return self.start + sign * gap * 0.5
        value    = 10.0 if margin is None else min(max(margin, 10.0 ** self.low), 10.0)
        fraction = (math.log10(value) - self.low) / (1 - self.low)
        return self.start + sign * gap + fraction * (span - sign * gap)

    def ticks(self) -> list[tuple[float, str]]:
        """Positions and labels of the decades, 1 being ``tolerance``."""
        return [(self.at(10.0 ** e), "tolerance" if e == 0 else f"10{_superscript(e)}") for e in range(self.low, 2)]


def _superscript(exponent: int) -> str:
    """An integer as superscript digits."""
    return str(exponent).translate(str.maketrans("-0123456789", "⁻⁰¹²³⁴⁵⁶⁷⁸⁹"))


def _low_decade(snapshots: list[Snapshot]) -> int:
    """The exponent the log axis starts at: a decade below the smallest positive margin, within bounds."""
    margins = [c.margin for s in snapshots for c in s.cases if c.margin]
    lowest  = math.floor(math.log10(min(margins))) - 1 if margins else -3
    return max(MIN_DECADE, min(lowest, -3))


def error_vs_tolerance(cases: list[Case], theme: str) -> str:
    """Each case's margin on one log axis, grouped by subsystem.

    Args:
        cases (list[Case]): The report's rows.
        theme (str):        ``light`` or ``dark``.

    Returns:
        str: The SVG.
    """
    axis   = MarginAxis(start = LABEL_W, end = WIDTH - NOTE_W, low = _low_decade([Snapshot("now", cases)]))
    groups = sorted({c.subsystem for c in cases})
    top    = 96
    height = top + ROW_H * (len(cases) + len(groups)) + 56
    body   = [
        _text(2, 30, "Measured error over tolerance", size = 26, weight = 500),
        _text(2, 58, summary(cases), size = 18, cls = "t m"),
    ]
    # The legend: a dot is a measurement, the ochre line is the tolerance.
    body.append(f'<circle class="dot" cx="{WIDTH - 420}" cy="24" r="6"/>' + _text(WIDTH - 406, 30, "measured", cls = "t m"))
    body.append(f'<line class="tol" x1="{WIDTH - 290}" y1="12" x2="{WIDTH - 290}" y2="36"/>' + _text(WIDTH - 278, 30, "tolerance: left of it passes", cls = "t m"))
    bottom = height - 50
    for x, label in axis.ticks():
        body.append(f'<line class="grid" x1="{x:.1f}" y1="{top - 8}" x2="{x:.1f}" y2="{bottom}"/>')
        body.append(_text(x, bottom + 24, label, size = 15, cls = "t w", anchor = "middle"))
    body.append(_text(axis.at(0), bottom + 24, "zero", size = 15, cls = "t w", anchor = "middle"))
    body.append(f'<line class="axis" x1="{axis.start}" y1="{bottom}" x2="{axis.end}" y2="{bottom}"/>')
    body.append(f'<line class="tol" x1="{axis.at(1):.1f}" y1="{top - 8}" x2="{axis.at(1):.1f}" y2="{bottom}"/>')

    y = float(top)
    for group in groups:
        body.append(_text(2, y + 20, group, size = 17, weight = 500))
        y += ROW_H
        rows = [c for c in cases if c.subsystem == group]
        for case in sorted(rows, key = lambda c: -(c.margin if c.margin is not None else math.inf)):
            middle = y + ROW_H / 2
            body.append(_text(26, middle + 6, case.name, size = 17, cls = "t m"))
            if case.status == "missing":
                note = "MISSING: declared, did not run"
            else:
                x = axis.at(case.margin)
                # The gap between the measurement and the tolerance is the room to spare.
                body.append(f'<line class="thin" x1="{x:.1f}" y1="{middle}" x2="{axis.at(1):.1f}" y2="{middle}"/>')
                mark = "dot" if case.status == "pass" else "ring"
                body.append(f'<circle class="{mark}" cx="{x:.1f}" cy="{middle}" r="6"/>')
                share = "over" if case.margin is None else _share(case)
                note  = f"{_shown(case.measured)} of {_shown(case.tolerance)} {case.unit} · {share}"
                if case.status != "pass":
                    note = "FAIL · " + note
            body.append(_text(axis.end + 16, middle + 6, note, size = 15, cls = "t m" if case.status == "pass" else "t", weight = 400 if case.status == "pass" else 600))
            y += ROW_H
    return _svg(WIDTH, height, theme, "Each validation case's measured error as a fraction of its tolerance, on a log scale", body)


def convergence(cases: list[Case], theme: str) -> str:
    """Error against step size for each convergence case, with the expected rate drawn through the data.

    Args:
        cases (list[Case]): The report's rows; those with points are drawn.
        theme (str):        ``light`` or ``dark``.

    Returns:
        str: The SVG. With no convergence cases it says so, rather than drawing an empty frame.
    """
    drawn = [c for c in cases if len(c.points) >= 2]
    body  = [_text(2, 30, "Convergence", size = 26, weight = 500)]
    if not drawn:
        body.append(_text(2, 62, "No case in the report measures an order of convergence yet.", cls = "t m"))
        return _svg(WIDTH, 84, theme, "Convergence: no cases yet", body)
    panel, gap, per_row = 360, 34, 4
    rows   = math.ceil(len(drawn) / per_row)
    height = 70 + rows * (panel + 70)
    body.append(_text(2, 58, "error against step, both on log scales · dashed: the expected rate through the finest point", cls = "t m"))
    for n, case in enumerate(drawn):
        left = 2 + (n % per_row) * (panel + gap) + 40
        top  = 90 + (n // per_row) * (panel + 70)
        size = panel - 60
        xs = [math.log10(h) for h, _ in case.points]
        ys = [math.log10(e) for _, e in case.points]
        x0, x1 = min(xs), max(xs)
        y0, y1 = min(ys) - 0.3, max(ys) + 0.3
        if case.order is not None:
            # Room for the expected rate, drawn from the finest point to the coarsest step.
            y1 = max(y1, min(zip(xs, ys))[1] + case.order[0] * (x1 - x0) + 0.3)
        def px(v: float, left: float = left, x0: float = x0, x1: float = x1) -> float:
            return left + (v - x0) / ((x1 - x0) or 1.0) * size

        def py(v: float, top: float = top, y0: float = y0, y1: float = y1) -> float:
            return top + size - (v - y0) / ((y1 - y0) or 1.0) * size

        body.append(f'<rect class="axis" x="{left}" y="{top}" width="{size}" height="{size}"/>')
        if case.order is not None:
            expected, _, measured = case.order
            fx, fy = min(zip(xs, ys))
            body.append(f'<line class="dash" x1="{px(x0):.1f}" y1="{py(fy + expected * (x0 - fx)):.1f}" x2="{px(x1):.1f}" y2="{py(fy + expected * (x1 - fx)):.1f}"/>')
            found = "–" if measured is None else f"{measured:.2f}"
            body.append(_text(left, top + size + 44, f"order {found}, expected {expected:g}", size = 15, cls = "t m"))
        path = " ".join(f"{px(x):.1f},{py(y):.1f}" for x, y in sorted(zip(xs, ys)))
        body.append(f'<polyline class="line" points="{path}"/>')
        body.extend(f'<circle class="dot" cx="{px(x):.1f}" cy="{py(y):.1f}" r="5"/>' for x, y in zip(xs, ys))
        body.append(_text(left, top - 10, case.name, size = 16))
        body.append(_text(left, top + size + 22, f"step 10{_superscript(round(x0))} to 10{_superscript(round(x1))}", size = 14, cls = "t w"))
    return _svg(WIDTH, height, theme, "Error against step size for each convergence case, with the expected rate", body)


def _history_lines(snapshots: list[Snapshot], axis: MarginAxis, left: float, right: float) -> list[str]:
    """Every case's margin across the snapshots as a thin line, the latest worst case in full ink."""
    step  = (right - left) / max(len(snapshots) - 1, 1)
    xs    = [left + n * step if len(snapshots) > 1 else (left + right) / 2 for n in range(len(snapshots))]
    worst = _worst(snapshots[-1].cases)
    body: list[str] = []
    names = sorted({c.name for s in snapshots for c in s.cases})
    for name in names:
        points: list[tuple[float, float]] = []
        for x, snapshot in zip(xs, snapshots):
            case = next((c for c in snapshot.cases if c.name == name), None)
            if case is not None and case.status != "missing":
                points.append((x, axis.at(case.margin)))
        if not points:
            continue
        lead = worst is not None and name == worst.name
        if len(points) > 1:
            path = " ".join(f"{x:.1f},{y:.1f}" for x, y in points)
            body.append(f'<polyline class="{"line" if lead else "thin"}" points="{path}"/>')
        radius = 6 if lead else 4
        body.extend(f'<circle class="dot" cx="{x:.1f}" cy="{y:.1f}" r="{radius}" fill-opacity="{1 if lead else 0.5}"/>' for x, y in points)
    return body


def history_plot(snapshots: list[Snapshot], theme: str) -> str:
    """Every case's margin at every commit that changed the report.

    Args:
        snapshots (list[Snapshot]): The report's versions, oldest first.
        theme     (str):            ``light`` or ``dark``.

    Returns:
        str: The SVG. The case with the least room to spare in the latest report is drawn in full ink and named.
    """
    top, bottom, left, right = 96, 470, 150, WIDTH - 420
    axis = MarginAxis(start = bottom, end = top, low = _low_decade(snapshots), along = False)
    body = [
        _text(2, 30, "Accuracy over time", size = 26, weight = 500),
        _text(2, 58, "each line is a case: its error as a fraction of its tolerance, at every commit that changed the report", cls = "t m"),
    ]
    for y, label in axis.ticks():
        body.append(f'<line class="grid" x1="{left - 10}" y1="{y:.1f}" x2="{right + 10}" y2="{y:.1f}"/>')
        body.append(_text(left - 18, y + 5, label, size = 15, cls = "t w", anchor = "end"))
    body.append(_text(left - 18, axis.at(0) + 5, "zero", size = 15, cls = "t w", anchor = "end"))
    body.append(f'<line class="tol" x1="{left - 10}" y1="{axis.at(1):.1f}" x2="{right + 10}" y2="{axis.at(1):.1f}"/>')
    body.extend(_history_lines(snapshots, axis, left + 30, right - 30))
    step = (right - left - 60) / max(len(snapshots) - 1, 1)
    every = max(1, math.ceil(len(snapshots) / 8))
    for n, snapshot in enumerate(snapshots):
        if n % every == 0 or n == len(snapshots) - 1:
            x = left + 30 + n * step if len(snapshots) > 1 else (left + right) / 2
            body.append(_text(x, bottom + 28, snapshot.label, size = 14, cls = "t w", anchor = "middle"))
    latest = snapshots[-1].cases
    worst  = _worst(latest)
    notes  = [f"{sum(c.status == 'pass' for c in latest)} of {len(latest)} pass in the latest report"]
    if worst is not None and worst.margin is not None:
        notes += ["least room to spare:", worst.name, f"{100 * worst.margin:.0f}% of its tolerance"]
    notes += [
        f"{sum(c.tolerance == 0 for c in latest)} cases must be exact (tolerance 0)",
        f"{sum(c.margin == 0 and c.tolerance != 0 for c in latest)} more measured zero",
    ]
    for n, note in enumerate(notes):
        body.append(_text(right + 40, top + 24 + 26 * n, note, size = 16, cls = "t m" if n != 2 else "t", weight = 500 if n == 2 else 400))
    return _svg(WIDTH, bottom + 50, theme, "Each validation case's error as a fraction of its tolerance over time", body)


def banner(snapshots: list[Snapshot], theme: str) -> str:
    """The README's line under the grid: the summary, and the cases' margins over time in small.

    Args:
        snapshots (list[Snapshot]): The report's versions, oldest first.
        theme     (str):            ``light`` or ``dark``.

    Returns:
        str: The SVG, as wide as the progress grid.
    """
    latest = snapshots[-1].cases
    height, left, right = 150, 1000, WIDTH - 110
    axis = MarginAxis(start = 118, end = 30, low = _low_decade(snapshots), along = False)
    body = [
        _text(2, 44, "Validation", size = 30, weight = 500),
        _text(2, 80, summary(latest), size = 20, cls = "t m"),
        _text(2, 112, "docs/validation/report.md: every case, what it is checked against, its error and its tolerance", size = 16, cls = "t w"),
        _text(left - 16, 22, "accuracy over time", size = 14, cls = "t w"),
        f'<line class="tol" x1="{left - 16}" y1="{axis.at(1):.1f}" x2="{right + 16}" y2="{axis.at(1):.1f}"/>',
        _text(right + 22, axis.at(1) + 5, "tolerance", size = 14, cls = "t w"),
        _text(right + 22, axis.at(0) + 5, "zero", size = 14, cls = "t w"),
        f'<line class="grid" x1="{left - 16}" y1="{axis.at(0):.1f}" x2="{right + 16}" y2="{axis.at(0):.1f}"/>',
    ]
    body.extend(_history_lines(snapshots, axis, left, right))
    return _svg(WIDTH, height, theme, f"Validation: {summary(latest)}", body)


# ============================================= MAIN =============================================


def main(argv: list[str] | None = None) -> int:
    """Draw the plots or the banner.

    Args:
        argv (list[str] | None): The command line; ``None`` for the process's own.

    Returns:
        int: 0 on success, 1 when the report cannot be read or holds no cases.
    """
    parser = argparse.ArgumentParser(description = (__doc__ or "").split("\n\n")[0])
    parser.add_argument("--report", default = str(ROOT / "docs/validation/report.json"), help = "the report.json to draw")
    parser.add_argument("--out", help = "a directory: write the three plots there, in both themes")
    parser.add_argument("--banner", help = "a file: write the README banner there ('-' for stdout)")
    parser.add_argument("--theme", choices = sorted(THEMES), default = "light", help = "the banner's theme")
    args = parser.parse_args(argv)

    report = Path(args.report)
    try:
        cases = parse(report.read_text(encoding = "utf-8"))
    except OSError as error:
        print(f"validation_plots: {report}: {error}", file = sys.stderr)
        return 1
    if not cases:
        print(f"validation_plots: {report} holds no cases", file = sys.stderr)
        return 1
    snapshots = history(report, cases)

    if args.out:
        out = Path(args.out)
        out.mkdir(parents = True, exist_ok = True)
        for theme in THEMES:
            plots = {
                "error-vs-tolerance": error_vs_tolerance(cases, theme),
                "convergence":        convergence(cases, theme),
                "history":            history_plot(snapshots, theme),
            }
            for name, svg in plots.items():
                (out / f"{name}-{theme}.svg").write_text(svg, encoding = "utf-8", newline = "\n")
    if args.banner:
        svg = banner(snapshots, args.theme)
        if args.banner == "-":
            sys.stdout.write(svg)
        else:
            Path(args.banner).write_text(svg, encoding = "utf-8", newline = "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
