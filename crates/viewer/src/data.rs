//! The two data paths. LIVE: `run_guarded` runs a scenario but catches any panic as a message — the
//! viewer stays alive whatever happens below it (`generate::run` is infallible today, so this only ever
//! fires on a genuine panic). LOADED: a serialised bundle read back through `state::load_bundle`,
//! rendered read-only with no re-run.

use generate::{run, Scenario};
use state::StateBundle;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Run a scenario, converting a panic into a message rather than a crash. `AssertUnwindSafe` because a
/// `Scenario` (holding a `Box<dyn SourceDynamics>`) is not `UnwindSafe`, and we are only catching to
/// keep the tool alive — not to resume the failed computation.
pub fn run_guarded(scenario: &Scenario) -> Result<StateBundle, String> {
    catch_unwind(AssertUnwindSafe(|| run(scenario))).map_err(|_| {
        "run panicked — the scenario was rejected (the viewer stays alive)".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::run_guarded;
    use crate::params::{build_scenario, ScenarioParams};
    use crate::scene::scene_at;
    use generate::{
        BodyMotion, Detector, DetectorArray, Isometry3, Scenario, Schedule, SourceDynamics,
    };
    use gravity::Cloud;
    use state::StateBundle;

    /// A source that panics the moment the forward model touches it — the induced failure.
    struct PanicSource;
    impl SourceDynamics for PanicSource {
        fn body_cloud(&self) -> &Cloud {
            panic!("induced failure")
        }
        fn pose_at(&self, _t: f64) -> Isometry3 {
            Isometry3::identity()
        }
        fn motion_at(&self, _t: f64) -> BodyMotion {
            unreachable!()
        }
    }

    #[test]
    fn fails_soft() {
        // A run that panics becomes a message (not a crash); a degenerate scene renders empty (not a
        // panic); a normal run succeeds. The tool stays alive through all three — the viewer's spine.
        let panicking = Scenario::new(
            Box::new(PanicSource),
            DetectorArray::single(Detector::new(0.0)),
            Schedule::uniform(2.0, 4),
            1,
        );
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {})); // silence the induced panic's backtrace
        let caught = run_guarded(&panicking);
        std::panic::set_hook(prev);
        assert!(
            caught.is_err(),
            "a panicking run must be caught as a message"
        );

        // A degenerate (empty) bundle renders an empty scene — no geometry, no panic.
        assert!(
            scene_at(&StateBundle::default(), 0).cubes.is_empty(),
            "a degenerate bundle should yield an empty scene, not a panic"
        );

        // And a normal run still succeeds — fails-soft does not swallow the good path.
        assert!(run_guarded(&build_scenario(&ScenarioParams::default())).is_ok());
    }

    #[test]
    fn load_bundle_renders() {
        // A run serialised to disk loads back and renders read-only — the loaded path assembles the
        // scene from the bundle ALONE (no scenario in hand), byte-identical to the live bundle.
        let live = super::run(&build_scenario(&ScenarioParams::default()));
        let path = std::env::temp_dir().join("cavendish_viewer_load.bin");
        state::save_bundle(&live, &path).expect("save");
        let loaded = state::load_bundle(&path).expect("load");
        std::fs::remove_file(&path).ok();

        assert_eq!(loaded, live, "loaded bundle differs from the live one");
        let scene = scene_at(&loaded, 0);
        assert!(
            !scene.cubes.is_empty(),
            "loaded bundle rendered an empty scene — no geometry survived serialisation"
        );
    }
}
