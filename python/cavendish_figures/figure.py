"""
A figure Cavendish exported, read back from its record and its data file.

    fig.json + fig.csv | fig.npy -> load("fig.pdf") -> Ok(Figure)

A figure ``fig.pdf`` written by ``cav figure``, the console's ``figure`` line or the MCP tool has
its data (``fig.csv``, or ``fig.npy`` when it is long) and its record ``fig.json`` beside it. The
record is of the figure, whatever the format: what was plotted, of which tower, from which scene,
engine revision and numerics, in which data file; and, under ``files``, every picture written of
it (``fig.pdf``, ``fig.png``, ...), each with its own preset, size, title, axes, series, marks,
colours and command. ``load`` reads the record and the data and checks them against each other.

    from cavendish_figures import load

    figure = load("periodogram-paper-column.pdf").unwrap()
    print(figure.file, figure.x.unit, figure.series[0].y.max())
"""

from __future__ import annotations

import csv
import json
from dataclasses import dataclass
from pathlib import Path
from typing import cast

import numpy as np
import numpy.typing as npt

from ._json import number, pair, rows_of, table
from .errors import DataMismatch, FigureError, Missing, NotAFigure, NotDrawable, UnsupportedVersion
from .result import Err, Ok, Result

FIGURE_VERSION: int = 2
"""The version of the record's layout this module reads (2: one record for every format)."""

Column = npt.NDArray[np.float64]
Flags  = npt.NDArray[np.bool_]

# ========================================= THE FIGURE =========================================


@dataclass(frozen = True)
class Axis:
    """
    One axis of a figure, with the marks Cavendish drew on it.

    The marks' values are in the data's unit, even where the figure wrote them in a multiple of
    it: a signal's ``ΔΦ`` is in rad and its marks may be written in mrad (the record's
    ``marks.vertical_axis_written_in`` says so).

    Attributes:
        name        (str):                 What is along it.
        unit        (str):                 The unit of the data along it.
        log         (bool):                Whether it is logarithmic.
        range       (tuple[float, float]): The values at its two ends.
        label       (str | None):          Its label as drawn (``"√S, rad/√Hz"``); ``None`` when
                                           none was drawn.
        ticks       (tuple[float, ...]):   The values of its major marks.
        tick_labels (tuple[str, ...]):     The text written at each major mark, one for each of
                                           ``ticks``; a power of ten is written ``10^-5``.
        minor_ticks (tuple[float, ...]):   The values of its minor marks; empty on a linear axis
                                           and where there was no room.
    """

    name:        str
    unit:        str
    log:         bool
    range:       tuple[float, float]
    label:       str | None
    ticks:       tuple[float, ...]
    tick_labels: tuple[str, ...]
    minor_ticks: tuple[float, ...]


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
class Colours:
    """
    The colours Cavendish drew a picture in, each ``#rrggbb``: the one palette, read from the
    record, so that a redraw cannot part from it.

    Attributes:
        background (str):             Behind everything.
        ink        (str):             Titles and the benchmark's diamond.
        rule       (str):             The frame and the grid.
        spectrum   (str):             A spectrum's own line.
        reference  (str):             Reference levels: the shot-noise floors, a predicted floor.
        series     (tuple[str, ...]): The series colours, in order; a body's comb takes them too.
    """

    background: str
    ink:        str
    rule:       str
    spectrum:   str
    reference:  str
    series:     tuple[str, ...]


@dataclass(frozen = True)
class Figure:
    """
    One picture of a figure, as its record and its data file describe it.

    Attributes:
        path       (Path):                The record (``.json``) it was read from.
        file       (str):                 The picture's file name: ``fig.pdf``.
        files      (tuple[str, ...]):     Every picture the record lists of this figure.
        what       (str):                 ``signal``, ``periodogram``, ``cadence`` or ``flight``.
        title      (str | None):          The picture's title; ``None`` when it was written without.
        preset     (str):                 The preset it was written in.
        size_mm    (tuple[float, float]): Its width and height, millimetres.
        x          (Axis):                The horizontal axis.
        y          (Axis):                The vertical axis.
        series     (tuple[Series, ...]):  The series drawn, in the order of the record.
        columns    (dict[str, Column]):   Every column of the data file by name, each ``[N]``.
        marks      (dict[str, object]):   What is drawn that is not a series, as the record has it.
        colours    (Colours):             The colours it was drawn in.
        picture    (dict[str, object]):   The picture's whole entry in the record's ``files``.
        provenance (dict[str, object]):   The whole record: scene, engine, numerics, data, files.
    """

    path:       Path
    file:       str
    files:      tuple[str, ...]
    what:       str
    title:      str | None
    preset:     str
    size_mm:    tuple[float, float]
    x:          Axis
    y:          Axis
    series:     tuple[Series, ...]
    columns:    dict[str, Column]
    marks:      dict[str, object]
    colours:    Colours
    picture:    dict[str, object]
    provenance: dict[str, object]


# ========================================== READING ==========================================


