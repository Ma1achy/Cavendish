# Adding a plot a pane can show, and export as a figure

**Example to read:** the flight diagram: `PaneContent::Flight` in `crates/cav-sim/src/view.rs`,
the function `flight` in `crates/cav-plot/src/plots.rs`, and `FigureKind::Flight` in
`crates/cav-engine/src/command.rs` and `crates/cav-plot/src/figure.rs`.

A plot is built once as a display list (`Drawing`: lines, rectangles, circles, text, in points,
1/72 inch) from what the engine answers. The same list is painted by egui in the window and for a
PNG, and written as SVG and PDF. So a plot never touches a painter: it asks the engine, places
marks with a `Style` and a text measurer, and returns a `Plot`.

Nothing fails when a new kind is left out of a list that names the kinds by hand (no test walks
`PaneContent::ALL` or `FigureKind::ALL` for you, and several `match`es have a catch-all arm), so
the steps below name each such place.

## The plot

1. **Its name.** A variant of `PaneContent` in `crates/cav-sim/src/view.rs`, with its word in
   `PaneContent::name` and its place in `PaneContent::ALL`: the console's `show <word>` and the
   `show` buttons of the View panel (`crates/cav-app/src/panels.rs`, three to a row) are made
   from that list. The usage line of `show` in `COMMANDS` (`crates/cav-engine/src/console.rs`) is
   written by hand: add the word there (`the_usage_lines_name_every_figure_and_every_pane_content`
   fails until you do).
2. **Its numbers.** Ask the engine: a `Query`, or what it already holds (`Engine::spec`,
   `Engine::shots`, `Engine::signal`). If the engine cannot answer yet, add the query first
   ([a-command.md](a-command.md)). A plot computes nothing the engine could be asked for: an
   agent must be able to read the same numbers as text.
3. **Its drawing.** A function in `crates/cav-plot/src/plots.rs` taking
   `(engine, subject, style, measure, size)` and returning a `Plot`, and its line in `build`.
   Every plot is laid out on a `Sheet` (private to `plots.rs`; read `flight` and `cadence`):
   - `Sheet::new` with a `Frame`: the titles, longest first (`Sheet::fit` writes the first that
     fits); whether it is titled (`Subject::title`); the legend lines; and in `Frame::left` the
     texts of the vertical axis's marks, worked out before the sheet, since the left margin is
     made as wide as they are. With `left` empty the numbers run into the vertical axis's name
     and `the_unit_is_the_vertical_axis_s_label` fails.
   - `Sheet::axes` (or `Sheet::x_marks` for a plot whose rows are its own, as the flight
     diagram's are) draws the marks and records them in `Drawn` (`ticks`, `tick_labels`,
     `minor_ticks`), which the figure's record and the Python redraw read: marks drawn any other
     way are not in the record. `Sheet::caption` is the horizontal axis's label.
   - With nothing to draw, `Sheet::nothing` and `Drawn::note`: a figure of a plot with no points
     and a note is refused with that note; without the note an empty figure is written.
   - `Sheet::finish` fills the rest of `Drawn` from `blank`.

   Use `Axis` and `place` to put a value on the axes, `thinned` for a long series (a drawing
   holds at most `MAX_POINTS` points a series and, for a signal, `MAX_SERIES` towers; the data
   file keeps every value), and take every colour, width and font size from the `Style`: the
   window's dark theme and the paper presets are the same code. Fill in `described` (the
   figure's provenance: axes with units, series with their columns, and in `Described::marks`
   whatever is drawn that is not a series).
4. **Its bound.** Whatever a scene can make large (rows, shots, labels) is cut to a stated
   number (a `MAX_…` constant) before anything is laid out, as `flights_drawn` does, and the plot
   says what it left out. Put the cut in one function that the figure's data table calls too
   (step 6), so the file holds the rows the picture drew.
