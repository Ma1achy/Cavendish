# Octree depth views and sections

Every node of the voxel bodies' octrees, coloured by depth on the sequential map (`overlay tree`),
faded by depth, and cut by section planes (`section`). Made headlessly with `cav render`
(TASK-S3-19 and TASK-S3-20); the legends are pasted in as the application paints them.
Regenerate one with its command (from the repository root, after `cargo build --release -p cav`).

## What the commands do

- `overlay tree [depth a..b] [fade deep|shallow] [faces] [alone]` draws every node of every voxel
  body's octree, internal and leaf, from the root (depth 0) down, each box in its depth's colour on
  the sequential map (`cav-render/src/colour.rs`, the blue ramp the `dataviz` validator passed for
  colour-blind vision and grey print; dark to light on a dark surface, so the deepest stands out).
  `fade deep` draws shallow nodes faint (opacity 0.15) rising to solid at the deepest; `fade
  shallow` the reverse. `faces` adds see-through faces (x-ray only); `alone` hides the leaf cubes.
  The legend (bottom right, above the field's) is made from the depths drawn, faded as drawn.
- `colour depth` colours the leaves by the same depth on the same range, so a leaf and a node of
  one depth are one colour; `colour level` is as before.
- `section <x|y|z> <metres>` / `section off`: seen square-on (top, bottom, north, south, east,
  west) the bodies are a 2-D section: each leaf the plane passes through is a square in the plane,
  filled by the colour field (empty where the body is hollow), with the tree overlay cut to its
  quadtree on top (a fiftieth of a pitch towards the camera, and the fills inset by 4 % of their
  side so outlines of one colour show). From any other side, the half nearer the camera is cut
  away and the blocks the plane crosses are clipped to it. The app's View panel has buttons and a
  slider for the plane's place; the console, `cav render --exec`, `cav exec` and MCP `command`
  take the same words, and each is journalled.
- **Bounded**: at most 20,000 tree nodes a view (`MAX_TREE_BOXES`), counted before any is made
  and only where a section keeps them; past it, whole depths are left out from the deepest up, the
  legend says "to depth d", the app lists the note and `cav render` prints it (see the scaffold
  and `sphere-r64-capped`). Cubes and cells keep S3's 20,000-a-view budget.
- The unit cube's tree is one leaf: its grid fits the cube, so the merge makes the root the whole
  body. Its pictures show that single box.
- In the scaffold, each tube's root is a cube of 2^9 voxels of 2 cm, about 10 m on a side, set at
  the corner of the tube's box: the big boxes reaching east and north are those roots.

## sphere-depth.png

every node, root to leaves, coloured by depth (sequential map); x-ray, cubes hidden.

```
cav render sphere --out docs/slices/S3/octree/sphere-depth.png --size 1600x1100 --style xray --exec "view iso; pan 2 0 2.5; zoom 2.6; overlay tree alone"
```

## sphere-opacity.png

opacity by depth: shallow nodes solid, deep ones faint (the coarse structure).

```
cav render sphere --out docs/slices/S3/octree/sphere-opacity.png --size 1600x1100 --style xray --exec "view iso; pan 2 0 2.5; zoom 2.6; overlay tree alone fade shallow"
```

## sphere-both.png

colour and opacity together: deep nodes solid, shallow faint, with see-through faces.

```
cav render sphere --out docs/slices/S3/octree/sphere-both.png --size 1600x1100 --style xray --exec "view iso; pan 2 0 2.5; zoom 2.6; colour depth; overlay tree alone fade deep faces"
```

## sphere-closeup.png

close-up, deep nodes solid.

```
cav render sphere --out docs/slices/S3/octree/sphere-closeup.png --size 1600x1100 --style xray --exec "view iso; pan 2.3 -0.3 2.8; zoom 1.1; overlay tree alone fade deep"
```

## sphere-cutaway.png

cut-away: the half above the plane removed, leaves coloured by depth, the tree over them.

```
cav render sphere --out docs/slices/S3/octree/sphere-cutaway.png --size 1600x1100 --style shaded --exec "view iso; pan 2 0 2.5; zoom 2.6; colour depth; overlay tree fade shallow; section z 2.51"
```

## sphere-section.png

plan-view section: the octree cut to a quadtree, material filled by depth.

```
cav render sphere --out docs/slices/S3/octree/sphere-section.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 1.9500000000000002; colour depth; overlay tree; section z 2.51"
```

## unit-cube-depth.png

every node, root to leaves, coloured by depth (sequential map); x-ray, cubes hidden.

```
cav render unit-cube --out docs/slices/S3/octree/unit-cube-depth.png --size 1600x1100 --style xray --exec "view iso; pan 2 0 2.5; zoom 3.4; overlay tree alone"
```

## unit-cube-opacity.png

opacity by depth: shallow nodes solid, deep ones faint (the coarse structure).

```
cav render unit-cube --out docs/slices/S3/octree/unit-cube-opacity.png --size 1600x1100 --style xray --exec "view iso; pan 2 0 2.5; zoom 3.4; overlay tree alone fade shallow"
```

## unit-cube-both.png

colour and opacity together: deep nodes solid, shallow faint, with see-through faces.

```
cav render unit-cube --out docs/slices/S3/octree/unit-cube-both.png --size 1600x1100 --style xray --exec "view iso; pan 2 0 2.5; zoom 3.4; colour depth; overlay tree alone fade deep faces"
```

## unit-cube-closeup.png

close-up, deep nodes solid.

```
cav render unit-cube --out docs/slices/S3/octree/unit-cube-closeup.png --size 1600x1100 --style xray --exec "view iso; pan 2.4 -0.4 2.9; zoom 1.3; overlay tree alone fade deep"
```

## unit-cube-cutaway.png

cut-away: the half above the plane removed, leaves coloured by depth, the tree over them.

```
cav render unit-cube --out docs/slices/S3/octree/unit-cube-cutaway.png --size 1600x1100 --style shaded --exec "view iso; pan 2 0 2.5; zoom 3.4; colour depth; overlay tree fade shallow; section z 2.51"
```

## unit-cube-section.png

plan-view section: the octree cut to a quadtree, material filled by depth.

```
cav render unit-cube --out docs/slices/S3/octree/unit-cube-section.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 2.55; colour depth; overlay tree; section z 2.51"
```

## platonic-row-depth.png

every node, root to leaves, coloured by depth (sequential map); x-ray, cubes hidden.

```
cav render platonic-row --out docs/slices/S3/octree/platonic-row-depth.png --size 1600x1100 --style xray --exec "view iso; pan 0 0 2.5; zoom 8.5; overlay tree alone"
```

## platonic-row-opacity.png

opacity by depth: shallow nodes solid, deep ones faint (the coarse structure).

```
cav render platonic-row --out docs/slices/S3/octree/platonic-row-opacity.png --size 1600x1100 --style xray --exec "view iso; pan 0 0 2.5; zoom 8.5; overlay tree alone fade shallow"
```

## platonic-row-both.png

colour and opacity together: deep nodes solid, shallow faint, with see-through faces.

```
cav render platonic-row --out docs/slices/S3/octree/platonic-row-both.png --size 1600x1100 --style xray --exec "view iso; pan 0 0 2.5; zoom 8.5; colour depth; overlay tree alone fade deep faces"
```

## platonic-row-closeup.png

close-up, deep nodes solid.

```
cav render platonic-row --out docs/slices/S3/octree/platonic-row-closeup.png --size 1600x1100 --style xray --exec "view iso; pan 2.09 0.68 2.5; zoom 1.6; overlay tree alone fade deep"
```

## platonic-row-cutaway.png

cut-away: the half above the plane removed, leaves coloured by depth, the tree over them.

```
cav render platonic-row --out docs/slices/S3/octree/platonic-row-cutaway.png --size 1600x1100 --style shaded --exec "view iso; pan 0 0 2.5; zoom 8.5; colour depth; overlay tree fade shallow; section z 2.51"
```

## platonic-row-section.png

plan-view section: the octree cut to a quadtree, material filled by depth.

```
cav render platonic-row --out docs/slices/S3/octree/platonic-row-section.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 0 0 2.5; zoom 6.375; colour depth; overlay tree; section z 2.51"
```

## scaffold-depth.png

every node, root to leaves, coloured by depth (sequential map); x-ray, cubes hidden.

```
cav render scaffold --out docs/slices/S3/octree/scaffold-depth.png --size 1600x1100 --style xray --exec "view iso; pan 0 0 5; zoom 17; overlay tree alone"
```

> note: more than 20000 leaves: drawn by the cut at level 1
> note: more than 20000 tree nodes: the tree overlay is drawn to depth 5 of 9

## scaffold-opacity.png

opacity by depth: shallow nodes solid, deep ones faint (the coarse structure).

```
cav render scaffold --out docs/slices/S3/octree/scaffold-opacity.png --size 1600x1100 --style xray --exec "view iso; pan 0 0 5; zoom 17; overlay tree alone fade shallow"
```

> note: more than 20000 leaves: drawn by the cut at level 1
> note: more than 20000 tree nodes: the tree overlay is drawn to depth 5 of 9

## scaffold-both.png

colour and opacity together: deep nodes solid, shallow faint, with see-through faces.

```
cav render scaffold --out docs/slices/S3/octree/scaffold-both.png --size 1600x1100 --style xray --exec "view iso; pan 0 0 5; zoom 17; colour depth; overlay tree alone fade deep faces"
```

> note: more than 20000 leaves: drawn by the cut at level 1
> note: more than 20000 tree nodes: the tree overlay is drawn to depth 5 of 9

## scaffold-closeup.png

close-up, deep nodes solid.

```
cav render scaffold --out docs/slices/S3/octree/scaffold-closeup.png --size 1600x1100 --style xray --exec "view iso; pan 0.5 -0.5 5; zoom 0.9; overlay tree alone fade deep"
```

> note: more than 20000 leaves: drawn by the cut at level 1
> note: more than 20000 tree nodes: the tree overlay is drawn to depth 5 of 9

## scaffold-cutaway.png

cut-away: the half above the plane removed, leaves coloured by depth, the tree over them.

```
cav render scaffold --out docs/slices/S3/octree/scaffold-cutaway.png --size 1600x1100 --style shaded --exec "view iso; pan 0 0 5; zoom 17; colour depth; overlay tree fade shallow; section z 5.0"
```

> note: more than 20000 leaves: drawn by the cut at level 1
> note: more than 20000 tree nodes: the tree overlay is drawn to depth 8 of 9

## scaffold-section.png

plan-view section: the octree cut to a quadtree, material filled by depth.

```
cav render scaffold --out docs/slices/S3/octree/scaffold-section.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 0 0 5; zoom 2.2; colour depth; overlay tree; section z 5.0"
```

## sphere-r32-opacity.png

the sphere at r/32 (several merged levels): opacity by depth, shallow solid.

```
cav render sphere --out docs/slices/S3/octree/sphere-r32-opacity.png --size 1600x1100 --style xray --exec "set bodies[0].shape.pitch 0.015625; view iso; pan 2 0 2.5; zoom 2.6; overlay tree alone fade shallow"
```

> note: more than 20000 leaves: drawn by the cut at level 1
> note: more than 20000 tree nodes: the tree overlay is drawn to depth 5 of 6

## sphere-r32-section.png

the sphere at r/32 through its middle: merged leaves of four sizes, and the quadtree over them.

```
cav render sphere --out docs/slices/S3/octree/sphere-r32-section.png --size 1600x1100 --style flat --exec "set bodies[0].shape.pitch 0.015625; view top; proj ortho; pan 2 0 2.5; zoom 1.9; colour depth; overlay tree; section z 2.51"
```

## sphere-r64-capped.png

r/64: more than 20,000 nodes, so the shallowest whole depths are drawn and the legend says where it stopped.

```
cav render sphere --out docs/slices/S3/octree/sphere-r64-capped.png --size 1600x1100 --style xray --exec "set bodies[0].shape.pitch 0.0078125; view iso; pan 2 0 2.5; zoom 2.6; overlay tree alone"
```

> note: more than 20000 leaves: drawn by the cut at level 2
> note: more than 20000 tree nodes: the tree overlay is drawn to depth 5 of 7

## hollow-section-z2.51.png

hollow sphere, plan-view section at z = 2.51 m (0.01 m above the centre): material by depth, quadtree outlines.

```
cav render hollow-sphere --out docs/slices/S3/octree/hollow-section-z2.51.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 1.9; colour depth; overlay tree; section z 2.51"
```

## hollow-section-z2.7.png

hollow sphere, plan-view section at z = 2.7 m (0.2 m above the centre): material by depth, quadtree outlines.

```
cav render hollow-sphere --out docs/slices/S3/octree/hollow-section-z2.7.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 1.9; colour depth; overlay tree; section z 2.7"
```

## hollow-section-z2.85.png

hollow sphere, plan-view section at z = 2.85 m (0.35 m above the centre): material by depth, quadtree outlines.

```
cav render hollow-sphere --out docs/slices/S3/octree/hollow-section-z2.85.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 1.9; colour depth; overlay tree; section z 2.85"
```

## hollow-section-z2.95.png

hollow sphere, plan-view section at z = 2.95 m (0.45 m above the centre): material by depth, quadtree outlines.

```
cav render hollow-sphere --out docs/slices/S3/octree/hollow-section-z2.95.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 1.9; colour depth; overlay tree; section z 2.95"
```

## hollow-section-fade.png

hollow sphere through its middle, the quadtree faded by depth (deep solid).

```
cav render hollow-sphere --out docs/slices/S3/octree/hollow-section-fade.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 1.9; colour depth; overlay tree fade deep; section z 2.51"
```

## solid-section-z2.51.png

solid sphere, plan-view section at z = 2.51 m (0.01 m above the centre): material by depth, quadtree outlines.

```
cav render sphere --out docs/slices/S3/octree/solid-section-z2.51.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 1.9; colour depth; overlay tree; section z 2.51"
```

## solid-section-z2.7.png

solid sphere, plan-view section at z = 2.7 m (0.2 m above the centre): material by depth, quadtree outlines.

```
cav render sphere --out docs/slices/S3/octree/solid-section-z2.7.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 1.9; colour depth; overlay tree; section z 2.7"
```

## solid-section-z2.85.png

solid sphere, plan-view section at z = 2.85 m (0.35 m above the centre): material by depth, quadtree outlines.

```
cav render sphere --out docs/slices/S3/octree/solid-section-z2.85.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 1.9; colour depth; overlay tree; section z 2.85"
```

## solid-section-z2.95.png

solid sphere, plan-view section at z = 2.95 m (0.45 m above the centre): material by depth, quadtree outlines.

```
cav render sphere --out docs/slices/S3/octree/solid-section-z2.95.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 1.9; colour depth; overlay tree; section z 2.95"
```

## solid-section-fade.png

solid sphere through its middle, the quadtree faded by depth (deep solid).

```
cav render sphere --out docs/slices/S3/octree/solid-section-fade.png --size 1600x1100 --style flat --exec "view top; proj ortho; pan 2 0 2.5; zoom 1.9; colour depth; overlay tree fade deep; section z 2.51"
```
