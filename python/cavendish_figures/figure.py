"""
A figure Cavendish exported, read back from its ``.json`` and its data file.

    fig.json + fig.csv | fig.npy -> load(path) -> Ok(Figure)

A figure ``fig.pdf`` written by ``cav figure``, the console's ``figure`` line or the MCP tool has
its data (``fig.csv``, or ``fig.npy`` when it is long) and ``fig.json`` beside it: what was
plotted, the axes with their units and scales, which columns each series is, the marks that are
not data, and where it all came from (the scene, the engine's revision, the numerics, the
command). ``load`` reads the two files and checks them against each other.

    from cavendish_figures import load

    figure = load("periodogram-paper-column.json").unwrap()
    print(figure.x.unit, figure.series[0].y.max())
"""

from __future__ import annotations

import csv
import json
from dataclasses import dataclass
from pathlib import Path
from typing import cast

import numpy as np
import numpy.typing as npt

from .errors import DataMismatch, FigureError, Missing, NotAFigure, NotDrawable, UnsupportedVersion
from .result import Err, Ok, Result

FIGURE_VERSION: int = 1
"""The version of the ``.json`` layout this module reads."""

Column = npt.NDArray[np.float64]
Flags  = npt.NDArray[np.bool_]

# ========================================= THE FIGURE =========================================


@dataclass(frozen = True)
class Axis:
    """
    One axis of a figure.

    Attributes:
        name  (str):                 What is along it.
        unit  (str):                 The unit of the data along it.
        log   (bool):                Whether it is logarithmic.
        range (tuple[float, float]): The values at its two ends.
    """

    name:  str
    unit:  str
    log:   bool
    range: tuple[float, float]


@dataclass(frozen = True)
class Series:
    """
    One series of a figure, with its data.

    Attributes:
        name   (str):    What it is: a tower's label, a spectrum.
        colour (str):    The colour Cavendish drew it in, ``#rrggbb``.
        x      (Column): Its horizontal values, shape ``[N]``, in the x axis's unit.
        y      (Column): Its vertical values, shape ``[N]``, in the y axis's unit.
        taken  (Flags):  Which of its values are measurements, shape ``[N]``. A signal's shot
                         that the tower did not take (a planned gap, a lost shot) keeps its slot
                         in the file, holding 0, and is ``False`` here; every other series is
                         ``True`` throughout.
    """

    name:   str
    colour: str
    x:      Column
    y:      Column
    taken:  Flags


@dataclass(frozen = True)
class Figure:
    """
    A figure as its ``.json`` and its data file describe it.

    Attributes:
        path       (Path):               The ``.json`` it was read from.
        what       (str):                ``signal``, ``periodogram``, ``cadence`` or ``flight``.
        title      (str):                The figure's title.
        preset     (str):                The preset it was written in.
        size_mm    (tuple[float, float]): Its width and height, millimetres.
        x          (Axis):               The horizontal axis.
        y          (Axis):               The vertical axis.
        series     (tuple[Series, ...]): The series, in the order of the file.
        columns    (dict[str, Column]):  Every column of the data file by name, each ``[N]``.
        marks      (dict[str, object]):  What is drawn that is not a series, as the ``.json`` has it.
        provenance (dict[str, object]):  The whole ``.json``: scene, engine, numerics, command.
    """

    path:       Path
    what:       str
    title:      str
    preset:     str
    size_mm:    tuple[float, float]
    x:          Axis
    y:          Axis
    series:     tuple[Series, ...]
    columns:    dict[str, Column]
    marks:      dict[str, object]
    provenance: dict[str, object]


# ========================================== READING ==========================================


def table(node: object) -> dict[str, object]:
    """``node`` as a JSON object; empty when it is not one."""
    return cast(dict[str, object], node) if isinstance(node, dict) else {}


def rows_of(node: object) -> list[object]:
    """``node`` as a JSON array; empty when it is not one."""
    return cast(list[object], node) if isinstance(node, list) else []


def number(node: object) -> float | None:
    """``node`` as a number, if it is one (and not a truth value)."""
    if isinstance(node, bool) or not isinstance(node, (int, float)):
        return None
    return float(node)


def _pair(node: object) -> tuple[float, float] | None:
    """``node`` as two numbers, if it is that."""
    values = [number(item) for item in rows_of(node)]
    if len(values) != 2 or values[0] is None or values[1] is None:
        return None
    return (values[0], values[1])


def _axis(node: object) -> Axis | None:
    """An axis from its JSON, if every field is there."""
    fields = table(node)
    name, unit, scale = fields.get("name"), fields.get("unit"), fields.get("scale")
    ends              = _pair(fields.get("range"))
    if not isinstance(name, str) or not isinstance(unit, str) or scale not in ("linear", "log") or ends is None:
        return None
    return Axis(name = name, unit = unit, log = scale == "log", range = ends)


def _read_csv(path: Path, names: list[str]) -> Result[dict[str, Column], FigureError]:
    """
    Reads a figure's ``.csv``: a header of ``name [unit]``, then numbers.

    Args:
        path  (Path):      The file.
        names (list[str]): The columns the ``.json`` lists, in order.
    Returns:
        Result[dict[str, Column], FigureError]: Each column by name, ``[N]``; ``DataMismatch``
            when the header is not those columns or a field is not a number.
    """
    with path.open(encoding = "utf-8", newline = "") as file:
        rows = list(csv.reader(file))
    header = [field.rsplit(" [", 1)[0] for field in rows[0]] if rows else []
    if header != names:
        return Err(DataMismatch(path = path, expected = f"columns {names}", found = f"columns {header}"))
    try:
        values = np.array([[float(field) for field in row] for row in rows[1:]], dtype = np.float64)
    except ValueError as why:
        return Err(DataMismatch(path = path, expected = "numbers", found = str(why)))
    if values.shape != (len(rows) - 1, len(names)):
        found = f"{values.shape} values under {len(names)} columns"
        return Err(DataMismatch(path = path, expected = "a number under every column", found = found))
    return Ok({name: values[:, n] for n, name in enumerate(names)})


