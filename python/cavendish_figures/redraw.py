"""
Draw a figure Cavendish exported again with matplotlib, in the same house style.

    load(path) -> Ok(Figure) -> redraw(figure, out) -> Ok(out)

``redraw`` draws what the figure's files say: the series, logarithmic axes where the figure had
them, the shot-noise floors, the stems of each body's comb (dashed where a harmonic is folded)
and the diamond at the ULDM benchmark. The diamond and the floors are reference marks of the
plan, not simulated signals; the ``.json`` says so and so does the legend here.

    from cavendish_figures import load, redraw

    figure = load("periodogram-paper-column.json").unwrap()
    redraw(figure, "redrawn.png").unwrap()

Nothing here opens a window: the drawing is made on matplotlib's ``Figure`` directly and saved
through the Agg, PDF or SVG back end, by the extension of ``out``.
"""

# matplotlib's own stubs leave every ``**kwargs`` untyped, so each call of a drawing method is
# "partially unknown" to pyright's strict mode. That one report is turned off here, in the one
# module that calls matplotlib, and nowhere else; everything else stays strict.
# pyright: reportUnknownMemberType=false

from __future__ import annotations

from pathlib import Path

import matplotlib.style
import numpy as np
from matplotlib.axes import Axes
from matplotlib.figure import Figure as Canvas

from .errors import FigureError, NotDrawable, NotWritten
from .figure import Figure, number, rows_of, table
from .result import Err, Ok, Result
from .style import INK, PRESETS, REFERENCE, SERIES, SPECTRUM, dash, sheets

# ========================================== DRAWING ==========================================

_SUBMULTIPLES: dict[str, float] = {"rad": 1.0, "mrad": 1e3, "µrad": 1e6, "nrad": 1e9}

_SHOTS_MARKED: int = 200
"""The most shots of a series that each get a marker (Cavendish's ``MAX_SHOTS_MARKED``)."""


def _signal(axes: Axes, figure: Figure, width: float) -> None:
    """
    Every tower's series, in the palette's colours and dashes, in the unit the figure used.

    A shot the tower did not take is not a measurement: its value is masked, so the line is
    broken there and no marker is drawn; with few shots a small hollow square at the foot of
    the axes marks the time it would have been read out.
    """
    unit  = figure.marks.get("vertical_axis_written_in")
    unit  = unit if isinstance(unit, str) and unit in _SUBMULTIPLES else figure.y.unit
    scale = _SUBMULTIPLES.get(unit, 1.0)
    foot  = figure.y.range[0] * scale
    for n, series in enumerate(figure.series):
        few    = len(series.x) <= _SHOTS_MARKED
        colour = SERIES[n % len(SERIES)]
        axes.plot(
            series.x, np.where(series.taken, series.y * scale, np.nan),
            color = colour, linestyle = dash(n, width), label = series.name,
            marker = "o" if few else "",
        )
        lost = series.x[~series.taken]
        lost = lost[(lost >= figure.x.range[0]) & (lost <= figure.x.range[1])]
        if few and len(lost) > 0:
            axes.plot(
                lost, np.full(len(lost), foot),
                color = colour, linestyle = "none", marker = "s", markerfacecolor = "none",
                markeredgewidth = width * 0.6, clip_on = False,
            )
    axes.set_ylim(figure.y.range[0] * scale, figure.y.range[1] * scale)
    axes.set_ylabel(f"{figure.y.name}, {unit}")
    if len(figure.series) > 1:
        axes.legend(loc = "lower left", bbox_to_anchor = (0.0, 1.0), ncols = len(figure.series), borderaxespad = 0.2, fontsize = "small")


def _stem(axes: Axes, x: float, height: float, floor: float, colour: str, folded: bool, width: float) -> None:
    """One stem from the bottom of the axes to ``height``, dashed if folded, with a dot on top."""
    style = (0.0, (4.0 * width, 3.0 * width)) if folded else "solid"
    axes.plot([x, x], [floor, max(height, floor)], color = colour, linestyle = style, linewidth = width * 0.6)
    axes.plot([x], [max(height, floor)], color = colour, marker = "o", linestyle = "none", clip_on = False)


