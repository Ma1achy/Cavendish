//! A body's shape, as plain data: what is voxelised, at what pitch, of what, and the tree it
//! becomes.
//!
//! A body with a shape is made of voxels ([`cav_core::voxel`]): its shape is centred on the
//! body's position, voxelised at `pitch` with `supersample³` sample points a finest cell, and read
//! leaf by leaf, each leaf a uniform cube. With no `material` it is uniform and its mass is the
//! body's `mass`, to rounding, however the shape voxelises. With a `material` (S19) its mass is
//! its materials' densities times their volumes as voxelised, and it may have `parts`: further
//! solids and brush strokes that add matter of another material, paint one into it, or erase,
//! in order ([`cav_core::voxel::layers`]). Frame: the shape's own axes are the world's (bodies do
//! not turn), metres.

use cav_core::Vec3;
use cav_core::polyhedral::Faceted;
use cav_core::voxel::layers::{self, Layer, Mode, Moved, Region, Stroke, Whole};
use cav_core::voxel::shape::{Cuboid, Cylinder, Ellipsoid, Platonic, Polyhedron, Shape, Shell, Sphere};
use cav_core::voxel::{Aabb, Budget, Grid, VoxTree, mesh::Mesh, voxelise_within};

use crate::materials::MaterialSpec;
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
    /// What it is made of: a material's label, the scene's own `materials` first, then the
    /// library's (`scenes/materials.ron`). With one, the body weighs its density times its volume
    /// as voxelised, and the body's `mass` is not used; with none, it is uniform and weighs its
    /// `mass`. (Written even when none, so that a scene's file and the inspector show the
    /// choice.)
    #[serde(default)]
    #[schemars(length(min = 1, max = 200))]
    pub material: Option<String>,
    /// Further solids laid in order after the shape, for a body with a material: matter of a
    /// material added, a material painted into what is there, or matter erased. At most 255.
    /// (Written even when empty, so that the inspector and the console can add to it.)
    #[serde(default)]
    #[schemars(length(max = 255))]
    pub parts: Vec<PartSpec>,
    /// How the forward model reads it: by its voxels (`Sampled`, the default, not written) or as
    /// the solid itself (`Exact`: a polyhedron or a mesh, of one material, with no parts).
    #[serde(default, skip_serializing_if = "Representation::is_sampled")]
    pub representation: Representation,
}

const fn default_supersample() -> u8 {
    4
}
/// How the forward model reads a body's shape (`docs/slices/S5.md` §1, §2). Written in a scene
/// file only when it is not [`Representation::Sampled`], so files and bundles made before it keep
/// their bytes.
///
/// TASK-S5-09 adds what a voxel body's boundary cells carry: `Fill` (the exact fill fraction at
/// the cell's centre), `Centroid` (and the piece's centroid), `Second` (and its second moment) and
/// `Pieces` (the clipped piece itself near an arm). They will sit between `Sampled` and `Exact`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Representation {
    /// Voxels, each boundary cell weighed by how many of its `supersample³` sample points lie
    /// inside (S3): the default, read leaf by leaf with the cube kernel.
    #[default]
    Sampled,
    /// No voxels: the solid itself, by the polyhedral closed form near and its exact moments far
    /// ([`crate::exact`]). For a polyhedron (`Cuboid`, `Platonic`, `Polyhedron`) or a closed mesh
    /// (`Mesh`, `Triangles`) of at most 10,000 triangles, of one material and with no parts. It is
    /// still drawn as voxels at `pitch`.
    Exact,
}

impl Representation {
    /// Every representation, in the order a choice lists them.
    pub const ALL: [Self; 2] = [Self::Sampled, Self::Exact];

    /// Whether it is the default ([`Self::Sampled`]): such a shape does not write the field.
    #[must_use]
    pub fn is_sampled(&self) -> bool {
        *self == Self::Sampled
    }
}

