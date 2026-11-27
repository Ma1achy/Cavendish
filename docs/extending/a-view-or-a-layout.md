# Adding a view type, or a layout of the figure

**Example to read:** the flight diagram as a view type (`PaneContent::Flight` in
`crates/cav-sim/src/view.rs`), and the Analysis layout in `crates/cav-app/ui/layouts.ron`.

The figure is a **mosaic of viewports** (`docs/design/workspace.md` §1). Three things are
separate, and each is data:

- **Where the viewports are**: a `Mosaic` (`crates/cav-sim/src/mosaic.rs`): rows of one-letter
  labels, a repeated label spanning cells, a weight a column and a row. Written
  `AAB;CDB cols 2 1 1 rows 1 1`, as matplotlib's `subplot_mosaic` writes it. The engine holds
  one (`Engine::mosaic`); `ViewCommand::Mosaic` sets it. At most `MAX_VIEWPORTS`.
- **What each shows**: a `PaneContent` (a *view type*: the scene, or a plot), set by
  `ViewCommand::Show` (`show <word> @N`).
- **Which is which**: viewport N is the Nth label of the alphabet among those present (`A`
  before `B`), wherever it stands. Its camera and its content are carried with its label through
  every change of the mosaic (`Engine::set_mosaic`), so a swap or a close moves no camera to
  another viewport, and the keyboard's order never follows the layout.

## A view type

A view type is a variant of `PaneContent`. One that is a plot is the whole of
[a-plot.md](a-plot.md): follow it (its name in `PaneContent::ALL`, its drawing in
`crates/cav-plot/src/plots.rs`, its figure, its tests). What the mosaic adds to that recipe:

