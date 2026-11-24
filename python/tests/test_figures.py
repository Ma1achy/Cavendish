"""
Tests of ``cavendish_figures``: ``Result``, the error catalogue, the style sheets, ``load`` and
``redraw`` with their refusals.

    python -m pytest python/tests                 (if pytest is installed)
    python python/tests/test_figures.py           (needs nothing but numpy and matplotlib)
    cargo xtask oracle                            (runs the second, with real figures)

With ``CAVENDISH_FIGURES`` naming a directory of figures that ``cav figure`` wrote, the last
test reads and redraws every one of them; ``cargo xtask oracle`` sets it.
"""

from __future__ import annotations

import json
import os
import struct
import sys
import tempfile
from dataclasses import FrozenInstanceError, replace
from pathlib import Path
from typing import Callable

import matplotlib.style
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import synthetic  # noqa: E402

from cavendish_figures import (  # noqa: E402
    DASHES,
    PRESETS,
    SERIES,
    DataMismatch,
    Err,
    Figure,
    Missing,
    NotAFigure,
    NotDrawable,
    NotWritten,
    Ok,
    Result,
    UnknownPreset,
    UnsupportedVersion,
    UnwrapError,
    dash,
    draw,
    error_catalog,
    load,
    redraw,
    sheets,
)
from cavendish_figures.errors import FigureError  # noqa: E402

# ========================================== HELPERS ==========================================


def scratch() -> Path:
    """A fresh directory for one test's files."""
    return Path(tempfile.mkdtemp(prefix = "cavendish-figures-"))


def bits(value: float) -> int:
    """The 64 bits of a float, as an integer."""
    return int(struct.unpack("<Q", struct.pack("<d", value))[0])


def png_size(path: Path) -> tuple[int, int]:
    """The width and height, pixels, a PNG file says it has."""
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", f"{path} is not a PNG"
    width, height = struct.unpack(">II", data[16:24])
    return (int(width), int(height))


def loaded(directory: Path, what: str, layout: str = "csv") -> Figure:
    """The synthetic figure of ``what``, written and read back."""
    return load(synthetic.write(directory, what, layout)).unwrap()


# =========================================== RESULT ===========================================


def test_result_has_rusts_methods() -> None:
    good: Result[int, str] = Ok(2)
    bad:  Result[int, str] = Err("no")
    assert good.is_ok() and not good.is_err() and bad.is_err() and not bad.is_ok()
    assert good.ok() == 2 and good.err() is None and bad.ok() is None and bad.err() == "no"
    def more(n: int) -> int:
        return n + 1

    def twice(n: int) -> Result[int, str]:
        return Ok(n * 2)

    def length(why: str) -> Result[int, str]:
        return Ok(len(why))

    assert good.map(more) == Ok(3) and bad.map(more) == Err("no")
    assert bad.map_err(len) == Err(2) and good.map_err(len) == Ok(2)
    assert good.and_then(twice) == Ok(4) and bad.and_then(twice) == Err("no")
    assert bad.or_else(length) == Ok(2) and good.or_else(length) == Ok(2)
    assert good.unwrap_or(9) == 2 and bad.unwrap_or(9) == 9 and bad.unwrap_or_else(len) == 2
    assert good.unwrap() == 2 and good.expect("a value") == 2 and bad.unwrap_err() == "no"
    # Control: the wrong variant raises, and says which result it was.
    for wrong in (bad.unwrap, lambda: bad.expect("needed"), good.unwrap_err):
        try:
            wrong()
        except UnwrapError as raised:
            assert raised.result in (good, bad)
        else:
            raise AssertionError("the wrong variant was unwrapped without a word")
    # Results are values: frozen, and equal when their contents are.
    try:
        setattr(good, "value", 3)
    except FrozenInstanceError:
        pass
    else:
        raise AssertionError("an Ok was changed in place")


def test_the_error_catalogue_is_whole() -> None:
    catalogue = error_catalog()
    assert sorted(catalogue) == [
        "figure.data_mismatch", "figure.missing", "figure.not_a_figure", "figure.not_drawable",
        "figure.not_written", "figure.unknown_preset", "figure.unsupported_version",
    ]
    for code, variant in catalogue.items():
        assert variant.code == code and issubclass(variant, FigureError) and variant is not FigureError
    # Errors carry their evidence as fields and cannot be changed.
    error = UnknownPreset(name = "a4", nearest = "slide", others = ("slide",))
    assert (error.code, error.name, error.nearest) == ("figure.unknown_preset", "a4", "slide")
    try:
        setattr(error, "name", "a5")
    except FrozenInstanceError:
        pass
    else:
        raise AssertionError("an error was changed in place")


