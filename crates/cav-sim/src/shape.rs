//! A body's shape, as plain data: what is voxelised, at what pitch, and the tree it becomes.
//!
//! A body with a shape is a solid of uniform density made of voxels ([`cav_core::voxel`]): its
//! shape is centred on the body's position, voxelised at `pitch` with `supersample³` sample points
//! a finest cell, and read leaf by leaf, each leaf a uniform cube. Its mass is the body's `mass`,
//! to rounding, however the shape voxelises. Frame: the shape's own axes are the world's (bodies
//! do not turn in S3), metres.

use cav_core::Vec3;
use cav_core::voxel::shape::{Cuboid, Cylinder, Ellipsoid, Platonic, Polyhedron, Shape, Shell, Sphere};
use cav_core::voxel::{Budget, Grid, VoxTree, mesh::Mesh, voxelise_within};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The most bytes of voxel trees one scene's bodies may hold at once, together
/// ([`VoxTree::bytes`]): what the engine keeps for drawing a scene, and what a scene's trees are
/// counted against before each is made (`docs/slices/S3.md` §3.1, V10). One tree at the
/// voxeliser's own caps (about 190 MiB) fits; tens of fine bodies do not, and the ones past it
/// are refused with the numbers.
pub const MAX_SCENE_TREE_BYTES: usize = 256 << 20;

/// A body's shape and how finely it is voxelised.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShapeSpec {
    /// What solid it is, centred on the body's position.
    pub kind: ShapeKind,
    /// The side of a finest voxel, metres. At most 1024 of them across the shape's widest side.
    #[schemars(range(max = 1.0e3), extend("exclusiveMinimum" = 0.0))]
    #[schemars(extend("x-unit" = "m"))]
    pub pitch: f64,
    /// Sample points along each side of a finest voxel, from 1 to 6: how finely a voxel the
    /// surface cuts is weighed. The default is 4 (64 points a voxel).
    #[serde(default = "default_supersample")]
    #[schemars(range(min = 1, max = 6))]
    #[schemars(extend("x-unit" = "1"))]
    pub supersample: u8,
}

const fn default_supersample() -> u8 {
    4
}

/// Which axis something lies along.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Axis {
    /// East.
    X,
    /// North.
    Y,
    /// Up.
    Z,
}

/// One of the five Platonic solids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PlatonicSolid {
    /// Four triangles.
    Tetrahedron,
    /// Six squares.
    Cube,
    /// Eight triangles.
    Octahedron,
    /// Twelve pentagons.
    Dodecahedron,
    /// Twenty triangles.
    Icosahedron,
}

/// The solids a body can be. Each is centred on the body's position (a mesh on its centroid).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum ShapeKind {
    /// A ball.
    Sphere {
        /// Metres.
        #[schemars(range(max = 1.0e3), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m"))]
        radius: f64,
    },
    /// A hollow ball: the points between two concentric spheres.
    Shell {
        /// Its outer radius, metres.
        #[schemars(range(max = 1.0e3), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m"))]
        outer: f64,
        /// The radius of its cavity, metres: from 0 (a ball) to less than `outer`.
        #[schemars(range(min = 0.0, max = 1.0e3))]
        #[schemars(extend("x-unit" = "m"))]
        inner: f64,
    },
    /// A box with its faces along the axes.
    Cuboid {
        /// Its size along x, y and z, metres; each above 0 and at most 1e3.
        #[schemars(extend("x-unit" = "m"))]
        size: [f64; 3],
    },
    /// A solid circular cylinder.
    Cylinder {
        /// Metres.
        #[schemars(range(max = 1.0e3), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m"))]
        radius: f64,
        /// Its length along its axis, metres.
        #[schemars(range(max = 1.0e3), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m"))]
        length: f64,
        /// Which axis it lies along.
        axis: Axis,
    },
    /// An ellipsoid with its axes along x, y and z.
    Ellipsoid {
        /// Its semi-axes along x, y and z, metres; each above 0 and at most 1e3.
        #[schemars(extend("x-unit" = "m"))]
        radii: [f64; 3],
    },
    /// A Platonic solid in its textbook orientation.
    Platonic {
        /// Which.
        solid: PlatonicSolid,
        /// The length of an edge, metres.
        #[schemars(range(max = 1.0e3), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m"))]
        edge: f64,
    },
    /// The convex hull of the vertices given, relative to the body's position: from 4 to 100
    /// of them.
    Polyhedron {
        /// The corners; each coordinate within ±1e3 m.
        #[schemars(length(min = 4, max = 100))]
        vertices: Vec<Vertex>,
    },
    /// A closed triangle mesh from a file (`.obj` or `.stl`), centred on its centroid.
    Mesh {
        /// The file, relative to the directory the program runs in. At most 32 MiB and 500,000
        /// triangles; every edge in two triangles.
        #[schemars(length(min = 1, max = 4000))]
        file: String,
        /// Metres per unit of the file.
        #[schemars(range(max = 1.0e6), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m"))]
        scale: f64,
    },
}

