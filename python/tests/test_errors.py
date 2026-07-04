"""M7 robust boundary: Rust errors convert to Python exceptions and no Rust panic crosses the FFI as
a panic (which would abort the interpreter)."""

import cavendish as cv
import pytest

from conftest import make_prior, make_scenario


def test_errors_convert():
    body = cv.cuboid(half=[0.15, 0.15, 0.15], pitch=0.15, mass=1000.0)
    array = cv.DetectorArray.line([0.0])
    schedule = cv.Schedule.uniform(2.0, 4)

    # A validation failure (inverted uniform range) surfaces as ValueError at construction.
    with pytest.raises(ValueError):
        cv.Prior(body, [("mass", cv.Dist.uniform(1500.0, 500.0))], array, schedule)

    # An induced Rust panic surfaces as RuntimeError (converted at the boundary), not an abort.
    with pytest.raises(RuntimeError):
        cv._panic_probe()


def test_no_panic_across():
    # The panic converts to an exception; crucially, the interpreter survives and stays usable.
    with pytest.raises(RuntimeError):
        cv._panic_probe()
    assert cv.run(make_scenario()).signal is not None
    # And a stream still runs after the panic was caught.
    bundles = list(cv.stream(make_prior(), 2, seed=1))
    assert len(bundles) == 2