# =========================================== STYLE ===========================================


def test_the_style_sheets_load_and_are_the_house_style() -> None:
    for preset, (width, height) in PRESETS.items():
        found = sheets(preset).unwrap()
        assert all(sheet.is_file() for sheet in found), found
        with matplotlib.style.context([str(sheet) for sheet in found]):
            params = matplotlib.rcParams
            size   = [float(side) for side in params["figure.figsize"]]
            assert abs(size[0] * 25.4 - width) < 0.01 and abs(size[1] * 25.4 - height) < 0.01, (preset, size)
            cycle  = list(params["axes.prop_cycle"])
            assert tuple("#" + str(entry["color"]).lstrip("#") for entry in cycle) == SERIES
            styles = [entry["linestyle"] for entry in cycle]
            assert styles[0] == "-" and [tuple(style[1]) for style in styles[1:]] == list(DASHES[1:])
            assert params["axes.facecolor"] == "white" and params["svg.fonttype"] == "path"
    assert len(SERIES) == 8 and len(set(SERIES)) == 8 and len(set(DASHES)) == 8
    # The line style of a series scales with the line's width; a ninth series is not the first.
    assert dash(0, 2.0) == "solid" and dash(1, 2.0) == (0.0, (12.0, 5.0))
    assert dash(8, 1.0) == dash(1, 1.0) != dash(0, 1.0)
    # Control: a name that is no preset is refused with the nearest one that is.
    refused = sheets("paper-colum")
    assert isinstance(refused, Err) and isinstance(refused.error, UnknownPreset)
    assert refused.error.nearest == "paper-column" and refused.error.others == tuple(PRESETS)
    assert isinstance(sheets("screen"), Err), "the dark theme has no sheet"


# ============================================ LOAD ============================================


def test_load_reads_every_value_to_the_bit() -> None:
    directory = scratch()
    for layout in ("csv", "npy"):
        figure        = loaded(directory / layout, "periodogram", layout)
        columns, rows = synthetic.table("periodogram")
        assert list(figure.columns) == [name for name, _ in columns]
        for n, (name, _) in enumerate(columns):
            read = figure.columns[name]
            assert read.dtype == np.float64 and read.shape == (len(rows),)
            assert [bits(float(value)) for value in read] == [bits(row[n]) for row in rows], (layout, name)
        assert (figure.what, figure.preset, figure.size_mm) == ("periodogram", "paper-column", (85.0, 60.0))
        assert (figure.x.unit, figure.y.unit, figure.y.log, figure.x.log) == ("Hz", "rad/√Hz", True, False)
        assert figure.y.range == (1e-9, 1.0)
        assert len(figure.series) == 1 and figure.series[0].y is figure.columns["√S"]
    # The awkward numbers are in there: a subnormal, the largest double, a sum that is not 0.3.
    values = loaded(directory / "again", "periodogram").columns["√S"]
    assert float(values[1]) == 5e-324 and float(values[2]) == 1.7976931348623157e308 and float(values[0]) != 0.3
    # Any file of the figure names it. A picture's name reads that picture's entry of the record;
    # the record's own name or the data's reads the first picture listed.
    by_picture = load(directory / "csv" / "periodogram.pdf").unwrap()
    assert by_picture.path == directory / "csv" / "periodogram.json"
    assert (by_picture.file, by_picture.files) == ("periodogram.pdf", ("periodogram.pdf", "periodogram.svg"))
    other = load(directory / "csv" / "periodogram.svg").unwrap()
    assert (other.file, other.preset, other.size_mm) == ("periodogram.svg", "slide", (254.0, 143.0))
    assert other.columns["√S"].tolist() == by_picture.columns["√S"].tolist(), "one data file for both"
    for name in ("periodogram.json", "periodogram.csv"):
        assert load(directory / "csv" / name).unwrap().file == "periodogram.pdf"
    # Control: a picture the record does not list is refused by name, not read as another.
    absent = load(directory / "csv" / "periodogram.png")
    assert isinstance(absent, Err) and isinstance(absent.error, NotAFigure)
    assert "lists no `periodogram.png`" in absent.error.why and "periodogram.svg" in absent.error.why
    # The colours are the record's, not a table of this package's.
    assert by_picture.colours.series == synthetic.PALETTE and by_picture.colours.spectrum == "#6e6e6e"
    # A signal's series are each tower's own columns.
    signal = loaded(directory / "signal", "signal")
    assert [series.name for series in signal.series] == ["tower 1", "tower 2"]
    assert signal.series[1].x is signal.columns["readout time of tower 2"]
    # Control: a number written to six figures is not read back as the number it was.
    assert bits(float(f"{0.1 + 0.2:.6g}")) != bits(0.1 + 0.2)