/// A corner of a polyhedron: `[x, y, z]` (east, north, up), metres, from the body's position.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[schemars(extend("x-unit" = "m"))]
pub struct Vertex(pub [f64; 3]);

/// The largest size of a shape, metres.
pub const MAX_SIZE: f64 = 1.0e3;

impl ShapeKind {
    /// The names of the kinds, as the spec writes them.
    pub const KINDS: [&'static str; 8] =
        ["Sphere", "Shell", "Cuboid", "Cylinder", "Ellipsoid", "Platonic", "Polyhedron", "Mesh"];

    /// The kind's name as written in a scene file.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Sphere { .. } => "Sphere",
            Self::Shell { .. } => "Shell",
            Self::Cuboid { .. } => "Cuboid",
            Self::Cylinder { .. } => "Cylinder",
            Self::Ellipsoid { .. } => "Ellipsoid",
            Self::Platonic { .. } => "Platonic",
            Self::Polyhedron { .. } => "Polyhedron",
            Self::Mesh { .. } => "Mesh",
        }
    }

    /// The solid, centred on the origin, ready to voxelise. A mesh is read from its file here.
    ///
    /// # Errors
    /// When a polyhedron's vertices span no volume, or a mesh cannot be read or is refused.
    pub fn solid(&self) -> Result<Box<dyn Shape>, String> {
        Ok(match self {
            Self::Sphere { radius } => Box::new(Sphere { centre: Vec3::ZERO, radius: *radius }),
            Self::Shell { outer, inner } => {
                Box::new(Shell { centre: Vec3::ZERO, outer: *outer, inner: *inner })
            }
            Self::Cuboid { size } => {
                Box::new(Cuboid { centre: Vec3::ZERO, half: Vec3::from_array(*size) * 0.5 })
            }
            Self::Cylinder { radius, length, axis } => Box::new(Cylinder {
                centre: Vec3::ZERO,
                radius: *radius,
                half_length: 0.5 * length,
                axis: *axis as usize,
            }),
            Self::Ellipsoid { radii } => {
                Box::new(Ellipsoid { centre: Vec3::ZERO, radii: Vec3::from_array(*radii) })
            }
            Self::Platonic { solid, edge } => Box::new(platonic(*solid).solid(*edge)),
            Self::Polyhedron { vertices } => {
                let points: Vec<Vec3> = vertices.iter().map(|v| Vec3::from_array(v.0)).collect();
                Box::new(Polyhedron::hull(&points).map_err(|e| e.0)?)
            }
            Self::Mesh { file, scale } => {
                Box::new(Mesh::read(std::path::Path::new(file), *scale).map_err(|e| e.0)?.centred())
            }
        })
    }

    /// Checks the numbers (the file of a mesh is read only when the body is voxelised).
    ///
    /// # Errors
    /// The first field that fails, by its path under `at`.
    pub fn validate(&self, at: &str) -> Result<(), String> {
        let at = format!("{at}.{}", self.name());
        let size = |name: &str, value: f64| {
            (value.is_finite() && value > 0.0 && value <= MAX_SIZE)
                .then_some(())
                .ok_or_else(|| format!("{at}.{name}: {value} m must be above 0 and at most {MAX_SIZE}"))
        };
        match self {
            Self::Sphere { radius } => size("radius", *radius),
            Self::Shell { outer, inner } => size("outer", *outer).and(
                (inner.is_finite() && *inner >= 0.0 && inner < outer).then_some(()).ok_or_else(|| {
                    format!("{at}.inner: {inner} m must be from 0 to less than the outer radius")
                }),
            ),
            Self::Cuboid { size: s } => s.iter().try_for_each(|v| size("size", *v)),
            Self::Cylinder { radius, length, .. } => size("radius", *radius).and(size("length", *length)),
            Self::Ellipsoid { radii } => radii.iter().try_for_each(|v| size("radii", *v)),
            Self::Platonic { edge, .. } => size("edge", *edge),
            Self::Polyhedron { vertices } => {
                if !(4..=cav_core::voxel::shape::MAX_POLYHEDRON_VERTICES).contains(&vertices.len()) {
                    return Err(format!("{at}.vertices: {} vertices; from 4 to 100", vertices.len()));
                }
                if vertices.iter().flat_map(|v| v.0).any(|c| !(c.is_finite() && c.abs() <= MAX_SIZE)) {
                    return Err(format!("{at}.vertices: every coordinate within ±{MAX_SIZE} m"));
                }
                let points: Vec<Vec3> = vertices.iter().map(|v| Vec3::from_array(v.0)).collect();
                Polyhedron::hull(&points).map(|_| ()).map_err(|e| format!("{at}.vertices: {e}"))
            }
            Self::Mesh { file, scale } => {
                if file.trim().is_empty() || file.chars().count() > 4000 {
                    return Err(format!("{at}.file: a file name of 1 to 4000 characters"));
                }
                (scale.is_finite() && *scale > 0.0 && *scale <= 1.0e6)
                    .then_some(())
                    .ok_or_else(|| format!("{at}.scale: {scale} must be above 0 and at most 1e6"))
            }
        }
    }
}