def _periodogram(axes: Axes, figure: Figure, width: float) -> None:
    """The spectrum in grey, the two floors, each body's comb as stems, the benchmark's diamond."""
    floor     = figure.y.range[0]
    reference = table(figure.marks.get("reference"))
    levels    = [level for item in rows_of(reference.get("shot_noise_floors_rad_per_sqrt_hz"))
                 if (level := number(item)) is not None]
    for n, level in enumerate(levels):
        label = "shot-noise floors, " + " and ".join(f"{value:g}" for value in levels) + " rad/√Hz"
        axes.axhline(level, color = REFERENCE, linewidth = width * 0.6, label = label if n == 0 else None)
    for series in figure.series:
        axes.plot(series.x, np.maximum(series.y, floor), color = SPECTRUM, linestyle = "solid")
    for n, node in enumerate(rows_of(figure.marks.get("combs"))):
        comb   = table(node)
        colour = SERIES[n % len(SERIES)]
        for item in rows_of(comb.get("stems")):
            stem             = table(item)
            appears, height  = number(stem.get("appears_at_hz")), number(stem.get("height"))
            if appears is not None and height is not None and figure.x.range[0] <= appears <= figure.x.range[1]:
                _stem(axes, appears, height, floor, colour, stem.get("folded") is True, width)
        body, fundamental = comb.get("body"), number(comb.get("fundamental_hz"))
        if isinstance(body, str) and fundamental is not None:
            axes.plot([], [], color = colour, label = f"{body} · f₀ = {fundamental:.4f} Hz")
    benchmark     = table(reference.get("uldm_benchmark"))
    at, height    = number(benchmark.get("frequency_hz")), number(benchmark.get("height_rad_per_sqrt_hz"))
    if at is not None and height is not None:
        axes.plot([at], [height], color = INK, marker = "D", linestyle = "none",
                  label = "ULDM benchmark (reference, not simulated)")
    axes.set_ylabel(f"{figure.y.name}, {figure.y.unit}")
    axes.legend(loc = "lower left", bbox_to_anchor = (0.0, 1.0), borderaxespad = 0.2, fontsize = "small")


def _cadence(axes: Axes, figure: Figure, width: float) -> None:
    """The window in grey, the predicted lines as stems and the predicted floor as a dashed level."""
    floor = figure.y.range[0]
    for series in figure.series:
        axes.plot(series.x, np.maximum(series.y, floor), color = SPECTRUM, linestyle = "solid",
                  label = "this seed's window")
    for n, item in enumerate(rows_of(figure.marks.get("predicted_lines"))):
        line        = table(item)
        at, height  = number(line.get("frequency_hz")), number(line.get("height"))
        if at is not None and height is not None:
            _stem(axes, at, height, floor, SERIES[0], False, width)
            if n == 0:
                axes.plot([], [], color = SERIES[0], label = "the lines the closed forms predict")
    level = number(figure.marks.get("predicted_floor"))
    if level is not None and level > 0.0:
        axes.axhline(max(level, floor), color = REFERENCE, linewidth = width * 0.6,
                     linestyle = (0.0, (5.0, 4.0)), label = "the floor they predict")
    axes.set_ylabel(figure.y.name)
    axes.legend(loc = "lower left", bbox_to_anchor = (0.0, 1.0), ncols = 3, borderaxespad = 0.2, fontsize = "small")