def test_load_refuses_what_is_not_a_figure() -> None:
    directory = scratch()
    source    = synthetic.write(directory, "periodogram")
    original  = source.read_text(encoding = "utf-8")
    data      = directory / "periodogram.csv"
    table     = data.read_text(encoding = "utf-8")

    def refusal(edit: Callable[[], object]) -> FigureError:
        source.write_text(original, encoding = "utf-8")
        data.write_text(table, encoding = "utf-8", newline = "\n")
        edit()
        result = load(source)
        assert isinstance(result, Err), "a broken figure was read"
        return result.error

    assert isinstance(load(directory / "nothing.json").unwrap_err(), Missing)
    assert isinstance(refusal(lambda: source.write_text("not json", encoding = "utf-8")), NotAFigure)
    assert isinstance(refusal(lambda: source.write_text("{\"what\": \"signal\"}", encoding = "utf-8")), NotAFigure)
    later = refusal(lambda: source.write_text(original.replace('"cavendish_figure": 2', '"cavendish_figure": 3'),
                                              encoding = "utf-8"))
    assert isinstance(later, UnsupportedVersion) and (later.found, later.supported) == (3, 2)
    no_axis = refusal(lambda: source.write_text(original.replace('"scale": "log"', '"scale": "cubic"'),
                                                encoding = "utf-8"))
    assert isinstance(no_axis, NotAFigure) and "axes" in no_axis.why
    gone = refusal(data.unlink)
    assert isinstance(gone, Missing) and gone.path == data
    other = refusal(lambda: data.write_text(table.replace("frequency [Hz]", "period [s]"), encoding = "utf-8"))
    assert isinstance(other, DataMismatch) and "frequency" in other.expected and "period" in other.found
    short = refusal(lambda: data.write_text("\n".join(table.splitlines()[:-1]) + "\n", encoding = "utf-8"))
    assert isinstance(short, DataMismatch) and short.expected == "10 rows" and short.found == "9 rows"
    words = refusal(lambda: data.write_text(table.replace("0.025,", "soon,", 1), encoding = "utf-8"))
    assert isinstance(words, DataMismatch) and "soon" in words.found
    # A view of the scene is a picture: it has no data, and says so rather than failing oddly.
    view = load(synthetic.write(directory, "view"))
    assert isinstance(view, Err) and isinstance(view.error, NotDrawable) and view.error.what == "view"
    # Control: the untouched figure still loads.
    source.write_text(original, encoding = "utf-8")
    data.write_text(table, encoding = "utf-8", newline = "\n")
    assert load(source).is_ok()


# =========================================== REDRAW ===========================================


