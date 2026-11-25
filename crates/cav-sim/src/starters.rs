//! Starters: the things that can be added to a scene by name (`add sphere at 2 0 1`), from
//! `scenes/starters.ron`, compiled in. The editor's Add menu, its palette and its Place tool, the
//! console, `cav exec` and MCP all add the same ones (`docs/slices/S18.md`).
//!
//! Each starter is a valid body or tower about the origin. [`make`] moves it to where it is put
//! (every point of its motion shifted by the same offset, so a pendulum keeps its bob below its
//! pivot) and gives it a label no other body or tower has. Frame: metres, x east, y north, z up.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::{BodySpec, SceneSpec, TowerSpec};

/// The starters' file, as compiled in.
pub const STARTERS_RON: &str = include_str!("../../../scenes/starters.ron");
/// Where that file is, relative to the repository's root.
pub const STARTERS_PATH: &str = "scenes/starters.ron";

/// One starter.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Starter {
    /// The word that adds it: `add <name>`.
    pub name: String,
    /// What a menu says.
    pub label: String,
    /// One line about it.
    pub about: String,
    /// The body it is, about the origin; or
    #[serde(default)]
    pub body: Option<BodySpec>,
    /// the tower it is, its base at the origin.
    #[serde(default)]
    pub tower: Option<TowerSpec>,
}

impl Starter {
    /// The list of the scene it goes into: `bodies` or `array.towers`.
    #[must_use]
    pub const fn list(&self) -> &'static str {
        if self.tower.is_some() { "array.towers" } else { "bodies" }
    }
}

/// Every starter, in the order the Add menu shows them.
///
/// # Panics
/// When the built-in file does not read: `every_starter_is_valid_and_unique` fails first.
pub fn starters() -> &'static [Starter] {
    static STARTERS: OnceLock<Vec<Starter>> = OnceLock::new();
    STARTERS.get_or_init(|| ron::from_str(STARTERS_RON).expect("scenes/starters.ron reads (a test holds it)"))
}

/// The starter called `name`.
#[must_use]
pub fn find(name: &str) -> Option<&'static Starter> {
    starters().iter().find(|s| s.name == name)
}

/// `base`, or `base 2`, `base 3`, … : the first that `taken` does not hold.
fn unique(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_owned();
    }
    (2..).map(|n| format!("{base} {n}")).find(|l| !taken(l)).expect("a free label")
}

/// How far above its anchor a body's lowest point is not: the height that puts it *on* a surface
/// rather than half through it, metres. A voxel shape's half height, a sphere's radius, a point's
/// nothing.
#[must_use]
pub fn lift(body: &BodySpec) -> f64 {
    use crate::ShapeKind;
    match &body.shape {
        Some(shape) => match &shape.kind {
            ShapeKind::Sphere { radius } => *radius,
            ShapeKind::Shell { outer, .. } => *outer,
            ShapeKind::Cuboid { size } => size[2] / 2.0,
            ShapeKind::Cylinder { radius, length, axis } => {
                if *axis == crate::Axis::Z {
                    length / 2.0
                } else {
                    *radius
                }
            }
            ShapeKind::Ellipsoid { radii } => radii[2],
            // The circumradius bounds every solid's half height.
            ShapeKind::Platonic { solid, edge } => edge * circumradius(*solid),
            ShapeKind::Polyhedron { vertices } => vertices.iter().map(|v| -v.0[2]).fold(0.0_f64, f64::max),
            ShapeKind::Mesh { .. } => 0.0,
        },
        None => body.radius,
    }
}

/// A Platonic solid's circumradius for a unit edge.
fn circumradius(solid: crate::PlatonicSolid) -> f64 {
    use crate::PlatonicSolid as P;
    match solid {
        P::Tetrahedron => (6.0_f64).sqrt() / 4.0,
        P::Cube => (3.0_f64).sqrt() / 2.0,
        P::Octahedron => (2.0_f64).sqrt() / 2.0,
        P::Dodecahedron => (3.0_f64).sqrt() * (1.0 + 5.0_f64.sqrt()) / 4.0,
        P::Icosahedron => (10.0 + 2.0 * 5.0_f64.sqrt()).sqrt() / 4.0,
    }
}

/// The item that `add <name> at <at>` puts into `spec`: the list it goes into and the value, as
/// JSON. The starter is moved by `at` (metres, the world's frame) and labelled so that no body
/// (or tower) of `spec` has its label.
///
/// # Errors
/// When there is no starter of that name; the message lists them.
pub fn make(spec: &SceneSpec, name: &str, at: [f64; 3]) -> Result<(&'static str, serde_json::Value), String> {
    let starter = find(name).ok_or_else(|| {
        let names: Vec<&str> = starters().iter().map(|s| s.name.as_str()).collect();
        format!("`{name}` is not something to add; the starters are: {}", names.join(", "))
    })?;
    if let Some(tower) = &starter.tower {
        let taken = |label: &str| spec.array.towers.iter().any(|t| t.label == label);
        let base = format!("tower {}", spec.array.towers.len() + 1);
        let tower = TowerSpec {
            label: unique(if taken(&base) { &tower.label } else { &base }, taken),
            position: [tower.position[0] + at[0], tower.position[1] + at[1], tower.position[2] + at[2]],
            ..tower.clone()
        };
        return Ok((starter.list(), serde_json::to_value(tower).expect("a tower is JSON")));
    }
    let body = starter.body.as_ref().expect("a starter is a body or a tower (a test holds it)");
    let taken = |label: &str| spec.bodies.iter().any(|b| b.label == label);
    let body =
        BodySpec { label: unique(&body.label, taken), motion: body.motion.shifted(at), ..body.clone() };
    Ok((starter.list(), serde_json::to_value(body).expect("a body is JSON")))
}

