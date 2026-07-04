//! M6b `stream_bounded`: streaming 10⁴ scenarios holds only a bounded number of bundles resident —
//! peak live memory does not scale with `n`. Measured with a counting global allocator; this file is a
//! single test so nothing else in the binary perturbs the counter.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let now = LIVE.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
        System.dealloc(p, l);
    }
}

#[global_allocator]
static GA: Counting = Counting;

use generate::{
    run, scenario_key, stream, Detector, DetectorArray, Dist, Prior, RunConfig, Schedule,
};
use shape::{voxelise, Cuboid, MassSpec, VoxelParams};

fn prior() -> Prior {
    Prior {
        cloud: voxelise(
            &Cuboid {
                half: [0.15, 0.15, 0.15],
            },
            &VoxelParams::pitch(0.15),
            MassSpec::Total(1000.0),
        )
        .unwrap(),
        fields: vec![
            (
                "mass".into(),
                Dist::Uniform {
                    lo: 500.0,
                    hi: 1500.0,
                },
            ),
            ("standoff".into(), Dist::Uniform { lo: 2.0, hi: 5.0 }),
        ],
        array: DetectorArray::new(vec![Detector::new(0.0)]),
        schedule: Schedule::uniform(2.0, 1),
        field_set: Default::default(),
        atmo: None,
    }
}

/// Peak live bytes above the pre-stream baseline while fully consuming `stream(n)`.
fn peak_above_baseline(n: usize, p: &Prior) -> usize {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let mut count = 0usize;
    for _bundle in stream(p, n, 1, RunConfig { batch: 64 }) {
        count += 1;
    }
    assert_eq!(count, n);
    PEAK.load(Ordering::Relaxed).saturating_sub(base)
}

#[test]
fn stream_bounded() {
    let p = prior();
    let small = peak_above_baseline(200, &p);
    let large = peak_above_baseline(10_000, &p);
    // 50× more scenarios must not mean 50× more peak — memory is bounded by the batch, not the run.
    assert!(
        large <= 2 * small.max(1),
        "stream peak grew with n: {small} -> {large} bytes"
    );

    // The lazy stream and a direct per-index sample agree bundle-for-bundle (batch-invariance seam):
    // scenario i's seed hangs off its index, not its position in the stream.
    let direct = run(&p.sample(scenario_key(1, 3)));
    let streamed = stream(&p, 4, 1, RunConfig { batch: 2 }).nth(3).unwrap();
    assert_eq!(direct.signal, streamed.signal);
}