def test_redraw_draws_the_house_style() -> None:
    directory = scratch()
    figure    = loaded(directory, "periodogram")
    canvas, axes = draw(figure).unwrap()
    # A logarithmic vertical axis over the figure's own range.
    assert axes.get_yscale() == "log" and axes.get_xscale() == "linear"
    assert tuple(axes.get_xlim()) == figure.x.range and tuple(axes.get_ylim()) == figure.y.range
    lines = axes.get_lines()

    def ys(line: object) -> list[float]:
        return [float(value) for value in np.asarray(getattr(line, "get_ydata")(), dtype = np.float64)]

    def xs(line: object) -> list[float]:
        return [float(value) for value in np.asarray(getattr(line, "get_xdata")(), dtype = np.float64)]

    # The two floors, grey, right across.
    floors = [line for line in lines if line.get_color() == "#8c8c8c"]
    assert sorted(ys(line)[0] for line in floors) == [1e-5, 1e-4]
    # A stem for each harmonic in the first series colour, from the bottom of the axes: the
    # folded one dashed, the others solid.
    stems = [line for line in lines if line.get_color() == SERIES[0] and len(xs(line)) == 2]
    assert sorted(xs(line)[0] for line in stems) == [0.05, 0.10, 0.15]
    assert all(ys(line)[0] == figure.y.range[0] for line in stems)
    dashed = [xs(line)[0] for line in stems if line.get_linestyle() != "-"]
    assert dashed == [0.15], dashed
    # The diamond at the benchmark, named as a reference mark.
    diamonds = [line for line in lines if line.get_marker() == "D"]
    assert len(diamonds) == 1 and (xs(diamonds[0]), ys(diamonds[0])) == ([0.04836], [1e-7])
    assert "not simulated" in str(diamonds[0].get_label())
    # The spectrum itself, every row of the data, in grey.
    spectrum = [line for line in lines if line.get_color() == "#6e6e6e"]
    assert len(spectrum) == 1 and len(xs(spectrum[0])) == 10
    # The figure is its stated size, and is saved at the preset's resolution.
    size = canvas.get_size_inches()
    assert abs(float(size[0]) * 25.4 - 85.0) < 0.01 and abs(float(size[1]) * 25.4 - 60.0) < 0.01
    out = redraw(figure, directory / "again.png").unwrap()
    # (85 mm at 600 pixels per inch is 2007.9 pixels: matplotlib cuts it to 2007, Cavendish rounds to 2008.)
    assert png_size(out) in ((2007, 1417), (2008, 1417)), png_size(out)
    for extension, start in (("pdf", b"%PDF"), ("svg", b"<?xml")):
        assert redraw(figure, directory / f"again.{extension}").unwrap().read_bytes().startswith(start)
    # Control: a signal has a linear axis, no floors and no diamond; its series wear the palette.
    signal    = loaded(directory, "signal")
    _, axes   = draw(signal).unwrap()
    assert axes.get_yscale() == "linear"
    drawn = [line for line in axes.get_lines() if line.get_marker() != "s"]
    assert [line.get_color() for line in drawn] == list(SERIES[:2])
    assert drawn[0].get_linestyle() == "-" and drawn[1].get_linestyle() != "-"
    assert not [line for line in drawn if line.get_marker() == "D" or line.get_color() == "#8c8c8c"]
    # The vertical axis is in the unit the figure was written in.
    assert axes.get_ylabel() == "ΔΦ, mrad" and abs(ys(drawn[1])[0] - (-1.0)) < 1e-12
    # The other kinds draw too.
    for what in ("cadence", "flight"):
        assert png_size(redraw(loaded(directory, what), directory / f"{what}.png").unwrap())[0] in (2007, 2008)


def test_a_shot_not_taken_is_not_plotted() -> None:
    # I2 of S2's review: the redraw never read the column that says which shots were taken, and
    # drew a lost shot as a measurement of 0 rad, joined into the line.
    directory = scratch()
    signal    = loaded(directory, "signal")
    # Tower 2 does not take its third shot (read out at 5.75 s); its slot in the file holds 0.
    assert [bool(flag) for flag in signal.series[1].taken] == [True, True, False, True, True, True]
    assert float(signal.series[1].y[2]) == 0.0 and bool(signal.series[0].taken.all())
    _, axes = draw(signal).unwrap()
    lines   = [line for line in axes.get_lines() if line.get_marker() != "s"]
    second  = np.asarray(lines[1].get_ydata(), dtype = np.float64)
    # The value is masked: matplotlib breaks the line there and draws no marker.
    assert [bool(np.isnan(value)) for value in second] == [False, False, True, False, False, False]
    kept = signal.series[1].y[signal.series[1].taken] * 1e3
    assert [float(value) for value in second[~np.isnan(second)]] == [float(value) for value in kept]
    # A hollow square at the foot of the axes at the lost shot's time, in the tower's colour.
    hollow = [line for line in axes.get_lines() if line.get_marker() == "s"]
    assert len(hollow) == 1 and hollow[0].get_color() == SERIES[1]
    assert str(hollow[0].get_markerfacecolor()) == "none" and hollow[0].get_linestyle() == "None"
    assert [float(value) for value in np.asarray(hollow[0].get_xdata(), dtype = np.float64)] == [5.75]
    assert [float(value) for value in np.asarray(hollow[0].get_ydata(), dtype = np.float64)] == [signal.y.range[0] * 1e3]
    # A figure written before the series named the column is read by the column's own name.
    source   = synthetic.write(directory / "older", "signal")
    document = json.loads(source.read_text(encoding = "utf-8"))
    for picture in document["files"]:
        for entry in picture["series"]:
            del entry["taken_column"]
    source.write_text(json.dumps(document, ensure_ascii = False), encoding = "utf-8")
    assert [bool(flag) for flag in load(source).unwrap().series[1].taken] == [True, True, False, True, True, True]
    # Control: tower 1 takes every shot: nothing of its line is masked, and it has no hollow marker.
    first = np.asarray(lines[0].get_ydata(), dtype = np.float64)
    assert not np.isnan(first).any() and len(first) == 6
    # Control: drawn as it was before (every slot joined), the lost shot is a point at 0 rad.
    assert float((signal.series[1].y * 1e3)[2]) == 0.0 and not np.isnan(signal.series[1].y).any()
    # A flight diagram's shot not taken is the bar's outline; the others are filled.
    flight  = loaded(directory, "flight")
    _, axes = draw(flight).unwrap()
    # (Each tower's filled bars are one collection; a collection with no face colour is outlines.)
    bars    = [(len(group.get_paths()), len(np.asarray(group.get_facecolor()))) for group in axes.collections]
    assert sorted(bars) == [(1, 0), (3, 1), (4, 1), (4, 1)], bars
    # Control: a spectrum has no flags, and every value of it is a measurement.
    assert bool(loaded(directory, "periodogram").series[0].taken.all())


