"""
Reading JSON whose shape is not known: each helper gives the node as the type asked for, or an
empty one, so that a caller narrows once and never indexes blindly.

    table(node) -> dict    rows_of(node) -> list    number(node) -> float | None

Private to the package: a figure's own fields (``Figure.marks``, ``Figure.picture``) are what a
caller reads.
"""

from __future__ import annotations

from typing import cast


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


def pair(node: object) -> tuple[float, float] | None:
    """``node`` as two numbers, if it is that."""
    values = [number(item) for item in rows_of(node)]
    if len(values) != 2 or values[0] is None or values[1] is None:
        return None
    return (values[0], values[1])
