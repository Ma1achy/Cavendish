"""
Search for the eight series colours of the light figure presets, and measure any palette against
the criterion of ``docs/slices/S2.md`` §7.14 (finding 28 of the slice's review).

    python scripts/palette_search.py                      (search, then print the table)
    python scripts/palette_search.py --measure 0b0dff,e76f11,c61604,8a6aae,28a682,cb64fe,910c59,6d1ba7

    sRGB grid -> the colours a mark on white may be -> pair matrix under seven visions
              -> for a target T, falling: eight colours moved one at a time until no pair is
                 nearer than T (seeded restarts) -> refined to single bytes -> put in order

The criterion: every pair of the eight differs by CIEDE2000 >= 12 in normal vision and under
protanopia, deuteranopia and tritanopia as simulated by Machado, Oliveira and Fernandes (2009,
severity 1.0) and by Vienot, Brettel and Mollon (1999; Brettel, Vienot and Mollon 1997 for
tritanopia): seven conditions. The first four differ pairwise by at least 8 L*. Each has at
least 3:1 contrast on white. The ``dataviz`` skill's validator passes on white, all pairs (OKLCH
lightness 0.43 to 0.77, chroma at least 0.10, OKLab x 100 at least 15 apart in normal vision
and at least 8 under its Machado simulations). The palette this replaces was also held to
OKLab x 100 >= 12 under normal vision and the three Machado simulations; that is kept.

The search is deterministic: its random numbers come from a fixed seed, so every run gives the
same palette. It needs numpy and takes a few minutes.
"""

from __future__ import annotations

import argparse
import itertools
import sys

import numpy as np
import numpy.typing as npt

Array = npt.NDArray[np.float64]

# ======================================== THE VISIONS ========================================

MACHADO: dict[str, list[list[float]]] = {
    # Machado, Oliveira and Fernandes, "A physiologically-based model for simulation of color
    # vision deficiency", IEEE TVCG 15(6), 2009: severity 1.0, applied to linear RGB.
    "protanopia":   [[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]],
    "deuteranopia": [[0.367322, 0.860646, -0.227968], [0.280085, 0.672501, 0.047413], [-0.011820, 0.042940, 0.968881]],
    "tritanopia":   [[1.255528, -0.076749, -0.178779], [-0.078411, 0.930809, 0.147602], [0.004733, 0.691367, 0.303900]],
}

VIENOT_LMS: list[list[float]] = [
    # Vienot, Brettel and Mollon, "Digital video colourmaps for checking the legibility of
    # displays by dichromats", Color Research and Application 24(4), 1999: linear RGB to LMS
    # (Smith and Pokorny fundamentals, the primaries of a standard monitor).
    [17.8824, 43.5161, 4.11935],
    [3.45565, 27.1554, 3.86714],
    [0.0299566, 0.184309, 1.46709],
]

