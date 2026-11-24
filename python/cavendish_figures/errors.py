"""
The errors of this package: frozen dataclasses in one family, each with its evidence as fields
and a stable ``code``.

    Error
    └── FigureError
        ├── Missing             figure.missing
        ├── NotAFigure          figure.not_a_figure
        ├── UnsupportedVersion  figure.unsupported_version
        ├── DataMismatch        figure.data_mismatch
        ├── UnknownPreset       figure.unknown_preset
        ├── NotDrawable         figure.not_drawable
        ├── NotWritten          figure.not_written
        └── NoFonts             figure.no_fonts

Narrow with ``isinstance`` or ``match``; never parse the text.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import ClassVar


@dataclass(frozen = True)
class Error:
    """
    The base of every error this package returns.

    Attributes:
        code (str): A stable name for the kind of error, ``family.kind``.
    """

    code: ClassVar[str] = "error"


@dataclass(frozen = True)
class FigureError(Error):
    """The family of everything that can go wrong reading or redrawing a figure."""

    code: ClassVar[str] = "figure"


@dataclass(frozen = True)
class Missing(FigureError):
    """
    A file of the figure is not there.

    Attributes:
        path (Path): The file that was looked for.
    """

    code: ClassVar[str] = "figure.missing"
    path: Path


@dataclass(frozen = True)
class NotAFigure(FigureError):
    """
    The ``.json`` is not a Cavendish figure's: not JSON, or without a field a figure has.

    Attributes:
        path (Path): The file.
        why  (str):  What is wrong with it, naming the field.
    """

    code: ClassVar[str] = "figure.not_a_figure"
    path: Path
    why:  str


@dataclass(frozen = True)
class UnsupportedVersion(FigureError):
    """
    The figure was written in a layout this package does not read.

    Attributes:
        path      (Path): The file.
        found     (int):  The layout's version in the file.
        supported (int):  The version this package reads.
    """

    code: ClassVar[str] = "figure.unsupported_version"
    path:      Path
    found:     int
    supported: int


@dataclass(frozen = True)
class DataMismatch(FigureError):
    """
    The data file is not what the ``.json`` says it is.

    Attributes:
        path     (Path): The data file.
        expected (str):  What the ``.json`` says: columns, rows.
        found    (str):  What the file holds.
    """

    code: ClassVar[str] = "figure.data_mismatch"
    path:     Path
    expected: str
    found:    str


@dataclass(frozen = True)
class UnknownPreset(FigureError):
    """
    A style was asked for by a name that is not a preset's.

    Attributes:
        name    (str):             The name asked for.
        nearest (str):             The preset whose name is closest.
        others  (tuple[str, ...]): Every preset there is.
    """

    code: ClassVar[str] = "figure.unknown_preset"
    name:    str
    nearest: str
    others:  tuple[str, ...]


@dataclass(frozen = True)
class NotDrawable(FigureError):
    """
    The figure is of a kind that has no data to redraw: a view of the scene is a picture.

    Attributes:
        what (str): The kind of figure.
    """

    code: ClassVar[str] = "figure.not_drawable"
    what: str


@dataclass(frozen = True)
class NotWritten(FigureError):
    """
    The redrawn figure could not be written.

    Attributes:
        path (Path): The file.
        why  (str):  The system's reason.
    """

    code: ClassVar[str] = "figure.not_written"
    path: Path
    why:  str


@dataclass(frozen = True)
class NoFonts(FigureError):
    """
    A folder of fonts was given and holds no face to draw with: it is not there, or the
    proportional face (``Ubuntu-Light.ttf``, which ``cav fonts --out <dir>`` writes) is not in it.

    Attributes:
        folder (Path): The folder given.
        why    (str):  What is missing from it.
    """

    code: ClassVar[str] = "figure.no_fonts"
    folder: Path
    why:    str


def error_catalog() -> dict[str, type[FigureError]]:
    """
    Every error of the family by its code.

    Returns:
        dict[str, type[FigureError]]: ``{code: class}``, one entry for each variant.
    """
    variants: tuple[type[FigureError], ...] = (
        Missing, NotAFigure, UnsupportedVersion, DataMismatch, UnknownPreset, NotDrawable, NotWritten,
        NoFonts,
    )
    return {variant.code: variant for variant in variants}
