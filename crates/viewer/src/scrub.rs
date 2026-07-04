//! The scrubber's index → time map. One path for uniform **and** gappy schedules: the map goes through
//! the actual `time` array, never `index = t × rate` (which desyncs the views silently on a gap). The 3D
//! pose and the 2D cursor both read this one map, so they show the same instant `ℓ`.

/// The measurement time at scrub index `idx`, or `None` if out of range.
pub fn time_at(times: &[f64], idx: usize) -> Option<f64> {
    times.get(idx).copied()
}

/// Clamp a scrub index into `[0, len)` (an empty schedule maps to 0).
pub fn clamp_index(idx: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        idx.min(len - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_maps_index() {
        // Uniform: index ℓ → times[ℓ] exactly.
        let uniform: Vec<f64> = (0..10).map(|i| 2.0 * i as f64).collect();
        for l in 0..uniform.len() {
            assert_eq!(time_at(&uniform, l), Some(2.0 * l as f64));
        }
        assert_eq!(time_at(&uniform, 10), None);

        // Gappy/jittered: the index still maps through the actual array — NOT index × rate. A naïve
        // uniform map would read index 2 as 2·rate and desync the 3D pose from the 2D cursor.
        let gappy = vec![0.0, 1.0, 5.0, 5.5, 20.0];
        assert_eq!(time_at(&gappy, 0), Some(0.0));
        assert_eq!(time_at(&gappy, 2), Some(5.0));
        assert_eq!(time_at(&gappy, 4), Some(20.0));
        assert_eq!(time_at(&gappy, 5), None);
        assert_ne!(
            time_at(&gappy, 2),
            Some(2.0),
            "must not assume uniform spacing"
        );
    }

    #[test]
    fn clamp_empty_and_over() {
        assert_eq!(clamp_index(4, 0), 0);
        assert_eq!(clamp_index(9, 5), 4);
        assert_eq!(clamp_index(2, 5), 2);
    }
}