1. Nothing to register: a viewport of any label shows any content, and every list the window
   offers a content from is made from `PaneContent::ALL` (the first control of every viewport's
   header, the console's `show <word>`). The action table's `show-…` items in
   `crates/cav-app/ui/actions.ron` are written by hand: add one, with its `about` (the header's
   drop-down shows it as the row's tooltip), and an icon in `icons::content`
   (`crates/cav-app/src/icons.rs`).
2. Its input goes in `canvas` in `crates/cav-app/src/panels.rs`, the one function that draws a
   viewport's canvas and takes its pointer, docked or floating: a plot's branch is the one that
   ends in `return` after the wheel and the click; the scene's is everything after it. What it
   asks of the engine is pushed to `Asked` and carried out once every viewport is drawn; never
   apply a command in the middle of the loop (the viewports after it would draw another
   generation).
3. A viewport is drawn at any size from `MIN_VIEWPORT` up, in any place, and twelve at once:
   keep whatever it draws inside the rectangle it is given, and bounded
   (`four_plots_in_a_small_window_keep_their_text_in_their_panes` is the pattern; show yours in
   one viewport of `mosaic "ABC;DEF"` in a test of your own).

## A view type's header controls

Every viewport has a header along its top (`crates/cav-app/src/header.rs`,
`docs/design/workspace.md` §2) and its canvas under it. `Layout::panes` is the viewport, header
and all; `Layout::canvases` is the canvas, and every place of a pointer in a view is measured
from the canvas's corner.

1. The header's controls are data: `header::CONTROLS`, in order, each with the view types it is
   for (`Applies::Scene`, `Applies::Plot`, `Applies::Any`) and the action of
   `crates/cav-app/ui/actions.ron` it runs. A control for your view type is an entry there (give
   `Applies` a variant if neither fits) and its drawing in `header::draw`, beside the plot's
   `span`: one call of `control` with its icon, its tooltip (`header::tooltip` of its action:
   the name, the key, what it does) and its look (disabled says why in the tooltip), then the
   line it runs pushed to `Asked::lines`, addressed to its own viewport (`span all @3`).
2. A menu's rows are actions too: the camera menu is the actions whose `menu` is one of
   `header::CAMERA_MENUS`, the overlays popover those of `header::OVERLAY_MENUS`. An action added
   to `View/Overlay` in the table is in the popover with no code; which row is the one chosen is
   worked out by parsing its line and comparing it with the view.
3. The keyboard needs nothing more: the active viewport's header is the scope `viewport/header`,
   whose stops are `CONTROLS` in that order whatever the viewport shows (a stop that does not
   apply is disabled, never left out). Push a `Stop` for yours.
4. No status in a header. Something to warn of (a stale input, something left out of the
   picture) goes in `header::warnings`: the viewport then shows one dot whose tooltip holds the
   words. Standing facts go to the status bar (`status_parts` in `panels.rs`).
5. Width: the left group gives way as a viewport narrows (words first, then the shading's four
   buttons fold into one drop-down, then controls are left out where the right group begins).
   Look at yours in `mosaic "ABCDE" cols 1 1 1 1 4`.

## A layout of the figure

(A layout made in the window: the top bar's `+`, or `layout save [name]`, keeps the present
mosaic and what each viewport shows as a tab for the session; `layout forget <id>` drops it. To
keep one for good, write it into the file as below.)

1. An entry in `crates/cav-app/ui/layouts.ron`: `id`, `label`, `lines`, `about`. The first line
   arranges the figure: `mosaic "AAC;BDC"` (in the file, `"mosaic \"AAC;BDC\""`; add
   `cols …` and `rows …` for weights), or the shorthands `panes 1`, `panes 2`, `panes 4` for
   `A`, `AB` and `AB;CD`. Then a `show <word> @N` for each viewport, and `pane N` for the active
   one. Number the viewports by their labels' alphabet, not by where they stand.
2. Its action in `crates/cav-app/ui/actions.ron`: `layout <id>`, with a key if it earns one
   (Ctrl+1 to 3 are taken) and an `about`. The `?` sheet, the palette and the View menu follow.
3. Nothing else: `layout "AAB;CDB"` typed at the console arranges the figure with no entry at
   all, and a person's splits and drags are `mosaic` lines in the journal like a layout's.

What a layout may not do is checked: its lines are view commands only (never an edit, a
selection or Home).

## Changing the mosaic from code

Split, close, swap, resize and even are pure functions from a mosaic to a mosaic
(`Mosaic::split_right`, `Mosaic::split_down`, `Mosaic::close`, `Mosaic::swap`, `Mosaic::resize`,
`Mosaic::even_at`, `Mosaic::even`): work out the new one and issue the one command,
`ViewCommand::Mosaic`, with `from` naming the viewport a new label copies (a split) or `None`.
The console's `split`, `close` and `swap` do exactly that. A drag in the window keeps its own
copy while it lasts (`splitter_moved`) and issues the command once, at the end: a layout change
is one journal line, never one a frame. Floating a viewport (`viewport pop-out`) and maximising
one are the window's state (`Shell::floating`, `Shell::maximised`): no command, no journal line,
and the mosaic keeps the viewport's place.

## Tests that then hold it

- `mosaic::tests::a_mosaic_is_the_rectangles_subplot_mosaic_gives` and
  `mosaic::tests::split_close_and_swap_keep_a_mosaic` (`cav-sim`): the data and its functions.
  A new function of a mosaic gets its line in the second, with what it refuses.
- `a_mosaic_replays_from_the_journal` (`cav-engine`): a session of splits, a drag and a swap
  replays to the same mosaic and cameras.
- `the_layouts_are_valid` and `every_layout_keeps_every_scope_and_the_figure_uncovered`: a new
  layout's lines and the window's rules in it.
- `crates/cav-app/tests/editor/s20.rs`: `the_header_holds_no_status_and_the_bar_says_the_engine_s`,
  `every_header_control_says_what_it_does` (a control with no tooltip, or whose tooltip lacks its
  action's key, fails it), `the_view_type_is_chosen_in_the_header`,
  `the_gizmo_snaps_to_an_axis_and_orbits`;
  `six_viewports_draw_and_the_thirteenth_is_refused`,
  `a_splitter_drag_keeps_the_sum_and_is_one_line`,
  `tab_and_arrows_follow_the_labels_whatever_floats`, `pop_out_and_dock_return_the_layout`,
  `the_big_scopes_are_unchanged_by_the_mosaic`.
- `pane_rects_are_the_mosaic_s_for_one_two_and_four` (`cav-render`): `A`, `AB` and `AB;CD` are
  the rectangles 1, 2 and 4 panes always had.

## Things that go wrong

- Numbering viewports by position: in `"BA"` viewport 1 is on the right. Ask
  `Mosaic::number` and `Mosaic::label`.
- A label whose cells are an L or two islands: refused in words by `Mosaic::parse`; so are
  ragged rows, a thirteenth label and a letter outside `A`–`Z`.
- Pointer arithmetic from `Layout::panes`: that rectangle includes the header. Use
  `Layout::canvases`.
- A `;` in a script outside quotes ends the statement: write `mosaic "AB;CD"`, quoted.
- Closing a viewport no single neighbour can grow into (`A` in `AAB;CDB`): refused; swap it
  with one that can be closed, or write the mosaic wanted.
