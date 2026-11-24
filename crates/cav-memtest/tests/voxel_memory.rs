//! A voxel tree's memory is bounded by its caps whatever the shape and the pitch (the S3 brief,
//! `docs/slices/S3.md` §3.1, rows V1 to V4 and V6). Measured with the counting allocator; nextest
//! runs each test in a process of its own.

use cav_core::Vec3;
use cav_core::voxel::shape::{Shape, Sphere};
use cav_core::voxel::{
    Budget, Grid, MAX_LEAVES, MAX_TREE_NODES, MAX_VOXEL_WORK, mesh, voxelise, voxelise_within,
};
use cav_memtest::{Counting, peak_during};

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn a_tree_is_bounded_and_refused_past_its_caps() {
    // The engine's caps are the stated ones.
    assert_eq!(
        Budget::DEFAULT,
        Budget { leaves: MAX_LEAVES, nodes: MAX_TREE_NODES, work: MAX_VOXEL_WORK, bytes: usize::MAX }
    );
    // V1: a pitch finer than one tree holds is refused before anything is built.
    let sphere = Sphere { centre: Vec3::ZERO, radius: 0.5 };
    assert!(Grid::around(sphere.bounds(), 1.0 / 1025.0, 1).is_err());
    // V2 at the real cap: a sphere 1024 cells across has more surface cells than a tree may hold.
    // It is refused, and what it held at the most is the leaves' buffer at the cap (eight bytes a
    // leaf, the buffer doubling once past it), not the tree it would have been.
    let grid = Grid::around(sphere.bounds(), 1.0 / 1024.0, 1).unwrap();
    let (result, peak) = peak_during(|| voxelise(&sphere, &grid, 1.0, 0));
    let why = result.unwrap_err().0;
    eprintln!("1024 across: refused ({why}); {peak} bytes live at most");
    assert!(why.contains("leaves"), "{why}");
    assert!(peak <= 40 << 20, "{peak} bytes");
    // V2 and V3 and V4 by their mechanism, with a small budget.
    let small = |leaves, nodes, work| Budget { leaves, nodes, work, bytes: usize::MAX };
    let grid = Grid::around(sphere.bounds(), 1.0 / 64.0, 2).unwrap();
    let full = voxelise(&sphere, &grid, 1.0, 0).unwrap();
    let (leaves, nodes) = (full.leaves.len(), full.nodes.len());
    assert!(voxelise_within(&sphere, &grid, 1.0, 0, small(leaves, nodes, u64::MAX)).is_ok());
    assert!(
        voxelise_within(&sphere, &grid, 1.0, 0, small(leaves - 1, nodes, u64::MAX))
            .unwrap_err()
            .0
            .contains("leaves")
    );
    let (refused, peak) =
        peak_during(|| voxelise_within(&sphere, &grid, 1.0, 0, small(leaves, nodes - 1, u64::MAX)));
    assert!(refused.unwrap_err().0.contains("nodes"));
    // The nodes are counted before their moments are reserved: what was live is the leaves.
    assert!(peak < leaves * 8 * 3 + (64 << 10), "{peak} bytes for {leaves} leaves");
    assert!(
        voxelise_within(&sphere, &grid, 1.0, 0, small(usize::MAX, usize::MAX, 1000))
            .unwrap_err()
            .0
            .contains("tests")
    );
    // Control: the tree that is built is all there and holds more than the refusal did.
    let (_, built) = peak_during(|| voxelise(&sphere, &grid, 1.0, 0).unwrap());
    assert!(built > peak, "{built} against {peak}");
    // The cap on bytes: a tree is refused one byte short of what it weighs, so what is counted
    // before it is made is never less than what it is; and it is made within exactly that.
    let bytes = full.bytes();
    assert!(voxelise_within(&sphere, &grid, 1.0, 0, Budget::with_bytes(bytes + (64 << 10))).is_ok());
    let (refused, peak) =
        peak_during(|| voxelise_within(&sphere, &grid, 1.0, 0, Budget::with_bytes(bytes - 1)));
    assert!(refused.unwrap_err().0.contains("bytes"));
    assert!(peak < leaves * 8 * 3 + (64 << 10), "{peak} bytes for {leaves} leaves");
}