/// What a part does where it lies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PartMode {
    /// Adds matter of its material at every point it holds.
    #[default]
    Add,
    /// Turns the matter already at its points to its material; adds none.
    Paint,
    /// Takes away the matter at its points.
    Erase,
}

/// One part of a body of several materials.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartSpec {
    /// What it does: add, paint or erase.
    #[serde(default)]
    pub mode: PartMode,
    /// Its material's label, for adding and painting; none for the body's own material. An
    /// erasing part has none. (Written even when none, as the body's own `material` is, so that
    /// the inspector and `set` reach it: S19's review found a part's material could not be set.)
    #[serde(default)]
    #[schemars(length(min = 1, max = 200))]
    pub material: Option<String>,
    /// Where it is: its centre (a stroke's origin), `[x, y, z]` metres from the body's position.
    #[serde(default)]
    #[schemars(extend("x-unit" = "m"))]
    pub at: [f64; 3],
    /// What solid it is.
    pub kind: PartKind,
}

/// The solids a part can be.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum PartKind {
    /// One of the shapes a body can be, centred on the part's `at`.
    Solid {
        /// Which, and its size.
        shape: ShapeKind,
    },
    /// A brush stroke: balls of `radius` along the path through `points` (from the part's
    /// `at`), as the Paint tool lays them.
    Stroke {
        /// Metres.
        #[schemars(range(max = 1.0e3), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m"))]
        radius: f64,
        /// The path, 1 to 1024 points, each within ±1e3 m of `at`.
        #[schemars(length(min = 1, max = 1024))]
        points: Vec<Vertex>,
    },
}

impl PartSpec {
    /// Checks the part's numbers and that its mode and material go together, under `at`.
    ///
    /// # Errors
    /// The first field that fails, by its path.
    pub fn validate(&self, at: &str) -> Result<(), String> {
        if self.at.iter().any(|c| !(c.is_finite() && c.abs() <= MAX_SIZE)) {
            return Err(format!("{at}.at: every coordinate within ±{MAX_SIZE} m"));
        }
        if self.mode == PartMode::Erase && self.material.is_some() {
            return Err(format!("{at}.material: an erasing part has no material"));
        }
        if let Some(m) = &self.material
            && (m.trim().is_empty() || m.chars().count() > 200)
        {
            return Err(format!("{at}.material: a label of 1 to 200 characters"));
        }
        match &self.kind {
            PartKind::Solid { shape } => shape.validate(&format!("{at}.kind.Solid.shape")),
            PartKind::Stroke { radius, points } => {
                let at = format!("{at}.kind.Stroke");
                if !(radius.is_finite() && *radius > 0.0 && *radius <= MAX_SIZE) {
                    return Err(format!("{at}.radius: {radius} m must be above 0 and at most {MAX_SIZE}"));
                }
                if !(1..=layers::MAX_STROKE_POINTS).contains(&points.len()) {
                    return Err(format!(
                        "{at}.points: {} points; from 1 to {}",
                        points.len(),
                        layers::MAX_STROKE_POINTS
                    ));
                }
                if points.iter().flat_map(|v| v.0).any(|c| !(c.is_finite() && c.abs() <= MAX_SIZE)) {
                    return Err(format!("{at}.points: every coordinate within ±{MAX_SIZE} m"));
                }
                Ok(())
            }
        }
    }

    /// Where it lies, the body's frame; a mesh is read from its file here.
    fn region(&self) -> Result<Box<dyn Region>, String> {
        let by = Vec3::from_array(self.at);
        Ok(match &self.kind {
            PartKind::Solid { shape } => Box::new(Moved { inner: Owned(shape.solid()?), by }),
            PartKind::Stroke { radius, points } => Box::new(Stroke {
                points: points.iter().map(|v| Vec3::from_array(v.0) + by).collect(),
                radius: *radius,
            }),
        })
    }

