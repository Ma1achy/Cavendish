"""Shared fixtures for the SDK fidelity tests. S=1 source, T=4 cycles, D=2 detectors."""

import cavendish as cv

# The optional fields and the FieldSet flag that gates each.
SHAPE_FIELDS = [
    "source_mass",
    "source_inertia",
    "source_moments",
    "source_axes",
    "source_quadrupole",
]
DECOMPOSITION_FIELDS = [
    "signal_uldm",
    "signal_targets",
    "signal_atmospheric",
    "signal_per_ifo",
]
OPTIONAL_FIELDS = SHAPE_FIELDS + DECOMPOSITION_FIELDS + ["periodogram"]


def make_scenario(shape=False, decomposition=False, periodogram=False, seed=3):
    """A static cuboid at 3 m standoff, two detectors, four uniform cycles, with a ULDM line."""
    body = cv.cuboid(half=[0.2, 0.2, 0.2], pitch=0.2, mass=1000.0)
    trajectory = cv.Trajectory(placement=[3.0, 0.0, 0.0])  # Static / Uniform(0) / Fixed(identity)
    array = cv.DetectorArray.line([0.0, 1.0])
    schedule = cv.Schedule.uniform(2.0, 4)
    fields = cv.FieldSet(shape=shape, decomposition=decomposition, periodogram=periodogram)
    return cv.Scenario(
        body,
        trajectory,
        array,
        schedule,
        seed=seed,
        field_set=fields,
        uldm=cv.UldmConfig(amplitude=1e-3, frequency=0.1),
    )


def run(shape=False, decomposition=False, periodogram=False, seed=3):
    return cv.run(make_scenario(shape, decomposition, periodogram, seed))
