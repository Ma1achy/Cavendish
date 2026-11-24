"""
Draw a figure Cavendish exported again with matplotlib: the same figure, from its record.

    load(path) -> Ok(Figure) -> redraw(figure, out) -> Ok(out)
                             -> redraw(figure, out, fonts = "fonts") -> Ok(out), in Cavendish's faces

``redraw`` draws what the figure's record says, not what this module would choose: the limits
and scales of its axes, the major marks at the record's ``ticks`` written as the record wrote
them (a power of ten with its exponent raised), the minor marks at its ``minor_ticks``, the axis
labels and the title as drawn (no title when it was written without one), and the series in the
record's own colours. A periodogram has its shot-noise floors, a stem for each harmonic of each
comb (dashed where folded) with the labels the figure wrote by them, and the diamond at the ULDM
benchmark with its label; a cadence window has its predicted lines, labelled with their heights,
and its predicted floor. The diamond and the floors are reference marks of the plan, not
simulated signals; the ``.json`` says so and so does the legend here.

    from cavendish_figures import load, redraw

    figure = load("periodogram-paper-column.json").unwrap()
    redraw(figure, "redrawn.png").unwrap()

The face is matplotlib's own (DejaVu Sans) unless a folder of fonts is given: ``cav fonts --out
fonts`` writes the two faces the binary carries, Ubuntu Light for text and Ubuntu Mono for the
numbers at the marks, with their licence beside them.

Nothing here opens a window: the drawing is made on matplotlib's ``Figure`` directly and saved
through the Agg, PDF or SVG back end, by the extension of ``out``.
"""

# matplotlib's own stubs leave every ``**kwargs`` untyped, so each call of a drawing method is
# "partially unknown" to pyright's strict mode. That one report is turned off here, in the one
# module that calls matplotlib, and nowhere else; everything else stays strict.
# pyright: reportUnknownMemberType=false

from __future__ import annotations

import re
from pathlib import Path

import matplotlib.style
import numpy as np
from matplotlib import font_manager
from matplotlib.axes import Axes
from matplotlib.axis import Axis as Ruler
from matplotlib.figure import Figure as Canvas
from matplotlib.ticker import FixedFormatter, FixedLocator, NullFormatter

from ._json import number, rows_of, table
from .errors import FigureError, NoFonts, NotDrawable, NotWritten
from .figure import Axis, Figure
from .result import Err, Ok, Result
from .style import PRESETS, dash, sheets

# ========================================== DRAWING ==========================================

_SUBMULTIPLES: dict[str, float] = {"rad": 1.0, "mrad": 1e3, "µrad": 1e6, "nrad": 1e9}

_SHOTS_MARKED: int = 200
"""The most shots of a series that each get a marker (Cavendish's ``MAX_SHOTS_MARKED``)."""

_SERIES: str = "series"
"""The ``gid`` of every artist that draws one of the record's series, so that they can be counted."""


def _multiple(figure: Figure) -> float:
    """
    How many of the unit a signal's vertical marks are written in make one of the data's unit.

    A signal's ``ΔΦ`` is in rad and may be written in mrad (``marks.vertical_axis_written_in``);
    every other figure is written in its data's unit, a multiple of 1.
    """
    unit = figure.marks.get("vertical_axis_written_in")
    return _SUBMULTIPLES.get(unit, 1.0) if isinstance(unit, str) else 1.0


def _label(axes: Axes, x: float, y: float, text: object, colour: str) -> None:
    """The text a figure wrote above a mark at ``(x, y)``, if it wrote one (``text`` is a string)."""
    if isinstance(text, str):
        axes.annotate(text, (x, y), xytext = (0.0, 2.5), textcoords = "offset points",
                      ha = "center", va = "bottom", color = colour, fontsize = "small")


