//! The dev and debug scenes of REBUILD §9.1: scenes whose answer is known.
//!
//! Each is a file in `scenes/dev/` and a builder here; a test holds the two together, so the file a
//! person reads and the scene a test builds cannot drift apart. S0 has the first two, both empty of
//! moving matter. Their null tests (every channel exactly zero; the floor's differenced phase zero)
//! need the forward model and arrive with S1 (TASK-S1-07).

use crate::{FloorKind, FloorSpec, SceneSpec};

/// `void`: the array and nothing else.
#[must_use]
pub fn void() -> SceneSpec {
    SceneSpec {
        name: "void".into(),
        description: "The array and nothing else: no room, no air, no ground. Every channel must read \
                      exactly zero; the renderer must cope with an empty world."
            .into(),
        room: None,
        floor: None,
        ..SceneSpec::room()
    }
}

/// `infinite-floor`: the array over a uniform half-space of concrete.
#[must_use]
pub fn infinite_floor() -> SceneSpec {
    SceneSpec {
        name: "infinite-floor".into(),
        description: "A uniform half-space below the array, drawn as a grid to the horizon. An infinite \
                      uniform slab gives uniform g and no gradient, so its differenced phase is zero. \
                      Empty in S0: the floor is declared and drawn; its physics arrives with S1."
            .into(),
        room: None,
        floor: Some(FloorSpec { height: 0.0, kind: FloorKind::HalfSpace, density: 2400.0 }),
        ..SceneSpec::room()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;

    #[test]
    fn void_is_the_array_and_nothing_else() {
        let void = load("void").unwrap();
        assert_eq!(void, super::void(), "scenes/dev/void.ron is the builder's scene");
        assert_eq!(void.array, SceneSpec::room().array, "the same array as the default scene");
        assert_eq!(void.array.towers.len(), 5);
        assert!(void.room.is_none() && void.floor.is_none());
        // Control: the default scene is not a void, so the comparison can tell them apart.
        assert_ne!(
            SceneSpec { name: "void".into(), description: void.description.clone(), ..SceneSpec::room() },
            void
        );
    }

    #[test]
    fn infinite_floor_adds_only_a_floor() {
        let floor = load("infinite-floor").unwrap();
        assert_eq!(floor, infinite_floor(), "scenes/dev/infinite-floor.ron is the builder's scene");
        let spec = floor.floor.clone().expect("the scene has a floor");
        assert_eq!(spec.kind, FloorKind::HalfSpace);
        assert_eq!(spec.height, 0.0, "the towers stand on it");
        // Apart from the floor and the words, it is the void.
        let stripped =
            SceneSpec { floor: None, name: "void".into(), description: void().description, ..floor };
        assert_eq!(stripped, void());
        // Control: the floor is what differs.
        assert_ne!(infinite_floor().floor, void().floor);
    }
}
