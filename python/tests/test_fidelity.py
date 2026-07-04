"""M7 fidelity crux: the torch tensors equal the Rust bundle value-for-value, and the CPU hand-off
is genuinely zero-copy (torch wraps the exact Rust allocation). CUDA is CPU-only for now — skipped
honestly, not claimed."""

import pytest
import torch

from conftest import run

# Fields with a native-list reference in Bundle._raw (spanning 1-D, 2-D, 3-D, bool, and optionals).
FIDELITY_FIELDS = [
    "time",
    "signal",
    "signal_noise",
    "mask",
    "source_position",
    "detector_placement",
    "signal_targets",
    "source_mass",
]


def test_fidelity():
    # `_raw(name)` rebuilds the field as native Python lists straight from the Rust Vec, bypassing
    # DLPack — an independent path. Equality proves the DLPack flatten/stride did not corrupt a value.
    b = run(shape=True, decomposition=True)
    for name in FIDELITY_FIELDS:
        tensor = getattr(b, name)
        assert tensor.tolist() == b._raw(name), f"{name}: DLPack tensor != raw Rust values"


def test_zero_copy_cpu():
    # A genuine zero-copy hand-off: torch wraps the exact host allocation the SDK built, so the
    # tensor's data pointer equals the Rust buffer address. A copy would report a different pointer.
    b = run()
    tensor, addr = b._zero_copy_probe()
    assert tensor.device.type == "cpu"
    assert tensor.data_ptr() == addr, "torch copied instead of sharing the Rust buffer"


def test_zero_copy_cuda():
    # generate::run is CPU-only, so no bundle is CUDA-resident: there is no device tensor to probe.
    # The DLPack path is device-general (dlpark carries the DLDevice), but this invariant cannot be
    # exercised until a GPU-resident bundle exists. Skipped, not claimed as a pass.
    pytest.skip("run() is CPU-only — no CUDA-resident bundle; CUDA zero-copy is scaffolded, not exercised")
    assert torch.cuda.is_available()  # unreachable; documents the intended precondition
