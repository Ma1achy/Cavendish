"""Compare the brand art this folder builds with what the repository ships, or copy it into place.

    build_all.sh / build_all.ps1  ->  brand/{icon,titles,colour,banners}/, brand/*.gif
    install_into_repo.py          ->  a table: each shipped file, its rebuilt counterpart, how far apart
    install_into_repo.py --write  ->  the rebuilt files copied over docs/readme and crates/cav-app

Without ``--write`` nothing is changed. The shipped art is the owner's delivery: copy a rebuild over it
only when the table says the rebuild is the same art (or when the art was changed on purpose).

    python brand/install_into_repo.py                # compare only
    python brand/install_into_repo.py --write        # copy every rebuilt file into place
    python brand/install_into_repo.py --write icon   # only the pairs whose shipped path contains "icon"

The difference measures, in 8-bit levels: ``max`` is the largest difference of any channel of any pixel,
``mean`` the mean absolute difference; for an animation both are taken per frame and the worst frame is
reported. SVGs and anything else that is not an image are compared as bytes.
"""

from __future__ import annotations

import argparse
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import numpy.typing as npt
from PIL import IcoImagePlugin, Image, ImageSequence

BRAND = Path(__file__).resolve().parent
ROOT  = BRAND.parent

# The pass criteria, written before the first comparison was run (README.md, "Regenerating on Windows").
STILL_MEAN    = 1.0  # PNG, ICO and JPEG: mean absolute difference, levels
ANIMATED_MEAN = 2.0  # GIF, WebP and APNG: the worst frame's mean absolute difference, levels

MARKS   = ("dark", "light", "bw-dark", "bw-light")
SIZES   = (16, 32, 48, 64, 128, 180, 192, 256, 512, 1024)
THEMES  = ("dark", "light")
README  = "docs/readme"
WEB     = "crates/cav-app/web"

# Shipped files no script here makes (BUILD.md, "What the build does not make").
NOT_BUILT = (
    f"{README}/signature-dark.svg",
    f"{README}/signature-light.svg",
    f"{README}/social-preview.jpg",
    f"{WEB}/social-preview.jpg",
    f"{WEB}/site.webmanifest",
)


# ========================================== THE PAIRS ==========================================


def pairs() -> list[tuple[str, str]]:
    """Every (rebuilt file, shipped file) pair.

    Returns:
        list[tuple[str, str]]: The rebuilt path relative to ``brand/`` and the shipped path relative to
                               the repository root, in the order the table prints them.
    """
    out: list[tuple[str, str]] = []
    for theme in THEMES:
        out.append((f"title-anim-{theme}.gif", f"{README}/title-anim-{theme}.gif"))
        out.append((f"titles/title-{theme}.svg", f"{README}/title-{theme}.svg"))
        out.append((f"titles/title-bw-{theme}.svg", f"{README}/title-bw-{theme}.svg"))
        for banner in ("hero", "banner-octree", "footer"):
            out.append((f"banners/{banner}-{theme}.jpg", f"{README}/{banner}-{theme}.jpg"))
    for mark in MARKS:
        for suffix in ("", "-transparent"):
            name = f"cavendish-icon-{mark}{suffix}.svg"
            out.append((f"icon/{name}", f"{README}/icon/{name}"))
        for size in SIZES:
            name = f"png/cavendish-icon-{mark}-{size}.png"
            out.append((f"icon/{name}", f"{README}/icon/{name}"))
    for name in ("favicon.svg", "favicon.ico", "apple-touch-icon.png"):
        out.append((f"icon/{name}", f"{README}/icon/{name}"))
        out.append((f"icon/{name}", f"{WEB}/{name}"))
    out.append(("icon/png/cavendish-icon-dark-192.png", f"{WEB}/icon-192.png"))
    out.append(("icon/png/cavendish-icon-dark-512.png", f"{WEB}/icon-512.png"))
    out.append(("icon/png/cavendish-icon-dark-256.png", "crates/cav-app/assets/icon-256.png"))
    for theme in THEMES:
        out.append((f"cavendish-icon-anim-{theme}.gif", f"{README}/icon/anim-bw/cavendish-icon-anim-{theme}.gif"))
        out.append(
            (f"cavendish-icon-anim-{theme}-512.gif", f"{README}/icon/anim-bw/cavendish-icon-anim-{theme}-512.gif")
        )
        for size in (256, 512):
            for ext in ("gif", "webp", "png"):
                name = f"cavendish-icon-anim-{theme}-{size}.{ext}"
                out.append((f"colour/{name}", f"{README}/icon/anim-colour/{name}"))
        for size in (32, 64):
            for ext in ("gif", "png"):
                name = f"favicon-anim-{theme}-{size}.{ext}"
                out.append((f"colour/{name}", f"{README}/icon/anim-colour/{name}"))
    return out