def _flight(axes: Axes, figure: Figure, width: float) -> None:
    """
    Each tower's shots as bars from launch to readout: the arms' arcs need the forward model.

    A shot the tower did not take is the bar's outline only.
    """
    tower, launch, readout = figure.columns.get("tower"), figure.columns.get("launch"), figure.columns.get("readout")
    if tower is None or launch is None or readout is None:
        return
    flags = figure.columns.get("taken")
    taken = flags != 0.0 if flags is not None else np.ones(len(tower), dtype = np.bool_)
    rows  = sorted({int(n) for n in tower})
    for n in rows:
        colour = SERIES[(n - 1) % len(SERIES)]
        for flown, face in ((taken, colour), (~taken, "none")):
            chosen = (tower == n) & flown
            spans  = [(float(a), float(b - a)) for a, b in zip(launch[chosen], readout[chosen])]
            if spans:
                axes.broken_barh(spans, (float(n) - 0.3, 0.6), facecolor = face, edgecolor = colour, linewidth = width * 0.6)
    axes.set_yticks([float(n) for n in rows], [f"tower {n}" for n in rows])
    axes.set_ylim(len(rows) + 0.6, 0.4)
    axes.set_ylabel("each bar a shot, launch to readout")


def redraw(figure: Figure, out: str | Path) -> Result[Path, FigureError]:
    """
    Draws a figure again with matplotlib, in the house style of its preset, and saves it.

    The size is the figure's own. A signal is every series in the palette's colours and dashes;
    a periodogram has a logarithmic axis, the two shot-noise floors, a stem for each harmonic of
    each comb (dashed where folded) and the diamond at the ULDM benchmark when the figure had
    it; a cadence window has its predicted lines and floor; a flight diagram is drawn as each
    shot's span, since the arms' arcs are the forward model's and not in the data. A figure in
    the ``screen`` preset is redrawn as ``paper-full``.

    Args:
        figure (Figure):     The figure, from ``load``.
        out    (str | Path): The file to write: ``.png``, ``.pdf`` or ``.svg``.
    Returns:
        Result[Path, FigureError]: The file written; ``NotDrawable`` for a kind this module
            does not draw, ``NotWritten`` when the file cannot be saved.
    """
    target = Path(out)
    drawn  = draw(figure)
    if isinstance(drawn, Err):
        return drawn
    style = sheets(_preset(figure))
    if isinstance(style, Err):
        return style
    # The preset's sheet gives the resolution a PNG is saved at.
    with matplotlib.style.context([str(sheet) for sheet in style.value]):
        try:
            drawn.value[0].savefig(target)
        except (OSError, ValueError) as why:
            return Err(NotWritten(path = target, why = str(why)))
    return Ok(target)


def _preset(figure: Figure) -> str:
    """The preset a figure is redrawn in: its own, or ``paper-full`` for one that has no sheet."""
    return figure.preset if figure.preset in PRESETS else "paper-full"


def draw(figure: Figure) -> Result[tuple[Canvas, Axes], FigureError]:
    """
    Draws a figure on a matplotlib ``Figure`` of its own, and saves nothing.

    For going on from the house style: add to the axes, then ``canvas.savefig(...)``.

    Args:
        figure (Figure): The figure, from ``load``.
    Returns:
        Result[tuple[Canvas, Axes], FigureError]: The matplotlib figure and its one axes;
            ``NotDrawable`` for a kind this module does not draw.
    """
    drawers = {"signal": _signal, "periodogram": _periodogram, "cadence": _cadence, "flight": _flight}
    drawer  = drawers.get(figure.what)
    if drawer is None:
        return Err(NotDrawable(what = figure.what))
    style = sheets(_preset(figure))
    if isinstance(style, Err):
        return style
    with matplotlib.style.context([str(sheet) for sheet in style.value]):
        width  = float(matplotlib.rcParams["lines.linewidth"])
        canvas = Canvas(figsize = (figure.size_mm[0] / 25.4, figure.size_mm[1] / 25.4), layout = "constrained")
        axes   = canvas.add_subplot()
        if figure.y.log:
            axes.set_yscale("log")
        axes.set_xlim(*figure.x.range)
        axes.set_ylim(*figure.y.range)
        axes.set_xlabel(f"{figure.x.name}, {figure.x.unit}")
        drawer(axes, figure, width)
        canvas.suptitle(figure.title, x = 0.01, ha = "left")
    return Ok((canvas, axes))
