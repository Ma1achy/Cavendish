# Adding to the window: an action, a tool, a layout, a panel

The window is data where it can be and code where it must be (`plan/REBUILD.md` principle 11;
`docs/slices/S18.md` §2). Data says what exists, where, under which keys and when it can run;
code says how a line is run, how a key is routed and how a widget is drawn. Read
`crates/cav-app/src/actions.rs`, `keyboard.rs` and `tools.rs` beside this, and the keyboard model
they implement, `docs/design/keyboard.md`.

**Example to read:** the action `proj-toggle` in `crates/cav-app/ui/actions.ron`, and the Place
tool: `Behaviour::Place` in `crates/cav-app/src/tools.rs` with its click in `figure`
(`crates/cav-app/src/panels.rs`).

## An action (a menu item, a key, a palette entry)

Read `proj-toggle` in `crates/cav-app/ui/actions.ron` beside this.

1. Make sure its console line runs: an engine command (`docs/extending/a-command.md`) or one of
   the window's own (`APP_COMMANDS` and `Shell::run_app_command` in `crates/cav-app/src/shell.rs`).
2. Add one entry to `crates/cav-app/ui/actions.ron`: `id`, `label`, `menu` (a path such as
   `View/Projection`, or empty), `line` (with `{selection}`, `{body}`, `{tower}` or `{field}` if
   it acts on the selection or the focused field), `scope` (where its keys work: a big scope's
   id, a scope under one such as `tree/sources`, or `global`), `keys`, `needs` (when it can run),
   `context` (on the scene tree's right-click menu), `about`.
3. Nothing else: the menu bar (`menus.rs`), the palette (`palette.rs`), the `?` sheet
   (`sheet.rs`) and the router (`Keyboard::route`) read the table.

Tests that then hold it: `actions::tests::the_data_files_are_valid` (the line parses; the scope
and menu exist; the chord is a chord), `no_shortcut_clashes_and_every_one_is_listed` (no two
actions on one chord where both can be heard; every key on the sheet and in the palette),
`every_menu_item_is_an_action_that_runs` (it runs).

## A tool

Read the Place tool (`Behaviour::Place`) beside this.

1. If it does something no tool does yet, add a `Behaviour` variant in `crates/cav-app/src/tools.rs`
   with its geometry as pure functions there (as `gizmo`, `hit`, `along`, `on_ground`), tested
   on their own.
2. Add it to `crates/cav-app/ui/tools.ron` (`id`, `label`, `behaviour`, `about`) and an
   action `tool <id>` with its key to `ui/actions.ron`, twice: in `toolbar` and in `viewport`
   (`tool-place` and `tool-place-in-view`). The tool's `label` is its one name and must be the
   action's `label` (the strip's tooltip, the options row, the menus and the `?` sheet all say
   it); the strip draws its icon by `behaviour`, so a new behaviour gets an icon in
   `crates/cav-app/src/icons.rs` (a few painter strokes in a 20 × 20 grid: the fonts have no
   pictographs).
3. Its pointer handling goes in `figure` in `crates/cav-app/src/panels.rs`, beside the gizmo's
   and the Place click; what it changes is a command (`Shell::apply`, `Shell::run`), never a
   field of the scene. Its options go in `tool_options_row`.
4. A tool is UI state: `choosing_a_tool_changes_no_scene` must still pass with it.

Tests that then hold it: `tools::tests::the_tool_strip_is_valid` (its action exists), a test of
its own in `crates/cav-app/tests/editor.rs` driven by input events, with a control arm (as
`the_place_tool_drops_a_starter_where_the_pointer_is`).

## A layout

Add an entry to `crates/cav-app/ui/layouts.ron` (`id`, `label`, the console `lines` that arrange
the figure, optionally the inspector's folding sections it opens, `unfold`, and `about`) and an
action `layout <id>` with its key. `the_layouts_are_valid` holds its lines (view commands only:
never an edit, Home or a selection) and its action;
`every_layout_keeps_every_scope_and_the_figure_uncovered` holds the window's rules in it.

## A panel or a window

1. A **window** (it may close; it is not a big scope): draw it in `crates/cav-app/src/dialogs.rs`
   as `history` is, register it with `shell.building.window(id, name)` and each of its parts
   with `Shell::register`, open it from a window command that calls `window_opened` (so the
   keyboard's focus goes in and comes back), and close it on `Effect::Close(id)` in
   `Shell::effect`. Give its id to `WINDOWS` in `actions.rs` if actions are scoped to it.
2. A **panel** that is always there is a place in one of the seven big scopes (`scopes` in
   `ui/actions.ron`); their order is fixed and never depends on what is shown
   (`tab_walks_the_big_scopes_in_order`). A new big scope is a change to that list and to every
   test that names it: ask first whether a section of an existing scope will do.
3. Every control registers itself (`Shell::register(scope, id, name, kind, rect)`) with the
   `Kind` that says what Enter and the arrows do on it, and takes what the keys asked of it
   (`take_click`, `take_adjust`, `take_type`, `take_pick`): then the keyboard reaches it with no
   more code. A control that is sometimes unusable stays registered, disabled
   (`building.disable`): order never depends on state.
4. Nothing it draws may cover the figure: `every_panel_is_laid_out_and_none_covers_the_figure`.

## Starters (what Add adds)

A starter is scene data, not window data: an entry of `scenes/starters.ron` (a body or a tower
about the origin). The Add menu, the palette, the Place tool, `add <name>` in the console, `cav
exec` and MCP all have it then. `every_starter_is_valid_and_unique` holds it.