# ========================================== COMPARING ==========================================


@dataclass(frozen = True)
class Difference:
    """How far a rebuilt file is from the shipped one.

    Attributes:
        passed  (bool): Whether the pair meets the criterion for its kind.
        measure (str):  The measure, as the table prints it.
    """

    passed:  bool
    measure: str


def _frames(path: Path) -> list[npt.NDArray[np.int16]]:
    """The frames of an image file as ``[H, W, 4]`` RGBA arrays (one per size for an ``.ico``)."""
    if path.suffix == ".ico":
        with path.open("rb") as file:
            ico = IcoImagePlugin.IcoFile(file)
            return [np.asarray(ico.getimage(size).convert("RGBA"), dtype = np.int16) for size in sorted(ico.sizes())]
    with Image.open(path) as image:
        return [np.asarray(f.convert("RGBA"), dtype = np.int16) for f in ImageSequence.Iterator(image)]


def _duration(path: Path) -> object:
    """The first frame's duration in milliseconds, or ``None`` for a still."""
    with Image.open(path) as image:
        return image.info.get("duration")


def compare(built: Path, shipped: Path) -> Difference:
    """Measure one pair.

    Identical bytes pass. Otherwise an image passes when its size, frame count and frame duration match
    and its mean difference is within ``STILL_MEAN`` (``ANIMATED_MEAN`` for the worst frame of an
    animation); anything else fails with the count of differing bytes.

    Args:
        built   (Path): The rebuilt file.
        shipped (Path): The shipped file.
    Returns:
        Difference: The verdict and its measure.
    """
    if not built.exists():
        return Difference(False, "not built")
    if not shipped.exists():
        return Difference(False, "not shipped")
    a, b = built.read_bytes(), shipped.read_bytes()
    if a == b:
        return Difference(True, "identical bytes")
    if built.suffix not in (".png", ".jpg", ".gif", ".webp", ".ico"):
        differing = sum(x != y for x, y in zip(a, b)) + abs(len(a) - len(b))
        return Difference(False, f"{differing} of {len(b)} bytes differ ({len(a)} rebuilt)")
    fa, fb = _frames(built), _frames(shipped)
    if len(fa) != len(fb) or any(x.shape != y.shape for x, y in zip(fa, fb)):
        return Difference(False, f"shape differs: {len(fa)} x {fa[0].shape} against {len(fb)} x {fb[0].shape}")
    if _duration(built) != _duration(shipped):
        return Difference(False, f"frame duration {_duration(built)} against {_duration(shipped)} ms")
    diffs  = [np.abs(x - y) for x, y in zip(fa, fb)]
    worst  = max(float(d.mean()) for d in diffs)
    peak   = max(int(d.max()) for d in diffs)
    limit  = ANIMATED_MEAN if len(fa) > 1 else STILL_MEAN
    frames = f"{len(fa)} frames, worst frame " if len(fa) > 1 else ""
    return Difference(worst <= limit, f"{frames}max {peak}, mean {worst:.3f}")


# ============================================ MAIN =============================================


def main() -> int:
    """Print the comparison table; with ``--write``, copy the rebuilt files into place.

    Returns:
        int: 0 when every pair passed (or was written), 1 when a pair failed or was missing.
    """
    parser = argparse.ArgumentParser(description = "Compare rebuilt brand art with the shipped art, or install it.")
    parser.add_argument("--write", action = "store_true", help = "copy the rebuilt files over the shipped ones")
    parser.add_argument("only", nargs = "*", help = "only pairs whose shipped path contains one of these")
    options = parser.parse_args()
    only: list[str] = options.only
    failed = 0
    for built, shipped in pairs():
        if only and not any(word in shipped for word in only):
            continue
        difference = compare(BRAND / built, ROOT / shipped)
        verdict    = "pass" if difference.passed else "FAIL"
        if options.write and (BRAND / built).exists():
            shutil.copyfile(BRAND / built, ROOT / shipped)
            verdict = "written"
        elif not difference.passed:
            failed += 1
        print(f"{verdict:7} | {shipped} | brand/{built} | {difference.measure}")
    for shipped in NOT_BUILT:
        if not only or any(word in shipped for word in only):
            print(f"{'-':7} | {shipped} | (no script makes it) |")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
