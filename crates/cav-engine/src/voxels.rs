//! What the panes draw of voxel bodies: each body's tree, kept while its shape and mass stay the
//! same, and each scene pane's [`VoxelFrame`] (cubes, overlays, the legend's range).
//!
//! Trees are made here when a body's shape or mass changes (a drag moves the body, not its
//! tree), within the voxeliser's caps and within what the scene's other trees leave of
//! [`TREE_CACHE_BYTES`], and kept in a cache of at most that, the oldest dropped first
//! (`docs/slices/S3.md` §3.1, V10). A tree is made on the thread that
//! applies the command: that is milliseconds for the dev scenes and at most the voxeliser's work
//! cap for any scene (a call worth a second look: it could move to a worker).

use std::sync::Arc;

use cav_core::voxel::VoxTree;
use cav_sim::voxel_view::{VoxelFrame, voxel_frame_cut};
use cav_sim::{SceneSpec, ViewSpec};

use crate::MAX_PANES;

/// The most bytes of trees kept for drawing: the scene's own ([`cav_sim::shape::MAX_SCENE_TREE_BYTES`])
/// and any kept from before, together.
pub const TREE_CACHE_BYTES: usize = cav_sim::shape::MAX_SCENE_TREE_BYTES;

/// The voxel bodies' trees and the panes' frames of them.
#[derive(Debug)]
pub struct Voxels {
    /// The most bytes of trees kept: [`TREE_CACHE_BYTES`], or less for a test.
    budget: usize,
    /// Trees by what they were made from (the shape and the mass, as JSON), oldest first.
    cache: Vec<(String, Arc<VoxTree>)>,
    /// Each body's tree, in the scene's order; None for a body that is not made of voxels.
    trees: Vec<Option<Arc<VoxTree>>>,
    /// Each pane's frame, when the scene has voxel bodies.
    frames: [Option<VoxelFrame>; MAX_PANES],
    /// Shapes already said to be refused, so that a drag does not say it every step.
    said: Vec<String>,
    /// Shapes refused for want of room, with the room there was: not tried again with no more.
    refused: Vec<(String, usize)>,
}

impl Default for Voxels {
    fn default() -> Self {
        Self::with_budget(TREE_CACHE_BYTES)
    }
}

impl Voxels {
    /// Voxels whose scene's trees, and all the trees they keep, weigh at most `budget` bytes
    /// (the engine's is [`TREE_CACHE_BYTES`]; a smaller one is for tests of the budget).
    #[must_use]
    pub fn with_budget(budget: usize) -> Self {
        Self {
            budget,
            cache: Vec::new(),
            trees: Vec::new(),
            frames: Default::default(),
            said: Vec::new(),
            refused: Vec::new(),
        }
    }

    /// How many trees are kept, in use or not.
    #[must_use]
    pub fn kept(&self) -> usize {
        self.cache.len()
    }

    /// Brings the trees and the frames up to `spec` at `time`, for `views` (the panes in use).
    /// Returns what a person should be told: a body whose shape could not be voxelised, once.
    ///
    /// The scene's trees together weigh at most the budget: each is made, or taken from the
    /// cache, only within what the bodies before it (in the scene's order) left, and a body past
    /// it is not drawn as voxels and is named with the numbers. Trees no longer in use are then
    /// dropped, oldest first, until all those kept are within the budget too.
    pub fn refresh(&mut self, spec: &SceneSpec, time: f64, views: &[ViewSpec]) -> Vec<String> {
        let mut notes = Vec::new();
        self.trees.clear();
        let mut in_use = 0usize;
        for body in &spec.bodies {
            let Some(shape) = &body.shape else {
                self.trees.push(None);
                continue;
            };
            // A body of materials is also what its materials are: a scene's own lead may change.
            let materials = shape.materials(&spec.materials, "").unwrap_or_default();
            let key = format!(
                "{}|{}|{}",
                serde_json::to_string(shape).unwrap_or_default(),
                body.mass.to_bits(),
                serde_json::to_string(&materials).unwrap_or_default()
            );
            let left = self.budget - in_use;
            let refuse = |why: String, notes: &mut Vec<String>, said: &mut Vec<String>| {
                if !said.contains(&key) {
                    notes.push(format!("`{}` is not drawn as voxels: {why}", body.label));
                    said.push(key.clone());
                }
            };
            if let Some(at) = self.cache.iter().position(|(k, _)| *k == key) {
                let bytes = self.cache[at].1.bytes();
                if bytes > left {
                    refuse(self.over(bytes, in_use), &mut notes, &mut self.said);
                    self.trees.push(None);
                    continue;
                }
                // Used again: it is the newest now.
                let kept = self.cache.remove(at);
                in_use += bytes;
                self.trees.push(Some(Arc::clone(&kept.1)));
                self.cache.push(kept);
                continue;
            }
            // A tree refused for want of room is not tried again until there is more room.
            if self.refused.iter().any(|(k, room)| *k == key && left <= *room) {
                self.trees.push(None);
                continue;
            }
            match shape.tree_within(body.mass, &spec.materials, left) {
                Ok(tree) => {
                    in_use += tree.bytes();
                    let tree = Arc::new(tree);
                    self.cache.push((key, Arc::clone(&tree)));
                    self.trees.push(Some(tree));
                }
                Err(why) => {
                    let why = if left < self.budget {
                        format!(
                            "{why} (the scene's trees may weigh {} bytes together, and {in_use} are in use)",
                            self.budget
                        )
                    } else {
                        why
                    };
                    self.refused.retain(|(k, _)| *k != key);
                    self.refused.push((key.clone(), left));
                    refuse(why, &mut notes, &mut self.said);
                    self.trees.push(None);
                }
            }
        }
        // The cache's budget: the oldest trees go first, never one in use now. Those in use are
        // within the budget by construction, so this ends within it.
        let mut bytes: usize = self.cache.iter().map(|(_, t)| t.bytes()).sum();
        while bytes > self.budget {
            let Some(at) =
                self.cache.iter().position(|(_, t)| !self.trees.iter().flatten().any(|u| Arc::ptr_eq(t, u)))
            else {
                break;
            };
            bytes -= self.cache.remove(at).1.bytes();
        }
        let any = self.trees.iter().any(Option::is_some);
        for (pane, slot) in self.frames.iter_mut().enumerate() {
            *slot = match views.get(pane) {
                Some(view) if any => {
                    let cutting = view.section.map(|section| cav_sim::voxel_view::Cutting {
                        section,
                        eye: cav_sim::voxel_view::eye_direction(view.yaw, view.pitch),
                    });
                    // A body the view hides has no cubes in it either.
                    let trees: Vec<_> = if view.hidden.is_empty() {
                        self.trees.clone()
                    } else {
                        self.trees
                            .iter()
                            .zip(&spec.bodies)
                            .map(|(t, b)| t.clone().filter(|_| !view.hidden.contains(&b.label)))
                            .collect()
                    };
                    Some(voxel_frame_cut(spec, &trees, view.colour, view.overlay, cutting, time))
                }
                _ => None,
            };
        }
        notes
    }

