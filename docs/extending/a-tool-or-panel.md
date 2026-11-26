# Adding to the window: an action, a tool, a layout, a panel

The window is data where it can be and code where it must be (`plan/REBUILD.md` principle 11;
`docs/slices/S18.md` §2). Data says what exists, where, under which keys and when it can run;
code says how a line is run, how a key is routed and how a widget is drawn. Read
`crates/cav-app/src/actions.rs`, `keyboard.rs` and `tools.rs` beside this, and the keyboard model
they implement, `docs/design/keyboard.md`.

**Example to read:** the action `proj-toggle` in `crates/cav-app/ui/actions.ron`, and the Place
tool: `Behaviour::Place` in `crates/cav-app/src/tools.rs` with its click in `canvas`
(`crates/cav-app/src/panels.rs`), its flyout and its options row
(`crates/cav-app/src/tool_palette.rs`).

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
   action's `label` (the palette's tooltip, the menus and the `?` sheet all say it); the
   palette draws its icon by `behaviour`, so a new behaviour gets an icon in
   `crates/cav-app/src/icons.rs` (a few painter strokes in a 20 × 20 grid: the fonts have no
   pictographs). That is all the palette needs: it lays out one `theme::ICON` button a tool
   (`tool_palette::plan`), in the file's order, and registers its keyboard stop
   (`tool_palette::register`).
3. Its pointer handling goes in `canvas` in `crates/cav-app/src/panels.rs` (the one function
   that draws a viewport's canvas and takes its pointer), beside the gizmo's and the Place
   click; what it changes is a command (`Shell::apply`, `Shell::run`), never a
   field of the scene.
4. **Variants** (a flyout: Place ▸ sphere, cube…): give the tool `variants: <source>` in
   `ui/tools.ron`. A source is a variant of `tools::Variants` whose `list` gives each choice's
   name, label and tooltip (`Starters` reads `scenes/starters.ron`); a new source is a new arm
   there, in `tool_palette::chosen` (which one is chosen now), in `tool_palette::variant_icon`
   and in the `tool` command of `Shell::run_app_command` (what `tool <id> <variant>` sets). The
   palette then draws the corner mark and opens the flyout on a hold (`tool_palette::HOLD`,
   0.4 s) or a right-click, and the keyboard has it as the drop-down stop after the tool's own
   (`toolbar/<id>/variants`: Enter opens it, the arrows choose, Enter takes the tool with that
   variant, Esc closes it).
5. **Options** (settings shown while the tool is in hand): return true for the behaviour in
   `Behaviour::has_options` and draw them in its arm of `tool_palette::options_row`: one row
   (`theme::HEADER` high) under the header of the viewport the palette is in. Terse data only
   (a switch, a drop-down, a number with its unit: `the_chrome_holds_no_sentences` reads what
   the row paints from the layout's `chrome`); how the tool is worked is a tooltip. Each
   control runs a console line (pushed to `Asked::lines`). A behaviour without options has no
   row at all: the canvas has the room.
6. A tool is UI state: `choosing_a_tool_changes_no_scene` must still pass with it.

Where the palette stands (`tool_palette::Anchor`, kept for each layout name in
`Shell::tool_anchors`; `tool-palette <anchor>` from the console) and which viewport it is in
(`tool_palette::host`: the active one if it shows the scene, else the first that does; none if
none does, when the tools' keys still take a tool) need nothing from a new tool.

Tests that then hold it: `tools::tests::the_tool_strip_is_valid` (its action exists),
`tools::tests::a_tool_s_variants_come_from_a_source_the_code_knows`, S20's
`the_palette_floats_in_the_canvas_and_moves` and
`place_opens_a_flyout_and_options_show_only_when_they_apply`, and a test of its own in
`crates/cav-app/tests/editor.rs` driven by input events, with a control arm (as
`the_place_tool_drops_a_starter_where_the_pointer_is`).

## A layout

Add an entry to `crates/cav-app/ui/layouts.ron` (`id`, `label`, the console `lines` that arrange
the figure, optionally the inspector's folding sections it opens, `unfold`, and `about`) and an
action `layout <id>` with its key. How the figure's own arrangement is written (a `mosaic` line)
is in [a-view-or-a-layout.md](a-view-or-a-layout.md). `the_layouts_are_valid` holds its lines (view commands only:
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
4. Nothing it draws may cover the figure: `every_panel_is_laid_out_and_none_covers_the_figure`
   (over a scene's canvas float only the tool palette, at an edge or a corner of it, and the
   navigation gizmo in its corner, by design; a viewport's own header, and the tool's options
   row under it, are above its canvas:
   [a view type's header controls](a-view-or-a-layout.md)).
5. **In the inspector** (`crates/cav-app/src/inspector.rs`, `docs/design/panels.md`): a part of
   the selection is a card (`inspector::card`: folds, a ⋮ menu by `inspector::part_menu`), its
   rows on the one grid whose label column is `Shell::label_width`; a value worked out and not
   edited goes in the Info card (`inspector::info`), never among the fields; an optional field is
   a checkbox (`gate`), never "none add"; no help paragraphs (the doc string is the name's hover).
   Every card has a ⋮ (`part_menu` for one part of the scene, `fields_menu` for several fields of
   one); a card of chosen fields draws them with `editors::draw_rows` under the path they are
   named from (draw a field under its own parent, or the boxes of the optional parts between
   appear before it); a drop-down that is not a field's is `editors::choice_row`. A body's cards
   are `inspector::body`, in the design's order. Every target is at least `theme::HIT` (20 pt) a
   side and every size is the theme's (`theme::HIT`, `ICON`, `GRID`, `ROW_GAP`, `HEADER`, the
   text sizes; `theme::dense` sets a panel's spacing): no literal row heights. A widget fills its
   column (`editors::value_room` is what a row has before the reset's place); a number is
   monospace, right-aligned, its unit after it. No sentence is painted in the chrome: say it in a
   tooltip. Every text colour on every ground is in `theme::text_pairs` (4.5:1), and the panel
   keeps its width, so a row must fit it (`editors::combo_width`, `bounded`).
   `every_value_in_the_inspector_starts_at_one_x`, `every_target_in_the_side_panels_is_big_enough`,
   `every_text_reads_on_its_ground`, `the_inspector_keeps_its_width_whatever_is_shown` and S20's
   `sizes_are_the_dense_design_s`, `a_field_takes_its_column_s_width`,
   `the_scroll_bar_s_width_is_reserved` and `the_chrome_holds_no_sentences` hold it.
6. **In the scene tree** (`crates/cav-app/src/outliner.rs`): rows name things (an icon a kind of
   thing, no numbers: those are the Readings tab's); a group folds; `the_outliner_is_one_tree_of_named_rows`.

## Starters (what Add adds)

A starter is scene data, not window data: an entry of `scenes/starters.ron` (a body or a tower
about the origin). The Add menu, the palette, the Place tool, `add <name>` in the console, `cav
exec` and MCP all have it then. `every_starter_is_valid_and_unique` holds it.