fn platonic(solid: PlatonicSolid) -> Platonic {
    match solid {
        PlatonicSolid::Tetrahedron => Platonic::Tetrahedron,
        PlatonicSolid::Cube => Platonic::Cube,
        PlatonicSolid::Octahedron => Platonic::Octahedron,
        PlatonicSolid::Dodecahedron => Platonic::Dodecahedron,
        PlatonicSolid::Icosahedron => Platonic::Icosahedron,
    }
}

impl ShapeSpec {
    /// Checks the shape and that its pitch fits one tree (at most 1024 voxels across).
    ///
    /// # Errors
    /// The first field that fails, by its path under `at` (such as `bodies[0].shape`).
    pub fn validate(&self, at: &str) -> Result<(), String> {
        self.kind.validate(&format!("{at}.kind"))?;
        if !(1..=cav_core::voxel::MAX_SUPERSAMPLE).contains(&self.supersample) {
            return Err(format!("{at}.supersample: {} must be from 1 to 6", self.supersample));
        }
        if !(self.pitch.is_finite() && self.pitch > 0.0 && self.pitch <= MAX_SIZE) {
            return Err(format!("{at}.pitch: {} m must be above 0 and at most {MAX_SIZE}", self.pitch));
        }
        // The grid needs only the shape's box, which a mesh does not give without its file.
        if let Some(bounds) = self.bounds() {
            Grid::around(bounds, self.pitch, self.supersample).map_err(|e| format!("{at}.pitch: {e}"))?;
        }
        Ok(())
    }

    /// The shape's box centred on the origin, when it is known without reading a file.
    fn bounds(&self) -> Option<cav_core::voxel::Aabb> {
        match self.kind {
            ShapeKind::Mesh { .. } => None,
            ShapeKind::Polyhedron { .. } => self.kind.solid().ok().map(|s| s.bounds()),
            _ => self.kind.solid().ok().map(|s| s.bounds()),
        }
    }