/// The copy of the item at `path` (`bodies[2]`, `array.towers[1]`) that `duplicate <path>` adds:
/// beside the original (half a metre east for a body, a metre for a tower), labelled anew.
///
/// # Errors
/// When the path is not a body or a tower of `spec`.
pub fn duplicate(spec: &SceneSpec, path: &str) -> Result<(&'static str, serde_json::Value), String> {
    let index = |prefix: &str| {
        path.strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(']'))
            .and_then(|n| n.parse::<usize>().ok())
    };
    if let Some(n) = index("bodies[") {
        let body = spec.bodies.get(n).ok_or_else(|| format!("there is no body {path}"))?;
        let taken = |label: &str| spec.bodies.iter().any(|b| b.label == label);
        let copy = BodySpec {
            label: unique(&body.label, taken),
            motion: body.motion.shifted([0.5, 0.0, 0.0]),
            ..body.clone()
        };
        return Ok(("bodies", serde_json::to_value(copy).expect("a body is JSON")));
    }
    if let Some(n) = index("array.towers[") {
        let tower = spec.array.towers.get(n).ok_or_else(|| format!("there is no tower {path}"))?;
        let taken = |label: &str| spec.array.towers.iter().any(|t| t.label == label);
        let [x, y, z] = tower.position;
        let copy =
            TowerSpec { label: unique(&tower.label, taken), position: [x + 1.0, y, z], ..tower.clone() };
        return Ok(("array.towers", serde_json::to_value(copy).expect("a tower is JSON")));
    }
    Err(format!("`{path}` is not a body or a tower: `duplicate bodies[0]`, `duplicate array.towers[2]`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_starter_is_valid_and_unique() {
        let all = starters();
        assert!(all.len() >= 12, "{}", all.len());
        let mut names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len(), "names are unique");
        for scene in [SceneSpec::room(), crate::library::load("void").unwrap()] {
            for starter in all {
                assert!(
                    starter.body.is_some() != starter.tower.is_some(),
                    "{}: one of body or tower",
                    starter.name
                );
                let (list, value) = make(&scene, &starter.name, [3.0, 2.0, 1.0]).unwrap();
                let (once, _) = scene
                    .inserted(list, None, &value)
                    .unwrap_or_else(|why| panic!("{}: {why}", starter.name));
                // A second of the same goes in too: its label is new.
                let (list, again) = make(&once, &starter.name, [3.0, -2.0, 1.0]).unwrap();
                let (twice, _) = once
                    .inserted(list, None, &again)
                    .unwrap_or_else(|why| panic!("{} twice: {why}", starter.name));
                if list == "bodies" {
                    let labels: Vec<&str> = twice.bodies.iter().map(|b| b.label.as_str()).collect();
                    assert_ne!(labels[labels.len() - 1], labels[labels.len() - 2], "{}", starter.name);
                    // It went where it was put: the motion shifted by `at`.
                    let body = &twice.bodies[twice.bodies.len() - 2];
                    let shifted = starter.body.as_ref().unwrap().motion.anchor().0;
                    assert_eq!(
                        body.motion.anchor().0,
                        [shifted[0] + 3.0, shifted[1] + 2.0, shifted[2] + 1.0]
                    );
                } else {
                    assert_eq!(twice.array.towers.len(), scene.array.towers.len() + 2);
                }
            }
        }
        // Control: a name that is not a starter is refused, with the list.
        let why = make(&SceneSpec::room(), "unicorn", [0.0; 3]).unwrap_err();
        assert!(why.contains("sphere") && why.contains("unicorn"), "{why}");
    }

    #[test]
    fn a_duplicate_is_beside_its_original_with_a_new_label() {
        let scene = crate::library::load("point").unwrap();
        let (list, value) = duplicate(&scene, "bodies[0]").unwrap();
        let (twice, _) = scene.inserted(list, None, &value).unwrap();
        let (a, b) = (&twice.bodies[0], &twice.bodies[1]);
        assert_ne!(a.label, b.label);
        let (pa, pb) = (a.motion.anchor().0, b.motion.anchor().0);
        assert_eq!([pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]], [0.5, 0.0, 0.0]);
        let (list, value) = duplicate(&scene, "array.towers[1]").unwrap();
        assert!(scene.inserted(list, None, &value).is_ok());
        assert!(duplicate(&scene, "room").is_err());
        assert!(duplicate(&scene, "bodies[7]").is_err());
    }

    #[test]
    fn lift_puts_a_shape_on_the_floor() {
        let sphere = find("sphere").unwrap().body.as_ref().unwrap();
        assert_eq!(lift(sphere), 0.25);
        let cube = find("cube").unwrap().body.as_ref().unwrap();
        assert_eq!(lift(cube), 0.25);
        let point = find("point-mass").unwrap().body.as_ref().unwrap();
        assert_eq!(lift(point), 0.0);
    }
}
