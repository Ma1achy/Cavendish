//! What each scene pane draws of the world's levels (S5, `cav_sim::world::lod_view`): its LOD
//! view's draw list and its world overlay's boxes, made from what the forward stage's world read
//! made (the octree's nodes, each tower's cut), with no walk of its own.
//!
//! Made again when a command changes the scene or a view, and when a world read arrives
//! ([`crate::Engine::poll`]); each frame is bounded by `lod_view`'s caps.

use std::sync::Arc;

use cav_sim::world::lod_view::{LodFrame, ReadView, lod_frame};
use cav_sim::{PaneContent, ViewSpec};

use crate::MAX_PANES;

/// Each pane's LOD frame, and the read they were made from.
#[derive(Debug, Default)]
pub struct Lods {
    frames: [Option<LodFrame>; MAX_PANES],
    /// The read the frames were made from (by pointer: a new read is a new value).
    read: Option<Arc<ReadView>>,
}

impl Lods {
    /// Makes each scene pane's frame for its view, from `read` (None before the world is read, and
    /// for a scene with no world: then a pane that asks for none has none).
    pub fn refresh(
        &mut self,
        views: &[ViewSpec],
        contents: &[PaneContent],
        read: Option<Arc<ReadView>>,
        world: bool,
    ) {
        self.read = read;
        for (pane, slot) in self.frames.iter_mut().enumerate() {
            *slot = match (views.get(pane), contents.get(pane)) {
                (Some(view), Some(PaneContent::Scene)) if world => {
                    Some(lod_frame(&view.lod_view, view.overlay.world, self.read.as_deref()))
                }
                _ => None,
            };
        }
    }

    /// Whether `read` is not the one the frames were made from.
    #[must_use]
    pub fn stale(&self, read: Option<&Arc<ReadView>>) -> bool {
        match (&self.read, read) {
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            (None, None) => false,
            _ => true,
        }
    }

    /// Pane `pane`'s frame, when it shows a scene with a world.
    #[must_use]
    pub fn frame(&self, pane: usize) -> Option<&LodFrame> {
        self.frames.get(pane).and_then(Option::as_ref)
    }
}