    /// The body's voxel tree: the shape voxelised at its pitch, `mass` kilograms in all, in the
    /// body's own frame (the shape's centre at its origin).
    ///
    /// # Errors
    /// When the shape cannot be made, or the voxeliser refuses it (too fine, too many leaves, no
    /// sample point inside), in words that name the number.
    pub fn tree(&self, mass: f64) -> Result<VoxTree, String> {
        self.tree_within(mass, usize::MAX)
    }

    /// [`Self::tree`] in at most `bytes` bytes of tree ([`VoxTree::bytes`]): what is left of a
    /// scene's [`MAX_SCENE_TREE_BYTES`] when the trees before it are made. Counted as the leaves
    /// are emitted and from the nodes before they are reserved, so a tree that would not fit is
    /// refused before it is whole.
    ///
    /// # Errors
    /// As [`Self::tree`], and when the tree would weigh more than `bytes`, with the numbers.
    pub fn tree_within(&self, mass: f64, bytes: usize) -> Result<VoxTree, String> {
        let solid = self.kind.solid()?;
        let grid = Grid::around(solid.bounds(), self.pitch, self.supersample).map_err(|e| e.0)?;
        voxelise_within(solid.as_ref(), &grid, mass, 0, Budget::with_bytes(bytes)).map_err(|e| e.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_voxelises_and_is_named() {
        let kinds = [
            ShapeKind::Sphere { radius: 0.3 },
            ShapeKind::Shell { outer: 0.3, inner: 0.2 },
            ShapeKind::Cuboid { size: [0.4, 0.3, 0.2] },
            ShapeKind::Cylinder { radius: 0.2, length: 0.6, axis: Axis::Y },
            ShapeKind::Ellipsoid { radii: [0.3, 0.2, 0.1] },
            ShapeKind::Platonic { solid: PlatonicSolid::Dodecahedron, edge: 0.2 },
            ShapeKind::Polyhedron {
                vertices: [[0.0; 3], [0.3, 0.0, 0.0], [0.0, 0.3, 0.0], [0.0, 0.0, 0.3]].map(Vertex).to_vec(),
            },
        ];
        assert_eq!(kinds.len() + 1, ShapeKind::KINDS.len(), "every kind but the mesh, which needs a file");
        for kind in kinds {
            let spec = ShapeSpec { kind, pitch: 0.05, supersample: 2 };
            assert_eq!(spec.validate("shape"), Ok(()));
            let tree = spec.tree(12.0).unwrap();
            assert!((tree.body().mass - 12.0).abs() < 1e-12, "{:?}", spec.kind);
            assert!(ShapeKind::KINDS.contains(&spec.kind.name()));
        }
        // Refusals name the field: a pitch too fine for one tree, a size of zero, k of 7.
        let fine = ShapeSpec { kind: ShapeKind::Sphere { radius: 1.0 }, pitch: 1e-3, supersample: 2 };
        assert!(fine.validate("bodies[0].shape").unwrap_err().starts_with("bodies[0].shape.pitch"));
        let flat =
            ShapeSpec { kind: ShapeKind::Cuboid { size: [1.0, 0.0, 1.0] }, pitch: 0.1, supersample: 2 };
        assert!(flat.validate("s").unwrap_err().starts_with("s.kind.Cuboid.size"));
        let seven = ShapeSpec { kind: ShapeKind::Sphere { radius: 1.0 }, pitch: 0.1, supersample: 7 };
        assert!(seven.validate("s").unwrap_err().starts_with("s.supersample"));
        let missing = ShapeSpec {
            kind: ShapeKind::Mesh { file: "no/such/file.obj".into(), scale: 1.0 },
            pitch: 0.1,
            supersample: 2,
        };
        assert_eq!(missing.validate("s"), Ok(()), "a file is read when the body is voxelised");
        assert!(missing.tree(1.0).is_err());
    }
}