5. **Its shots.** A plot that draws or counts shots takes them from the engine and never works
   a time out from `k·cycle` (each tower has its own cycle and 2T, and its own count of shots):
   `Engine::shots()` (the scene's `cav_sim::read::Shots`, made once
   an edit: `touching` finds those in a span, `flown` gives a shot's launch, readout and whether
   the tower takes it), or the signal's own `taken` flags for what was read. A shot not taken
   is not a measurement: it is drawn as not taken (a gap in a line, a dashed arc, an outline),
   never as its slot's zero, and the caption counts the shots taken.
6. **In a pane.** Choices made by content, each with a default that is some other plot's:
   `Subject::of_pane` (which tower: the selected one, except for a signal); `of_pane` in
   `plots.rs` (only a signal and a periodogram are drawn twice while comparing); and in the pane
   loop of `crates/cav-app/src/panels.rs` how far the wheel may zoom out (the `whole` of the
   horizontal axis: a catch-all arm gives it half the mean shot rate, a periodogram's) and
   whether a click seeks (a signal and a flight diagram only).

## As a figure

7. A variant of `FigureKind` in `crates/cav-engine/src/command.rs`: its word in `name`, its
   place in `ALL`. `Figure::needs_signal` says whether the export waits for a refined signal: it
   is a list, and a kind drawn from the signal must be added to it. A plot of one tower is given
   the selected tower (or the first) when none is asked for in three places, each a list of kinds
   (`Periodogram | Cadence`): `parse_figure` in `console.rs`, `figure()` in
   `crates/cav/src/main.rs`, and the MCP `figure` tool in `crates/cav/src/mcp.rs`.
8. In `crates/cav-plot/src/figure.rs`: its `PaneContent` in `build`; its data table in `table`,
   the columns written beside the figure, each named with its unit, each value the engine's own
   to the bit (a table as large as the signal is not built: it is read from what the engine holds
   as the file is written, `Table::write_csv`, `Table::write_npy`); and which columns each
   series is (`x_column`, `y_column`), in the `match` in `build` whose catch-all arm is the
   flight diagram's (launch, tower). What the record says of the figure whatever the format goes
   in `Built::figure`; what is one picture's own (anything that depends on the preset or the
   size) goes in its entry, `Built::provenance`.
9. The words written by hand: the usage line of `figure` in `COMMANDS`; `USAGE` and the refusal
   of an unknown kind in `crates/cav/src/main.rs` (the help line is asserted whole by
   `cli_figure_writes_a_figure_with_its_data_twice_the_same`); the refusal and the doc of
   `FigureArgs::what` (the MCP schema) in `crates/cav/src/mcp.rs`; the module docs of
   `crates/cav-plot/src/lib.rs` and `plots.rs`, and the `cav-plot` row of `docs/ARCHITECTURE.md`.
10. In `python/cavendish_figures/redraw.py`, a function that draws it from the data file alone,
    and its entry in the `drawers` of `draw` (a kind not there is `NotDrawable`). It chooses
    nothing the record says: `draw` puts the marks at the axes' `ticks` and `minor_ticks`,
    written as `tick_labels`, with the axes' `label` and the title; the function writes each label
    the plot's `marks` name (`null` where none was drawn), uses the record's `colours`, and gives
    each series' artist the `gid` `"series"`, one artist for each of the record's series, in its
    colour. `draw` also names the flight diagram, whose vertical axis is its own: a plot like it
    is added there.

The console's `figure <word> … to <file>`, `cav figure` and MCP's `figure` then parse the word
from `FigureKind::ALL`, and `cav render` and MCP's `render_view` paint a pane through `paint`.

## Tests that then hold it