def _signal(axes: Axes, figure: Figure, width: float) -> None:
    """
    Every tower's series, in the record's colours and the palette's dashes, in the unit the
    figure's marks are written in.

    A shot the tower did not take is not a measurement: its value is masked, so the line is
    broken there and no marker is drawn; with few shots a small hollow square at the foot of
    the axes marks the time it would have been read out.
    """
    multiple = _multiple(figure)
    palette  = figure.colours.series
    foot     = figure.y.range[0] * multiple
    for n, series in enumerate(figure.series):
        few    = len(series.x) <= _SHOTS_MARKED
        colour = palette[n % len(palette)]
        axes.plot(
            series.x, np.where(series.taken, series.y * multiple, np.nan),
            color = colour, linestyle = dash(n, width), label = series.name,
            marker = "o" if few else "", gid = _SERIES,
        )
        lost = series.x[~series.taken]
        lost = lost[(lost >= figure.x.range[0]) & (lost <= figure.x.range[1])]
        if few and len(lost) > 0:
            axes.plot(
                lost, np.full(len(lost), foot),
                color = colour, linestyle = "none", marker = "s", markerfacecolor = "none",
                markeredgewidth = width * 0.6, clip_on = False,
            )
    if len(figure.series) > 1:
        axes.legend(loc = "lower left", bbox_to_anchor = (0.0, 1.0), ncols = len(figure.series),
                    borderaxespad = 0.2, fontsize = "small")


def _stem(axes: Axes, x: float, top: float, floor: float, colour: str, folded: bool, width: float) -> None:
    """One stem from the bottom of the axes to ``top``, dashed if folded, with a dot on top."""
    style = (0.0, (4.0 * width, 3.0 * width)) if folded else "solid"
    axes.plot([x, x], [floor, top], color = colour, linestyle = style, linewidth = width * 0.6)
    axes.plot([x], [top], color = colour, marker = "o", linestyle = "none", clip_on = False)


def _periodogram(axes: Axes, figure: Figure, width: float) -> None:
    """
    The spectrum, the two floors, each body's comb as stems with the labels the figure wrote by
    them, and the benchmark's diamond with its label.
    """
    colours   = figure.colours
    floor     = figure.y.range[0]
    reference = table(figure.marks.get("reference"))
    levels    = [level for item in rows_of(reference.get("shot_noise_floors_rad_per_sqrt_hz"))
                 if (level := number(item)) is not None]
    for n, level in enumerate(levels):
        label = "shot-noise floors, " + " and ".join(f"{value:g}" for value in levels) + " rad/√Hz"
        axes.axhline(level, color = colours.reference, linewidth = width * 0.6, label = label if n == 0 else None)
    for series in figure.series:
        axes.plot(series.x, np.maximum(series.y, floor), color = colours.spectrum, linestyle = "solid", gid = _SERIES)
    for n, node in enumerate(rows_of(figure.marks.get("combs"))):
        comb   = table(node)
        own    = comb.get("colour")
        colour = own if isinstance(own, str) else colours.series[n % len(colours.series)]
        for item in rows_of(comb.get("stems")):
            stem            = table(item)
            appears, height = number(stem.get("appears_at_hz")), number(stem.get("height"))
            if appears is not None and height is not None and figure.x.range[0] <= appears <= figure.x.range[1]:
                _stem(axes, appears, max(height, floor), floor, colour, stem.get("folded") is True, width)
                _label(axes, appears, max(height, floor), stem.get("label"), colour)
        body, fundamental = comb.get("body"), number(comb.get("fundamental_hz"))
        if isinstance(body, str) and fundamental is not None:
            axes.plot([], [], color = colour, label = f"{body} · f₀ = {fundamental:.4f} Hz")
    benchmark  = table(reference.get("uldm_benchmark"))
    at, height = number(benchmark.get("frequency_hz")), number(benchmark.get("height_rad_per_sqrt_hz"))
    if at is not None and height is not None:
        axes.plot([at], [height], color = colours.ink, marker = "D", linestyle = "none",
                  label = "ULDM benchmark (reference, not simulated)")
        _label(axes, at, height, benchmark.get("label"), colours.ink)
    axes.legend(loc = "lower left", bbox_to_anchor = (0.0, 1.0), borderaxespad = 0.2, fontsize = "small")


