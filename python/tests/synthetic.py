"""
Figures made by hand for the tests: the files ``cav figure`` writes, small and with awkward
numbers in them, so that the tests need neither Cavendish nor a device.

    write(directory, "periodogram") -> directory/periodogram.json + periodogram.csv
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

AWKWARD: tuple[float, ...] = (0.1 + 0.2, 5e-324, 1.7976931348623157e308, 1e-7, 123456789.12345679, 2.5e-5)
"""Numbers that only the shortest round-trip text reads back to the bit."""


def text(value: float) -> str:
    """A number as Cavendish writes it in a ``.csv``: the shortest text that reads back the same."""
    return repr(value)


def provenance(what: str, columns: list[tuple[str, str]], rows: int, layout: str = "csv") -> dict[str, object]:
    """
    The ``.json`` of a figure of ``what`` with the given columns.

    Args:
        what    (str):                   ``signal``, ``periodogram``, ``cadence``, ``flight`` or ``view``.
        columns (list[tuple[str, str]]): Each column's name and unit.
        rows    (int):                   How many rows the data file has.
        layout  (str):                   ``csv`` or ``npy``.
    Returns:
        dict[str, object]: The whole document, as ``json.dumps`` takes it.
    """
    log     = what in ("periodogram", "cadence")
    series: list[dict[str, object]] = []
    if what == "signal":
        series = [
            {"name": f"tower {n + 1}", "colour": "#000000", "x": columns[3 * n][0], "y": columns[3 * n + 1][0]}
            for n in range(len(columns) // 3)
        ]
    elif what in ("periodogram", "cadence"):
        series = [{"name": "the spectrum", "colour": "#6e6e6e", "x": columns[0][0], "y": columns[-1][0]}]
    marks: dict[str, object] = {}
    if what == "periodogram":
        marks = {
            "combs": [{
                "body": "oscillator", "fundamental_hz": 0.05, "colour": "#0b0dff",
                "stems": [
                    {"n": 1, "frequency_hz": 0.05, "appears_at_hz": 0.05, "folded": False, "height": 1e-2},
                    {"n": 2, "frequency_hz": 0.10, "appears_at_hz": 0.10, "folded": False, "height": 1e-3},
                    {"n": 7, "frequency_hz": 0.35, "appears_at_hz": 0.15, "folded": True,  "height": 1e-6},
                ],
            }],
            "reference": {
                "shot_noise_floors_rad_per_sqrt_hz": [1e-4, 1e-5],
                "uldm_benchmark": {"frequency_hz": 0.04836, "height_rad_per_sqrt_hz": 1e-7, "note": "a reference mark"},
            },
        }
    elif what == "cadence":
        marks = {"predicted_lines": [{"what": "alias", "frequency_hz": 0.2, "height": 0.25}], "predicted_floor": 5e-3}
    elif what == "signal":
        marks = {"vertical_axis_written_in": "mrad", "shots": rows}
    return {
        "cavendish_figure": 1,
        "what":             what,
        "file":             f"{what}.pdf",
        "format":           "pdf",
        "preset":           "paper-column",
        "size_mm":          [85.0, 60.0],
        "title":            f"A synthetic {what}",
        "axes": {
            "x": {"name": "frequency" if log else "time", "unit": "Hz" if log else "s", "scale": "linear",
                  "range": [0.0, 0.25] if log else [0.0, 12.0]},
            "y": {"name": "√S" if log else "ΔΦ", "unit": "rad/√Hz" if log else "rad",
                  "scale": "log" if log else "linear", "range": [1e-9, 1.0] if log else [-0.002, 0.002]},
        },
        "series": series,
        "marks":  marks,
        "data": {
            "file":    f"{what}.{layout}",
            "format":  layout,
            "rows":    rows,
            "columns": [{"name": name, "unit": unit} for name, unit in columns],
        },
    }


def table(what: str) -> tuple[list[tuple[str, str]], list[list[float]]]:
    """
    The columns and the rows of the synthetic figure of ``what``.

    Args:
        what (str): ``signal``, ``periodogram``, ``cadence`` or ``flight``.
    Returns:
        tuple[list[tuple[str, str]], list[list[float]]]: Each column's name and unit, and the
            rows, ``[N][C]``.
    """
    if what == "signal":
        columns = [(f"{kind} of tower {n + 1}", unit) for n in range(2)
                   for kind, unit in (("readout time", "s"), ("ΔΦ", "rad"), ("shot taken by", "1 if taken or 0 if not"))]
        rows    = [[1.46 + 2.0 * k, 1e-3 * ((-1) ** k) * AWKWARD[k % 2 * 3], 1.0,
                    1.75 + 2.0 * k, 4e-4 * k - 1e-3, float(k != 2)] for k in range(6)]
        return columns, rows
    if what == "periodogram":
        columns = [("frequency", "Hz"), ("S", "rad²/Hz"), ("√S", "rad/√Hz")]
        rows    = [[0.025 * (k + 1), value * value, value] for k, value in enumerate((*AWKWARD, 1e-2, 3e-4, 0.0, 1e-8))]
        return columns, rows
    if what == "cadence":
        columns = [("frequency offset from a tone", "Hz"), ("|W| relative to the tone", "1")]
        return columns, [[0.025 * k, 1.0 / (1.0 + k * k)] for k in range(11)]
    columns = [("tower", "its number from 1"), ("launch", "s"), ("readout", "s")]
    return columns, [[float(n), 2.0 * k + 0.3 * n, 2.0 * k + 0.3 * n + 1.46] for n in (1, 2, 3) for k in range(4)]


def write(directory: Path, what: str, layout: str = "csv") -> Path:
    """
    Writes the synthetic figure of ``what``: its ``.json`` and its data file.

    Args:
        directory (Path): Where to write.
        what      (str):  ``signal``, ``periodogram``, ``cadence``, ``flight`` or ``view``.
        layout    (str):  ``csv`` or ``npy`` for the data file.
    Returns:
        Path: The ``.json`` written.
    """
    directory.mkdir(parents = True, exist_ok = True)
    target = directory / f"{what}.json"
    if what == "view":
        document = provenance("view", [], 0)
        del document["data"], document["axes"], document["series"]
        target.write_text(json.dumps(document, ensure_ascii = False, indent = 2), encoding = "utf-8")
        return target
    columns, rows = table(what)
    if layout == "csv":
        lines = [",".join(f"{name} [{unit}]" for name, unit in columns)]
        lines.extend(",".join(text(value) for value in row) for row in rows)
        (directory / f"{what}.csv").write_text("\n".join(lines) + "\n", encoding = "utf-8", newline = "\n")
    else:
        # A version 1.0 .npy by hand: the magic, a padded header, little-endian doubles in C order.
        header = f"{{'descr': '<f8', 'fortran_order': False, 'shape': ({len(rows)}, {len(columns)}), }}"
        header = header + " " * (63 - (10 + len(header)) % 64) + "\n"
        body   = b"".join(struct.pack("<d", value) for row in rows for value in row)
        (directory / f"{what}.npy").write_bytes(
            b"\x93NUMPY\x01\x00" + struct.pack("<H", len(header)) + header.encode("latin-1") + body
        )
    document = provenance(what, columns, len(rows), layout)
    target.write_text(json.dumps(document, ensure_ascii = False, indent = 2), encoding = "utf-8")
    return target