- `crates/cav-plot/tests/plots.rs`: a test of what the plot shows, read from `Drawn` and the
  drawing's items (`a_signal_plot_has_a_key` is the pattern), with a control arm. Add the kind to
  the lists of `a_figure_may_have_no_title` and `the_unit_is_the_vertical_axis_s_label` (and,
  for a plot against the scene's time, `the_playhead_is_one_colour`).
- `figure.regenerates` (`validation_figure_regenerates` in `crates/cav-plot/tests/figures.rs`):
  add your kind to the figures `session` writes; the count of files it asserts (three a figure)
  and the case's `against` text then change with it.
- `figure_data_is_the_plotted_data`: the data file's columns against the engine's numbers, to
  the bit.
- `a_figure_is_bounded_in_series_and_its_data_is_streamed` (`tests/bounded.rs`, with the
  counting allocator; a signal's export at 1000 towers): what an export of your kind holds for a
  scene at its caps.
- `a_figure_is_bounded_in_points_and_pixels`: the scene of 21,000 shots there would make your
  plot large; assert the drawing and the table hold what step 4 says.
- `crates/cav-app/tests/shell.rs`: no test shows every content in a pane (there are four panes
  and more plots). `panes_show_plots_and_the_scene` shows a signal and a periodogram, and
  `four_plots_in_a_small_window_keep_their_text_in_their_panes` four plots; adding a pane to
  either breaks the counts they assert after. Write a test of your own that shows the plot in a
  pane, checks that its text stays inside it, and shows the scene again as the control.
- Python (run by `cargo xtask oracle`): a synthetic figure of the kind in
  `python/tests/synthetic.py` (`_axes`, `provenance`, `table`), the kind in the list of
  `test_the_redraw_is_the_same_figure` in `python/tests/test_figures.py`, and a real figure of
  it in `FIGURES` in `xtask/src/python.rs`, which `test_real_figures_are_read_and_redrawn`
  redraws and holds to its record. Without that line nothing tries the redraw on what `cav`
  writes.
- Kind-independent, needing nothing of a new kind: `a_figures_record_lists_every_format_written_of_it`
  and `a_figure_is_the_stated_size` (a periodogram's), `svg_and_pdf_are_well_formed` (a drawing
  made in the test).
- New tests raise the run count: `quick` then says to raise its floor in `xtask/floors.txt`.

Followed end to end on 24 Nov 2026 with a made-up kind (`launches`: each tower's launch times
modulo the cycle against time, a dot a shot, hollow where not taken): pane, `show`, `figure`,
`cav figure`, the record, the redraw and the tests above. The compiler asked only for the arm in
`plots.rs`'s `build` and, in `figure.rs`, those of `build` and `table`. `quick` failed in three
places: the count of files in `figure.regenerates`; `the_unit_is_the_vertical_axis_s_label`, once
the kind was in its list, because `Frame::left` had been left empty; and
`panes_show_plots_and_the_scene`, where the first form of this recipe said the pane went, whose
later count of plots then broke. What that form got wrong or left out, and this one says: the
`Sheet` and its `Frame` were not named; `needs_signal` is `Figure`'s, and a list; the tower
defaults, the zoom extent, the series' columns and the redraw's flight case are lists or
catch-all arms a new kind falls into without a word; the usage, help, refusal and schema texts
are written by hand and did not follow the kind (the two usage lines are now held by a test);
`panes_show_plots_and_the_scene` does not show every content, and `svg_and_pdf_are_well_formed`
and `a_figure_is_the_stated_size` do not run over every kind; the Python side needs a synthetic
figure and a line in `FIGURES`; and the floor. The recipe check now reads `python/` too, so the
Python files and tests named here are held to exist.

## Things that go wrong

- Drawing with a colour or a size that is not in the `Style`: it looks right in the window and
  wrong on paper (white on white).
- Measuring text with egui in a function that is also used for export: use the `Measure` given.
- Labels placed without looking at each other: place them, then drop or move the ones that
  collide; look at the PNG of the `paper-column` preset, the smallest, before calling it done.
- A number in the picture that is not in the data file: someone will want to replot it.
- A figure's table that cuts the rows another way than the plot does: the redraw then has dots
  the picture has not.
