#!/usr/bin/env python3
# pyright: strict
"""Write the `.npy` files that hold Cavendish's own writer to what `numpy.save` writes.

    explicit bit patterns  ──>  numpy.save  ──>  crates/cav-core/tests/fixtures/numpy_*.npy

`crates/cav-core/src/npy.rs` rebuilds each array from the same bit patterns and its test
`the_writer_reproduces_a_file_numpy_wrote` asserts that its bytes are these files' bytes. The files are
committed and are not regenerated to make a test pass: a difference is a change in numpy's format or in
the writer, and is explained first.

The committed files were written by numpy 2.4.3 (Python 3.14, Windows).

Usage:
    python scripts/npy_fixture.py
"""

from __future__ import annotations

from pathlib import Path

import numpy as np
import numpy.typing as npt

OUT = Path(__file__).resolve().parent.parent / "crates" / "cav-core" / "tests" / "fixtures"

# The values a text format loses: -0.0, the smallest and the largest subnormal, the largest finite number,
# both infinities, a quiet NaN with a payload and a negative signalling NaN. `AWKWARD` in npy.rs is this list.
AWKWARD: list[int] = [
    0x8000_0000_0000_0000,
    0x0000_0000_0000_0001,
    0x000F_FFFF_FFFF_FFFF,
    0x7FEF_FFFF_FFFF_FFFF,
    0x7FF0_0000_0000_0000,
    0xFFF0_0000_0000_0000,
    0x7FF8_DEAD_BEEF_0001,
    0xFFF0_0000_0000_0001,
]
PI = 0x4009_21FB_5444_2D18


# ========================================== THE ARRAYS ==========================================


def ordinary(i: int) -> int:
    """The bits of the grid's value `i`: between 1 and 2 in size, negative when `i` is odd."""
    return (0x3FF0_0000_0000_0000 + i * 0x0003_1415_9265_3589) | ((i & 1) << 63)


def from_bits(bits: list[int], shape: tuple[int, ...]) -> npt.NDArray[np.float64]:
    """A float64 array of this shape (C order) whose values have exactly these bits."""
    return np.array(bits, dtype = "<u8").view("<f8").reshape(shape)


def arrays() -> dict[str, npt.NDArray[np.float64] | npt.NDArray[np.bool_]]:
    """Each fixture by file name: the grid, a scalar, an empty array, bools, and numpy's two paddings.

    Fifteen axes of length 1 make a header that the room numpy leaves for the first axis to grow tips
    into a second 64-byte block; the last shape's header ends exactly on a block, which numpy follows
    with a whole block of spaces.
    """
    return {
        "numpy_5x7":        from_bits(AWKWARD + [ordinary(i) for i in range(8, 35)], (5, 7)),
        "numpy_scalar":     from_bits([PI], ()),
        "numpy_empty":      np.zeros((0, 3), dtype = "<f8"),
        "numpy_bool_2x3":   np.array([[True, False, True], [False, False, True]], dtype = np.bool_),
        "numpy_15_axes":    np.full((1,) * 15, -1.5, dtype = "<f8"),
        "numpy_full_block": np.arange(100, dtype = "<f8").reshape((1,) * 13 + (100,)),
    }


# ============================================= MAIN =============================================


def main() -> None:
    OUT.mkdir(parents = True, exist_ok = True)
    for name, array in arrays().items():
        path = OUT / f"{name}.npy"
        np.save(path, array, allow_pickle = False)
        print(f"{path.relative_to(OUT.parent.parent.parent.parent)}  {array.dtype.str}  {array.shape}  "
              f"{path.stat().st_size} bytes  (numpy {np.__version__})")


if __name__ == "__main__":
    main()
