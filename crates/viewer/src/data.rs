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
    use crate::params::{build_scenario, ScenarioParams};
    use crate::scene::scene_at;

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
            !scene.instances.is_empty(),
            "loaded bundle rendered an empty scene — no geometry survived serialisation"
        );
    }
}
