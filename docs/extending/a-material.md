# Adding a material, or a body of several

**Example to read:** `scenes/materials.ron` (the library), and the case
`materials.two_material_body_is_the_sum_of_its_parts` in `crates/cav-sim/tests/voxel_bodies.rs`.

A material is a `MaterialSpec` (`crates/cav-sim/src/materials.rs`): a label, a density in kg/m³,
a colour `#rrggbb`, and where it has them its Young's modulus (Pa), Poisson's ratio, thermal
conductivity (W/(m K)), specific heat (J/(kg K)) and linear expansion (1/K), with where the numbers
come from. Only the density is read by the forward model today; the others wait for rigid dynamics
(S7) and heat (S13). The world volume (S4) uses the same kind of material, by index.

## A material for every scene: the library

Add an entry to `scenes/materials.ron`, in the file's units, with a `source` (a handbook and what
the value is typical of: "near 20 °C", "dry", "a range in nature"). The library is compiled in,
so the page has it too. `the_library_is_valid` holds every entry to its ranges and the labels to
being unique; the inspector's material drop-down lists the library in the file's order.

## A material for one scene

A scene's own `materials` list is looked in before the library (`materials::find`), so a scene can
add a material or change a library one for itself:

```ron
materials: [(label: "lead", density: 11000.0, colour: "#5b6572")],
```

## A body of a material, and of several

A voxel body's `shape.material` names one; the body then weighs its density times its volume as
voxelised (`n × (h/k)³`, `n` its sample points inside), and its `mass` field is not used (the
inspector shows the mass the material gives; what any body weighs is `BodySpec::mass_in`, which the
bundle's `source_mass` writes). A body with parts has a material of its own: set it first. With no material the body is uniform and weighs
its `mass`, as before S19.

`shape.parts` are laid in order after the shape: `Add` puts matter of the part's material where
the part is, `Paint` turns the matter already there to it, `Erase` takes matter away. A part is a
`Solid` (any shape a body can be, centred on its `at`) or a `Stroke` (balls along a path, as the
Paint tool lays them). The voxeliser is `cav_core::voxel::layers::voxelise_layers`: each sample
point's material by the layers in order; a finest cell more than one material shares keeps the
sum of their masses (`VoxTree::mixed`; the exact-skin note's "moments add"), its leaf's
`material` the one with most points, for colour.

```
set bodies[1].shape.material steel
add bodies[1].shape.parts {"mode":"Add","material":"copper","at":[0.4,0,0],"kind":{"Solid":{"shape":{"Cuboid":{"size":[0.4,0.4,0.4]}}}}}
colour material
```

## What holds it

- `materials.shared_cells_are_within_their_dipole_bound` (validation): a body whose interface lies
  halfway through a layer of cells, against the two closed-form prisms, within the shared cells'
  dipole bound |Δρ|·v·h/8 (S19's review). `source_mass_is_what_each_body_weighs` (bundle): ρ·V.

- `a_shared_cell_keeps_both_masses` and `the_walk_equals_the_raster` (`cav-core`, layers): a
  cell's mass, and the tree against the point-by-point raster, bit for bit.
- `mass_is_kept_to_the_bit_through_material_edits` (`cav-app`, editor): every edit undone gives
  the same bits back.
- The validation case above: a body of two materials reads as the sum of its parts.
- `colour_by_material_uses_each_material_s_colour` (`cav-sim`, voxel view).