def _cadence(axes: Axes, figure: Figure, width: float) -> None:
    """
    The window, the predicted lines as stems labelled with their heights, and the predicted
    floor as a dashed level.
    """
    colours = figure.colours
    floor   = figure.y.range[0]
    for series in figure.series:
        axes.plot(series.x, np.maximum(series.y, floor), color = colours.spectrum, linestyle = "solid",
                  label = "this seed's window", gid = _SERIES)
    for n, item in enumerate(rows_of(figure.marks.get("predicted_lines"))):
        line       = table(item)
        at, height = number(line.get("frequency_hz")), number(line.get("height"))
        if at is not None and height is not None:
            _stem(axes, at, max(height, floor), floor, colours.series[0], False, width)
            _label(axes, at, max(height, floor), line.get("label"), colours.series[0])
            if n == 0:
                axes.plot([], [], color = colours.series[0], label = "the lines the closed forms predict")
    level = number(figure.marks.get("predicted_floor"))
    if level is not None and level > 0.0:
        axes.axhline(max(level, floor), color = colours.reference, linewidth = width * 0.6,
                     linestyle = (0.0, (5.0, 4.0)), label = "the floor they predict")
    axes.legend(loc = "lower left", bbox_to_anchor = (0.0, 1.0), ncols = 3, borderaxespad = 0.2, fontsize = "small")


def _flight(axes: Axes, figure: Figure, width: float) -> None:
    """
    Each tower's shots as bars from launch to readout: the arms' arcs need the forward model.

    The vertical axis is therefore this drawing's own (a row for each tower), not the record's
    heights. A shot the tower did not take is the bar's outline only.
    """
    tower, launch, readout = figure.columns.get("tower"), figure.columns.get("launch"), figure.columns.get("readout")
    if tower is None or launch is None or readout is None:
        return
    flags   = figure.columns.get("taken")
    taken   = flags != 0.0 if flags is not None else np.ones(len(tower), dtype = np.bool_)
    rows    = sorted({int(n) for n in tower})
    palette = figure.colours.series
    for n in rows:
        colour = palette[(n - 1) % len(palette)]
        for flown, face, gid in ((taken, colour, _SERIES), (~taken, "none", None)):
            chosen = (tower == n) & flown
            spans  = [(float(a), float(b - a)) for a, b in zip(launch[chosen], readout[chosen])]
            if spans or gid is not None:
                axes.broken_barh(spans, (float(n) - 0.3, 0.6), facecolor = face, edgecolor = colour,
                                 linewidth = width * 0.6, gid = gid)
    axes.set_yticks([float(n) for n in rows], [f"tower {n}" for n in rows])
    axes.set_ylim(len(rows) + 0.6, 0.4)
    axes.set_ylabel("each bar a shot, launch to readout")


# =========================================== MARKS ===========================================

_POWER = re.compile(r"^(.*?)10\^([-−]?\d+)$")


def _written(text: str) -> str:
    """A mark's text as matplotlib writes it: ``10^-5`` becomes ten with a raised exponent."""
    power = _POWER.match(text)
    if power is None:
        return text
    lead, exponent = power.groups()
    return f"{lead}$\\mathdefault{{10^{{{exponent.replace('−', '-')}}}}}$"