    /// Why a kept tree of `bytes` bytes is not used: it does not fit beside `in_use`.
    fn over(&self, bytes: usize, in_use: usize) -> String {
        format!(
            "its tree weighs {bytes} bytes, and the scene's trees may weigh {} together, {in_use} of them in use",
            self.budget
        )
    }

    /// Pane `pane`'s frame of the voxel bodies, if the scene has any.
    #[must_use]
    pub fn frame(&self, pane: usize) -> Option<&VoxelFrame> {
        self.frames.get(pane).and_then(Option::as_ref)
    }

    /// Each body's tree as last made, in the scene's order.
    #[must_use]
    pub fn trees(&self) -> &[Option<Arc<VoxTree>>] {
        &self.trees
    }

    /// The bytes of trees kept.
    #[must_use]
    pub fn cached_bytes(&self) -> usize {
        self.cache.iter().map(|(_, t)| t.bytes()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cav_sim::{BodySpec, Motion, ShapeKind, ShapeSpec, dev};

    /// The void with one voxel sphere of `radius` metres.
    fn ball(radius: f64) -> SceneSpec {
        let mut scene = dev::void();
        let shape = ShapeSpec::new(ShapeKind::Sphere { radius }, 0.05, 2);
        scene.bodies.push(BodySpec::shaped(
            "ball",
            10.0,
            shape,
            Motion::Static { position: [2.0, 0.0, 2.0] },
        ));
        scene
    }

    #[test]
    fn the_tree_cache_stays_within_its_budget() {
        let views = [ViewSpec::home(&dev::void())];
        let (small, large) = (ball(0.3), ball(0.6));
        // The budget: what the larger tree weighs, and a little: one tree no longer in use fits
        // beside it only if it is the smaller.
        let mut probe = Voxels::default();
        probe.refresh(&large, 0.0, &views);
        let large_bytes = probe.cached_bytes();
        probe.refresh(&small, 0.0, &views);
        let small_bytes = probe.cached_bytes() - large_bytes;
        assert!(small_bytes > 0 && large_bytes > small_bytes);
        let budget = large_bytes + small_bytes / 2;
        let mut voxels = Voxels::with_budget(budget);
        voxels.refresh(&small, 0.0, &views);
        voxels.refresh(&large, 0.0, &views);
        // The small tree, no longer in use, was dropped to bring the cache within its budget;
        // the large one, in use, was kept.
        assert_eq!(voxels.kept(), 1);
        assert_eq!(voxels.cached_bytes(), large_bytes);
        assert!(voxels.cached_bytes() <= budget);
        assert!(voxels.trees()[0].is_some());
        // A tree that does not fit the scene's budget is not made, and is named with the numbers.
        let mut tight = Voxels::with_budget(large_bytes - 1);
        let notes = tight.refresh(&large, 0.0, &views);
        assert_eq!((tight.kept(), tight.cached_bytes()), (0, 0));
        assert!(tight.trees()[0].is_none());
        assert!(notes.len() == 1 && notes[0].contains(&format!("{}", large_bytes - 1)), "{notes:?}");
        // … and is said once, not at every refresh.
        assert!(tight.refresh(&large, 0.0, &views).is_empty());
        // Two bodies whose trees each fit but not together: the second in the scene's order is
        // refused, and the scene's trees stay within the budget.
        let mut both = large.clone();
        both.bodies.push(small.bodies[0].clone());
        let mut pair = Voxels::with_budget(large_bytes + small_bytes - 1);
        let notes = pair.refresh(&both, 0.0, &views);
        assert!(pair.trees()[0].is_some() && pair.trees()[1].is_none(), "{notes:?}");
        assert!(pair.cached_bytes() < large_bytes + small_bytes);
        assert!(notes[0].contains("in use"), "{notes:?}");
        // Control: the engine's budget keeps both.
        let mut roomy = Voxels::default();
        roomy.refresh(&small, 0.0, &views);
        roomy.refresh(&large, 0.0, &views);
        assert_eq!(roomy.kept(), 2);
        assert_eq!(roomy.cached_bytes(), large_bytes + small_bytes);
    }
}
