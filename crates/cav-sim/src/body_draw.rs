//! How a view draws a body that has a shape (the owner's ruling of 25 Nov 2026, "draw modes for
//! bodies"; `docs/slices/S5.md` §1 "Draw a body four ways"): as its voxels, as its voxels shaded by
//! their exact fill, as its own triangles, or as both. A draw mode is a view setting
//! ([`crate::ViewSpec::body_draw`]): journalled, never undone, like a camera move.
//!
//! What a mode needs beyond the voxel tree is made here as plain data: the shape's surface as
//! triangles ([`surface`]), in the body's frame (the shape centred on the origin, as
//! [`crate::ShapeKind::solid`] centres it), metres. The engine keeps it beside the body's tree and
//! the renderer draws it moved to the body's position (bodies do not turn).
//!
//! **Bounded** (`docs/slices/S5.md` §3.1, X7): a surface has at most [`MAX_DRAWN_TRIANGLES`]
//! triangles, counted before any is made; a body past it is drawn as voxels and the frame says
//! so.

use cav_core::Vec3;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ShapeKind;

/// The most triangles a view draws of bodies' surfaces (and, with `pieces`, of their cut cells'
/// pieces) together: about what [`crate::voxel_view::MAX_DRAWN_CUBES`] cubes make (twelve each),
/// so that no mode draws more than the voxels may. Counted before any is made.
pub const MAX_DRAWN_TRIANGLES: usize = 250_000;

/// How finely a sphere's surface is drawn: an icosahedron with each face split in four this many
/// times (20 · 4ⁿ triangles; 5,120 for 4).
pub const SPHERE_SPLITS: u32 = 4;

/// How many sides a cylinder's drawn surface has.
pub const CYLINDER_SIDES: u32 = 96;

/// How a view draws the bodies that have a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum BodyDraw {
    /// The voxels, coloured by the view's field (`colour`): S3's picture, the default.
    #[default]
    Voxels,
    /// The voxels, each shaded by its exact fill (the fraction of the cell the shape fills, from
    /// the cell's clipped piece), with a legend: the "mass anti-aliasing" of
    /// `docs/research/exact-skin-and-partial-fill.md`. A body read `Sampled` is clipped for the
    /// picture (its read is unchanged); one with no clipper or with parts shows its sampled fill,
    /// and the frame says so.
    Fill,
    /// The shape's own triangles: a mesh's, a polyhedron's faces, a box's; a curved solid
    /// tessellated (sphere and ellipsoid from a split icosahedron, a cylinder of
    /// [`CYLINDER_SIDES`] sides), and the frame says so.
    Mesh,
    /// The voxels as outlines in the view's field, and the shape's triangles solid inside them: how
    /// the cells straddle the surface.
    MeshOverVoxels,
}

impl BodyDraw {
    /// Every mode, in the order menus list them.
    pub const ALL: [Self; 4] = [Self::Voxels, Self::Fill, Self::Mesh, Self::MeshOverVoxels];

    /// The word commands use.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Voxels => "voxels",
            Self::Fill => "fill",
            Self::Mesh => "mesh",
            Self::MeshOverVoxels => "mesh-over-voxels",
        }
    }

    /// The mode a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.name() == word)
    }

    /// Whether it is the default ([`Self::Voxels`]): a view that draws voxels does not write the
    /// field, so views saved before it keep their bytes.
    #[must_use]
    pub fn is_voxels(&self) -> bool {
        *self == Self::Voxels
    }

    /// Whether it draws the shape's surface.
    #[must_use]
    pub const fn draws_surface(self) -> bool {
        matches!(self, Self::Mesh | Self::MeshOverVoxels)
    }

    /// Whether it draws the voxels (solid or as outlines).
    #[must_use]
    pub const fn draws_voxels(self) -> bool {
        !matches!(self, Self::Mesh)
    }

    /// Whether its voxels must carry exact pieces: the body is clipped for the picture when its
    /// own tree has none.
    #[must_use]
    pub const fn wants_cuts(self) -> bool {
        matches!(self, Self::Fill)
    }
}

/// A shape's surface as triangles, in the body's frame (metres; the shape centred on the origin),
/// each anticlockwise seen from outside.
#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    /// The triangles.
    pub triangles: Vec<[Vec3; 3]>,
    /// Whether it is a tessellation of a curved surface (a sphere, an ellipsoid, a shell, a
    /// cylinder), not the shape's own faces: a picture of the shape, not the shape the read takes.
    pub tessellated: bool,
}

