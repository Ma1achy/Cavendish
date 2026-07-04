"""M7 stream + GIL: the stream is a memory-bounded, seed-replayable iterator, and heavy Rust work
runs with the GIL released so Python threads overlap it."""

import gc
import threading
import weakref

import cavendish as cv
import torch

from conftest import heavy_scenario, make_prior


def test_gil_released():
    # A pure-Python heartbeat holding the GIL cannot tick while a non-Python thread holds it — unless
    # the Rust compute releases the GIL. So ticks accumulated *during* run() prove the release.
    ticks = [0]
    running = [True]

    def heartbeat():
        while running[0]:
            ticks[0] += 1

    thread = threading.Thread(target=heartbeat)
    thread.start()
    try:
        before = ticks[0]
        cv.run(heavy_scenario())
        delta = ticks[0] - before
    finally:
        running[0] = False
        thread.join()

    assert delta > 100, f"heartbeat barely progressed during run ({delta}) — GIL not released"


def test_stream_iterates():
    # DataLoader-style: consume 30 bundles keeping only the current one. Each previous bundle must be
    # collectable once we advance — proving the stream holds one bundle resident, not the whole run.
    prior = make_prior()
    count = 0
    prev_ref = None
    for bundle in cv.stream(prior, 30, seed=1):
        if prev_ref is not None:
            gc.collect()
            assert prev_ref() is None, "a previous bundle is still alive — the stream accumulates"
        prev_ref = weakref.ref(bundle)
        _ = bundle.signal  # touch it, as a training loop would
        count += 1
        del bundle
    assert count == 30


def test_seeded_replay():
    # Two passes over the same (prior, n, seed) yield identical tensors, value-for-value.
    prior = make_prior()
    first = [b.signal.clone() for b in cv.stream(prior, 8, seed=42)]
    second = [b.signal.clone() for b in cv.stream(prior, 8, seed=42)]
    assert len(first) == len(second) == 8
    for a, b in zip(first, second):
        assert torch.equal(a, b), "seeded stream did not replay identically"