def _marks(ruler: Ruler, axis: Axis, multiple: float) -> None:
    """
    The record's marks on one axis: major marks at ``ticks`` with ``tick_labels`` written by
    them, minor marks at ``minor_ticks`` with nothing written.

    Args:
        ruler    (Ruler):  matplotlib's axis to mark.
        axis     (Axis):   The record's axis.
        multiple (float):  How many of the unit the marks are drawn in make one of the data's.
    """
    ruler.set_major_locator(FixedLocator([tick * multiple for tick in axis.ticks]))
    ruler.set_major_formatter(FixedFormatter([_written(text) for text in axis.tick_labels]))
    ruler.set_minor_locator(FixedLocator([tick * multiple for tick in axis.minor_ticks]))
    ruler.set_minor_formatter(NullFormatter())


# =========================================== FACES ===========================================

_TEXT_FACE:    str = "Ubuntu-Light.ttf"
_NUMBERS_FACE: str = "UbuntuMono-Regular.ttf"


def _faces(folder: Path) -> Result[tuple[dict[str, object], str | None], FigureError]:
    """
    Registers every ``.ttf`` in a folder with matplotlib, and says how to draw in them.

    Args:
        folder (Path): A folder of fonts, as ``cav fonts --out <dir>`` writes it.
    Returns:
        Result[tuple[dict[str, object], str | None], FigureError]: The settings that draw text
            in Ubuntu Light, and the family of the face for numbers (``None`` when the folder
            has no Ubuntu Mono); ``NoFonts`` when the folder is not there, holds no ``.ttf``,
            holds no Ubuntu Light, or a file in it is not a font.
    """
    if not folder.is_dir():
        return Err(NoFonts(folder = folder, why = "it is not a folder"))
    found = sorted(folder.glob("*.ttf"))
    if not found:
        return Err(NoFonts(folder = folder, why = "it holds no .ttf file; `cav fonts --out <dir>` writes them"))
    if not any(file.name == _TEXT_FACE for file in found):
        return Err(NoFonts(folder = folder, why = f"it holds no {_TEXT_FACE}; `cav fonts --out <dir>` writes it"))
    manager = font_manager.fontManager
    for file in found:
        if not any(Path(entry.fname) == file for entry in manager.ttflist):
            try:
                manager.addfont(file)
            except (OSError, RuntimeError, ValueError) as why:
                return Err(NoFonts(folder = folder, why = f"{file.name} is not a font: {why}"))
    entries = {Path(entry.fname).name: entry for entry in manager.ttflist if Path(entry.fname).parent == folder}
    text    = entries[_TEXT_FACE]
    numbers = entries.get(_NUMBERS_FACE)
    weight  = text.weight
    rc: dict[str, object] = {
        "font.family":        [text.name],
        "font.weight":        weight,
        "axes.titleweight":   weight,
        "axes.labelweight":   weight,
        "figure.titleweight": weight,
    }
    return Ok((rc, numbers.name if numbers is not None else None))


def _style(
    figure: Figure,
    fonts:  str | Path | None,
) -> Result[tuple[list[str | dict[str, object]], str | None], FigureError]:
    """The style sheets and font settings a figure is drawn and saved in, and the face for numbers."""
    style = sheets(_preset(figure))
    if isinstance(style, Err):
        return style
    layers: list[str | dict[str, object]] = [str(sheet) for sheet in style.value]
    if fonts is None:
        return Ok((layers, None))
    faces = _faces(Path(fonts).resolve())
    if isinstance(faces, Err):
        return faces
    rc, numbers = faces.value
    return Ok(([*layers, rc], numbers))


# ========================================== REDRAWING ==========================================