/// Refuses a count of triangles past [`MAX_DRAWN_TRIANGLES`], before any is made.
fn within(count: usize, what: &str) -> Result<(), String> {
    if count > MAX_DRAWN_TRIANGLES {
        return Err(format!(
            "{what} has {count} triangles, more than the {MAX_DRAWN_TRIANGLES} a view draws"
        ));
    }
    Ok(())
}

/// The unit sphere as an icosahedron split `splits` times, anticlockwise seen from outside.
fn unit_sphere(splits: u32) -> Vec<[Vec3; 3]> {
    let t = 0.5 * (1.0 + 5f64.sqrt());
    let v: Vec<Vec3> = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ]
    .into_iter()
    .map(|p| Vec3::from_array(p).normalised())
    .collect();
    let faces: [[usize; 3]; 20] = [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    let mut out: Vec<[Vec3; 3]> = faces.iter().map(|f| f.map(|i| v[i])).collect();
    for _ in 0..splits {
        out = out
            .into_iter()
            .flat_map(|[a, b, c]| {
                let (ab, bc, ca) = ((a + b).normalised(), (b + c).normalised(), (c + a).normalised());
                [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]
            })
            .collect();
    }
    out
}

/// A solid circular cylinder of `radius` and `half` its length along axis `axis` (0, 1, 2 for x,
/// y, z), centred on the origin, as a prism of [`CYLINDER_SIDES`] sides with its two caps.
fn cylinder(radius: f64, half: f64, axis: usize) -> Vec<[Vec3; 3]> {
    // Made along z, then the axes turned so that z becomes `axis` (a cyclic turn keeps the
    // winding).
    let turn = |p: Vec3| match axis {
        0 => Vec3::new(p.z, p.x, p.y),
        1 => Vec3::new(p.y, p.z, p.x),
        _ => p,
    };
    let n = CYLINDER_SIDES;
    let at = |k: u32, z: f64| {
        let a = std::f64::consts::TAU * f64::from(k % n) / f64::from(n);
        Vec3::new(radius * a.cos(), radius * a.sin(), z)
    };
    let mut out = Vec::with_capacity(4 * n as usize);
    let (top, bottom) = (Vec3::new(0.0, 0.0, half), Vec3::new(0.0, 0.0, -half));
    for k in 0..n {
        let (a0, a1, b0, b1) = (at(k, -half), at(k + 1, -half), at(k, half), at(k + 1, half));
        out.push([a0, a1, b1]);
        out.push([a0, b1, b0]);
        out.push([top, b0, b1]);
        out.push([bottom, a1, a0]);
    }
    out.into_iter().map(|t| t.map(turn)).collect()
}

/// The surface of `kind` as triangles, in the body's frame ([`Surface`]): a mesh's own triangles
/// (read from its file or the session's held copy), a polyhedron's faces, a box's twelve; a curved
/// solid tessellated.
///
/// # Errors
/// When the shape cannot be made (a mesh that cannot be read, a polyhedron with no volume), or its
/// triangles would pass [`MAX_DRAWN_TRIANGLES`] (counted first), in words.
pub fn surface(kind: &ShapeKind) -> Result<Surface, String> {
    let tessellated = |triangles: Vec<[Vec3; 3]>| Surface { triangles, tessellated: true };
    Ok(match kind {
        ShapeKind::Sphere { radius } => {
            tessellated(unit_sphere(SPHERE_SPLITS).into_iter().map(|t| t.map(|p| p * *radius)).collect())
        }
        ShapeKind::Ellipsoid { radii } => {
            let r = Vec3::from_array(*radii);
            tessellated(
                unit_sphere(SPHERE_SPLITS)
                    .into_iter()
                    .map(|t| t.map(|p| Vec3::new(p.x * r.x, p.y * r.y, p.z * r.z)))
                    .collect(),
            )
        }
        ShapeKind::Shell { outer, inner } => {
            let sphere = unit_sphere(SPHERE_SPLITS);
            let mut out: Vec<[Vec3; 3]> = sphere.iter().map(|t| t.map(|p| p * *outer)).collect();
            if *inner > 0.0 {
                // The cavity's wall faces inwards: each triangle wound the other way.
                out.extend(sphere.iter().map(|[a, b, c]| [*a * *inner, *c * *inner, *b * *inner]));
            }
            tessellated(out)
        }
        ShapeKind::Cylinder { radius, length, axis } => {
            tessellated(cylinder(*radius, 0.5 * length, *axis as usize))
        }
        ShapeKind::Mesh { file, scale, hash } => {
            // The mesh the solid is made of (`ShapeKind::solid`), in the same frame.
            let mesh = crate::assets::mesh(file, hash.as_deref())?;
            within(mesh.triangles.len(), file)?;
            let s = *scale;
            Surface {
                triangles: mesh.triangles.iter().map(|t| t.map(|i| mesh.vertices[i as usize] * s)).collect(),
                tessellated: false,
            }
        }
        ShapeKind::Triangles { vertices, triangles } => {
            within(triangles.len(), "the held mesh")?;
            Surface {
                triangles: triangles
                    .iter()
                    .map(|t| t.0.map(|i| Vec3::from_array(vertices[i as usize].0)))
                    .collect(),
                tessellated: false,
            }
        }
        ShapeKind::Cuboid { .. } | ShapeKind::Platonic { .. } | ShapeKind::Polyhedron { .. } => {
            let solid = kind.faceted()?;
            let v = solid.vertices();
            within(solid.faces().len(), kind.name())?;
            Surface {
                triangles: solid.faces().iter().map(|f| f.corners.map(|i| v[i as usize])).collect(),
                tessellated: false,
            }
        }
    })
}