    /// Its box, the body's frame, when it is known without reading a file.
    fn bounds(&self) -> Option<Aabb> {
        match &self.kind {
            PartKind::Solid { shape } => {
                let b = shape.bounds()?;
                let by = Vec3::from_array(self.at);
                Some(Aabb::new(b.lo + by, b.hi + by))
            }
            PartKind::Stroke { .. } => self.region().ok().map(|r| r.bounds()),
        }
    }
}

/// A shape owned as a region.
struct Owned(Box<dyn Shape>);

impl Region for Owned {
    fn bounds(&self) -> Aabb {
        self.0.bounds()
    }
    fn contains(&self, point: Vec3) -> bool {
        self.0.contains(point)
    }
    fn cover(&self, boxed: &Aabb) -> cav_core::voxel::Cover {
        self.0.cover(boxed)
    }
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
    /// A closed triangle mesh from a file (`.obj` or `.stl`), centred on the middle of its box
    /// (as a held mesh is): what an import of more than 10,000 triangles makes.
    Mesh {
        /// The file, relative to the directory the program runs in. At most 32 MiB and 500,000
        /// triangles; every edge in two triangles.
        #[schemars(length(min = 1, max = 4000))]
        file: String,
        /// Metres per unit of the file.
        #[schemars(range(max = 1.0e6), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m"))]
        scale: f64,
        /// The hash of the file's bytes, 16 hexadecimal digits ([`crate::assets::hash`]), as an
        /// import writes it: the mesh is then the one this session holds under it (a dropped
        /// file's, in a page), and a file read by its path must have it. None for a file read
        /// as it is whenever it is voxelised.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(length(min = 16, max = 16))]
        hash: Option<String>,
    },
    /// A closed triangle mesh held in the scene itself, as an imported file becomes one (S19,
    /// `import`): so the scene needs no file beside it, in a page as natively. Corners from the
    /// body's position, metres; at most 10,000 triangles. Checked whole (every edge in two
    /// triangles, no crossing) when the body is voxelised.
    Triangles {
        /// The corners; each coordinate within ±1e3 m.
        #[schemars(length(min = 4, max = 30000))]
        #[schemars(extend("x-whole" = true))]
        vertices: Vec<Vertex>,
        /// Each triangle's three corners by index into `vertices`, anticlockwise seen from
        /// outside.
        #[schemars(length(min = 4, max = 10000))]
        #[schemars(extend("x-whole" = true))]
        triangles: Vec<Corners>,
    },
}

/// The most triangles a mesh held in a scene may have ([`ShapeKind::Triangles`]): about half a
/// megabyte of scene, within a scene file's 1 MiB.
pub const MAX_INLINE_TRIANGLES: usize = 10_000;
/// The most corners a mesh held in a scene may have.
pub const MAX_INLINE_VERTICES: usize = 30_000;

/// The most triangles the solid of an [`Representation::Exact`] body may have (`docs/slices/S5.md`
/// §3.1, X1): S19's held-mesh limit. A mesh's triangles are counted when it is read, before the
/// solid is made from them; one with more is refused in words.
pub const MAX_EXACT_FACES: usize = MAX_INLINE_TRIANGLES;

/// A triangle of a held mesh: its three corners by index into the mesh's `vertices`,
/// anticlockwise seen from outside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[schemars(extend("x-unit" = "1"))]
pub struct Corners(pub [u32; 3]);

/// A corner of a polyhedron: `[x, y, z]` (east, north, up), metres, from the body's position.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[schemars(extend("x-unit" = "m"))]
pub struct Vertex(pub [f64; 3]);

/// The largest size of a shape, metres.
pub const MAX_SIZE: f64 = 1.0e3;