/// A scene at the body cap, every body a distinct voxel sphere of about `bytes` bytes of tree.
fn crowd(bodies: usize) -> cav_sim::SceneSpec {
    let mut scene = cav_sim::dev::void();
    for n in 0..bodies {
        let radius = 0.1 + 1e-6 * n as f64;
        let shape =
            cav_sim::ShapeSpec { kind: cav_sim::ShapeKind::Sphere { radius }, pitch: 0.01, supersample: 1 };
        let at = [(n % 40) as f64 * 0.3 - 6.0, (n / 40) as f64 * 0.3 - 4.0, 1.0];
        scene.bodies.push(cav_sim::BodySpec::shaped(
            &format!("ball {n}"),
            1.0,
            shape,
            cav_sim::Motion::Static { position: at },
        ));
    }
    scene
}

#[test]
fn the_scene_s_trees_stay_within_their_budget_at_the_body_cap() {
    // V10: tens of distinct fine bodies held gigabytes when only trees no longer in use counted
    // (S3's review, finding 4). Now the scene's trees share one budget, counted before each is
    // made. Here a small budget stands for the engine's 256 MiB, at the body cap.
    let scene = crowd(cav_sim::spec::limits::BODIES);
    assert_eq!(scene.validate(), Ok(()));
    let views = [cav_sim::ViewSpec::home(&scene)];
    let budget = 4 << 20;
    let mut voxels = cav_engine::voxels::Voxels::with_budget(budget);
    let (notes, peak) = peak_during(|| voxels.refresh(&scene, 0.0, &views));
    let drawn = voxels.trees().iter().flatten().count();
    let one = voxels.trees().iter().flatten().next().unwrap().bytes();
    eprintln!(
        "{drawn} of {} bodies drawn as voxels ({one} bytes a tree); {peak} bytes live at most",
        scene.bodies.len()
    );
    assert!(voxels.cached_bytes() <= budget);
    assert!(drawn > 10 && drawn < scene.bodies.len(), "{drawn}");
    assert!(notes.len() == scene.bodies.len() - drawn && notes[0].contains("in use"), "{:?}", &notes[..1]);
    // What was live: the trees, the frame of what is drawn (at most 20,000 cubes) and the notes.
    assert!(peak <= budget + (12 << 20), "{peak} bytes");
    // Control: the same bodies without the budget hold their trees' worth, far over it.
    let few = crowd(400);
    let mut unbounded = cav_engine::voxels::Voxels::with_budget(usize::MAX / 2);
    let (_, held) = peak_during(|| unbounded.refresh(&few, 0.0, &[cav_sim::ViewSpec::home(&few)]));
    assert!(held > budget + (12 << 20) && unbounded.cached_bytes() > 4 * budget, "{held}");
}

#[test]
fn a_mesh_file_is_read_up_to_its_cap() {
    let dir = std::env::temp_dir().join(format!("cav-mesh-cap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let big = dir.join("big.obj");
    // A file one kilobyte over the cap: read to the cap and one byte, then refused.
    std::fs::write(&big, vec![b'#'; (mesh::MAX_MESH_BYTES + 1024) as usize]).unwrap();
    let (result, peak) = peak_during(|| mesh::Mesh::read(&big, 1.0));
    assert!(result.unwrap_err().0.contains("more than"));
    assert!(peak as u64 <= 2 * (mesh::MAX_MESH_BYTES + 1), "{peak} bytes");
    // Control: a small closed mesh reads.
    let small = dir.join("box.obj");
    let (v, t) = mesh::box_mesh(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
    std::fs::write(&small, mesh::to_obj(&v, &t)).unwrap();
    assert!((mesh::Mesh::read(&small, 0.5).unwrap().volume() - 1.0).abs() < 1e-12);
    std::fs::remove_dir_all(&dir).unwrap();
}
