# Making the editor natural: layout, tools, palette, inspector

Research note, 2026-11-25. Question: the owner finds the GUI hard to navigate (where things are, adding and
removing, setting up scenes, the editor). What do other editors do, and what should Cavendish build, in what
order? No code was changed. Where a source could not be read, that is said in
[What I could not verify](#what-i-could-not-verify).

## One-page summary

**What is wrong now.** The window works, but it is a viewer with a console behind it, not an editor. A first-time
user can add exactly one thing from the GUI (a 1 kg point mass at a fixed spot, the `+ mass` button), cannot add a
shape or a tower, cannot place anything except by dragging in the screen plane, cannot duplicate, has no Delete key
or right-click menu, cannot see the undo history, and finds the rest of the instrument's features only by typing
into the console. The left panel mixes the scene tree with camera and draw-style controls; the right panel shows
the same Cadence and Instrument blocks whatever is selected. Four menu items that now have working commands are
still greyed out with stale slice labels. Details, with evidence, in [section 1](#1-what-exists-and-what-a-first-time-user-cannot-do).

**What editors do that works.** Every editor surveyed converges on the same five ideas
([section 2](#2-patterns-that-make-editors-intuitive)): a tree of what exists beside the 3-D view and an inspector for
the selection; a toolbar of a few tools with one active, whose handles appear on the selected thing; a searchable
"add" list and a search-everything command box that also shows shortcuts; a visible, clickable undo history;
and docked panels with a few named layouts. None conflicts with Cavendish's rules. All of them can sit on the
existing `Command` path.

**The one architectural fact that makes this cheap.** Every edit in Cavendish already is a `Command`
(`Set`, `Insert`, `Remove`, `Select`, `Undo`, …) and a drag already coalesces into one undo step
([ARCHITECTURE.md](ARCHITECTURE.md), `editors.rs` `command_for`). A gizmo, a click-to-place tool, a palette entry
and an inspector field are four front ends to the same `Command::Set` / `Insert`. Nothing needs a second path; two
small engine additions are needed (multi-selection and a grouped edit; [section 3.4](#34-two-routes-one-command-path)).

**Top ten, ranked.** Number 1 is a requirement of the editor slice, not an option (owner's ruling,
[docs/design/keyboard.md](../design/keyboard.md), read from the unmerged branch `design-keyboard`); full table in
[section 3.9](#39-ranking-and-slice-scope).

1. **Principia's keyboard model in full**: scope tree as data, navigation and interaction modes, orange and blue rings, breadcrumb, a fixed key router, the `?` sheet, state-machine tests. Applied to Cavendish's layout in [section 3.10](#310-the-keyboard-model-applied). It is the spine the other nine register into.
2. Command palette (Ctrl+K) over every command, menu item and spec path, with shortcuts, generated from the same data as the `?` sheet.
3. An **Add** menu and searchable palette of bodies and shapes, with sensible starters; the greyed-out menu items that work fixed.
4. A scene tree that works like a tree: right-click add/duplicate/delete/rename, Delete, Ctrl+D, F2, F; View settings moved out of it.
5. Toolbar with one active tool (Select, Move, Rotate, Scale, Place, Paint, Measure, Slice, Probe), a tool-options strip, and gizmos with snapping and typed numbers in the 3-D area; click-to-place on floor or surface.
6. Inspector improvements: sections, list add/remove/reorder, reset-to-default, units, field search.
7. A visible undo history and toasts for what was done or refused.
8. Empty-state guidance, a "New scene" wizard with templates, Save/Open enabled.
9. Docked workspaces (Scene, Instrument, Analysis) with saved layouts.
10. The second half of the tools: Probe, Measure, section handle, voxel brushes, box-select, multi-edit.

UX tests ([section 4](#4-validation-how-to-test-ux-claims-here)) belong to every item, not a separate one.

**Fits one slice (A):** 1 to 8, with the first half of 5 (Select, Move, Place; translate gizmo; snapping; typed
distance). **Later (B):** 9, 10, rotate and scale.

---

## 1. What exists and what a first-time user cannot do

Sources read: [viewer-mockup/README.md](../viewer-mockup/README.md) (the mock-up's rules: figure never covered,
badges in a header strip, Home the one control on the figure, rare settings in windows, Tab walks scopes, F3 hides the
GUI, footer opens the console), [M13 §9](../../reference/milestones/M13-viewer-v2.md) (layout, menus, panels, keys),
[REBUILD.md §6](../../plan/REBUILD.md), [ARCHITECTURE.md](../ARCHITECTURE.md), the code of `crates/cav-app/src`
(`menus.rs`, `panels.rs`, `editors.rs`, `shell.rs`), and the screenshots
[S0 default](../slices/S0/app-default.png), [S0 console](../slices/S0/app-console.png),
[S0 small window](../slices/S0/app-small-window.png), [S1 signal](../slices/S1/signal.png),
[S2 editors](../slices/S2/editors.png), [S2 cadence](../slices/S2/cadence-gaps-loss.png),
[S3 mixed array](../slices/S3/mixed-array.png), [S3 shapes](../slices/S3/shapes.png),
[S3 overlay-cut](../slices/S3/overlay-cut.png), [S17 web](../slices/S17/web-live.png). I looked at these as
images; I did not run the app (the owner is working on this PC).

### 1.1 What the design promised, and what is built

M13 §9.2 specified docked panels (`egui_dock`), three layout presets (Explore, Configure, Dataset), a command
palette (§9.7: "Ctrl-K: fuzzy search over the same commands and over spec paths"), an edit history window,
right-click context menus on outliner rows ("context menu = the Scene and Array menus for that row", §9.4), gizmos
for the selection (§9.4, 3-D view), Tab to cycle sources, `1–9` tower select, and per-field reset. In the code on
`main`: the layout is fixed (left outliner, centre figure, right properties, bottom timeline; no docking crate in
`Cargo.lock`), there is no palette, no context menu (`grep` finds no `context_menu` in `cav-app/src`), no gizmo
anywhere in the workspace, and `Key::` handling in `shell.rs` covers only F3, Esc, `` ` ``, Space, O, Home, `?`,
Ctrl+Z, arrows and `1–9`.

### 1.2 Concrete problems, by task

| Task a newcomer tries | What happens today | Evidence |
|---|---|---|
| Add a body | One button, `+ mass`, under the Sources list. It runs `mass 1 at 2 2 1`: a 1 kg point mass at a fixed place beside the array. No choice of shape, mass or position first. | `panels.rs` `outliner`: `button_row(… "mass 1 at 2 2 1")`; [S2 editors](../slices/S2/editors.png) |
| Add a sphere, cube or other shape (S3 voxel shapes) | Not possible from the GUI. Shapes are reached by loading a dev scene (`load sphere`) or typing `add bodies {…}` in the console. | `menus.rs`: `later("Add source", "S1")`; shapes visible in [S3 shapes](../slices/S3/shapes.png) only because the scene was loaded |
| Add a tower | Greyed `Array > Add tower` ("S2"). A tower can be removed from its properties, not added. | `menus.rs`; `editors.rs` `structure` |
| Place or move a body | Drag in the screen plane (works, S1). No gizmo, no axis constraint, no snapping, no click-to-place. Exact placement is by typing in the `centre, m` fields of the properties panel. | `shell.rs` `drag_body_by`; [S2 editors](../slices/S2/editors.png) |
| Delete | A small `remove this body` button right-aligned under the fields of the selection. No Delete key, no right-click. | `editors.rs` `structure` |
| Duplicate / rename | Duplicate does not exist. Rename is the `label` text field, found only by looking in properties. | `panels.rs` `properties` |
| See and use the undo history | Ctrl+Z and Edit > Undo work. `Edit history…` is greyed ("S2"), though the history exists in the engine (`history.rs`). The status line shows one message at a time. | `menus.rs`; `docs/ARCHITECTURE.md` (history: the inverse edit of each edit) |
| Choose what to edit | Click a body in the view, click an outliner row, or press `1–9` (towers only). No Tab, no multi-select, no focus ring or breadcrumb (promised by the mock-up). `Select` takes one `Target`. | `shell.rs` `keys`; `command.rs` `Select(Option<Target>)` |
| Set up a scene from scratch | `File > New scene from library` offers `room`, `void`, `infinite-floor`. There is no "New empty scene" or template. An empty scene's guidance is one grey line, "none · `load point` has one", which points at the console. | `menus.rs`; [S0 default](../slices/S0/app-default.png) |
| Save and load | `Open scene…`, `Save scene`, `Export…` are greyed ("S2"), but `save <file>` and `export bundle` exist as console commands and `Command::Export` handles them. A user is told a feature is not there that is. On the web, reads by path are refused ([ARCHITECTURE.md](../ARCHITECTURE.md), seams table), so Open needs a file picker or drop. | `menus.rs`; `console.rs` |
| Discover commands | The console (a small word `console` at the bottom right, or the backtick key) is the only complete list; its hint text suggests six commands. The `?` window lists 11 keys. No palette. | [S0 console](../slices/S0/app-console.png) |
| Understand how outliner, properties and view relate | Selecting a row highlights it in the view (ring in [S0 default](../slices/S0/app-default.png)), but the left panel is also the camera/draw-style control panel (look, project, panes, draw, show, colour, overlay, section) under the heading View, and the right panel always appends Cadence, Instrument and Camera blocks. A body's mass and the array's `T` read as one list. | `panels.rs` `outliner`, `properties` |
| Keyboard | Only through the `?` window or the menus' right-hand hints. No Delete, F (frame), Ctrl+D, Ctrl+S, Tab, or tool keys. | `shell.rs` `keys` |
| Cadence and timing | Cadence (`jitter / alternate / gaps / window` each "none [add]") is a block in the properties of *every* selection. The timeline's best feature, Shift-drag a lane to move that tower's clock, is announced only in the caption text. | [S2 cadence](../slices/S2/cadence-gaps-loss.png), `panels.rs` |
| Plots | The pane's content is one of nine toggle buttons (`scene signal periodogram flight cadence …`) in the left panel, applied to the *active pane*; to see the scene and a plot together a user must set panes to 2, activate one, then pick content. | `panels.rs` `outliner` (View); [S1 signal](../slices/S1/signal.png) |
| Octree and section tools | `colour`, `overlay`, `section` rows appear in the left panel only once a voxel body exists; they are rows of text buttons, one clipped at the panel edge (`tree, deep soli…` in [S3 overlay-cut](../slices/S3/overlay-cut.png)). The section plane is a slider, not a handle in the view. | `panels.rs` `voxel_rows` |
| Small windows | In the 1024×700 capture the left panel's lower sections are cut off and the `?` window floats over the figure and the panels ([S0 small window](../slices/S0/app-small-window.png)). The mock-up's rule is that windows are for rarely changed settings; a key reference is one, but it still hides the figure. | image |

Two further observations. First, the *mock-up's* approach (a header strip of badges, Home the only on-figure
control) is good and should stay; what is missing is the editor behind it. Second, a lot of the plumbing the
editor needs is already there: the schema editors (`editors.rs`), the `Insert`/`Remove` commands, undo by bytes,
the journal, the layout recorder for tests. The gap is chiefly front-end.

---

## 2. Patterns that make editors intuitive

Each row: the pattern, why it works, whether it fits Cavendish's rules (R1 every edit is a command; R2
stale-not-blocked; R3 the figure is never covered; R4 the web build).

### 2.1 Game engines

| Pattern | Source | Why it works | Fit |
|---|---|---|---|
| **Hierarchy (tree), Inspector, Scene view as one triad**: selecting in any selects in all; the Inspector shows the selected object's properties | Unity: [Hierarchy](https://docs.unity.com/en-us/engine/current/manual/unity-editor/editor-windows-views-reference/hierarchy-window/hierarchy) (drag to reparent, right-click menu, Create Empty Parent, per-row visibility eye and pick-lock hand, H/L keys); Unreal: [Level Editor](https://dev.epicgames.com/documentation/en-us/unreal-engine/level-editor-in-unreal-engine) (Outliner as tree of all Actors; Details panel for the selected Actor); Godot: [first look](https://docs.godotengine.org/en/stable/getting_started/introduction/first_look_at_the_editor.html) (Scene dock lists nodes, Inspector edits the selected node) | One mental model of "the thing I selected", three views of it. Users learn it once. | Fits. Cavendish already has outliner, properties, view; fix is making each row actionable and removing unrelated blocks from properties. |
| **Place Actors / Add menu with search**; drag an item into the viewport to place it | Unreal [Place Actors panel](https://dev.epicgames.com/documentation/en-us/unreal-engine/placing-actors-in-unreal-engine): search bar, drag into the level | Recognition over recall: the list *is* the documentation of what can be added. Dropping at the cursor teaches that placement is spatial. | Fits (R1: drop = `Insert` with a position). R4: egui drag-and-drop works on wasm. |
| **Transform tools and gizmos** on the selection: Move, Rotate, Scale; local/global switch; snapping | Unity [positioning GameObjects](https://docs.unity.com/en-us/engine/6000.5/manual/working-with-scenes/scenes-manage-gameobjects/positioning-game-objects) (W/E/R; world-grid, surface and vertex snapping; Ctrl while dragging snaps to increments) | Direct manipulation: the effect is the drag. One-key tool switching. | Fits (R1: a drag is a run of `Set` with `continues: true`, already coalesced to one undo). |
| **Workspaces / main-screen buttons** (2D, 3D, Script …), bottom panel folded by default for console, animation etc. | Godot [first look](https://docs.godotengine.org/en/stable/getting_started/introduction/first_look_at_the_editor.html) | A task name is easier than a panel arrangement; the console stays out of the way until wanted. | Fits R3 well: Cavendish's footer-opens-console is the same idea. |
| **Command palette** (Ctrl+Shift+P) | Godot key map: `editor/command_palette` Ctrl+Shift+P ([4.3 key map](https://docs.godotengine.org/en/4.3/tutorials/editor/default_key_mapping.html), via search summary) | One place for every action, searchable by name. | Fits strongly: every menu item is already a console line. |
| **Play mode** (run in editor, changes during play are discarded) | Godot [first look](https://docs.godotengine.org/en/stable/getting_started/introduction/first_look_at_the_editor.html) ("changes made here are not saved when the game stops") | Separates editing from running. | Not needed: Cavendish's scrub-the-playhead plus stale-not-blocked already does this better. Skip. |
| **Prefabs / assets browser** | Unity, Unreal, Godot (Content Browser, FileSystem dock) | Reuse. | Later: a "library" of saved bodies is a good second slice; templates in 3.7 cover the first need. |

### 2.2 Model and voxel editors

| Pattern | Source | Why it works | Fit |
|---|---|---|---|
| **Search the menus (F3)**: type part of an operator name, see the menu it lives in; some operators exist only in search | Blender [Menu Search](https://docs.blender.org/manual/en/3.6/interface/controls/templates/operator_search.html) (via search summary; the page text itself could not be fetched, see below) | Combines discoverability (it teaches where things live) and speed. | Fits. The owner's F3 is already "hide the GUI" (mock-up rule); the palette should take Ctrl+K and let F3 stay, or move hide to a different key; a decision for the owner. |
| **Numeric input while transforming**: press a transform key, type a number; Tab to the next axis; `=` for expressions and units | Blender [Numeric Input](https://docs.blender.org/manual/en/5.0/scene_layout/object/editing/transform/control/numeric_input.html) (via search summary) | Precision without leaving the gesture. | Fits: a "type a distance" box in the tool-options strip, issuing one `Set`. Scientific users want exact metres. |
| **Undo History list**: pick a past state, a new change truncates the later ones | Blender [Undo & Redo](https://docs.blender.org/manual/en/4.5/interface/undo_redo.html) (via search summary) | Undo becomes visible and navigable, not a blind stack. | Fits: `history.rs` already stores inverse edits by bytes. Note a branch-truncating history is what Cavendish's linear undo/redo already does. |
| **A few tools, one active, tool options strip** | MagicaVoxel: Attach / Erase / Paint, Select (box), with T/R/G keys ([community manual](https://leonida.gitbooks.io/magicavoxel-user-reference-manual/content/modelling.html), secondary source); Algodoo: separate Tool and Tool-options toolbars ([Wikipedia](https://en.wikipedia.org/wiki/Algodoo), secondary) ; Blender's Tool Settings | The mode is always visible; options belong to the tool. | Fits: the tool is app state (like the draw style), the edit is a command. |
| **Hover/pick readouts**, eyedropper (Alt-click picks the colour) | MagicaVoxel ([forum, secondary](https://forum.playboundless.com/t/magic-voxel-help/25988/5)) | Inspect without editing. | Fits as a **Probe** tool: click a point, read the field, phase contribution and distance to each tower. Read-only (a `Query`). |
| **Teardown's editor** | Not verified (see below). | | |

### 2.3 Parametric CAD

| Pattern | Source | Why it works | Fit |
|---|---|---|---|
| **Feature list with a rollback bar**: the model shows the state up to the bar; right-click "Roll to here"; rename in place | Onshape [Feature list](https://cad.onshape.com/help/Content/feature_list.htm) and [tech tip](https://www.onshape.com/en/resource-center/tech-tips/tech-tip-roll-to-here-roll-to-end) | The tree *is* the history and the structure. | Partly: Cavendish's scene is a spec, not a feature history. The *rollback* idea maps to the undo-history list and to scrubbing; a full feature tree does not fit and is not recommended. In-place rename and right-click on rows do fit. |
| Fusion's timeline | Not verified. | | |

### 2.4 Scientific and educational tools

| Pattern | Source | Why it works | Fit |
|---|---|---|---|
| **Pipeline Browser**: a tree of sources and filters with an eye per row for visibility; an *Apply* button when the parameters differ from the shown data; display properties apply at once | ParaView: [Pipeline Browser](https://cvw.cac.cornell.edu/paraview/user-interface/pipeline-browser), [Users Guide, displaying data](https://docs.paraview.org/en/v6.1.0/_sources/UsersGuide/displayingData.rst.txt), [tutorial](https://docs.paraview.org/en/latest/Tutorials/ClassroomTutorials/beginningSourcesAndFilters.html) (all via search summary) | Exactly Cavendish's stale-not-blocked: the highlighted Apply says "your parameters are out of sync with what you see". | Fits and is already the model: Cavendish's `Run` shows `computing` / `refined` and keeps the old signal. Recommend a per-row stale mark and a visible "edited since last run". Do *not* copy a manual Apply button: edits already apply. |
| **Model Builder tree, Model Wizard, Study steps, Results nodes**; right-click a node for context menu; click a node, settings open in a panel | COMSOL: [Model Wizard](https://doc.comsol.com/6.4/doc/com.comsol.help.comsol/comsol_ref_environment.18.43.html), [tree branches](https://doc.comsol.com/6.4/doc/com.comsol.help.comsol/comsol_ref_modeling.19.009.html) (via search summary) | A wizard asks the three questions that start any model (space dimension, physics, study) and then drops you into the tree with sensible defaults. Results are nodes in the same tree. | Fits as the "New scene" wizard (3.7) and as an **Analysis** branch in the tree (signals, periodograms, cadence as nodes next to the scene). |
| **Direct manipulation, few controls, immediate coupling between action and picture** | PhET: [interview papers](https://phet.colorado.edu/publications/archive/PhET%20interview%20Paper%20Part%20II.htm) (Adams et al., interface design; via search summary, text not read); Algodoo, [Wikipedia](https://en.wikipedia.org/wiki/Algodoo): drag-and-drop sandbox, tool and tool-options toolbars | Users who can *push* the thing learn the model by pushing it. | Fits: the 3-D area becomes the primary editing surface; a drag already updates the tower numbers live. The stale-not-blocked engine is what makes this feasible at 10⁹ cells. |
| **Houdini node graphs** | Not surveyed. | | Not relevant: Cavendish's scene is a list of bodies, not a dataflow. Skipped. |

### 2.5 General heuristics

[Nielsen's ten heuristics](https://www.nngroup.com/articles/ten-usability-heuristics/) give the vocabulary used in the
audit: visibility of system status (undo history, toasts), recognition rather than recall (Add menu, palette,
visible shortcuts), user control and freedom (undo, Esc backs out one step, which exists), flexibility and
efficiency (shortcuts hidden from novices), error prevention (starters that are valid scenes), help and
documentation (empty-state guidance). The [VS Code command palette guidelines](https://code.visualstudio.com/api/ux-guidelines/command-palette)
are short and apply directly: descriptive names, a shared category prefix, show keyboard shortcuts, no
emojis in names.

### 2.6 egui-specific

| Need | Option | Notes |
|---|---|---|
| Docking | [`egui_dock`](https://docs.rs/crate/egui_dock/latest) (binary split tree, tabs, drag tabs, optional serde of `DockState`) or [`egui_tiles`](https://lib.rs/crates/egui_tiles) (horizontal, vertical and grid layouts; a `Behavior` trait to customise; Rerun's) | Search summaries state `egui_tiles` is "much earlier in development" and lacks tab close buttons and built-in scroll areas; neither's compatibility with the workspace's pinned `egui =0.37.0` was checked. **A one-hour spike must check this first**, plus wasm. Both can float tabs out into windows; that must be disabled or limited because it would cover the figure (R3). |
| 3-D gizmo | [`transform-gizmo-egui`](https://docs.rs/transform-gizmo-egui) (translate, rotate, scale; `gizmo.interact(ui, &[transform])` each frame; snap distance, angle and scale increments are config in the underlying `transform-gizmo`); [`egui-gizmo`](https://docs.rs/egui-gizmo) is older and unrelated | Takes view and projection matrices and returns new transforms, so it is renderer-independent. Compatibility with egui 0.37 unchecked. A gizmo is ~400 lines to write by hand for translate-only if the dependency does not fit ("dependencies only where they earn their place"). |
| Context menus | `Response::context_menu` | In egui core; known to be suppressed during drags ([CHANGELOG](https://github.com/emilk/egui/blob/master/CHANGELOG.md), via search summary). |
| Drag and drop | `Ui::dnd_drag_source`, `Ui::dnd_drop_zone` ([source](https://docs.rs/egui/0.34.3/src/egui/drag_and_drop.rs.html)) | In core. For dropping a palette item onto the 3-D figure, the drop target is the figure's response. |
| Command palette | No standard crate found; build it: a `TextEdit` plus a filtered list in a centred `Area`, over the command table | The table already exists as data (`menus.rs` `lines()`) and as the console's `help`. Fuzzy match: a 30-line subsequence scorer, no dependency. |
| egui_kittest | Not in `Cargo.lock` or any `Cargo.toml`. The existing headless tests use a home-made harness (`headless::run_frames`, the layout recorder `shell.layout()`), see section 4. | Adopting `egui_kittest` is optional and would add a dev-dependency; the existing harness already clicks, drags and presses keys. |

---

## 3. Recommended design

### 3.1 Principles for Cavendish

1. **The 3-D area is the primary editing surface; panels are the precise route.** Anything done in one shows in the other at once, because both issue the same commands.
2. **One active tool, always visible.** A tool is app state, like the draw style; a tool *action* is an engine command.
3. **Add, search, undo: all one keystroke from anywhere.** Ctrl+K opens the palette, `Delete` removes, `Ctrl+D` duplicates, `Ctrl+Z` undoes.
4. **Nothing is greyed out that works.** A feature not built yet is hidden or shown once in `Help`, not as a menu row with a slice label.
5. **The figure is never covered** (existing rule). Palettes and toasts are placed over panels or the margin, not the figure, and the palette is the one allowed transient overlay, closing on Esc.

### 3.2 Layout and workspaces

Keep the current areas (they are what the mock-up shows) but make them docked and named. Default workspace **Scene**:

```
┌ cavendish · scene.ron*  File Edit View Add Array Analysis Help      [Ctrl+K search]   t 9.10 · 60 fps ┐
├ tools ┬ Scene tree ─────────┬──────────── 3-D view ────────────────┬ Properties ───────────────────┤
│ ▢ sel │ ▾ Bodies            │ iso · perspective · shaded · signal  │ walker   (bodies[0])  ⟲ reset  │
│ ✥ mov │    ● walker   👁    │ ┌ tool options: Move · snap 0.25 m ┐ │ ▾ Placement   x y z (m)         │
│ ⟳ rot │    ● radiator      │ │         ╱╲  gizmo on selection   │ │ ▾ Mass & shape  1 kg  sphere ▾  │
│ ⤢ scl │ ▾ Array             │ │  (click floor to place)          │ │ ▾ Motion        Linear ▾        │
│ ✚ add │    tower 1 … 5      │ │                       [Home]     │ │ ▸ Advanced                      │
│ 📏 ms │ ▾ Surroundings      │ └──────────────────────────────────┘ ├ Reading (at the cursor) ───────┤
│ ◎ prb │ ▾ Analysis          │ hover: x 2.0 y 1.1 z 0 · 3.1 m T2   │ tower 1  −2.626 mrad  ↻ stale  │
├───────┴─────────────────────┴──────────────────────────────────────┴────────────────────────────────┤
│ Timeline (lanes per tower) · play · t 9.10                                          history ▴ console ▴ │
├──────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ✓ Moved walker by (2, 0, 0) m · forward stale (tower 1, 3)   [Undo]                                   │  toast, then log
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Three workspaces, switched by the top bar's tabs, the palette, and Ctrl+1/2/3 natively (not bare digits: those select
towers). Ctrl+digit is the browser's own tab switch on the web, a behaviour I know of but did not test, so on the web
use the tabs and the palette. A workspace changes which panes are visible, never the Tab order (3.10.1):

| Workspace | For | Shows |
|---|---|---|
| **Scene** | building and placing | tool bar, tree, one big 3-D view, properties, timeline |
| **Instrument** | array and timing | tower tree, cadence editor and lanes large, flight diagram, instrument preset and compare; the 3-D view small |
| **Analysis** | reading results | signal / periodogram / cadence plots as tabs in a large pane, the 3-D view small with a probe, "all towers at the cursor" table |

Viewport settings (look, projection, panes, draw, colour, overlay, section) leave the left panel and become **a
View popover in the header strip** (the badges there become buttons; the mock-up's strip is the home for them) plus
the same lines in the palette. The Cadence and Instrument blocks leave the body/tower properties and live in the
Instrument workspace and under the tree's Array node (selecting "Array" shows them).

*Implementation:* `egui_dock` or `egui_tiles` after a spike; if neither fits, three hard-coded arrangements with a
saved split fraction are enough for this slice (workspace switching, no free docking). Layout is UI state, saved
with the app's preferences, not in the scene.

### 3.3 Scene tree, Add menu and palette

**Tree.** Sections Bodies, Array, Surroundings, Analysis (one row for each signal/periodogram view; see 3.8). Each row: swatch, name, one-line detail (as now), an eye (hide in the view) and a stale mark when its contribution is being recomputed. Interactions:

| Action | Gesture | Command |
|---|---|---|
| Select | click; Shift-click range; Ctrl-click toggle | `Select` (extended to a set, 3.4) |
| Rename | F2 or double-click | `Set bodies[i].label` |
| Duplicate | Ctrl+D or context menu | `Insert bodies <copy of value>` with a small offset so it is visible |
| Delete | Delete or context menu | `Remove bodies[i]` |
| Frame | F or double-click in the tree | `View` command (camera only, not an edit) |
| Add child-of-section | the `+` on the section header | opens the Add palette filtered to that section |
| Reorder | drag a row | `Remove` then `Insert` at index as one group (3.4) |

The context menu on a row is the Scene/Array menu for that row, as M13 §9.4 specified. It is built from the same
list as the menu bar: one data table of actions, each with its console line, shortcut and enabling rule.

**Add menu and palette.** A top-level **Add** menu (Blender's and Unity's placement; replaces `Scene > Add source` and `Array > Add tower`):

```
Add ▸ Body ▸  Point mass · Sphere · Cube · Box · Cylinder · Platonic solid ▸ · Voxel body (from file…) · Walker · Oscillator · Pendulum
      Tower
      Room · Floor · Air…   (only those not already present)
      Template ▸ …          (section 3.7)
```

Each entry is a **starter**: a valid `BodySpec` that the spec validator accepts, with a unit mass and a position
defaulting to "in front of the camera on the floor plane, or at the camera target". `editors.rs` already has a
`starter(spec, path, variant)` function and `paths::template(path)` for optional parts and motions; shapes need the
same treatment (I did not check whether `cav_sim::shape` has `starter`-like constructors; the S3 dev scenes in
`scenes/dev/*.ron` give ready examples). In the palette the same list is searchable ("sph" → Sphere). The Add tool
(3.5) makes a click or a drag-in place it.

**Command palette (Ctrl+K).** One box, three kinds of result, grouped with a category prefix as the VS Code
guidelines advise: *Commands* (every console command and menu item, with its shortcut on the right), *Add* (the
starters), *Fields* (every spec path, `bodies[0].mass`, selecting it focuses its editor in the inspector).
Results show the one-line help the console already has. Enter runs; the console line it ran is echoed, so the
palette teaches the console. Recently used first. Escape closes. This is also the cheapest answer to "where is
everything", because it needs no layout change. Both the menu bar, the context menus and the palette read the
same action table; a test checks they agree (section 4).

### 3.4 Two routes, one command path

The owner wants a body editable either in the viewport or in the panel, in sync. Both already end at `Engine::apply`:

```
 viewport route                                         panel route
 ──────────────                                         ───────────
 gizmo drag / click-place / palette drop / delete key   inspector field / add / remove / reorder button
        │                                                        │
        ▼                                                        ▼
  Command::Set{path, value, continues}  ◄───── identical ─────►  Command::Set / Insert / Remove
        │   (Insert{path:"bodies", value} for place; Remove{path} for delete)
        ▼
   Engine::prepare (validate: refusal changes nothing but the log) → perform → history (inverse bytes) → journal → forward (stale, not blocked)
        │
        ▼
   next frame: gizmo and inspector both redraw from engine.spec()  — neither holds scene state
```

Properties of this arrangement that must be kept and tested:

- **A gizmo drag is a run of `Set` commands with `continues: true`**, so it is one journal entry and one undo, the same as a dragged number today (`editors.rs`, `a_dragged_editor_is_one_edit` in `crates/cav-app/tests/shell.rs`). A gizmo is a front end for `bodies[i].motion.<anchor>`; which fields a Move, Rotate or Scale writes depends on `BodySpec` (a body has `motion` variants and `radius`; rotation and non-uniform scale may not exist as fields; **I did not check**: if they do not, Rotate and Scale should not be offered for that body rather than faked).
- **Tools are app commands, not engine commands**, in the way `gui`, `console`, `panes` are (`Shell::is_app_command`). `tool move`, `tool place sphere` are console lines so the palette and keys are the same path, but they are not journalled (they change no scene).
- **Placing** is: ray from the pointer through the camera, intersected with the floor plane or with a body's surface (the renderer and `Shell::body_under` already project and pick; a surface hit needs a ray-vs-shape test, which for the voxel world can use the tree), then `Command::Insert` of the starter with that position. A snap setting rounds it.
- **Two engine additions are needed**, and I found neither in `command.rs`: (a) a *set* selection (`Select(Option<Target>)` is single), required by box-select, Shift-click, and multi-drag; (b) a *grouped* edit (several `Set`/`Insert`/`Remove` as one undo step and one journal line), required by multi-move, duplicate-with-children, reorder and templates. The history stores "the inverse edit of each edit, by bytes"; a group is a list of those. Both are small and belong in the engine, not the app, so MCP and `cav exec` get them too.
- **Hover and probe are `Query`s**, not commands: `Query::Reading(Option<t>)` exists for a tower reading at a time; a point probe needs a new query ("field/phase contribution at point p"), which is read-only and cannot break replay.

### 3.5 Toolbar, tools and the tool-options strip

A vertical tool strip at the left of the 3-D view (Blender, MagicaVoxel and Algodoo all put tools at an edge), one active, with shortcuts; the tool's options appear in a thin strip at the top of the view (Algodoo's "tool options toolbar", Blender's Tool Settings):

| Tool | Key | What it does | Options strip | Slice |
|---|---|---|---|---|
| Select | Q | click selects, Shift/Ctrl extend, drag a box selects several, hover highlights | filter: bodies / towers / all | A (click), later (box) |
| Move | W | gizmo on the selection; drag axis, plane, or free in view plane | snap on/off and step (`0.25 m`), local/global, **type a distance** | A |
| Rotate | E | ring gizmo (only for bodies that have an orientation) | snap angle, type an angle | later |
| Scale | R | uniform or per-axis, only for bodies with a size field | snap, type a factor | later |
| Place | A | the body chosen in Add follows the pointer, click to drop, Esc to cancel; repeats | which starter; drop on floor / on surface / at grid | A |
| Paint (voxels) | B | Attach / Erase / Paint on a voxel body (MagicaVoxel's three brushes), brush size | brush, material | later; needs a voxel edit command that does not exist |
| Measure | M | click two points, read distance and the range to each tower | | later |
| Slice | C | drag a plane handle in the view for the section (replaces the three slider rows) | axis, fade | later |
| Probe | I | click a point: read the field, the phase contribution of the cell there, distance to each tower; follows the hover | field (mass, density, …) | later |

Letters are scope shortcuts under the keyboard model (3.10): they work in the Toolbar and Viewport scopes. Space
(play/pause), `O` (projection), `P` (plan view, M13 §9.8), `Home` and `1–9` (select tower) keep their meanings, and
X/Y/Z are reserved for axis locks in a transform session, which is why the tools are Q W E R A B M C I (3.10.4 checks
every clash). Q/W/E/R follow Unity and Godot's gizmo-mode keys (Unity: W/E/R, per the
[manual](https://docs.unity.com/en-us/engine/6000.3/manual/working-with-scenes/scenes-manage-gameobjects/positioning-game-objects)).
Blender's "press, move, type a number" modal model is adopted as the *transform session* of 3.10.3, not as tool
letters (R cannot mean both scale and rotate).

### 3.6 Inspector driven by the schema

**What exists** (read in `crates/cav-app/src/editors.rs`, `crates/cav-sim/src/paths.rs`):

- `paths::Entry` carries `kind` (Number, Integer, Text, Flag, Vector(n), List, Group, Choice(variants)), the current `value`, a `unit` (from the schema's `x-unit`; a test `fields_without_a_unit` holds every numeric field to having one), `minimum`/`maximum`, and a doc string; `x-stage` says which stage an edit makes stale.
- `Widget` maps each kind to a widget (drag/type number with unit, integer, vector of n boxes, text, tick box, drop-down, variant drop-down that replaces its data, and an `Absent` "add" button for optional parts); `shown()` formats numbers; `speed()` scales drag; the range comes from the schema.
- Each change is `Command::Set` through `command_for`; a drag is one edit; the test `every_schema_field_has_a_widget` guards coverage; a list is capped at `MAX_FIELDS = 200`.
- `structure()` gives `remove this body / tower / room / floor`; `starter()` gives valid initial values for optional parts, motions and schedule parts.

**What is missing for the owner's request:**

| Missing | Why it matters | Where it lands |
|---|---|---|
| **Generic list controls**: add, remove, reorder (and duplicate) for any `Kind::List`, not just bodies and towers through special cases | "lists with add/remove/reorder" | `structure()` generalised on `Kind::List`; reorder = grouped `Remove`+`Insert` (3.4 b). `Insert`/`Remove` and `paths::insert/remove` already exist. |
| **Groups as collapsible sections** with remembered open state; a sensible order and a short "basic / advanced" split (schema attribute, e.g. `x-advanced`) | A body's fields and the array's fields are one long list today | Schema attribute plus `CollapsingHeader` per `Kind::Group`. |
| **Per-field reset to default** and a "differs from default / edited since last run" marker | M13-R11 and §9.4 promised both | Default from `paths::template` or the loaded preset; the marker compares to the last refined signal's spec hash (the engine already keys contributions by hash). |
| **Unit-aware entry and display**: show `mrad` for phases, `km` for the planet scale, accept `2 m`, `3 km`, `45°`; keep the stored unit SI | Typing a unit is how scientists check their own numbers; Blender's `=` expression mode is the precedent ([numeric input](https://docs.blender.org/manual/en/5.0/scene_layout/object/editing/transform/control/numeric_input.html), via search summary) | A small parser in `cav-app` (or `cav-sim`, for the console to share it); validated against the schema's `x-unit`. |
| **Enums with descriptions** (shape kinds, motion kinds), shown in the drop-down | The shape list *is* the documentation | Doc strings per variant in the schema, if the derive carries them. |
| **Search box** that filters the fields of the selection, and `Ctrl+K` jumping to a field | Field counts will grow (S8 onward) | Filter by path and doc text. |
| **Multi-selection editing**: fields with equal values show the value, differing ones show "mixed"; an edit is a grouped `Set` | Follows from 3.4 (a) and (b) | Later. |
| **Vector widgets with a link toggle and axis colours** (x red, y green, z blue, matching the gizmo) | Makes position fields and the gizmo read as the same thing | Small. |
| **Derived read-outs next to the fields that cause them** (mass from density × volume for a shape; tower reading) | Already partly there (`now at`, Reading) | Keep, move into the relevant section. |

So the inspector is roughly 60% of the way there; what is missing is structure (sections, lists, reset) and units,
not a new generator. Keep the rule that no panel holds scene state: every field reads `engine.spec()` each frame.

### 3.7 Empty state, templates, saving

- **Empty scene** (`void`, or File > New): the 3-D view shows a faint grid and, centred on the figure's margin (not over the tower tags), three buttons: *Add a mass*, *Add a shape ▾*, *Start from a template ▾*. The tree sections show a ghost row "Add a body (A)". This replaces the grey line pointing at the console.
- **New scene wizard** (COMSOL's Model Wizard, [source](https://doc.comsol.com/6.4/doc/com.comsol.help.comsol/comsol_ref_environment.18.43.html)): one dialog, four choices: the array (AION-10 five towers / one tower / none / preset), surroundings (void / floor / room), a starting body (none / template), duration. Result: a valid scene opened with the first body selected and the Move tool active.
- **Templates** (each is a short list of `Insert`s as one grouped command, so one undo removes the lot):
  *A mass passing the array* (a walker or `Linear` mass, with the playhead set to its closest approach, S1's `pass` scene); *An oscillator* (S1's `comb`/`oscillator`, with the periodogram pane opened); *A voxel body* (a sphere shape with colour-by-level and the section tool offered). These exist already as `scenes/dev/*.ron`; File > New from template lists them with their one-line description (already in each scene's `description`).
- **Save/Open**: enable `Save scene` (Ctrl+S, `save <file>`) and `Open scene…` natively, with a title-bar `*` for unsaved edits (the engine knows the history length since last save: to confirm). On the web, Save = download and Open = a file picker or drop onto the window ([seams table](../ARCHITECTURE.md): reads by path are refused).

### 3.8 Feedback: history, toasts, status

- **Edit history** window or bottom tab: the journal's edits as rows (`Moved walker by (2,0,0) m`, `Added sphere`, with the stage each made stale), a marker at the current state, click a row to undo or redo to it (Blender, [Undo & Redo](https://docs.blender.org/manual/en/4.5/interface/undo_redo.html)). This also serves as "what did I just do".
- **Toasts** for every command's outcome: `Added sphere` with an Undo link; a refusal as `Cannot move there: inside the room wall` in the warning colour. The engine's log already has each entry (`engine.log`, errors open the console); a toast is a short-lived view of the same log entry, placed over the footer, never over the figure.
- **Status** strip keeps the `forward stale (tower 3)` language (M13 §9.5); each stale tower's row in the tree gets the stale mark (ParaView's Apply-highlight idea without the button).
- **Analysis as a workspace** (the owner asked for it coherent): signals, periodogram, cadence and flight become tabs of one large pane, not four toggle buttons applied to the "active pane". Pane choice stays available. The tree's Analysis section lists them with the tower filter; the cadence plot sits beside the Cadence editor in the Instrument workspace; a probe's reading tables feed the Analysis workspace's "at the cursor" table.

### 3.9 Ranking and slice scope

Value = how many of the owner's complaints it removes; effort in builder-days (rough, my estimate, not measured).

| # | Recommendation | Complaint it answers | Effort | Slice |
|---|---|---|---|---|
| 1 | **Keyboard model in full (required)**: scope tree as data, two modes, rings, breadcrumb, key router, `?` sheet, state-machine tests (3.10) | owner's ruling; discoverability and speed | 6 d (router and tree are new; widgets register into them) | **A, first** |
| 2 | Command palette, Ctrl+K: commands, Add, fields; shortcuts shown; menu bar, context menus, `?` sheet and palette from one action table | discovering commands, where things are | 2 d | **A** |
| 3 | Add menu + searchable body/shape palette with starters; un-grey (or hide) menu items that work (Save, Open, Add tower, Edit history) | adding things; stale labels | 3 d | **A** |
| 4 | Tree actions: context menu, Delete, Ctrl+D, F2, F; View controls out of the tree into a header popover; Cadence/Instrument out of body properties | adding/removing, layout, outliner/properties relation | 3 d | **A** |
| 5 | Tool strip (Select, Move, Place first) with options strip; translate gizmo with snapping and typed distance; click-to-place; drop from the palette; the transform session from the keyboard | tools, placing and moving | 6 d (dependency spike +1 d) | **A** (first half); rotate, scale, box-select, surface placement later |
| 6 | Inspector: sections, list add/remove/reorder, reset, units, field search | the editor | 4 d | **A** (sections, lists, reset); units and multi-edit later |
| 7 | Undo history list and toasts | undo visibility, console-only feedback | 2 d | **A** |
| 8 | Empty-state guidance, New scene wizard, three templates, Save/Open | setting up from scratch, saving | 3 d | **A** |
| 9 | Docked workspaces (Scene, Instrument, Analysis), saved layouts, Analysis as tabs | layout, cadence/plots coherence | 5 d (spike +1 d) | **B** |
| 10 | Probe, Measure, Slice handle in view, Paint (voxel brushes), box-select, multi-select edits | octree/section tools, analysis | 6 d+ | **B / C** |

**Proposed Slice A ("the editor")**, about 5 working weeks of a builder's time at the estimates above, ordered so each
step ends with a working app: (0) the keyboard state machine and the scope tree as data, with the *existing* panels
registered into it and the existing keys moved onto it (no new features yet; the old `keys_issue_engine_commands` test
passes through the new router); (1) the action table, `?` sheet and palette, generated from the scope data;
(2) Add menu and starters, tree context menu and keys; (3) tool strip, translate gizmo, place tool and the transform
session (spike: `transform-gizmo-egui` against egui 0.37; else a hand-written translate gizmo); (4) engine: `Select`
set and grouped edit; (5) inspector sections, lists, reset; (6) history list, toasts, empty state, wizard, templates,
Save/Open. Slice B: workspaces and docking, probe, measure, slice handle, paint, units, multi-edit. Slice A touches
`cav-engine` only for (4); everything else is in `cav-app` and `menus.rs`, so the engine's oracle and bundle tests are
not at risk. The keyboard is step 0 because every later widget must register its id under a scope as it is written;
retrofitting registration is the expensive way.

*Calls worth the owner's second look:* the palette key and F3 (3.10.6); Q/W/E/R tool letters (Unity) vs Blender's
G/R/S; the palette's arrow-key exception (3.10.6); Esc inside a transform session (3.10.3); the number-field typing
rule (3.10.5); whether `Add tower` should be allowed and what the real instrument's layout rules are (a question for
George under "For George": array layout).

### 3.10 The keyboard model applied

The owner's spec, [docs/design/keyboard.md](../design/keyboard.md) (Principia's model, "adopted in full";
I read it from the branch `design-keyboard`, commit `77182bb`, where it had not yet reached `main`), is taken
as given: a tree of scopes walked in two modes (navigation, orange ring; interaction, blue ring); keyboard navigation
on or off separately from focus; Tab between big scopes, Enter in, Esc out, arrows move or adjust; Shift ×10 and
Alt ×0.1 everywhere; per-scope letter shortcuts; DAS/ARR repeat (500 ms, 40 ms); text fields eat keys except
Esc/Enter/Tab; clicks move focus only by rule; windows are lists of sections whose order never depends on state; a
breadcrumb in the top bar; `?` sheet; keys routed in a fixed priority (text field, popup, global, scope shortcuts
inside out, mode handling); tested as a state machine with control arms. This section only fixes the parts that are
Cavendish's: the scopes, their order, their letters, and how tools, gizmos and the palette sit on the model. Where
the spec and this note's earlier sections disagree, the spec wins (as its status line says it does over the
mock-up): that is why Tab no longer "cycles sources" (M13 §9.8) and why bare `Shift+←/→` no longer steps shots.

#### 3.10.1 Big scopes and Tab order

Fixed list, in visual order left to right and top to bottom, **independent of workspace and of state** (a hidden pane
is a disabled stop, not a missing one, as spec §9 requires). Tab from 8 wraps to 1.

| # | Big scope | Contains (sub-scopes, reached with Enter) |
|---|---|---|
| 1 | **Top bar** | menus (File, Edit, View, Add, Array, Analysis, Help), workspace tabs, the header strip's view badges (each a button), the palette button |
| 2 | **Toolbar** | the nine tools, in order: Select, Move, Rotate, Scale, Place, Paint, Measure, Slice, Probe; then Snap toggle |
| 3 | **Scene tree** | sections Bodies, Array, Surroundings, Analysis (sub-scopes); rows inside |
| 4 | **Viewport** | *Canvas* (camera: a leaf), *Selection* (the gizmo and the tool's handles on the selection: a leaf), *Tool options* strip (fields) |
| 5 | **Inspector** | sections (Placement, Mass and shape, Motion, Advanced …) generated from the schema; each section's fields; reset is each section's last element |
| 6 | **Timeline** | scrubber, then one lane per tower |
| 7 | **Analysis** | tab strip (Signal, Periodogram, Cadence, Flight), then the plot canvas, then the "at the cursor" table |
| 8 | **Footer** | status line, undo-history button, console button, notification area |

Windows and overlays are not scopes (spec §9): **Console** (sections: header bar, log), **Edit history** (list, then
Undo/Redo buttons), **Keyboard sheet** (`?`), **Palette** (3.10.6), **New-scene wizard** (sections: array,
surroundings, starting body, duration, buttons; Esc cancels), **Preferences**. A window that opens by itself (the
console on an error, a toast) takes no focus. Tab inside a window moves between its sections; Esc at its top level
closes it and focus returns to whatever opened it.

The breadcrumb in the top bar reads, for example, `Viewport › Selection › Move`, `Inspector › Placement › x`,
`Scene tree › Bodies › walker`, `Console › Log`.

#### 3.10.2 What each scope's letters and arrows do

Letters and Space are active anywhere inside the scope, in either mode, except while typing in a text field. Arrows
adjust only in interaction mode (Enter first). "Clash check" is in 3.10.4.

| Scope | Navigation-mode arrows | Interaction mode (blue) | Letter and key shortcuts |
|---|---|---|---|
| 1 Top bar | ← → along the bar; ↓ opens a menu (a popup: ↑↓ choose, Enter, Esc closes unchanged) | n/a (buttons need none) | none beyond the globals |
| 2 Toolbar | ↑ ↓ among tools; Enter activates (a click) | n/a | the tool letters: **Q** Select · **W** Move · **E** Rotate · **R** Scale · **A** Place (Add) · **B** Paint (Brush) · **M** Measure · **C** Slice (Cut) · **I** Probe (Info); **S** toggles snap |
| 3 Scene tree | ↑ ↓ rows, ← collapse or go to parent, → expand or first child; Shift+↑↓ extends the selection; Enter selects the row (a click) and stays in navigation mode; Space toggles it in the selection (Ctrl-click) | Enter on a row starts a *drag session*: ↑ ↓ reorder it (Shift ×10), Enter drops (one grouped `Remove`+`Insert`), Esc returns | **Delete** remove · **Ctrl+D** duplicate · **F2** rename (a text field) · **F** frame in the viewport · **H** hide or show · **A** Add (opens the palette filtered to the row's section) |
| 4 Viewport, Canvas | ← → ↑ ↓ move between Canvas, Selection and Tool options | arrows pan, `+` `−` zoom (the spec's canvas rule); Shift/Alt scale the base step | the tool letters of the Toolbar (same meanings); **F** frame selection · **O** projection · **P** plan view (existing, M13 §9.8) · **1–9** select tower n (existing) · **Delete**, **Ctrl+D** as in the tree · Space play/pause (existing) · **Esc** in navigation mode clears the selection |
| 4 Viewport, Selection | as above | the **transform session** (3.10.3) | **X Y Z** axis lock inside a session; the tool letters otherwise |
| 4 Viewport, Tool options | ← → across fields | a number field adjusts (↑↓ or ←→ ± step); a typed value replaces it | none |
| 5 Inspector | ↑ ↓ fields within a section, ← → across a vector's components, Tab goes to the next *scope* (not section: spec §3, Tab between big scopes outside windows); sections are sub-scopes entered with Enter; Enter on a section header folds it | Enter on a number: arrows ± step (Shift ×10, Alt ×0.1, DAS/ARR); a drop-down opens with Enter, ↑↓ choose, Enter picks, Esc closes unchanged; a text field: Enter starts typing | **Backspace** reset the field to its default (not R: R is Scale in the adjacent Viewport) · **Ctrl+C / Ctrl+V** copy and paste a value, or a whole body's record (the spec's context copy) · **/** or **Ctrl+F** focus the field search · on a list: **+** add an item, **Delete** remove it, **Ctrl+↑ / Ctrl+↓** reorder |
| 6 Timeline | ↑ ↓ among the scrubber and tower lanes | scrubber: ← → step 0.1 s (Shift 1 s, Alt 0.01 s), held repeats; a lane: ← → move that tower's clock offset (the same edit as Shift-dragging the lane), each step a `Set` with `continues` so one undo | **Space** play/pause · **[** and **]** previous and next shot of the selected tower (replaces Shift+←/→ of M13 §9.8, which would clash with Shift ×10) · **X** mask or unmask the shot under the playhead |
| 7 Analysis | ← → along the tab strip; ↓ into the canvas, then the table | canvas: arrows pan, `+` `−` zoom the time or frequency axis; table: ↑↓ rows | **L** log or linear · **F** fit the view · **K** lock the cursor · **1–9** show or hide tower n's trace · Space play/pause |
| 8 Footer | ← → among its buttons; Enter on a button clicks | n/a | **Ctrl+H** edit history; the backtick key (existing global) opens the console |

Globals, working in either mode wherever focus is (they yield to text fields): **Home** reset the active view (one
undoable command, as spec §3 and the present app), **Ctrl+Z / Ctrl+Shift+Z** undo and redo, **?** the sheet,
**Ctrl+K** the palette, **F3** hide the GUI (existing; the spec's Esc-ladder keeps it), **`** console,
**Ctrl+S / Ctrl+O** save and open, **Ctrl+1/2/3** workspaces (native only). Modifier hygiene as the spec says: Ctrl+Home
is not Home; the reset keys do not auto-repeat.

Migration of the existing keys (all are in `Shell::keys`, `crates/cav-app/src/shell.rs`): Space, O, `1–9` and arrows move
from "global" to the scopes above, keeping their meaning where the focus is the Viewport or Timeline; the playhead
arrows stop being global (a user in the tree pressing ← now collapses a row, as in every tree). That is a behaviour
change the owner should expect. Esc keeps its ladder, now defined by the spec: close a window, then up a scope, and
at a big scope in navigation mode in the Viewport clear the selection (the one place the spec is silent; flagged).

#### 3.10.3 Viewport tools and gizmos from the keyboard: the transform session

The mouse route is: pick a tool, drag a handle, optionally type a distance in the options strip. The keyboard route
is the same model, not a second one:

1. **Pick the tool**: its letter (in Toolbar or Viewport), or Toolbar arrows and Enter. The tool is UI state (spec §2:
   never undoable, never seen by the engine).
2. **Land on the selection's handles**: Tab to Viewport, move to the *Selection* leaf (→ from Canvas); the gizmo shows
   an **orange** ring (navigation). Enter starts the **session**: ring turns **blue**.
3. **In the session** (Move shown; Rotate and Scale are analogous):
   - **← →** move along world x, **↑ ↓** along y, **PageUp / PageDown** along z, each by the snap step
     (default 0.25 m; **Shift ×10, Alt ×0.1**; held = DAS/ARR). Each step is a `Command::Set` with
     `continues: true`, so the whole session, however many key repeats, is **one undo step and one journal entry**,
     the same as a mouse drag (3.4).
   - **X / Y / Z** lock an axis (Blender's constraint). With an axis locked, ↑ ↓ or ← → adjust only it.
   - **Digits, `-`, `.`, `=`** begin typing a distance into the tool-options field (Blender's numeric input, where
     typing digits after G enters the amount, Tab goes to the next axis, `=` enables units
     ([manual](https://docs.blender.org/manual/en/5.0/scene_layout/object/editing/transform/control/numeric_input.html),
     via search summary)). While typing, the text-field rule applies: every key goes to the field except Esc, Enter
     and Tab; **Tab moves to the next axis field** here (a documented use of the spec's "Tab leaves the field").
     The typed value is an offset in the field's unit (`2` = 2 m; `2 km` accepted once units land, 3.6); each
     keystroke re-issues the `Set` (`continues`), so the view follows the number.
   - **Enter** ends typing, then ends the session (navigation mode again on the Selection leaf); **Esc** does the same
     with the edit **kept**, because the spec fixes Esc as "back to navigation" and a text field keeps its text. This
     differs from Blender, where Esc cancels the transform; here **Ctrl+Z** undoes the whole session in one step. If the
     owner prefers Blender's cancel, it is a local exception to record in the spec.
4. **Place** (letter **A**): the chosen starter follows the pointer; **Enter** or a click drops it at the pointer's
   floor intersection (or, with the pointer outside the view, at the camera target); with no pointer (pure keyboard)
   it lands at the target and the session opens on it at once so ← → ↑ ↓ and typing position it. **Esc** returns to
   navigation (the body stays, one undo removes it).
5. **Measure, Probe, Slice** need the pointer for the first point; from the keyboard, ← → ↑ ↓ move a cursor over the
   figure by a step (Shift ×10), **Enter** takes the point, **Esc** leaves. Probe's reading table is a leaf in Analysis,
   reachable by Tab, so a keyboard user can read what a click would show.
6. **Mouse interplay (spec §8):** a click or drag in the viewport (including grabbing a gizmo) hides the ring and
   turns keyboard navigation off; a click that selects or drags in the canvas does *not* move the keyboard focus;
   the next key shows the ring on the scope that already had focus. Hover readouts are neither click nor drag and
   leave the ring as it is.

The ring and breadcrumb are painted by the app after the figure, over the figure's *edge* for scope highlights (the
figure is never covered: the ring is a 2 px outline, a whole-scope highlight is a border, never a fill).

#### 3.10.4 Clash check

The spec says to avoid one letter with two meanings in adjacent scopes, and, if a new action clashes with an
existing binding, to keep the binding and move the action (as Principia moved "keep" from K to Space). The check, by
letter, over scopes that can be adjacent in Tab order or nest:

| Key | Meanings | Verdict |
|---|---|---|
| Space | play/pause in Timeline, Analysis, Viewport (existing); toggles a tree row in the Tree | different scopes, different nouns (time vs selection); Tree is not adjacent to Timeline. Kept; the Tree Space is the one to watch in user testing. |
| Q W E R A B M C I | tools, in Toolbar and Viewport only | same meaning in both; not shared with any other scope |
| A | tool Place (Toolbar, Viewport); Add in the Tree | the same verb (add); the Tree is adjacent to Toolbar: acceptable because both open "add something" |
| R | Scale (Viewport); **not** Reset (moved to Backspace in Inspector); not Timeline | resolved by moving reset |
| S | snap toggle (Toolbar) | M13's "S cycle slice" is dropped (the Slice tool is C) |
| F | frame (Tree, Viewport); fit (Analysis) | same meaning (bring it into view) |
| P | plan view (Viewport, existing) | the Probe is I, not P |
| O | projection (Viewport, existing) | free elsewhere |
| X Y Z | axis lock, only inside a transform session; X mask a shot in Timeline | the session shadows the Viewport scope's own keys (router priority 4, innermost first); Timeline is not adjacent to the Selection leaf |
| 1–9 | select tower (Viewport, Tree); show/hide trace (Analysis) | noun is "tower n" in both |
| H | hide (Tree, Viewport); Ctrl+H history (Footer) | Ctrl is a different key (spec modifier hygiene) |
| L K | Analysis only | free |
| Delete / Ctrl+D | Tree and Viewport; list item in Inspector | acts on the focused thing; guarded by the text-field rule |
| Backspace | reset (Inspector) | a text field keeps Backspace |
| F3 | hide GUI (existing) | kept; see 3.10.6 |
| `?` `Home` `Ctrl+Z` | global (spec) | unchanged |

The router's data makes this check a *test*: the action table lists each shortcut with its owning scope; the test
fails if two different actions share a letter in one scope chain, or if a shortcut is missing from the `?` sheet.

#### 3.10.5 The inspector and the keyboard

The schema-generated inspector (3.6) gives each section its sub-scope and each field its id *from the schema path*
(`bodies[0].motion.centre[2]`), so registration is automatic: the generator that already makes a widget for every
field (`every_schema_field_has_a_widget`) also registers it, and a test can assert that every field is reachable by
Tab and arrows. Order is the schema's order; a field that is disabled or absent stays as a stop (spec §9: "order
never depends on state"); an optional part that is absent is an **Add** button in its place (the existing `Absent`
widget), so the stop is there before and after. Number fields: Enter then arrows ± step, where the step comes from
the schema (`speed()` in `editors.rs` is a hundredth of the value; the spec wants *one table* of steps, so move it
to a per-unit step in the schema, not per widget). Typing a number into a focused number field in interaction mode
starts at a digit, `-`, `.` or `=` (as in 3.10.3) rather than Enter, because the spec's Enter in interaction mode
means "back"; this is an addition to the spec's slider rule and is flagged for the owner.

#### 3.10.6 Composition with the command palette

The palette is a **window with two sections** (query field, results list), opened by the global **Ctrl+K**, and
it composes with the model as follows:

- **Opening** from the keyboard turns navigation on and lands in the query field *already typing* (an exception to
  "a text field reached by keyboard lands in navigation mode", justified because a palette that needs Enter before
  typing is not a palette; flagged). Opening it with the mouse (the top bar button) gives it focus with no ring.
- **Results are generated from the scope data**, the same table that draws the `?` sheet: each action's name, its
  console line, its shortcut and its owning scope. Results in the *current scope* are listed first under "Here", so
  the palette is also "what can I press right now". Fields (spec paths) are a third group: choosing one moves focus
  to that field in the Inspector in navigation mode and the breadcrumb shows it.
- **Keys inside**: typing filters; **↑ ↓ choose and Enter runs** are a documented exception to "a text field eats
  every key but Esc/Enter/Tab", the same exception an open dropdown makes (router priority 2). The strict
  alternative is type, Tab to the list, ↑↓, Enter; offer it as a preference if the owner dislikes the exception.
  **Esc** closes it unchanged and returns focus to where it was.
- **Running** an action issues the same console line the menu would, journalled and undoable as any command; the
  line is echoed in the toast, so the palette teaches the console. Tool and scope commands (`tool move`,
  `focus inspector`) are app commands, never journalled.
- **F3 versus Ctrl+K**: F3 stays "hide the GUI" (the mock-up's and the present rule); Blender users' F3-to-search
  habit ([Menu Search](https://docs.blender.org/manual/en/3.6/interface/controls/templates/operator_search.html),
  via search summary) is served by Ctrl+K, and `Ctrl+Shift+P` (Godot, VS Code) can be added as an alias at no cost.
  Whether to swap is the owner's call.
- A global key still yields to a text field, so Ctrl+K does not open from inside a typing field; Esc first.

#### 3.10.7 Building it

Follows spec §11 and adds the Cavendish specifics:

- A new module, `cav-app/src/scopes.rs`: the scope tree as data (`Scope { id, name, children, elements }`), the
  action table (`Action { id, label, line, shortcut, scope, enabled }`), the `Keyboard { nav_on, mode, focus_path }`
  struct and one pure function `route(&mut Keyboard, &Tree, Event) -> Vec<Effect>`. `menus.rs` becomes a view of the
  action table (its `Item::Later` rows are deleted as features land); the existing `Shell::keys` is replaced by a
  call to `route`. The struct lives in `Shell` but is never in `Engine`, the journal or the history.
- Widgets register `(scope, id, rect)` each frame into the existing `Layout` recorder; navigation walks the tree,
  not the geometry.
- Held keys use the frame clock `run_frames` already takes, so DAS/ARR (500 ms, 40 ms) is testable without a real
  clock; a held nudge is one `continues` chain (one undo).
- The `?` sheet, the palette, the tool strip tooltips and the menu shortcut hints all read the action table.
- The web: the browser reserves some keys (Ctrl+K, Ctrl+S, Ctrl+D, Ctrl+1–3, F3, backtick behaviour varies by
  layout), so the page must `preventDefault` those and the sheet must show the web alternative; I did not test
  which can be intercepted. This is an item for the slice's web check (`cargo xtask web`).

---

## 4. Validation: how to test UX claims here

**What already exists** (read, not run): `crates/cav-app/tests/shell.rs` drives the real `Shell` through
`headless::run_frames` with synthetic pointer and key events and reads a per-frame layout record
(`shell.layout()`: panel rectangles, editor rects by path, button rects by console line, outline rows). Tests
include `every_panel_is_laid_out_and_none_covers_the_figure`, `every_menu_item_is_a_console_command`,
`keys_issue_engine_commands`, `buttons_and_the_pointer_issue_engine_commands`, `dragging_a_body_moves_it_under_the_pointer`,
`every_schema_field_has_a_widget`, `a_dragged_editor_is_one_edit`, and `all_entry_points_issue_the_same_edit`.
The harness is home-made: I found no `egui_kittest` in the workspace, so the "headless UI tests via egui_kittest"
in the brief does not match the repository. The existing harness does what is needed; switching would be a
separate decision.

**What to assert for the new UX** (each with a control arm that fails when the property is broken, per CLAUDE.md):

| Claim | Assertion | Control arm |
|---|---|---|
| Every action is reachable three ways | for each entry of the action table, the menu, the palette and (if it has one) the shortcut produce the same `Command` list | remove one entry's palette row: the test must fail |
| Nothing works that is greyed out | no `Item::Later` whose console line parses and is handled | add a `later("Save scene")` back: fails |
| The palette covers the console | every command in `console.rs`'s help table has a palette entry or is listed as deliberately excluded | add a console command without one: fails |
| Viewport and panel are one path | drag the Move gizmo by 2 m; the journal and spec equal those of typing `set bodies[0].motion… 2` into the inspector field (extend `all_entry_points_issue_the_same_edit`) | make the gizmo write a different path: fails |
| A gizmo drag is one undo | one `Undo` restores the start; the journal has one entry | split the drag into separate edits: fails |
| Placing by click inserts a valid body at the clicked point | click at pixel p; `body.position` equals the floor intersection of p's ray within 1 mm; the spec validates | shift the ray origin: fails |
| Delete/duplicate keys act on the selection and only when no text field has focus | press Delete with and without a focused `TextEdit` | ignore the focus check: fails (a typed Delete would remove a body) |
| The figure is never covered | existing no-overlap test extended to the palette, toasts, popovers, workspaces | existing |
| Tools do not leak | switching tool changes no scene hash | make a tool journal: fails |
| The inspector shows a valid unit-bearing editor for every field of every new starter | extend `every_schema_field_has_a_widget` over each Add entry | add a starter with a unitless number field: fails |
| Keyboard navigation starts off and any key shows the ring *after* acting | fresh `Keyboard`: no ring; press a letter shortcut: it acts, then `nav_on` is true | a build that shows the ring first: fails |
| Enter with navigation off only shows the ring and presses nothing (spec §2) | focus a button, Enter: engine unchanged, ring on | make Enter press it: fails |
| A click hides the ring and moves focus only by rule (§2, §8) | click the canvas with a pick: `nav_on` false, `focus_path` unchanged | let a canvas click move focus: fails |
| Tab walks the eight big scopes in the fixed order and wraps, whatever is open or selected | Tab ×8 from every workspace, with and without a selection and a hidden pane: same sequence | skip a hidden pane: fails |
| Windows do not change the big-scope order; Tab inside a window stays in it; Esc closes and returns focus | open console, palette, wizard in turn | |
| Text fields eat keys except Esc/Enter/Tab; `?` and global keys type | type `?`, `Home`, `q` in a field: text only, no tool change | route `q` to the tool: fails |
| Router priority (§11): field, popup, global, scope shortcuts inside out, mode | one key sequence per adjacent pair, asserted by the effect | swap two priorities: fails |
| A held key repeats at 500 ms then every 40 ms, and the whole run is one undo | synthetic clock through `run_frames`; hold ← in a transform session for 2 s; count steps; one `Undo` restores | undo per repeat: fails |
| Shift ×10 and Alt ×0.1 apply to every adjust widget from one step table | each widget type: step, Shift step, Alt step ratios | hard-code one widget's step: fails |
| No shortcut collides within a scope chain, and every shortcut is on the `?` sheet and in the palette | enumerate the action table | add a duplicate letter: fails |
| The transform session by keyboard equals the mouse drag and the inspector edit | arrows and typed `2` give the same spec and journal as dragging 2 m and typing 2 in the field | |
| Empty state guides | in `void`, the figure margin has the three buttons; clicking `Add a mass` selects a new body | existing behaviour as control |

**Task-based tests, scripted end to end.** Each is a list of input events (clicks at laid-out rects, keys, drags)
through `run_frames`, then an assertion on engine state and, where it matters, the number. Example for "add a
sphere, move it 2 m, read tower 1's phase":

1. start from `room` (or the wizard's output); press Ctrl+K, type `sphere`, Enter (the Add starter is selected, Move tool active);
2. assert `bodies.len() == 1` and the new body is selected;
3. drag the gizmo's x arrow by exactly 2 m (convert metres to pixels with `Shell::body_point`, or type `2` in the options strip);
4. assert the body's x differs by 2.000 m, one undo step;
5. `Engine::ask` for the reading, assert `readings()[0].gradiometer` equals the phase from `cav_sim` computed directly on the edited spec to 1e-12 rad (the same "known answer" the project's oracles use), and `state == refined`;
6. assert the step count: the whole task takes at most N events (N is the acceptance budget: record the first passing number and fail on growth).

Others: *delete a body* (select, Delete, one undo restores); *build the "mass passing the array" scene from the wizard
in four events*; *save and reload gives an identical spec hash*; *rename via F2*; *change a tower's offset in the
Instrument workspace and see its lane move*. Counting events gives a cheap, repeatable proxy for ease; it does not
replace people.

**First-run checklist** (a person, not a script; George or someone who has not used it; 20 minutes; record what
they said, not only whether they succeeded; stop a task at 3 minutes and note where they looked):

| # | Task | Pass when | Note |
|---|---|---|---|
| 1 | Open the app; say what you can do here | names adding a body, the playhead and the signal | what they looked at first |
| 2 | Add a sphere and a mass; place the sphere about 3 m from tower 2 | within 5 s of the first click on Add | which route: menu, palette, button |
| 3 | Move it 2 m east, exactly | position reads 2 m further; no console | gizmo or panel |
| 4 | Read what tower 1 reads | says the phase with its unit | where they found it |
| 5 | Undo the move; redo it; say what you did in the last minute | uses the history list or toasts | |
| 6 | Delete the mass; duplicate the sphere | both without the console | key or menu |
| 7 | Make the sphere oscillate at 0.05 Hz | motion changed through properties | |
| 8 | Find the periodogram and say which frequency stands out | | |
| 9 | Save the scene; close; reopen | the scene is back | web: download/upload |
| 10 | Start a new scene from a template | | |
| 11 | Find a command you only know the name of ("section") | palette | |
| 12 | With the mouse put away: select tower 2, add a body, move it 1 m, change its mass | all by keys; the breadcrumb always says where they are | where they lost the ring |
| 13 | Press `?`; find the key for "measure" | finds C | |

Pass bar for the slice: tasks 1–6 and 9 without help for a first-time user; any task needing the console is a bug
in the UI. Also keep a **heuristic review** using the ten heuristics above as a one-page checklist at the end of
each slice (the reviewer agent's UX pass).

---

## What I could not verify

- **Several primary pages could not be read as text.** The Blender manual pages (Menu Search, Numeric Input, Undo & Redo, snapping) came back as navigation only (200k characters of site chrome); I relied on search summaries of them, so quotes are paraphrase, not verbatim. Same for Unity's snapping page, Godot's key map, ParaView, COMSOL, Onshape and PhET: the Unity Hierarchy page, Godot's first-look page, the VS Code palette guidelines and Nielsen's heuristics were read directly; the rest are search summaries and are marked "via search summary".
- **Teardown's editor, Fusion's timeline, Houdini:** not researched (no primary source found within the time box); nothing here depends on them.
- **MagicaVoxel and Algodoo** are cited from community manual, forum and Wikipedia pages, which are secondary.
- **`egui_dock`, `egui_tiles` and `transform-gizmo-egui` versions against the workspace's `egui =0.37.0`**, and their wasm builds: not checked. Their feature claims are from docs.rs/lib.rs summaries. Resolve with a one-hour spike in a scratch crate before committing to either.
- **`BodySpec` rotation and size fields**, whether `shape` has starter constructors, whether `Engine` tracks "unsaved edits", and what a grouped edit would cost in `history.rs`: not examined. The Rotate/Scale rows of 3.5 and the title-bar asterisk of 3.7 depend on them.
- **The app was not run.** All behaviour is from code and the committed screenshots (main at `987d411`; the S4 work in the other checkout was not looked at and may have changed panels).
- **Effort figures are my estimates**, not measurements; the slice's own first task (the palette) will calibrate them.
- **`egui_kittest`:** the brief said it exists; I did not find it (section 4).
- **The keyboard spec** was read from the local branch `design-keyboard` (not on `main` when I wrote this); the link in this note works once it merges. Which browser-reserved keys a page can intercept (Ctrl+K, Ctrl+S, Ctrl+D, Ctrl+1–3, F3) was not tested.

## Further questions (not chased)

- A **library/prefab** of saved bodies and instruments, beyond the three templates.
- **Screen-reader support** in egui (AccessKit); the scope tree could feed it, but this was not examined.
- **Touch and web specifics** (long-press for context menu, no hover on touch, no file paths).
- **Scripting from the palette** (running a saved journal as a macro).
- **Voxel edit commands** (attach/erase/paint at 10⁹ cells): the command, its undo cost, and how the world volume (S4) streams edits.
- **Per-tower array layout tools** (line, ring, grid) and what the real site constrains: a question for George.
- Whether **F3** should remain "hide the GUI" (the mock-up's rule) now that Blender users expect F3 to search.