/// The volume a closed surface's triangles enclose, cubic metres: `Σ a·(b × c)/6` (the divergence
/// theorem; anticlockwise seen from outside gives a positive volume). For tests and for checking
/// a drawn surface against the shape.
#[must_use]
pub fn enclosed_volume(triangles: &[[Vec3; 3]]) -> f64 {
    triangles.iter().map(|[a, b, c]| a.dot(b.cross(*c))).sum::<f64>() / 6.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::{Axis, PlatonicSolid};

    #[test]
    fn a_surface_encloses_its_shape() {
        // Exact for the solids drawn by their own faces; within the tessellation's error for the
        // curved ones (an icosahedron split four times holds 99.6 % of its sphere's volume; a
        // 96-sided prism 99.93 % of its cylinder's).
        let cases: [(ShapeKind, f64); 6] = [
            (ShapeKind::Cuboid { size: [0.4, 0.3, 0.2] }, 1e-14),
            (ShapeKind::Platonic { solid: PlatonicSolid::Icosahedron, edge: 0.6 }, 1e-14),
            (ShapeKind::Sphere { radius: 0.5 }, 5e-3),
            (ShapeKind::Ellipsoid { radii: [0.3, 0.2, 0.1] }, 5e-3),
            (ShapeKind::Shell { outer: 0.5, inner: 0.3 }, 5e-3),
            (ShapeKind::Cylinder { radius: 0.2, length: 0.6, axis: Axis::Y }, 1e-3),
        ];
        for (kind, tolerance) in cases {
            let drawn = surface(&kind).unwrap();
            let want = kind.solid().unwrap().volume();
            let got = enclosed_volume(&drawn.triangles);
            assert!(((got - want) / want).abs() < tolerance, "{}: {got} against {want}", kind.name());
            assert_eq!(drawn.tessellated, tolerance > 1e-10, "{}", kind.name());
            // Control: the same triangles each wound the other way enclose minus the volume.
            let flipped: Vec<[Vec3; 3]> = drawn.triangles.iter().map(|[a, b, c]| [*a, *c, *b]).collect();
            assert!((enclosed_volume(&flipped) + got).abs() < 1e-12 * want.abs().max(1.0));
        }
    }

    #[test]
    fn a_surface_past_its_cap_is_refused_before_it_is_made() {
        assert!(within(MAX_DRAWN_TRIANGLES, "a mesh").is_ok());
        let refused = within(MAX_DRAWN_TRIANGLES + 1, "a mesh").unwrap_err();
        assert!(refused.contains(&format!("{}", MAX_DRAWN_TRIANGLES + 1)), "{refused}");
    }

    #[test]
    fn draw_modes_are_named() {
        for mode in BodyDraw::ALL {
            assert_eq!(BodyDraw::parse(mode.name()), Some(mode));
        }
        assert_eq!(BodyDraw::parse("solid"), None);
        assert!(BodyDraw::default().is_voxels());
    }
}