VIENOT_RULES: dict[str, list[list[float]]] = {
    # The same paper: the cone a dichromat lacks, written from the other two.
    "protanopia":   [[0.0, 2.02344, -2.52581], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    "deuteranopia": [[1.0, 0.0, 0.0], [0.494207, 0.0, 1.24827], [0.0, 0.0, 1.0]],
}

BRETTEL_TRITAN: tuple[list[list[float]], list[list[float]], list[float]] = (
    # Brettel, Vienot and Mollon, "Computerized simulation of color appearance for dichromats",
    # JOSA A 14(10), 1997: tritanopia is two half-planes in LMS. Here as two matrices on linear
    # sRGB and the normal of the plane between them (the values of libDaltonLens, N. Burrus).
    [[1.01277, 0.13548, -0.14826], [-0.01243, 0.86812, 0.14431], [0.07589, -0.80500, 0.72911]],
    [[0.93678, 0.18979, -0.12657], [0.06154, 0.81526, 0.12320], [-0.37562, 1.12767, 0.24796]],
    [0.03901, -0.02788, -0.01113],
)


def vienot(kind: str) -> Array:
    """The matrix on linear RGB of Vienot's simulation of ``kind``: RGB to LMS, the rule, back."""
    lms = np.array(VIENOT_LMS)
    return np.linalg.inv(lms) @ np.array(VIENOT_RULES[kind]) @ lms


def visions() -> list[str]:
    """The seven conditions, in the order of every table here."""
    return [
        "normal", "Machado protanopia", "Machado deuteranopia", "Machado tritanopia",
        "Vienot protanopia", "Vienot deuteranopia", "Brettel tritanopia",
    ]


def seen(linear: Array) -> list[Array]:
    """
    Linear RGB as each of the seven conditions sees it, clamped to the gamut.

    Args:
        linear (Array): Linear sRGB, ``[N, 3]``.
    Returns:
        list[Array]: Seven arrays ``[N, 3]``, in the order of ``visions()``.
    """
    out = [linear]
    out += [np.clip(linear @ np.array(MACHADO[kind]).T, 0.0, 1.0) for kind in ("protanopia", "deuteranopia", "tritanopia")]
    out += [np.clip(linear @ vienot(kind).T, 0.0, 1.0) for kind in ("protanopia", "deuteranopia")]
    first, second, normal = (np.array(part) for part in BRETTEL_TRITAN)
    side = (linear @ normal >= 0.0)[:, None]
    out.append(np.clip(np.where(side, linear @ first.T, linear @ second.T), 0.0, 1.0))
    return out


# ======================================== THE MEASURES ========================================


def linear_of(rgb: Array) -> Array:
    """sRGB bytes ``[N, 3]`` as linear RGB (IEC 61966-2-1)."""
    c = rgb / 255.0
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def lab_of(linear: Array) -> Array:
    """Linear sRGB ``[N, 3]`` as CIE L*a*b* (D65)."""
    xyz = linear @ np.array([
        [0.4124564, 0.3575761, 0.1804375], [0.2126729, 0.7151522, 0.0721750], [0.0193339, 0.1191920, 0.9503041],
    ]).T / np.array([0.95047, 1.0, 1.08883])
    f = np.where(xyz > 216.0 / 24389.0, np.cbrt(xyz), (24389.0 / 27.0 * xyz + 16.0) / 116.0)
    return np.stack([116.0 * f[:, 1] - 16.0, 500.0 * (f[:, 0] - f[:, 1]), 200.0 * (f[:, 1] - f[:, 2])], axis = 1)


def oklab_of(linear: Array) -> Array:
    """Linear sRGB ``[N, 3]`` as OKLab x 100 (Ottosson 2020)."""
    lms = np.cbrt(linear @ np.array([
        [0.4122214708, 0.5363325363, 0.0514459929], [0.2119034982, 0.6806995451, 0.1073969566],
        [0.0883024619, 0.2817188376, 0.6299787005],
    ]).T)
    return 100.0 * lms @ np.array([
        [0.2104542553, 0.7936177850, -0.0040720468], [1.9779984951, -2.4285922050, 0.4505937099],
        [0.0259040371, 0.7827717662, -0.8086757660],
    ]).T


def ciede2000(first: Array, second: Array) -> Array:
    """
    CIEDE2000 between every colour of ``first`` and every colour of ``second`` (Sharma, Wu and
    Dalal 2005, unit weights).

    Args:
        first  (Array): L*a*b*, ``[N, 3]``.
        second (Array): L*a*b*, ``[M, 3]``.
    Returns:
        Array: The differences, ``[N, M]``.
    """
    l1, a1, b1 = (first[:, k][:, None] for k in range(3))
    l2, a2, b2 = (second[:, k][None, :] for k in range(3))
    seventh    = lambda c: np.sqrt(c ** 7 / (c ** 7 + 25.0 ** 7))  # noqa: E731
    g          = 0.5 * (1.0 - seventh((np.hypot(a1, b1) + np.hypot(a2, b2)) / 2.0))
    a1, a2     = (1.0 + g) * a1, (1.0 + g) * a2
    c1, c2     = np.hypot(a1, b1), np.hypot(a2, b2)
    h1         = np.where(c1 == 0.0, 0.0, np.degrees(np.arctan2(b1, a1)) % 360.0)
    h2         = np.where(c2 == 0.0, 0.0, np.degrees(np.arctan2(b2, a2)) % 360.0)
    grey       = c1 * c2 == 0.0
    turn       = np.where(grey, 0.0, np.where(np.abs(h2 - h1) <= 180.0, h2 - h1, np.where(h2 - h1 > 180.0, h2 - h1 - 360.0, h2 - h1 + 360.0)))
    dl, dc     = l2 - l1, c2 - c1
    dh         = 2.0 * np.sqrt(c1 * c2) * np.sin(np.radians(turn / 2.0))
    l, c       = (l1 + l2) / 2.0, (c1 + c2) / 2.0
    h          = np.where(grey, h1 + h2, np.where(np.abs(h1 - h2) <= 180.0, (h1 + h2) / 2.0, np.where(h1 + h2 < 360.0, (h1 + h2 + 360.0) / 2.0, (h1 + h2 - 360.0) / 2.0)))
    t          = (1.0 - 0.17 * np.cos(np.radians(h - 30.0)) + 0.24 * np.cos(np.radians(2.0 * h))
                  + 0.32 * np.cos(np.radians(3.0 * h + 6.0)) - 0.20 * np.cos(np.radians(4.0 * h - 63.0)))
    rotation   = -np.sin(np.radians(60.0 * np.exp(-(((h - 275.0) / 25.0) ** 2)))) * 2.0 * seventh(c)
    sl         = 1.0 + 0.015 * (l - 50.0) ** 2 / np.sqrt(20.0 + (l - 50.0) ** 2)
    sc, sh     = 1.0 + 0.045 * c, 1.0 + 0.015 * c * t
    return np.sqrt((dl / sl) ** 2 + (dc / sc) ** 2 + (dh / sh) ** 2 + rotation * (dc / sc) * (dh / sh))


def contrast_on_white(linear: Array) -> Array:
    """WCAG contrast of each colour ``[N, 3]`` against white."""
    return 1.05 / (linear @ np.array([0.2126, 0.7152, 0.0722]) + 0.05)


def apart(first: Array, second: Array) -> Array:
    """
    How far apart two sets of colours are, as one number a pair: the least of everything the
    criterion holds a pair to, each shifted so that 12 is the pass mark of all of them.

    Args:
        first  (Array): sRGB bytes, ``[N, 3]``.
        second (Array): sRGB bytes, ``[M, 3]``.
    Returns:
        Array: ``[N, M]``; a pair passes when its number is at least 12.
    """
    a, b  = seen(linear_of(first)), seen(linear_of(second))
    worst = np.full((len(first), len(second)), np.inf)
    for n, (left, right) in enumerate(zip(a, b)):
        # With ``--hold`` the conditions the palette before was held to (normal vision and
        # Machado's three) are shifted so that they still pass only at 12, whatever the target.
        shift = SHIFT if n < 4 else 0.0
        worst = np.minimum(worst, ciede2000(lab_of(left), lab_of(right)) + shift)
        if n < 4:
            # OKLab x 100: at least 12 under normal vision and Machado's three (the palette
            # before was held to it), and at least 15 in normal vision (the validator's floor).
            ok    = np.linalg.norm(oklab_of(left)[:, None, :] - oklab_of(right)[None, :, :], axis = 2)
            worst = np.minimum(worst, (ok - 3.0 if n == 0 else ok) + shift)
    return worst


SHIFT: float = 0.0
"""What ``apart`` adds to the conditions of the palette before: ``target - 12`` with ``--hold``, else 0."""


# ========================================= THE SEARCH =========================================


def candidates(step: int) -> Array:
    """
    The colours a mark on white may be, on a grid of ``step`` bytes: OKLCH lightness 0.43 to
    0.77, chroma at least 0.10, at least 3:1 against white and against the validator's surface.
    """
    axis   = np.arange(0, 256, step, dtype = np.float64)
    grid   = np.array(list(itertools.product(axis, axis, axis)))
    return grid[allowed(grid)]


def allowed(rgb: Array) -> npt.NDArray[np.bool_]:
    """Which of the colours ``[N, 3]`` (sRGB bytes) a mark on white may be."""
    linear = linear_of(rgb)
    ok     = oklab_of(linear) / 100.0
    chroma = np.hypot(ok[:, 1], ok[:, 2])
    # The validator's light surface is #fcfcfb, a shade under white: 3:1 there is 3.07:1 on white.
    return (ok[:, 0] >= 0.435) & (ok[:, 0] <= 0.765) & (chroma >= 0.105) & (contrast_on_white(linear) >= 3.08)


def greys(rgb: Array) -> float:
    """The least L* gap among the best four of the colours ``[8, 3]`` for print in grey."""
    lightness = lab_of(linear_of(rgb))[:, 0]
    return max(min(abs(lightness[a] - lightness[b]) for a, b in itertools.combinations(four, 2))
               for four in itertools.combinations(range(len(rgb)), 4))


def score(rgb: Array) -> float:
    """The least of a palette's pairs (``apart``) and of its grey rule, 12 the pass mark of both."""
    pairs = apart(rgb, rgb)
    least = float(np.min(pairs[np.triu_indices(len(rgb), 1)]))
    return min(least, greys(rgb) + 4.0 + SHIFT)


def greys_of_first(rgb: Array) -> float:
    """The least L* gap among the first four colours of ``[8, 3]``."""
    lightness = lab_of(linear_of(rgb))[:4, 0]
    return min(abs(lightness[a] - lightness[b]) for a, b in itertools.combinations(range(4), 2))


TARGETS: tuple[float, ...] = (14.0, 13.5, 13.0, 12.75, 12.5, 12.25, 12.0, 11.75, 11.5, 11.25, 11.0)
"""The least pair asked for, tried from the top: the first that is met is the one refined."""

HELD_TARGETS: tuple[float, ...] = (12.0, 11.75, 11.5, 11.25, 11.0, 10.75, 10.5)
"""With ``--hold``: the least pair asked of Vienot's and Brettel's simulations alone."""


def settle(pairs: Array, lightness: Array, target: float, rng: np.random.Generator, moves: int) -> tuple[list[int], float]:
    """
    Eight colours of the pool with no pair nearer than ``target``, if the moves find them.

    From eight random colours: the colour with the most to answer for is moved to the candidate
    that leaves the least shortfall (the sum over its pairs of ``(target - apart)^2``, and for
    the first four slots of ``(8 - |L* gap|)^2``); when nothing better exists a random colour is
    replaced by a random candidate.

    Args:
        pairs     (Array):     ``apart`` of the pool with itself, ``[N, N]``.
        lightness (Array):     L* of each candidate, ``[N]``.
        target    (float):     The least pair asked for.
        rng       (Generator): Where the random choices come from.
        moves     (int):       How many moves to make before giving up.
    Returns:
        tuple[list[int], float]: The eight candidates' indices, and the shortfall left (0 when met).
    """
    count   = len(pairs)
    members = [int(n) for n in rng.choice(count, size = 8, replace = False)]

    def shortfall(slot: int, others: list[int], grey: list[int]) -> Array:
        """What each candidate would leave short in ``slot``, ``[N]``."""
        short = np.sum(np.maximum(0.0, target - pairs[:, others]) ** 2, axis = 1)
        if slot < 4 and grey:
            short = short + 4.0 * np.sum(np.maximum(0.0, 8.0 - np.abs(lightness[:, None] - lightness[grey][None, :])) ** 2, axis = 1)
        return short

    best: tuple[list[int], float] = (members, np.inf)
    for _ in range(moves):
        costs = []
        for slot in range(8):
            others = members[:slot] + members[slot + 1:]
            grey   = [members[n] for n in range(4) if n != slot]
            costs.append(float(shortfall(slot, others, grey)[members[slot]]))
        total = sum(costs) / 2.0
        if total < best[1]:
            best = (list(members), total)
        if total == 0.0:
            break
        slot   = int(np.argmax(np.array(costs) + rng.random(8) * 1e-6))
        others = members[:slot] + members[slot + 1:]
        short  = shortfall(slot, others, [members[n] for n in range(4) if n != slot])
        short[others] = np.inf
        there  = int(np.argmin(short))
        if short[there] < costs[slot] - 1e-12:
            members[slot] = there
        else:
            members[int(rng.integers(8))] = int(rng.integers(count))
            while len(set(members)) < 8:
                members[int(rng.integers(8))] = int(rng.integers(count))
    return best


def search(step: int = 12, starts: int = 64, hold: bool = False) -> Array:
    """
    The best eight colours found: for each target of ``TARGETS`` in turn, ``starts`` seeded
    attempts to place eight colours with no pair nearer than it; the first target met is taken
    (or, if none is, the attempt that came nearest), then refined to single bytes.

    Args:
        step   (int):  The grid's spacing, sRGB bytes.
        starts (int):  How many attempts a target is given.
        hold   (bool): Hold normal vision and Machado's three at 12 (and OKLab at its floors)
                       and lower only the target of Vienot's and Brettel's simulations.
    Returns:
        Array: The palette, sRGB bytes ``[8, 3]``, the first four those held apart in grey.
    """
    global SHIFT
    pool      = candidates(step)
    pairs     = apart(pool, pool)
    lightness = lab_of(linear_of(pool))[:, 0]
    rng       = np.random.default_rng(20261124)
    print(f"{len(pool)} candidate colours on a grid of {step}", file = sys.stderr)
    nearest: tuple[float, list[int]] | None = None
    for target in (HELD_TARGETS if hold else TARGETS):
        if hold:
            SHIFT = target - 12.0
            pairs = apart(pool, pool)
        for attempt in range(starts):
            members, short = settle(pairs, lightness, target, rng, moves = 400)
            if nearest is None or short < nearest[0]:
                nearest = (short, members)
            if short == 0.0:
                print(f"target {target}: met at attempt {attempt + 1}", file = sys.stderr)
                return refine(pool[members])
        print(f"target {target}: not met in {starts} attempts", file = sys.stderr)
    assert nearest is not None
    return refine(pool[nearest[1]])


def refine(rgb: Array) -> Array:
    """Moves each colour within 6 bytes a channel, a colour at a time, while the score rises."""
    offsets = np.array(list(itertools.product(range(-6, 7, 2), repeat = 3)), dtype = np.float64)
    current = score(rgb)
    for _ in range(4):
        moved = False
        for slot in range(len(rgb)):
            near = np.clip(rgb[slot] + offsets, 0.0, 255.0)
            near = near[allowed(near)]
            for trial in near:
                tried       = rgb.copy()
                tried[slot] = trial
                if (worth := score(tried)) > current + 1e-9:
                    rgb, current, moved = tried, worth, True
        if not moved:
            break
    return rgb


def ordered(rgb: Array) -> Array:
    """
    The palette in its order: first the four that are furthest apart in grey (darkest first,
    then alternating, so neighbours in a key differ most), then the rest by lightness.
    """
    lightness = lab_of(linear_of(rgb))[:, 0]
    four      = max(itertools.combinations(range(len(rgb)), 4),
                    key = lambda four: min(abs(lightness[a] - lightness[b]) for a, b in itertools.combinations(four, 2)))
    by_grey   = sorted(four, key = lambda n: lightness[n])
    first     = [by_grey[0], by_grey[2], by_grey[1], by_grey[3]]
    rest      = sorted((n for n in range(len(rgb)) if n not in four), key = lambda n: lightness[n])
    return rgb[first + rest]


# ========================================= THE TABLE =========================================


def table(rgb: Array) -> bool:
    """Prints the palette and its least difference under each condition; whether it passes."""
    hexes  = ["#" + "".join(f"{int(byte):02x}" for byte in colour) for colour in rgb]
    linear = linear_of(rgb)
    labs   = [lab_of(view) for view in seen(linear)]
    upper  = np.triu_indices(len(rgb), 1)
    passed = True
    print("palette:   " + ", ".join(hexes))
    for name, lab in zip(visions(), labs):
        pairs       = ciede2000(lab, lab)
        at          = int(np.argmin(pairs[upper]))
        a, b, least = int(upper[0][at]), int(upper[1][at]), float(pairs[upper][at])
        passed      = passed and least >= 12.0
        print(f"  {name:<22} least CIEDE2000 {least:6.2f}  ({hexes[a]} and {hexes[b]})")
    lightness = labs[0][:, 0]
    gap       = min(abs(lightness[a] - lightness[b]) for a, b in itertools.combinations(range(min(4, len(rgb))), 2))
    contrast  = contrast_on_white(linear)
    print("  L*: " + ", ".join(f"{value:.1f}" for value in lightness) + f"; least gap of the first four {gap:.1f}")
    print("  contrast on white: " + ", ".join(f"{value:.2f}" for value in contrast))
    return passed and gap >= 8.0 and bool(np.all(contrast >= 3.0))


def main() -> int:
    """Searches (or measures ``--measure``'s colours), prints the table; 0 when it passes."""
    parser = argparse.ArgumentParser(description = __doc__.split("\n\n")[0])
    parser.add_argument("--measure", help = "eight colours as rrggbb,rrggbb,...: measure them, search nothing")
    parser.add_argument("--step", type = int, default = 12, help = "the grid's spacing, sRGB bytes")
    parser.add_argument("--starts", type = int, default = 64, help = "how many first colours to start from")
    parser.add_argument("--hold", action = "store_true", help = "hold the conditions of the palette before at 12; lower only Vienot's and Brettel's")
    args = parser.parse_args()
    if args.measure:
        rgb = np.array([[int(colour.strip().lstrip("#")[k:k + 2], 16) for k in (0, 2, 4)]
                        for colour in args.measure.split(",")], dtype = np.float64)
    else:
        rgb = ordered(search(args.step, args.starts, args.hold))
    return 0 if table(rgb) else 1


if __name__ == "__main__":
    sys.exit(main())