def _read_npy(path: Path, names: list[str]) -> Result[dict[str, Column], FigureError]:
    """
    Reads a figure's ``.npy``: one ``[N, C]`` array of ``float64``.

    Args:
        path  (Path):      The file.
        names (list[str]): The columns the ``.json`` lists, in order.
    Returns:
        Result[dict[str, Column], FigureError]: Each column by name, ``[N]``; ``DataMismatch``
            when the array is not ``[N, len(names)]`` of ``float64``.
    """
    try:
        values = cast(npt.NDArray[np.generic], np.load(path, allow_pickle = False))
    except (OSError, ValueError) as why:
        return Err(DataMismatch(path = path, expected = "a .npy array", found = str(why)))
    if values.ndim != 2 or values.shape[1] != len(names) or values.dtype != np.float64:
        found = f"shape {values.shape}, dtype {values.dtype}"
        return Err(DataMismatch(path = path, expected = f"[N, {len(names)}] of float64", found = found))
    grid = cast(Column, values)
    return Ok({name: grid[:, n] for n, name in enumerate(names)})


def load(path: str | Path) -> Result[Figure, FigureError]:
    """
    Reads a figure from its ``.json`` and the data file beside it.

    Any of the figure's files may be named (``fig.pdf``, ``fig.csv``, ``fig.json``): the
    ``.json`` of the same name is what is read. Every value comes back as the ``float64``
    Cavendish wrote: the text of a ``.csv`` is the shortest that reads back to the same number.

    Args:
        path (str | Path): The figure's ``.json``, or any file of the figure.
    Returns:
        Result[Figure, FigureError]: The figure; ``Missing`` when the ``.json`` or the data file
            is not there, ``NotAFigure`` when the ``.json`` is not a figure's (with the field at
            fault), ``UnsupportedVersion``, ``NotDrawable`` for a view of the scene (a picture,
            with no data), ``DataMismatch`` when the data file is not what the ``.json`` says.
    """
    source = Path(path).with_suffix(".json")
    if not source.is_file():
        return Err(Missing(path = source))
    try:
        raw: object = json.loads(source.read_text(encoding = "utf-8"))
    except ValueError as why:
        return Err(NotAFigure(path = source, why = f"not JSON: {why}"))
    top     = table(raw)
    version = top.get("cavendish_figure")
    if not isinstance(version, int):
        return Err(NotAFigure(path = source, why = "no `cavendish_figure` version: not a figure's .json"))
    if version != FIGURE_VERSION:
        return Err(UnsupportedVersion(path = source, found = version, supported = FIGURE_VERSION))
    what, title, preset = top.get("what"), top.get("title"), top.get("preset")
    size = _pair(top.get("size_mm"))
    if not isinstance(what, str) or not isinstance(title, str) or not isinstance(preset, str) or size is None:
        return Err(NotAFigure(path = source, why = "`what`, `title`, `preset` or `size_mm` is missing"))
    if what == "view":
        return Err(NotDrawable(what = what))
    axes = table(top.get("axes"))
    x, y = _axis(axes.get("x")), _axis(axes.get("y"))
    if x is None or y is None:
        return Err(NotAFigure(path = source, why = "`axes.x` or `axes.y` is missing or incomplete"))
    data  = table(top.get("data"))
    names = [name for column in rows_of(data.get("columns")) if isinstance(name := table(column).get("name"), str)]
    file, layout = data.get("file"), data.get("format")
    if not isinstance(file, str) or layout not in ("csv", "npy") or not names:
        return Err(NotAFigure(path = source, why = "`data.file`, `data.format` or `data.columns` is missing"))
    beside = source.with_name(file)
    if not beside.is_file():
        return Err(Missing(path = beside))
    columns = _read_csv(beside, names) if layout == "csv" else _read_npy(beside, names)
    if isinstance(columns, Err):
        return columns
    rows = data.get("rows")
    held = len(next(iter(columns.value.values())))
    if rows != held:
        return Err(DataMismatch(path = beside, expected = f"{rows} rows", found = f"{held} rows"))
    series: list[Series] = []
    for node in rows_of(top.get("series")):
        entry = table(node)
        name, colour, across, up = entry.get("name"), entry.get("colour"), entry.get("x"), entry.get("y")
        if not isinstance(name, str) or not isinstance(colour, str):
            return Err(NotAFigure(path = source, why = "a series has no `name` or no `colour`"))
        if not isinstance(across, str) or not isinstance(up, str):
            continue
        if across not in columns.value or up not in columns.value:
            return Err(NotAFigure(path = source, why = f"series `{name}` names columns the data does not have"))
        # A signal's series names the column that says which shots were taken (figures written
        # before it did have the column all the same, under the tower's name).
        at    = entry.get("taken_column")
        flags = names[at] if isinstance(at, int) and not isinstance(at, bool) and 0 <= at < len(names) else f"shot taken by {name}"
        taken = columns.value[flags] != 0.0 if flags in columns.value else np.ones(held, dtype = np.bool_)
        series.append(Series(name = name, colour = colour, x = columns.value[across], y = columns.value[up], taken = taken))
    return Ok(Figure(
        path       = source,
        what       = what,
        title      = title,
        preset     = preset,
        size_mm    = size,
        x          = x,
        y          = y,
        series     = tuple(series),
        columns    = columns.value,
        marks      = table(top.get("marks")),
        provenance = top,
    ))