def redraw(figure: Figure, out: str | Path, fonts: str | Path | None = None) -> Result[Path, FigureError]:
    """
    Draws a figure again with matplotlib, as its record says it was drawn, and saves it.

    The size is the figure's own; see ``draw`` for what is drawn. A figure in the ``screen``
    preset is redrawn as ``paper-full``.

    Args:
        figure (Figure):            The figure, from ``load``.
        out    (str | Path):        The file to write: ``.png``, ``.pdf`` or ``.svg``.
        fonts  (str | Path | None): A folder of fonts (``cav fonts --out <dir>`` writes one) to
                                    draw in Cavendish's faces; ``None`` for matplotlib's own.
    Returns:
        Result[Path, FigureError]: The file written; ``NotDrawable`` for a kind this module
            does not draw, ``NoFonts`` when the folder of fonts has none to draw with,
            ``NotWritten`` when the file cannot be saved.
    """
    target = Path(out)
    drawn  = draw(figure, fonts = fonts)
    if isinstance(drawn, Err):
        return drawn
    style = _style(figure, fonts)
    if isinstance(style, Err):
        return style
    # The preset's sheet gives the resolution a PNG is saved at; the fonts are those of the drawing.
    with matplotlib.style.context(style.value[0]):
        try:
            drawn.value[0].savefig(target)
        except (OSError, ValueError) as why:
            return Err(NotWritten(path = target, why = str(why)))
    return Ok(target)


def _preset(figure: Figure) -> str:
    """The preset a figure is redrawn in: its own, or ``paper-full`` for one that has no sheet."""
    return figure.preset if figure.preset in PRESETS else "paper-full"


def draw(figure: Figure, fonts: str | Path | None = None) -> Result[tuple[Canvas, Axes], FigureError]:
    """
    Draws a figure on a matplotlib ``Figure`` of its own, as its record says, and saves nothing.

    Each axis has the record's limits and scale, its major marks at the record's ``ticks``
    written as the record wrote them, its minor marks at ``minor_ticks`` and its label (none
    where the record's is ``None``); the title is the record's, or none. Every artist that draws
    one of the record's series has the ``gid`` ``"series"``. A flight diagram is drawn as each
    shot's span, since the arms' arcs are the forward model's and not in the data: its vertical
    axis is a row for each tower, not the record's heights.

    For going on from the house style: add to the axes, then ``canvas.savefig(...)``.

    Args:
        figure (Figure):            The figure, from ``load``.
        fonts  (str | Path | None): A folder of fonts to draw in Cavendish's faces (Ubuntu
                                    Light for text, Ubuntu Mono for the marks' numbers);
                                    ``None`` for matplotlib's own.
    Returns:
        Result[tuple[Canvas, Axes], FigureError]: The matplotlib figure and its one axes;
            ``NotDrawable`` for a kind this module does not draw, ``NoFonts`` when the folder
            of fonts has none to draw with.
    """
    drawers = {"signal": _signal, "periodogram": _periodogram, "cadence": _cadence, "flight": _flight}
    drawer  = drawers.get(figure.what)
    if drawer is None:
        return Err(NotDrawable(what = figure.what))
    style = _style(figure, fonts)
    if isinstance(style, Err):
        return style
    layers, numbers = style.value
    with matplotlib.style.context(layers):
        width    = float(matplotlib.rcParams["lines.linewidth"])
        canvas   = Canvas(figsize = (figure.size_mm[0] / 25.4, figure.size_mm[1] / 25.4), layout = "constrained")
        axes     = canvas.add_subplot()
        multiple = _multiple(figure)
        axes.set_xscale("log" if figure.x.log else "linear")
        axes.set_yscale("log" if figure.y.log else "linear")
        # The limits come before the drawing, so that nothing drawn moves them (the flight
        # diagram sets its own vertical ones).
        axes.set_xlim(*figure.x.range)
        axes.set_ylim(figure.y.range[0] * multiple, figure.y.range[1] * multiple)
        drawer(axes, figure, width)
        _marks(axes.xaxis, figure.x, 1.0)
        axes.set_xlabel(figure.x.label or "")
        if figure.what != "flight":
            _marks(axes.yaxis, figure.y, multiple)
            axes.set_ylabel(figure.y.label or "")
        if numbers is not None:
            axes.tick_params(labelfontfamily = numbers)
        if figure.title is not None:
            canvas.suptitle(figure.title, x = 0.01, ha = "left")
    return Ok((canvas, axes))
