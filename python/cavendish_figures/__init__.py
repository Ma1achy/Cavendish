"""
Redraw the figures Cavendish exports, from their data, with matplotlib and the same house style.

    cav figure comb periodogram --out fig.pdf    ->  fig.pdf, fig.csv, fig.json
    load("fig.pdf") -> Ok(Figure) -> redraw(figure, "again.png") -> Ok(Path)

    from cavendish_figures import load, redraw

    figure = load("fig.pdf").unwrap()
    print(figure.what, figure.x.unit, [series.name for series in figure.series])
    redraw(figure, "again.png").unwrap()

Everything that can fail for a reason outside the program returns a ``Result``; the errors are
frozen dataclasses with a stable ``code`` (``errors.error_catalog()`` lists them). The style
sheets can be used on their own: ``sheets("paper-column")`` gives their paths.
"""

from __future__ import annotations

from .errors import (
    DataMismatch,
    Error,
    FigureError,
    Missing,
    NoFonts,
    NotAFigure,
    NotDrawable,
    NotWritten,
    UnknownPreset,
    UnsupportedVersion,
    error_catalog,
)
from .figure import FIGURE_VERSION, Axis, Colours, Figure, Series, load
from .redraw import draw, redraw
from .result import Err, Ok, Result, UnwrapError
from .style import DASHES, PRESETS, SERIES, dash, sheets

__all__ = [
    "DASHES",
    "FIGURE_VERSION",
    "PRESETS",
    "SERIES",
    "Axis",
    "Colours",
    "DataMismatch",
    "Err",
    "Error",
    "Figure",
    "FigureError",
    "Missing",
    "NoFonts",
    "NotAFigure",
    "NotDrawable",
    "NotWritten",
    "Ok",
    "Result",
    "Series",
    "UnknownPreset",
    "UnsupportedVersion",
    "UnwrapError",
    "dash",
    "draw",
    "error_catalog",
    "load",
    "redraw",
    "sheets",
]
