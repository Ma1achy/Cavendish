# Adding a plot a pane can show, and export as a figure

**Example to read:** the flight diagram: `PaneContent::Flight` in `crates/cav-sim/src/view.rs`,
the function `flight` in `crates/cav-plot/src/plots.rs`, and `FigureKind::Flight` in
`crates/cav-engine/src/command.rs` and `crates/cav-plot/src/figure.rs`.

A plot is built once as a display list (`Drawing`: lines, rectangles, circles, text, in points,
1/72 inch) from what the engine answers. The same list is painted by egui in the window and for a
PNG, and written as SVG and PDF. So a plot never touches a painter: it asks the engine, places
marks with a `Style` and a text measurer, and returns a `Plot`.

## The plot

1. **Its name.** A variant of `PaneContent` in `crates/cav-sim/src/view.rs`, with its word in
   `PaneContent::name` and its place in `PaneContent::ALL`: the console's `show <word>` and the
   `show` buttons of the View panel (`crates/cav-app/src/panels.rs`) are made from that list.
2. **Its numbers.** Ask the engine with a `Query`; if the engine cannot answer yet, add the query
   first ([a-command.md](a-command.md)). A plot computes nothing the engine could be asked for:
   an agent must be able to read the same numbers as text.
3. **Its drawing.** A function in `crates/cav-plot/src/plots.rs` taking
   `(engine, subject, style, measure, size)` and returning a `Plot`, and its line in `build`.
   Use `Axis` and `place` for the axes, `thinned` for a long series (a drawing holds at most
   2000 points a series and, for a signal, `MAX_SERIES` towers; the data file keeps every
   value), and take every colour, width and font
   size from the `Style`: the window's dark theme and the paper presets are the same code. Fill
   in `Drawn` (the areas, axes and marks tests read) and `described` (the figure's provenance:
   axes with units, series with their columns).
4. **Its bound.** Whatever a scene can make large (rows, shots, labels) is cut to a stated
   number before anything is laid out, as `flights_drawn` does, and the plot says what it left
   out.
5. **Its shots.** A plot that draws or counts shots takes them from the engine and never works
   a time out from `k·cycle`: `Engine::shots()` (the scene's `cav_sim::read::Shots`, made once
   an edit: `touching` finds those in a span, `flown` gives a shot's launch, readout and whether
   the tower takes it), or the signal's own `taken` flags for what was read. A shot not taken
   is not a measurement: it is drawn as not taken (a gap in a line, a dashed arc, an outline),
   never as its slot's zero, and the caption counts the shots taken.

## As a figure

6. A variant of `FigureKind` in `crates/cav-engine/src/command.rs` (its word in `name`, its
   place in `ALL`; `needs_signal` says whether it waits for a refined signal), and in
   `crates/cav-plot/src/figure.rs` its `PaneContent` and its data table: the columns written
   beside the figure, each named with its unit, each value the engine's own to the bit. A table
   as large as the signal is not built: it is read from what the engine holds as the file is
   written (`Table::write_csv`, `Table::write_npy`). What the record says of the figure
   whatever the format goes in `Built::figure`; what is one picture's own (anything that
   depends on the preset or the size) goes in its entry, `Built::provenance`.
7. In `python/cavendish_figures/redraw.py`, a function that draws it from the data file alone,
   and its line in `draw`. It chooses nothing the record says: `draw` puts the marks at the
   axes' `ticks` and `minor_ticks`, written as `tick_labels`, with the axes' `label` and the
   title; the function writes each label the plot's `marks` name (`null` where none was
   drawn), uses the record's `colours`, and gives each series' artist the `gid` `"series"`.

Console (`figure <word> … to <file>`), `cav figure` and MCP's `figure` follow from the kind.

## Tests that then hold it

- `crates/cav-plot/tests/plots.rs`: a test of what the plot shows, read from `Drawn` and the
  drawing's items (`a_signal_plot_has_a_key` is the pattern), with a control arm.
- `figure.regenerates` (`validation_figure_regenerates` in `crates/cav-plot/tests/figures.rs`):
  add your kind to the figures written twice.
- `figure_data_is_the_plotted_data`: the data file's columns against the engine's numbers.
- `a_figures_record_lists_every_format_written_of_it`: one record for the formats of a figure.
- `a_figure_is_bounded_in_series_and_its_data_is_streamed` (`tests/bounded.rs`, with the
  counting allocator): what an export holds for a scene at its caps.
- `a_figure_is_bounded_in_points_and_pixels`: add a scene that would make your plot large.
- `svg_and_pdf_are_well_formed` and `a_figure_is_the_stated_size` run over every kind.
- `python/tests/test_figures.py` (run by `cargo xtask oracle`): the redraw.
- `crates/cav-app/tests/shell.rs`: `panes_show_plots_and_the_scene` shows every content in a pane.

## Things that go wrong

- Drawing with a colour or a size that is not in the `Style`: it looks right in the window and
  wrong on paper (white on white).
- Measuring text with egui in a function that is also used for export: use the `Measure` given.
- Labels placed without looking at each other: place them, then drop or move the ones that
  collide; look at the PNG of the `paper-column` preset, the smallest, before calling it done.
- A number in the picture that is not in the data file: someone will want to replot it.