impl ShapeKind {
    /// The names of the kinds, as the spec writes them.
    pub const KINDS: [&'static str; 9] =
        ["Sphere", "Shell", "Cuboid", "Cylinder", "Ellipsoid", "Platonic", "Polyhedron", "Mesh", "Triangles"];

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
            Self::Triangles { .. } => "Triangles",
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
            Self::Mesh { file, scale, hash } => {
                Box::new(crate::assets::mesh(file, hash.as_deref())?.scaled(*scale))
            }
            Self::Triangles { vertices, triangles } => Box::new(
                Mesh::new(
                    vertices.iter().map(|v| Vec3::from_array(v.0)).collect(),
                    triangles.iter().map(|t| t.0).collect(),
                )
                .map_err(|e| e.0)?,
            ),
        })
    }

    /// Whether the polyhedral kernel can read it as it is: a polyhedron (`Cuboid`, `Platonic`,
    /// `Polyhedron`) or a closed mesh (`Mesh`, `Triangles`), not a curved solid.
    #[must_use]
    pub const fn is_polyhedral(&self) -> bool {
        matches!(
            self,
            Self::Cuboid { .. }
                | Self::Platonic { .. }
                | Self::Polyhedron { .. }
                | Self::Mesh { .. }
                | Self::Triangles { .. }
        )
    }

    /// The solid as the polyhedral kernel reads it, centred on the origin as [`Self::solid`]
    /// centres it (the body's frame, metres). A mesh is read from its file here, and its triangles
    /// counted against [`MAX_EXACT_FACES`] before anything is made of them.
    ///
    /// # Errors
    /// For a curved solid; a mesh past [`MAX_EXACT_FACES`] triangles, with the count; a mesh that
    /// cannot be read; a solid the kernel refuses (not closed, no volume), in its words.
    pub fn faceted(&self) -> Result<Faceted, String> {
        let within = |n: usize| {
            if n > MAX_EXACT_FACES {
                Err(format!(
                    "{n} triangles is more than the {MAX_EXACT_FACES} an Exact body may have; read it by its voxels \
                     (Sampled) or simplify the mesh"
                ))
            } else {
                Ok(())
            }
        };
        let made = match self {
            Self::Cuboid { size } => {
                Faceted::from_cuboid(&Cuboid { centre: Vec3::ZERO, half: Vec3::from_array(*size) * 0.5 })
            }
            Self::Platonic { solid, edge } => Faceted::platonic(platonic(*solid), *edge),
            Self::Polyhedron { vertices } => {
                let points: Vec<Vec3> = vertices.iter().map(|v| Vec3::from_array(v.0)).collect();
                let hull = Polyhedron::hull(&points).map_err(|e| e.0)?;
                within(hull.faces.iter().map(|f| f.len().saturating_sub(2)).sum())?;
                Faceted::from_polyhedron(&hull)
            }
            Self::Mesh { file, scale, hash } => {
                let mesh = crate::assets::mesh(file, hash.as_deref())?.scaled(*scale);
                within(mesh.triangles.len())?;
                Faceted::from_mesh(&mesh)
            }
            Self::Triangles { vertices, triangles } => {
                within(triangles.len())?;
                let mesh = Mesh::new(
                    vertices.iter().map(|v| Vec3::from_array(v.0)).collect(),
                    triangles.iter().map(|t| t.0).collect(),
                )
                .map_err(|e| e.0)?;
                Faceted::from_mesh(&mesh)
            }
            _ => {
                return Err(format!("a {} is not a polyhedron or a mesh: Exact cannot read it", self.name()));
            }
        };
        made.map_err(|e| e.0)
    }

    /// Its box about the body's position when it is known without reading a file or making the
    /// mesh: a held mesh's from its corners.
    #[must_use]
    pub fn bounds(&self) -> Option<Aabb> {
        match self {
            // A held mesh's box is known; a file's is not until it is read.
            Self::Mesh { hash, scale, .. } => {
                let mesh = crate::assets::held(hash.as_deref()?)?;
                let b = cav_core::voxel::Shape::bounds(mesh.as_ref());
                Some(Aabb::new(b.lo * *scale, b.hi * *scale))
            }
            Self::Triangles { vertices, .. } => {
                let first = Vec3::from_array(vertices.first()?.0);
                Some(vertices.iter().fold(Aabb::new(first, first), |b, v| {
                    let p = Vec3::from_array(v.0);
                    b.union(&Aabb::new(p, p))
                }))
            }
            _ => self.solid().ok().map(|s| s.bounds()),
        }
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
            Self::Mesh { file, scale, hash } => {
                if file.trim().is_empty() || file.chars().count() > 4000 {
                    return Err(format!("{at}.file: a file name of 1 to 4000 characters"));
                }
                if let Some(h) = hash
                    && !crate::assets::is_hash(h)
                {
                    return Err(format!("{at}.hash: `{h}` is not 16 lower-case hexadecimal digits"));
                }
                (scale.is_finite() && *scale > 0.0 && *scale <= 1.0e6)
                    .then_some(())
                    .ok_or_else(|| format!("{at}.scale: {scale} must be above 0 and at most 1e6"))
            }
            // The counts, the indices and the numbers; whether it is closed and does not cross
            // itself is checked when it is voxelised (as a file's mesh is), not on every edit.
            Self::Triangles { vertices, triangles } => {
                if !(4..=MAX_INLINE_VERTICES).contains(&vertices.len()) {
                    return Err(format!(
                        "{at}.vertices: {} corners; from 4 to {MAX_INLINE_VERTICES}",
                        vertices.len()
                    ));
                }
                if !(4..=MAX_INLINE_TRIANGLES).contains(&triangles.len()) {
                    return Err(format!(
                        "{at}.triangles: {} triangles; from 4 to {MAX_INLINE_TRIANGLES}",
                        triangles.len()
                    ));
                }
                if vertices.iter().flat_map(|v| v.0).any(|c| !(c.is_finite() && c.abs() <= MAX_SIZE)) {
                    return Err(format!("{at}.vertices: every coordinate within ±{MAX_SIZE} m"));
                }
                match triangles.iter().flat_map(|t| t.0).find(|&i| i as usize >= vertices.len()) {
                    Some(i) => Err(format!("{at}.triangles: corner {i} of {} corners", vertices.len())),
                    None => Ok(()),
                }
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
    /// A uniform shape of `kind` at `pitch` metres, `supersample` points a side of a cell: no
    /// material, no parts.
    #[must_use]
    pub const fn new(kind: ShapeKind, pitch: f64, supersample: u8) -> Self {
        Self {
            kind,
            pitch,
            supersample,
            material: None,
            parts: Vec::new(),
            representation: Representation::Sampled,
        }
    }

    /// Checks the shape, its parts, and that its pitch fits one tree (at most 1024 voxels
    /// across, the parts that add matter included). Whether its materials exist is the scene's
    /// check ([`Self::materials`]).
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
        if let Some(m) = &self.material
            && (m.trim().is_empty() || m.chars().count() > 200)
        {
            return Err(format!("{at}.material: a label of 1 to 200 characters"));
        }
        if !self.parts.is_empty() && self.material.is_none() {
            return Err(format!("{at}.parts: a body with parts has a material of its own"));
        }
        if self.parts.len() > layers::MAX_LAYERS - 1 {
            return Err(format!(
                "{at}.parts: {} parts; at most {}",
                self.parts.len(),
                layers::MAX_LAYERS - 1
            ));
        }
        for (n, part) in self.parts.iter().enumerate() {
            part.validate(&format!("{at}.parts[{n}]"))?;
        }
        if self.representation == Representation::Exact {
            if !self.kind.is_polyhedral() {
                return Err(format!(
                    "{at}.representation: Exact reads a polyhedron or a mesh (Cuboid, Platonic, Polyhedron, Mesh, \
                     Triangles), not a {}; a curved solid is read by its voxels (Sampled)",
                    self.kind.name()
                ));
            }
            if !self.parts.is_empty() {
                return Err(format!(
                    "{at}.representation: an Exact body is of one material with no parts; this one has {} parts",
                    self.parts.len()
                ));
            }
        }
        // A leaf's material is a byte: the body's own and its parts' distinct materials together
        // fit one (the voxeliser would refuse more only when the body is made).
        let palette = self.palette().len();
        if palette > layers::MAX_BODY_MATERIALS {
            return Err(format!(
                "{at}.parts: {palette} materials in one body; at most {}",
                layers::MAX_BODY_MATERIALS
            ));
        }
        // The grid needs only the boxes, which a mesh does not give without its file.
        if let Some(bounds) = self.bounds() {
            Grid::around(bounds, self.pitch, self.supersample).map_err(|e| format!("{at}.pitch: {e}"))?;
        }
        Ok(())
    }

    /// The box of the shape and of the parts that add matter, the shape centred on the origin,
    /// when it is known without reading a file.
    fn bounds(&self) -> Option<Aabb> {
        let own = self.kind.bounds()?;
        self.parts
            .iter()
            .filter(|p| p.mode == PartMode::Add)
            .try_fold(own, |b, p| Some(b.union(&p.bounds()?)))
    }

    /// The labels of the materials the body is made of, in the order its leaves index them: its
    /// own, then each part's that is not already there. Empty for a uniform body.
    #[must_use]
    pub fn palette(&self) -> Vec<&str> {
        let Some(own) = self.material.as_deref() else { return Vec::new() };
        let mut out = vec![own];
        for part in &self.parts {
            if let Some(m) = part.material.as_deref()
                && !out.contains(&m)
            {
                out.push(m);
            }
        }
        out
    }

    /// The materials of [`Self::palette`], found among `materials` (a scene's own) and the
    /// library.
    ///
    /// # Errors
    /// The first label neither has, by its path under `at`.
    pub fn materials<'a>(
        &self,
        materials: &'a [MaterialSpec],
        at: &str,
    ) -> Result<Vec<&'a MaterialSpec>, String> {
        self.palette()
            .into_iter()
            .map(|label| {
                crate::materials::find(materials, label).ok_or_else(|| {
                    format!("{at}: `{label}` is not a material of the scene's or of the library's")
                })
            })
            .collect()
    }

    /// The body's voxel tree: the shape voxelised at its pitch, in the body's own frame (the
    /// shape's centre at its origin): `mass` kilograms in all for a uniform body; its materials'
    /// densities (found among `materials`, a scene's own, then the library) for a body with a
    /// material, whose `mass` is not used.
    ///
    /// # Errors
    /// When the shape cannot be made, a material is not found, or the voxeliser refuses it (too
    /// fine, too many leaves, no sample point inside), in words that name the number.
    pub fn tree(&self, mass: f64, materials: &[MaterialSpec]) -> Result<VoxTree, String> {
        self.tree_within(mass, materials, usize::MAX)
    }

    /// [`Self::tree`] in at most `bytes` bytes of tree ([`VoxTree::bytes`]): what is left of a
    /// scene's [`MAX_SCENE_TREE_BYTES`] when the trees before it are made. Counted as the leaves
    /// are emitted and from the nodes before they are reserved, so a tree that would not fit is
    /// refused before it is whole.
    ///
    /// # Errors
    /// As [`Self::tree`], and when the tree would weigh more than `bytes`, with the numbers.
    pub fn tree_within(
        &self,
        mass: f64,
        materials: &[MaterialSpec],
        bytes: usize,
    ) -> Result<VoxTree, String> {
        let solid = self.kind.solid()?;
        let budget = Budget::with_bytes(bytes);
        if self.material.is_none() {
            let grid = Grid::around(solid.bounds(), self.pitch, self.supersample).map_err(|e| e.0)?;
            return voxelise_within(solid.as_ref(), &grid, mass, 0, budget).map_err(|e| e.0);
        }
        let palette = self.palette();
        let densities: Vec<f64> = self.materials(materials, "material")?.iter().map(|m| m.density).collect();
        let index = |label: Option<&str>| {
            let label = label.unwrap_or(palette[0]);
            palette.iter().position(|p| *p == label).unwrap_or(0) as u8
        };
        let parts: Vec<Box<dyn Region>> =
            self.parts.iter().map(PartSpec::region).collect::<Result<_, _>>()?;
        let own = Whole(solid.as_ref());
        let mut stack = vec![Layer { region: &own, material: 0, mode: Mode::Add }];
        let mut bounds = solid.bounds();
        for (part, region) in self.parts.iter().zip(&parts) {
            let mode = match part.mode {
                PartMode::Add => Mode::Add,
                PartMode::Paint => Mode::Paint,
                PartMode::Erase => Mode::Erase,
            };
            if mode == Mode::Add {
                bounds = bounds.union(&region.bounds());
            }
            stack.push(Layer { region: region.as_ref(), material: index(part.material.as_deref()), mode });
        }
        let grid = Grid::around(bounds, self.pitch, self.supersample).map_err(|e| e.0)?;
        layers::voxelise_layers(&stack, &grid, &densities, budget).map_err(|e| e.0)
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
            {
                let half = Vec3::new(0.2, 0.15, 0.1);
                let (v, t) = cav_core::voxel::mesh::box_mesh(-half, half);
                ShapeKind::Triangles {
                    vertices: v.iter().map(|p| Vertex(p.to_array())).collect(),
                    triangles: t.into_iter().map(Corners).collect(),
                }
            },
        ];
        assert_eq!(kinds.len() + 1, ShapeKind::KINDS.len(), "every kind but the mesh, which needs a file");
        for kind in kinds {
            let spec = ShapeSpec::new(kind, 0.05, 2);
            assert_eq!(spec.validate("shape"), Ok(()));
            let tree = spec.tree(12.0, &[]).unwrap();
            assert!((tree.body().mass - 12.0).abs() < 1e-12, "{:?}", spec.kind);
            assert!(ShapeKind::KINDS.contains(&spec.kind.name()));
        }
        // Refusals name the field: a pitch too fine for one tree, a size of zero, k of 7.
        let fine = ShapeSpec::new(ShapeKind::Sphere { radius: 1.0 }, 1e-3, 2);
        assert!(fine.validate("bodies[0].shape").unwrap_err().starts_with("bodies[0].shape.pitch"));
        let flat = ShapeSpec::new(ShapeKind::Cuboid { size: [1.0, 0.0, 1.0] }, 0.1, 2);
        assert!(flat.validate("s").unwrap_err().starts_with("s.kind.Cuboid.size"));
        let seven = ShapeSpec::new(ShapeKind::Sphere { radius: 1.0 }, 0.1, 7);
        assert!(seven.validate("s").unwrap_err().starts_with("s.supersample"));
        let missing = ShapeSpec::new(
            ShapeKind::Mesh { file: "no/such/file.obj".into(), scale: 1.0, hash: None },
            0.1,
            2,
        );
        assert_eq!(missing.validate("s"), Ok(()), "a file is read when the body is voxelised");
        assert!(missing.tree(1.0, &[]).is_err());
    }

    #[test]
    fn a_body_s_materials_fit_a_byte() {
        // S19 review minor 6: 255 parts of 255 distinct materials and the body's own make 256,
        // one more than a leaf's byte holds; validation refuses it by the field. Control: 254
        // distinct part materials (255 in all) pass.
        let part = |n: usize| PartSpec {
            mode: PartMode::Paint,
            material: Some(format!("m{n}")),
            at: [0.0; 3],
            kind: PartKind::Solid { shape: ShapeKind::Sphere { radius: 0.1 } },
        };
        let mut shape = ShapeSpec::new(ShapeKind::Sphere { radius: 0.5 }, 0.1, 1);
        shape.material = Some("own".into());
        shape.parts = (0..255).map(part).collect();
        let why = shape.validate("bodies[0].shape").unwrap_err();
        assert!(why.starts_with("bodies[0].shape.parts: 256 materials"), "{why}");
        shape.parts[254].material = Some("m0".into());
        assert_eq!(shape.validate("bodies[0].shape"), Ok(()));
    }
}
