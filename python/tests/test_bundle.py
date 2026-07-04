"""M7 bundle surface: optional fields gate on their FieldSet flag; every field's shape and dtype
match the bundle contract (tab:bundle). S=1, T=4, D=2."""

import torch

from conftest import DECOMPOSITION_FIELDS, OPTIONAL_FIELDS, SHAPE_FIELDS, run


def test_optional_none():
    # All flags off: every optional attribute is None (not absent, not an error).
    off = run(shape=False, decomposition=False, periodogram=False)
    for name in OPTIONAL_FIELDS:
        assert getattr(off, name) is None, f"{name} should be None when its flag is unset"

    # Flags on: the gated attributes become tensors (or, for periodogram, a dict).
    on = run(shape=True, decomposition=True, periodogram=True)
    for name in SHAPE_FIELDS + DECOMPOSITION_FIELDS:
        assert torch.is_tensor(getattr(on, name)), f"{name} should be a tensor when its flag is set"
    assert on.periodogram is not None


def test_shapes_dtypes():
    b = run(shape=True, decomposition=True, periodogram=True)
    s, t, d = 1, 4, 2

    expected = {
        "time": (t,),
        "signal": (t, d),
        "signal_noise": (t, d),
        "source_position": (s, t, 3),
        "source_velocity": (s, t, 3),
        "source_accel": (s, t, 3),
        "source_orientation": (s, t, 4),
        "source_angular_velocity": (s, t, 3),
        "source_angular_accel": (s, t, 3),
        "detector_placement": (d, 7),
        "source_mass": (s,),
        "source_inertia": (s, 3, 3),
        "source_moments": (s, 3),
        "source_axes": (s, 3, 3),
        "source_quadrupole": (s, 3, 3),
        "signal_uldm": (t,),
        "signal_targets": (t, d),
        "signal_atmospheric": (t, d),
        "signal_per_ifo": (t, d, 2),
    }
    for name, shape in expected.items():
        tensor = getattr(b, name)
        assert tuple(tensor.shape) == shape, f"{name}: {tuple(tensor.shape)} != {shape}"
        assert tensor.dtype == torch.float64, f"{name}: dtype {tensor.dtype}"

    # mask is a boolean cycle flag.
    assert tuple(b.mask.shape) == (t,)
    assert b.mask.dtype == torch.bool

    # periodogram: {freqs (F,), power (D, F)}.
    pg = b.periodogram
    assert pg["freqs"].ndim == 1 and pg["freqs"].dtype == torch.float64
    f = pg["freqs"].shape[0]
    assert tuple(pg["power"].shape) == (d, f)

    # meta is a plain dict.
    assert b.meta["seed"] == 3