def _colours(node: object) -> Colours | None:
    """A picture's colours from their JSON, if every one is there."""
    fields = table(node)
    plain  = [fields.get(name) for name in ("background", "ink", "rule", "spectrum", "reference")]
    series = rows_of(fields.get("series"))
    if not all(isinstance(colour, str) for colour in plain) or not series:
        return None
    if not all(isinstance(colour, str) for colour in series):
        return None
    background, ink, rule, spectrum, reference = (str(colour) for colour in plain)
    return Colours(
        background = background,
        ink        = ink,
        rule       = rule,
        spectrum   = spectrum,
        reference  = reference,
        series     = tuple(str(colour) for colour in series),
    )


def _numbers(node: object) -> tuple[float, ...] | None:
    """``node`` as an array of numbers, if it is that."""
    if not isinstance(node, list):
        return None
    values = [number(item) for item in rows_of(cast(object, node))]
    if any(value is None for value in values):
        return None
    return tuple(value for value in values if value is not None)


def _axis(node: object) -> Axis | None:
    """An axis from its JSON, if every field is there: its marks and its label too."""
    fields = table(node)
    name, unit, scale = fields.get("name"), fields.get("unit"), fields.get("scale")
    ends              = pair(fields.get("range"))
    if not isinstance(name, str) or not isinstance(unit, str) or scale not in ("linear", "log") or ends is None:
        return None
    label        = fields.get("label")
    ticks, minor = _numbers(fields.get("ticks")), _numbers(fields.get("minor_ticks"))
    written      = rows_of(fields.get("tick_labels"))
    if "label" not in fields or not (label is None or isinstance(label, str)) or ticks is None or minor is None:
        return None
    if len(written) != len(ticks) or not all(isinstance(text, str) for text in written):
        return None
    return Axis(
        name        = name,
        unit        = unit,
        log         = scale == "log",
        range       = ends,
        label       = label,
        ticks       = ticks,
        tick_labels = tuple(str(text) for text in written),
        minor_ticks = minor,
    )


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
    Reads one picture of a figure from the figure's record and the data file beside it.

    Any file of the figure may be named. A picture's name (``fig.pdf``) reads that picture's
    entry; the record's or the data file's (``fig.json``, ``fig.csv``) reads the first picture
    the record lists. Every value comes back as the ``float64`` Cavendish wrote: the text of a
    ``.csv`` is the shortest that reads back to the same number.

    Args:
        path (str | Path): A picture of the figure, or its ``.json`` or data file.
    Returns:
        Result[Figure, FigureError]: The figure; ``Missing`` when the record or the data file
            is not there, ``NotAFigure`` when the ``.json`` is not a figure's record or does not
            list the picture named (with the field at fault), ``UnsupportedVersion``,
            ``NotDrawable`` for a view of the scene (a picture, with no data), ``DataMismatch``
            when the data file is not what the record says.
    """
    named  = Path(path)
    source = named.with_suffix(".json")
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
    what = top.get("what")
    if not isinstance(what, str):
        return Err(NotAFigure(path = source, why = "`what` is missing"))
    if what == "view":
        return Err(NotDrawable(what = what))
    entries = [table(entry) for entry in rows_of(top.get("files"))]
    listed  = tuple(name for entry in entries if isinstance(name := entry.get("file"), str))
    if not listed or len(listed) != len(entries):
        return Err(NotAFigure(path = source, why = "`files` is missing, empty or has an entry with no `file`"))
    pictures = {".svg", ".pdf", ".png"}
    wanted   = named.name if named.suffix.lower() in pictures else listed[0]
    if wanted not in listed:
        return Err(NotAFigure(path = source, why = f"the record lists no `{wanted}`: it lists {', '.join(listed)}"))
    picture = entries[listed.index(wanted)]
    title, preset = picture.get("title"), picture.get("preset")
    size          = pair(picture.get("size_mm"))
    titled        = "title" in picture and (title is None or isinstance(title, str))
    if not titled or not isinstance(preset, str) or size is None:
        return Err(NotAFigure(path = source, why = f"`title`, `preset` or `size_mm` of `{wanted}` is missing"))
    axes = table(picture.get("axes"))
    x, y = _axis(axes.get("x")), _axis(axes.get("y"))
    if x is None or y is None:
        why = f"`axes.x` or `axes.y` of `{wanted}` is missing or incomplete (name, unit, scale, range, label, marks)"
        return Err(NotAFigure(path = source, why = why))
    colours = _colours(picture.get("colours"))
    if colours is None:
        return Err(NotAFigure(path = source, why = f"`colours` of `{wanted}` is missing or incomplete"))
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
    for node in rows_of(picture.get("series")):
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
        file       = wanted,
        files      = listed,
        what       = what,
        title      = title if isinstance(title, str) else None,
        preset     = preset,
        size_mm    = size,
        x          = x,
        y          = y,
        series     = tuple(series),
        columns    = columns.value,
        marks      = table(picture.get("marks")),
        colours    = colours,
        picture    = picture,
        provenance = top,
    ))