def test_redraw_refuses_what_it_cannot_do() -> None:
    directory = scratch()
    figure    = loaded(directory, "cadence")
    # A file that cannot be written is an error with the path, not an exception.
    nowhere = redraw(figure, directory / "no such directory" / "x.png")
    assert isinstance(nowhere, Err) and isinstance(nowhere.error, NotWritten)
    assert nowhere.error.path == directory / "no such directory" / "x.png"
    odd = redraw(figure, directory / "x.tiff3")
    assert isinstance(odd, Err) and isinstance(odd.error, NotWritten), odd
    # A kind this package does not draw.
    unknown = replace(figure, what = "hologram")
    refused = redraw(unknown, directory / "x.png")
    assert isinstance(refused, Err) and isinstance(refused.error, NotDrawable) and refused.error.what == "hologram"
    # Control: the same figure, a good name, is written.
    assert redraw(figure, directory / "x.png").is_ok()


def test_real_figures_are_read_and_redrawn() -> None:
    where = os.environ.get("CAVENDISH_FIGURES")
    if where is None:
        print("    (CAVENDISH_FIGURES is not set: no figures written by cav to read; `cargo xtask oracle` sets it)")
        return
    found = sorted(Path(where).glob("*.json"))
    assert len(found) >= 3, f"{where} holds {len(found)} figures: expected the ones xtask writes"
    kinds: set[str] = set()
    for source in found:
        record = json.loads(source.read_text(encoding = "utf-8"))
        if record.get("what") == "view":
            # A view of the scene is a picture with no data: a typed refusal, not an exception.
            refused = load(source)
            assert isinstance(refused, Err) and isinstance(refused.error, NotDrawable), refused
            kinds.add("view")
            continue
        figure = load(source).unwrap()
        kinds.add(figure.what)
        assert figure.files and figure.file == figure.files[0]
        rows = len(next(iter(figure.columns.values())))
        assert all(len(column) == rows for column in figure.columns.values())
        out = redraw(figure, source.with_name(source.stem + "-redrawn.png")).unwrap()
        width, _ = png_size(out)
        assert width > 300, f"{out} is {width} pixels wide"
        if figure.what == "periodogram":
            _, axes = draw(figure).unwrap()
            assert axes.get_yscale() == "log"
            assert any(line.get_marker() == "D" for line in axes.get_lines()), "no diamond at the benchmark"
    assert {"periodogram", "signal", "cadence"} <= kinds, kinds


# =========================================== RUNNING ===========================================

if __name__ == "__main__":
    tests = [(name, test) for name, test in sorted(globals().items()) if name.startswith("test_") and callable(test)]
    for name, test in tests:
        test()
        print(f"ok  {name}")
    print(f"{len(tests)} tests passed")
