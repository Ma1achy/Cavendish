//! The exact pieces of cut cells as solids, for `Pieces`: each boundary cell's part of the shape as
//! signed convex polyhedra, read by the polyhedral closed form near an arm ([X] §5 item 3).

use crate::Vec3;
use crate::polyhedral::Faceted;

use super::super::shape::Shape;
use super::super::{Grid, VoxTree, VoxelError, morton};
use super::Convex;

/// Within this many cells of an arm point (from the cell's centre) a cut cell is read as its exact
/// piece by the polyhedral closed form; beyond, as `Second` (`docs/slices/S5.md` §2).
pub const PIECE_WITHIN: f64 = 3.0;

/// The most triangles the pieces of one body may have together, counted before any is made:
/// about 200 bytes each prepared, so 800 MB at most.
pub const MAX_PIECE_TRIANGLES: usize = 4_000_000;

/// One cut cell's piece as solids: convex polyhedra, each with its sign (`+1`, or `−1` for a
/// mesh's prisms below a triangle facing down), in the frame of the cell's centre (metres). Their
/// signed sum is the shape's part of the cell.
#[derive(Clone, Debug, PartialEq)]
pub struct PieceSolid {
    /// The cell's centre, metres, the body's frame.
    pub centre: Vec3,
    /// `(sign, solid)`, the solid in the cell's frame.
    pub parts: Vec<(f64, Faceted)>,
}

/// A convex piece's triangles: a fan on each face, in the piece's frame.
fn triangles(piece: &Convex) -> Vec<[Vec3; 3]> {
    let mut out = Vec::new();
    for face in &piece.faces {
        let first = piece.vertices[face[0] as usize];
        for t in 1..face.len() - 1 {
            out.push([first, piece.vertices[face[t] as usize], piece.vertices[face[t + 1] as usize]]);
        }
    }
    out
}

/// The pieces of every cut cell of `tree` (in the order of [`VoxTree::cuts`]) as solids, clipped
/// again from `shape` on the tree's grid.
///
/// # Errors
/// When the shape has no clipper or its clipper gives no polyhedra (the curved solids, whose pieces
/// are integrals of their surface, not polyhedra); when the triangles would pass
/// [`MAX_PIECE_TRIANGLES`] (counted first); when a piece is not a closed solid the kernel takes.
pub fn piece_solids(shape: &dyn Shape, tree: &VoxTree) -> Result<Vec<PieceSolid>, VoxelError> {
    let Some(clip) = shape.clip() else {
        return Err(VoxelError("this shape has no cut-cell clipper".into()));
    };
    let grid: &Grid = &tree.grid;
    let clipper = clip.prepare(grid)?;
    let mut convex = Vec::with_capacity(tree.cuts.len());
    let mut count = 0usize;
    for cut in &tree.cuts {
        let corner = morton::decode(cut.key);
        let boxed = grid.block_box(corner, 1);
        let Some(parts) = clipper.convex(corner, &boxed) else {
            return Err(VoxelError(
                "Pieces reads a polyhedron or a mesh: this shape's pieces are not polyhedra (read it as Second)"
                    .into(),
            ));
        };
        count += parts.iter().map(|(_, p)| p.faces.iter().map(|f| f.len() - 2).sum::<usize>()).sum::<usize>();
        if count > MAX_PIECE_TRIANGLES {
            return Err(VoxelError(format!(
                "the body's exact pieces would have more than {MAX_PIECE_TRIANGLES} triangles: use a coarser pitch \
                 or read it as Second"
            )));
        }
        convex.push((boxed.centre(), parts));
    }
    convex
        .into_iter()
        .map(|(centre, parts)| {
            let parts = parts
                .into_iter()
                .filter(|(_, p)| !p.is_empty())
                .map(|(sign, p)| {
                    Faceted::new(&triangles(&p))
                        .map(|f| (sign, f))
                        .map_err(|e| VoxelError(format!("a cut cell's piece at {centre:?}: {}", e.0)))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PieceSolid { centre, parts })
        })
        .collect()
}
