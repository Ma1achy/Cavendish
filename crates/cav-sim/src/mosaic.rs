//! The figure's mosaic of viewports, as data (`docs/design/workspace.md` §1): a small grid of
//! one-letter labels in which a label that repeats spans cells, as
//! `matplotlib.pyplot.subplot_mosaic` writes it, with a weight for every column and every row.
//!
//! ```text
//! AAB      A is wide, over C and D; B is tall, beside them
//! CDB
//! ```
//!
//! ## The written form
//! `AAB;CDB` — rows separated by `;`, one letter `A`–`Z` a cell. Weights follow when they are
//! not all 1: `AAB;CDB cols 2 1 1 rows 1 3` (one positive number a column, then one a row;
//! either may be left out). The grid may stand in double quotes, which a console line needs
//! (`mosaic "AAB;CDB"`: a `;` outside quotes ends a statement). [`Mosaic::parse`] reads it and
//! `Display` writes it; in JSON a mosaic is that text.
//!
//! ## Order
//! A viewport's **number** is the rank of its label in the alphabet among the labels present
//! (`A` before `B` before `D`), from 0 here and from 1 on screen. It never depends on where the
//! label stands in the grid, so swapping two viewports' places, or floating one over the
//! window, changes no number and no order of keys. In a mosaic written in the usual way
//! (`AAB;CDB`) that is also the reading order of first appearance.
//!
//! Everything here is a pure function: splitting, closing, swapping and resizing give a new
//! mosaic and are tested with no window. Nothing here knows of cameras or of what a viewport
//! shows: the engine carries those with the labels.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The most viewports a figure holds. A thirteenth is refused with a message that says so.
pub const MAX_VIEWPORTS: usize = 12;

/// The most rows, and the most columns, a mosaic is written with.
pub const MAX_TRACKS: usize = 24;

/// A boundary between two neighbouring columns or rows: what a splitter drags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Boundary {
    /// Between column `n` and column `n + 1`, from 0.
    Column(usize),
    /// Between row `n` and row `n + 1`, from 0.
    Row(usize),
}

/// The cells a label covers: a rectangle of the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// Its first column, from 0.
    pub column: usize,
    /// How many columns it covers, at least 1.
    pub columns: usize,
    /// Its first row, from 0.
    pub row: usize,
    /// How many rows it covers, at least 1.
    pub rows: usize,
}

/// A mosaic of viewports: see the module's text. Always valid: every label's cells are one
/// rectangle, the rows are as long as one another, the weights are positive and finite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Mosaic {
    /// The labels, a row at a time.
    grid: Vec<Vec<char>>,
    /// Each column's weight: its share of the figure's width is its weight over their sum.
    columns: Vec<f64>,
    /// Each row's weight, likewise of the height.
    rows: Vec<f64>,
}

impl JsonSchema for Mosaic {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Mosaic".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "A mosaic of viewports as subplot_mosaic writes it: rows of one-letter labels (A to Z) \
                separated by `;`, a repeated label spanning cells, then optional weights: `AAB;CDB cols 2 1 1 rows 1 1`. \
                At most 12 labels; each label's cells one rectangle.",
        })
    }
}

impl Default for Mosaic {
    fn default() -> Self {
        Self::single()
    }
}

impl From<Mosaic> for String {
    fn from(mosaic: Mosaic) -> Self {
        mosaic.to_string()
    }
}

impl TryFrom<String> for Mosaic {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        Self::parse(&text)
    }
}

impl std::fmt::Display for Mosaic {
    /// The written form, the grid without quotes: `AAB;CDB cols 2 1 1`. Weights are written
    /// with every digit, so the text reads back to the same mosaic to the bit.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.grid_text())?;
        for (word, weights) in [("cols", &self.columns), ("rows", &self.rows)] {
            if weights.iter().any(|w| *w != 1.0) {
                write!(f, " {word}")?;
                for weight in weights {
                    write!(f, " {weight}")?;
                }
            }
        }
        Ok(())
    }
}

/// A list of weights as the written form has them.
fn weights_of(words: &[&str], name: &str, tracks: usize, what: &str) -> Result<Vec<f64>, String> {
    if words.len() != tracks {
        return Err(format!(
            "`{name}` takes {tracks} weight{}, one a {what}; {} {} given",
            if tracks == 1 { "" } else { "s" },
            words.len(),
            if words.len() == 1 { "was" } else { "were" },
        ));
    }
    words
        .iter()
        .map(|word| {
            word.parse::<f64>()
                .ok()
                .filter(|w| w.is_finite() && *w > 0.0)
                .ok_or_else(|| format!("a weight is a positive number, not `{word}`"))
        })
        .collect()
}

/// Where each track (column or row) of `weights` lies along `extent` pixels with `gap` pixels
/// between neighbours: `(start, length)`. The rule is `cav_render::pane_rects`'s: the gaps come
/// off first (all of the extent if it is smaller than them), what is left is cut at
/// `floor(left · cumulative weight / total)`, and the last track takes the remainder.
fn tracks(weights: &[f64], extent: u32, gap: u32) -> Vec<(u32, u32)> {
    let n = weights.len() as u32;
    let gaps = gap.saturating_mul(n - 1).min(extent);
    let left = extent - gaps;
    let total: f64 = weights.iter().sum();
    let mut edges = Vec::with_capacity(weights.len() + 1);
    edges.push(0_u32);
    let mut cumulative = 0.0;
    for weight in &weights[..weights.len() - 1] {
        cumulative += weight;
        let edge = (f64::from(left) * cumulative / total).floor() as u32;
        edges.push(edge.clamp(*edges.last().expect("it starts with 0"), left));
    }
    edges.push(left);
    (0..weights.len())
        .map(|k| {
            let before = if n > 1 { (u64::from(gaps) * k as u64 / u64::from(n - 1)) as u32 } else { 0 };
            (edges[k] + before, edges[k + 1] - edges[k])
        })
        .collect()
}

impl Mosaic {
    /// One viewport, `A`: what a fresh figure is.
    #[must_use]
    pub fn single() -> Self {
        Self { grid: vec![vec!['A']], columns: vec![1.0], rows: vec![1.0] }
    }

    /// The mosaic `panes <n>` is short for: `A`, `AB` (side by side) or `AB;CD` (a 2 × 2 grid in
    /// reading order). `None` for any other count.
    #[must_use]
    pub fn panes(count: usize) -> Option<Self> {
        let text = match count {
            1 => "A",
            2 => "AB",
            4 => "AB;CD",
            _ => return None,
        };
        Some(Self::parse(text).expect("the three shorthands are mosaics"))
    }

    /// Reads the written form (the module's text): `AAB;CDB`, `"AAB;CDB" cols 2 1 1 rows 1 1`.
    ///
    /// # Errors
    /// In words: a character that is not a letter `A`–`Z`, an empty row, rows of different
    /// lengths, more than [`MAX_VIEWPORTS`] labels, a label whose cells are not one rectangle,
    /// weights that are not one positive number a column or a row.
    pub fn parse(text: &str) -> Result<Self, String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let Some((grid, rest)) = words.split_first() else {
            return Err("a mosaic is rows of labels, as `AAB;CDB`; this is empty".into());
        };
        let grid = grid.strip_prefix('"').and_then(|g| g.strip_suffix('"')).unwrap_or(grid);
        let grid: Vec<Vec<char>> = grid.split(';').map(|row| row.chars().collect()).collect();
        for (n, row) in grid.iter().enumerate() {
            if row.is_empty() {
                return Err(format!("row {} of the mosaic is empty", n + 1));
            }
            if let Some(other) = row.iter().find(|c| !c.is_ascii_uppercase()) {
                return Err(format!("`{other}` is not a label: a label is one letter, A to Z"));
            }
            if row.len() != grid[0].len() {
                return Err(format!(
                    "row {} has {} label{} and row 1 has {}: every row is as long as the first",
                    n + 1,
                    row.len(),
                    if row.len() == 1 { "" } else { "s" },
                    grid[0].len()
                ));
            }
        }
        if grid.len() > MAX_TRACKS || grid[0].len() > MAX_TRACKS {
            return Err(format!("a mosaic has at most {MAX_TRACKS} rows and {MAX_TRACKS} columns"));
        }
        let (mut columns, mut rows) = (vec![1.0; grid[0].len()], vec![1.0; grid.len()]);
        let mut rest = rest;
        let mut seen = [false; 2];
        while let Some((&word, after)) = rest.split_first() {
            let which = match word {
                "cols" => 0,
                "rows" => 1,
                other => {
                    return Err(format!(
                        "`{other}`: after the grid come `cols <weights>` and `rows <weights>`"
                    ));
                }
            };
            if std::mem::replace(&mut seen[which], true) {
                return Err(format!("`{word}` is given twice"));
            }
            let end = after.iter().position(|w| matches!(*w, "cols" | "rows")).unwrap_or(after.len());
            if which == 0 {
                columns = weights_of(&after[..end], word, columns.len(), "column")?;
            } else {
                rows = weights_of(&after[..end], word, rows.len(), "row")?;
            }
            rest = &after[end..];
        }
        let mosaic = Self { grid, columns, rows };
        let labels = mosaic.labels();
        if labels.len() > MAX_VIEWPORTS {
            return Err(format!(
                "a figure holds at most {MAX_VIEWPORTS} viewports; this mosaic has {}",
                labels.len()
            ));
        }
        for label in labels {
            let span = mosaic.bounds(label);
            let cells = mosaic.grid.iter().flatten().filter(|c| **c == label).count();
            if cells != span.columns * span.rows {
                return Err(format!(
                    "the cells of `{label}` are not one rectangle: a viewport is a rectangle of the grid"
                ));
            }
        }
        Ok(mosaic)
    }

    /// The written form with the grid in double quotes, as a console line needs it:
    /// `"AAB;CDB" cols 2 1 1`.
    #[must_use]
    pub fn quoted(&self) -> String {
        let text = self.to_string();
        let grid = self.grid_text();
        format!("\"{grid}\"{}", &text[grid.len()..])
    }

    /// The grid alone, rows separated by `;`, with no weights: `AAB;CDB`.
    #[must_use]
    pub fn grid_text(&self) -> String {
        let rows: Vec<String> = self.grid.iter().map(|row| row.iter().collect()).collect();
        rows.join(";")
    }

    /// The labels, in the viewports' order (the alphabet's): the label at index `n` is viewport
    /// `n`'s.
    #[must_use]
    pub fn labels(&self) -> Vec<char> {
        let mut labels: Vec<char> = self.grid.iter().flatten().copied().collect();
        labels.sort_unstable();
        labels.dedup();
        labels
    }

    /// How many viewports there are: at least 1, at most [`MAX_VIEWPORTS`].
    #[must_use]
    pub fn count(&self) -> usize {
        self.labels().len()
    }

    /// The number (from 0) of the viewport labelled `label`, if there is one.
    #[must_use]
    pub fn number(&self, label: char) -> Option<usize> {
        self.labels().iter().position(|l| *l == label)
    }

    /// The label of viewport `number` (from 0), if there is one.
    #[must_use]
    pub fn label(&self, number: usize) -> Option<char> {
        self.labels().get(number).copied()
    }

    /// Each column's weight, left to right.
    #[must_use]
    pub fn column_weights(&self) -> &[f64] {
        &self.columns
    }

    /// Each row's weight, top to bottom.
    #[must_use]
    pub fn row_weights(&self) -> &[f64] {
        &self.rows
    }

    /// The bounding box of a label's cells (the label must be present).
    fn bounds(&self, label: char) -> Span {
        let (mut c0, mut c1, mut r0, mut r1) = (usize::MAX, 0, usize::MAX, 0);
        for (r, row) in self.grid.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if *cell == label {
                    (c0, c1, r0, r1) = (c0.min(c), c1.max(c), r0.min(r), r1.max(r));
                }
            }
        }
        Span { column: c0, columns: c1 - c0 + 1, row: r0, rows: r1 - r0 + 1 }
    }

    /// The cells `label` covers, if it is one of the mosaic's.
    #[must_use]
    pub fn span(&self, label: char) -> Option<Span> {
        self.number(label).map(|_| self.bounds(label))
    }

    /// The share of the figure `label` takes, the splitters' gaps apart: `[x, y, width, height]`
    /// as fractions of the figure's width and height from its top left corner, each a sum of
    /// weights over the sum of all (what `subplot_mosaic` gives with `width_ratios` and
    /// `height_ratios`).
    #[must_use]
    pub fn share(&self, label: char) -> Option<[f64; 4]> {
        let span = self.span(label)?;
        let part = |weights: &[f64], from: usize, count: usize| {
            let total: f64 = weights.iter().sum();
            let before: f64 = weights[..from].iter().sum();
            let within: f64 = weights[from..from + count].iter().sum();
            (before / total, within / total)
        };
        let (x, width) = part(&self.columns, span.column, span.columns);
        let (y, height) = part(&self.rows, span.row, span.rows);
        Some([x, y, width, height])
    }

    /// Each viewport's rectangle in a figure of `size` pixels (`[width, height]`) with `gap`
    /// pixels of splitter between neighbouring columns and rows: `[x, y, width, height]` in
    /// whole pixels from the figure's top left corner, in the viewports' order. The rectangles
    /// tile the figure less the gaps; a viewport that spans tracks covers the gaps between them.
    /// The rounding is `cav_render::pane_rects`'s, so `A`, `AB` and `AB;CD` give its rectangles
    /// for 1, 2 and 4 panes to the pixel.
    #[must_use]
    pub fn rects(&self, size: [u32; 2], gap: u32) -> Vec<[u32; 4]> {
        let across = tracks(&self.columns, size[0], gap);
        let down = tracks(&self.rows, size[1], gap);
        let along = |tracks: &[(u32, u32)], from: usize, count: usize| {
            let (start, _) = tracks[from];
            let (last, length) = tracks[from + count - 1];
            (start, last + length - start)
        };
        self.labels()
            .into_iter()
            .map(|label| {
                let span = self.bounds(label);
                let (x, width) = along(&across, span.column, span.columns);
                let (y, height) = along(&down, span.row, span.rows);
                [x, y, width, height]
            })
            .collect()
    }

    /// The splitters: every stretch of gap between two different viewports, with the boundary
    /// it moves, as `[x, y, width, height]` in the pixels of [`Self::rects`] (same `size` and
    /// `gap`). Where a viewport spans a boundary there is no splitter.
    #[must_use]
    pub fn splitters(&self, size: [u32; 2], gap: u32) -> Vec<(Boundary, [u32; 4])> {
        let across = tracks(&self.columns, size[0], gap);
        let down = tracks(&self.rows, size[1], gap);
        let mut out = Vec::new();
        for c in 0..self.columns.len() - 1 {
            let x = across[c].0 + across[c].1;
            let width = across[c + 1].0 - x;
            // Rows that follow one another with the same two neighbours are one stretch.
            let mut r = 0;
            while r < self.rows.len() {
                let pair = (self.grid[r][c], self.grid[r][c + 1]);
                let mut end = r + 1;
                while end < self.rows.len() && (self.grid[end][c], self.grid[end][c + 1]) == pair {
                    end += 1;
                }
                if pair.0 != pair.1 {
                    let y = down[r].0;
                    out.push((Boundary::Column(c), [x, y, width, down[end - 1].0 + down[end - 1].1 - y]));
                }
                r = end;
            }
        }
        for r in 0..self.rows.len() - 1 {
            let y = down[r].0 + down[r].1;
            let height = down[r + 1].0 - y;
            let mut c = 0;
            while c < self.columns.len() {
                let pair = (self.grid[r][c], self.grid[r + 1][c]);
                let mut end = c + 1;
                while end < self.columns.len() && (self.grid[r][end], self.grid[r + 1][end]) == pair {
                    end += 1;
                }
                if pair.0 != pair.1 {
                    let x = across[c].0;
                    out.push((Boundary::Row(r), [x, y, across[end - 1].0 + across[end - 1].1 - x, height]));
                }
                c = end;
            }
        }
        out
    }

    /// The same mosaic with rows and columns exchanged: what turns a split to the right into a
    /// split downward.
    fn transposed(&self) -> Self {
        let grid = (0..self.columns.len()).map(|c| self.grid.iter().map(|row| row[c]).collect()).collect();
        Self { grid, columns: self.rows.clone(), rows: self.columns.clone() }
    }

    /// Why there is no viewport `label`.
    fn missing(&self, label: char) -> String {
        let labels: Vec<String> = self.labels().iter().map(char::to_string).collect();
        format!("there is no viewport `{label}`; the mosaic has {}", labels.join(", "))
    }

    /// The letter a new viewport takes: the one after the highest in use, so that the viewports
    /// already there keep their numbers (a viewport's number is its label's place in the
    /// alphabet: a new letter before the others would renumber them all; S20's review, minor 10).
    /// Past `Z`, the first letter no viewport has.
    fn unused(&self) -> Result<char, String> {
        let labels = self.labels();
        if labels.len() >= MAX_VIEWPORTS {
            return Err(format!(
                "a figure holds at most {MAX_VIEWPORTS} viewports: close one before splitting another"
            ));
        }
        let after = labels.last().and_then(|l| char::from_u32(u32::from(*l) + 1)).filter(|l| *l <= 'Z');
        Ok(after.unwrap_or_else(|| {
            ('A'..='Z').find(|l| !labels.contains(l)).expect("fewer than 26 labels are in use")
        }))
    }

    /// Splits viewport `label` into a left and a right half: `label` keeps the left, and a new
    /// viewport with the next letter (after the highest in use) takes the right. A viewport that spans several
    /// columns is cut at the column boundary nearest its middle; one that spans a single column
    /// has that column made two, each of half its weight (so the widths of everything else are
    /// unchanged, and the sum of the weights is kept). Answers with the new mosaic and the new
    /// viewport's label.
    ///
    /// # Errors
    /// When there is no such viewport, or the figure already holds [`MAX_VIEWPORTS`].
    pub fn split_right(&self, label: char) -> Result<(Self, char), String> {
        let span = self.span(label).ok_or_else(|| self.missing(label))?;
        let new = self.unused()?;
        let mut next = self.clone();
        let rows = span.row..span.row + span.rows;
        if span.columns >= 2 {
            let weights = &self.columns[span.column..span.column + span.columns];
            let half = weights.iter().sum::<f64>() / 2.0;
            // The boundary inside the span whose weight to its left is nearest half.
            let mut cumulative = 0.0;
            let (mut best, mut least) = (1, f64::INFINITY);
            for (k, weight) in weights[..weights.len() - 1].iter().enumerate() {
                cumulative += weight;
                if (cumulative - half).abs() < least {
                    (best, least) = (k + 1, (cumulative - half).abs());
                }
            }
            for r in rows {
                next.grid[r][span.column + best..span.column + span.columns].fill(new);
            }
        } else {
            if self.columns.len() >= MAX_TRACKS {
                return Err(format!("a mosaic has at most {MAX_TRACKS} columns"));
            }
            let c = span.column;
            for (r, row) in next.grid.iter_mut().enumerate() {
                let cell = if rows.contains(&r) { new } else { row[c] };
                row.insert(c + 1, cell);
            }
            let half = self.columns[c] / 2.0;
            next.columns[c] = half;
            next.columns.insert(c + 1, self.columns[c] - half);
        }
        Ok((next, new))
    }

    /// As [`Self::split_right`], top and bottom: `label` keeps the top, the new viewport takes
    /// the bottom.
    ///
    /// # Errors
    /// As [`Self::split_right`].
    pub fn split_down(&self, label: char) -> Result<(Self, char), String> {
        let (next, new) = self.transposed().split_right(label)?;
        Ok((next.transposed(), new))
    }

    /// Closes viewport `label`: a neighbour that shares one whole edge with it grows into its
    /// place (the one to its left if there is one, else the one above, to the right, below; so
    /// closing what a split to the right made gives back what was split). Columns, and rows, that have become
    /// the same as their neighbour are then made one, their weights summed.
    ///
    /// # Errors
    /// When there is no such viewport, it is the last one, or no neighbour shares a whole edge
    /// with it (nothing could take its place and stay a rectangle).
    pub fn close(&self, label: char) -> Result<Self, String> {
        let span = self.span(label).ok_or_else(|| self.missing(label))?;
        if self.count() == 1 {
            return Err("the last viewport cannot be closed".into());
        }
        let (c0, c1) = (span.column, span.column + span.columns - 1);
        let (r0, r1) = (span.row, span.row + span.rows - 1);
        let same_rows = |n: Span| n.row == span.row && n.rows == span.rows;
        let same_columns = |n: Span| n.column == span.column && n.columns == span.columns;
        let left = (c0 > 0).then(|| self.grid[r0][c0 - 1]).filter(|n| same_rows(self.bounds(*n)));
        let above = (r0 > 0).then(|| self.grid[r0 - 1][c0]).filter(|n| same_columns(self.bounds(*n)));
        let right = (c1 + 1 < self.columns.len())
            .then(|| self.grid[r0][c1 + 1])
            .filter(|n| same_rows(self.bounds(*n)));
        let below = (r1 + 1 < self.rows.len())
            .then(|| self.grid[r1 + 1][c0])
            .filter(|n| same_columns(self.bounds(*n)));
        let Some(heir) = left.or(above).or(right).or(below) else {
            return Err(format!(
                "`{label}` cannot be closed: no neighbour shares a whole edge with it, so none can take its \
                 place and stay a rectangle (swap it with one that can, or write the mosaic)"
            ));
        };
        let mut next = self.clone();
        for cell in next.grid.iter_mut().flatten() {
            if *cell == label {
                *cell = heir;
            }
        }
        Ok(next.merged().transposed().merged().transposed())
    }

    /// The same mosaic with each run of identical neighbouring columns made one column of their
    /// summed weight.
    fn merged(mut self) -> Self {
        let mut c = 0;
        while c + 1 < self.columns.len() {
            if self.grid.iter().all(|row| row[c] == row[c + 1]) {
                for row in &mut self.grid {
                    row.remove(c + 1);
                }
                let weight = self.columns.remove(c + 1);
                self.columns[c] += weight;
            } else {
                c += 1;
            }
        }
        self
    }

    /// Exchanges the places of viewports `a` and `b`: their labels in the grid. Each keeps its
    /// label and so its number, and whatever is carried with the label (its camera, what it
    /// shows); only where it stands changes.
    ///
    /// # Errors
    /// When either is not a viewport of the mosaic.
    pub fn swap(&self, a: char, b: char) -> Result<Self, String> {
        for label in [a, b] {
            if self.number(label).is_none() {
                return Err(self.missing(label));
            }
        }
        let mut next = self.clone();
        for cell in next.grid.iter_mut().flatten() {
            *cell = match *cell {
                l if l == a => b,
                l if l == b => a,
                l => l,
            };
        }
        Ok(next)
    }

    /// The two weights either side of a boundary.
    fn pair(&mut self, boundary: Boundary) -> Result<(&mut [f64], usize), String> {
        let (weights, n, what) = match boundary {
            Boundary::Column(n) => (&mut self.columns, n, "column"),
            Boundary::Row(n) => (&mut self.rows, n, "row"),
        };
        if n + 1 >= weights.len() {
            return Err(format!(
                "there is no {what} boundary {}; the mosaic has {} {what}{}",
                n + 1,
                weights.len(),
                if weights.len() == 1 { "" } else { "s" }
            ));
        }
        Ok((weights, n))
    }

    /// Moves a boundary: the weight before it grows by `delta` (in the weights' own unit) and
    /// the weight after it shrinks by as much, so their sum, and every other track, is as it
    /// was.
    ///
    /// # Errors
    /// When there is no such boundary, or the move would leave either side nothing (a weight
    /// must stay positive).
    pub fn resize(&self, boundary: Boundary, delta: f64) -> Result<Self, String> {
        let mut next = self.clone();
        let (weights, n) = next.pair(boundary)?;
        let sum = weights[n] + weights[n + 1];
        let before = weights[n] + delta;
        let after = sum - before;
        if !(before.is_finite() && before > 0.0 && after > 0.0) {
            return Err(format!("a move of {delta} would leave one side of the boundary nothing"));
        }
        (weights[n], weights[n + 1]) = (before, after);
        Ok(next)
    }

    /// Evens the two sides of a boundary: each takes half their sum (a double click on a
    /// splitter).
    ///
    /// # Errors
    /// When there is no such boundary.
    pub fn even_at(&self, boundary: Boundary) -> Result<Self, String> {
        let mut next = self.clone();
        let (weights, n) = next.pair(boundary)?;
        let half = (weights[n] + weights[n + 1]) / 2.0;
        (weights[n], weights[n + 1]) = (half, half);
        Ok(next)
    }

    /// The same grid with every weight 1.
    #[must_use]
    pub fn even(&self) -> Self {
        Self {
            grid: self.grid.clone(),
            columns: vec![1.0; self.columns.len()],
            rows: vec![1.0; self.rows.len()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cav_render::pane_rects` as S19 had it, written out: 1, 2 or 4 panes.
    fn s19_pane_rects(panes: usize, [width, height]: [u32; 2], gap: u32) -> Vec<[u32; 4]> {
        let split = |extent: u32| {
            let gap = gap.min(extent);
            let first = (extent - gap) / 2;
            (first, first + gap, extent - gap - first)
        };
        let (left, x1, right) = split(width);
        let (top, y1, bottom) = split(height);
        match panes {
            1 => vec![[0, 0, width, height]],
            2 => vec![[0, 0, left, height], [x1, 0, right, height]],
            _ => vec![[0, 0, left, top], [x1, 0, right, top], [0, y1, left, bottom], [x1, y1, right, bottom]],
        }
    }

    #[test]
    fn a_mosaic_is_the_rectangles_subplot_mosaic_gives() {
        // M1. Criterion (written before measuring): `AAB;CDB` with weights gives each label's
        // cell span and its share of the figure exact in f64; `A`, `AB`, `AB;CD` give S19's pane
        // rectangles to the pixel at every size and gap tried. Controls: an L-shaped label, an
        // empty row, ragged rows, a 13th label and an unknown view type are each refused in words.
        let mosaic = Mosaic::parse("\"AAB;CDB\" cols 2 1 1 rows 1 3").unwrap();
        assert_eq!(mosaic.labels(), ['A', 'B', 'C', 'D']);
        assert_eq!(mosaic.span('A'), Some(Span { column: 0, columns: 2, row: 0, rows: 1 }));
        assert_eq!(mosaic.span('B'), Some(Span { column: 2, columns: 1, row: 0, rows: 2 }));
        assert_eq!(mosaic.span('C'), Some(Span { column: 0, columns: 1, row: 1, rows: 1 }));
        assert_eq!(mosaic.span('D'), Some(Span { column: 1, columns: 1, row: 1, rows: 1 }));
        assert_eq!(mosaic.span('E'), None);
        // What subplot_mosaic gives with width_ratios [2, 1, 1] and height_ratios [1, 3].
        assert_eq!(mosaic.share('A'), Some([0.0, 0.0, 0.75, 0.25]));
        assert_eq!(mosaic.share('B'), Some([0.75, 0.0, 0.25, 1.0]));
        assert_eq!(mosaic.share('C'), Some([0.0, 0.25, 0.5, 0.75]));
        assert_eq!(mosaic.share('D'), Some([0.5, 0.25, 0.25, 0.75]));
        // In pixels with no gap the rectangles are those shares of the figure.
        assert_eq!(
            mosaic.rects([400, 200], 0),
            [[0, 0, 300, 50], [300, 0, 100, 200], [0, 50, 200, 150], [200, 50, 100, 150]]
        );
        // With a gap they tile the figure less the gaps: A covers the gap between its columns.
        let rects = mosaic.rects([404, 202], 2);
        assert_eq!(rects, [[0, 0, 302, 50], [304, 0, 100, 202], [0, 52, 200, 150], [202, 52, 100, 150]]);
        let area: u32 = rects.iter().map(|r| r[2] * r[3]).sum();
        let splitters = mosaic.splitters([404, 202], 2);
        let gaps: u32 = splitters.iter().map(|(_, r)| r[2] * r[3]).sum();
        // The viewports and the splitters between them are the figure, but for the two 2 × 2
        // crossings where a column's gap meets the row's (under A; and between A, D and B).
        assert_eq!(area + gaps + 2 * 4, 404 * 202, "{splitters:?}");
        assert_eq!(splitters.len(), 5, "C|D, A|B, D|B, A/C and A/D: {splitters:?}");
        assert!(splitters.contains(&(Boundary::Column(0), [200, 52, 2, 150])), "{splitters:?}");
        assert!(splitters.contains(&(Boundary::Row(0), [0, 50, 200, 2])), "{splitters:?}");
        // The written form reads back to the same mosaic, weights to the bit; so does JSON.
        let odd = Mosaic::parse("AB cols 0.1 0.30000000000000004").unwrap();
        assert_eq!(Mosaic::parse(&odd.to_string()).unwrap(), odd);
        assert_eq!(mosaic.to_string(), "AAB;CDB cols 2 1 1 rows 1 3");
        assert_eq!(mosaic.quoted(), "\"AAB;CDB\" cols 2 1 1 rows 1 3");
        assert_eq!(Mosaic::parse(&mosaic.quoted()).unwrap(), mosaic);
        assert_eq!(Mosaic::parse("AAB;CDB").unwrap().to_string(), "AAB;CDB");
        let json = serde_json::to_string(&mosaic).unwrap();
        assert_eq!(json, "\"AAB;CDB cols 2 1 1 rows 1 3\"");
        assert_eq!(serde_json::from_str::<Mosaic>(&json).unwrap(), mosaic);
        assert!(serde_json::from_str::<Mosaic>("\"AB;BA\"").is_err(), "JSON is checked as text is");

        // S19's panes, to the pixel: odd and even sizes, gaps none to larger than the figure.
        let mut compared = 0;
        for (panes, text) in [(1, "A"), (2, "AB"), (4, "AB;CD")] {
            assert_eq!(Mosaic::panes(panes), Some(Mosaic::parse(text).unwrap()));
            for width in [1, 2, 3, 7, 100, 101, 1279, 1440] {
                for height in [1, 2, 5, 80, 81, 900] {
                    for gap in [0, 1, 2, 3, 4, 9] {
                        assert_eq!(
                            Mosaic::parse(text).unwrap().rects([width, height], gap),
                            s19_pane_rects(panes, [width, height], gap),
                            "{text} at {width} × {height}, gap {gap}"
                        );
                        compared += 1;
                    }
                }
            }
        }
        assert_eq!(compared, 3 * 8 * 6 * 6);
        assert_eq!(Mosaic::panes(3), None);

        // Controls: each refused, in words that say what is wrong.
        let thirteen = "ABCD;EFGH;IJKL;MMMM";
        for (text, why) in [
            ("AB;BA", "not one rectangle"),
            ("AA;AB", "not one rectangle"),
            ("ABA", "not one rectangle"),
            ("AB;;CD", "row 2 of the mosaic is empty"),
            ("", "empty"),
            ("AAB;CD", "row 2 has 2 labels and row 1 has 3"),
            (thirteen, "at most 12 viewports; this mosaic has 13"),
            ("Ab", "`b` is not a label"),
            ("A1", "`1` is not a label"),
            ("AB cols 1", "`cols` takes 2 weights"),
            ("AB cols 1 0", "positive number, not `0`"),
            ("AB cols 1 nan", "positive number, not `nan`"),
            ("AB wide", "`wide`"),
            ("AB rows 1 rows 1", "twice"),
        ] {
            let refused = Mosaic::parse(text).expect_err(text);
            assert!(refused.contains(why), "`{text}`: `{refused}` should say `{why}`");
        }
        // Twelve are held.
        assert_eq!(Mosaic::parse("ABCD;EFGH;IJKL").unwrap().count(), MAX_VIEWPORTS);
        // And a viewport shows one of the view types: a word that names none is no content.
        assert_eq!(crate::PaneContent::parse("periodogram"), Some(crate::PaneContent::Periodogram));
        assert_eq!(crate::PaneContent::parse("teapot"), None);
    }

    #[test]
    fn split_close_and_swap_keep_a_mosaic() {
        // M2. Criterion: split right of B in `AAB;CDB` then close of the new viewport gives the
        // start back; every result parses from its own text; the weights' sums are kept to
        // 1e-12. Control: closing a viewport no neighbour can fill is refused.
        let start = Mosaic::parse("AAB;CDB cols 2 1 1.5 rows 1 3").unwrap();
        let sums = |m: &Mosaic| (m.column_weights().iter().sum::<f64>(), m.row_weights().iter().sum::<f64>());
        let holds = |m: &Mosaic, what: &str| {
            assert_eq!(&Mosaic::parse(&m.to_string()).unwrap(), m, "{what}: {m}");
            let ((columns, rows), (c0, r0)) = (sums(m), sums(&start));
            assert!((columns - c0).abs() <= 1e-12 && (rows - r0).abs() <= 1e-12, "{what}: {m}");
        };
        let (split, new) = start.split_right('B').unwrap();
        assert_eq!((split.to_string().as_str(), new), ("AABE;CDBE cols 2 1 0.75 0.75 rows 1 3", 'E'));
        holds(&split, "split right");
        assert_eq!(split.close('E').unwrap(), start, "closing what the split made gives the start back");
        // Downward: B keeps the top, the new one the bottom; its two rows are already there.
        let (down, new) = start.split_down('B').unwrap();
        assert_eq!((down.to_string().as_str(), new), ("AAB;CDE cols 2 1 1.5 rows 1 3", 'E'));
        holds(&down, "split down");
        // Closing it: its left neighbour D shares its whole edge and comes first.
        assert_eq!(down.close('E').unwrap().grid_text(), "AAB;CDD");
        // A spanning viewport is cut where it already has a boundary; a single cell adds a track.
        let (wide, _) = start.split_right('A').unwrap();
        assert_eq!(wide.grid_text(), "AEB;CDB");
        holds(&wide, "split a span");
        let (tall, new) = start.split_down('C').unwrap();
        assert_eq!((tall.to_string().as_str(), new), ("AAB;CDB;EDB cols 2 1 1.5 rows 1 1.5 1.5", 'E'));
        holds(&tall, "split a cell down");
        assert_eq!(tall.close('E').unwrap(), start);
        // The new label is the letter after the highest in use (S20's review, minor 10: the first
        // unused letter, B here, would come before C and renumber it), so every viewport there
        // keeps its number; past Z, the first unused letter.
        let gappy = Mosaic::parse("AC").unwrap();
        let (split, new) = gappy.split_right('A').unwrap();
        assert_eq!((split.grid_text().as_str(), new), ("ADC", 'D'));
        assert_eq!((split.number('A'), split.number('C')), (gappy.number('A'), gappy.number('C')));
        let (split, new) = Mosaic::parse("AZ").unwrap().split_right('Z').unwrap();
        assert_eq!((split.grid_text().as_str(), new), ("AZB", 'B'), "past Z: the first free letter");
        // Closing: the left neighbour first, then above, right, below.
        assert_eq!(Mosaic::parse("AB;CD").unwrap().close('D').unwrap().grid_text(), "AB;CC");
        assert_eq!(Mosaic::parse("AB;CD").unwrap().close('A').unwrap().grid_text(), "BB;CD");
        let merged = Mosaic::parse("ABC cols 1 2 4").unwrap().close('B').unwrap();
        assert_eq!(merged.to_string(), "AC cols 3 4", "A grew into B, and the two columns are one");
        // Swap: the labels change places; each keeps its number.
        let swapped = start.swap('A', 'D').unwrap();
        assert_eq!(swapped.grid_text(), "DDB;CAB");
        assert_eq!(swapped.labels(), start.labels());
        assert_eq!(swapped.number('D'), Some(3), "D is still the fourth viewport, now at the top");
        holds(&swapped, "swap");
        assert_eq!(swapped.swap('D', 'A').unwrap(), start);
        // Resize and even: two weights change, their sum kept.
        let resized = start.resize(Boundary::Column(1), 0.3).unwrap();
        assert_eq!(resized.column_weights(), [2.0, 1.3, 1.2]);
        holds(&resized, "resize");
        let evened = resized.even_at(Boundary::Column(1)).unwrap();
        assert_eq!(evened.column_weights(), [2.0, 1.25, 1.25]);
        holds(&evened, "even at");
        assert_eq!(start.even().to_string(), "AAB;CDB");
        // The cap: the twelfth viewport is made, the thirteenth refused with the number.
        let mut grown = Mosaic::single();
        for _ in 1..MAX_VIEWPORTS {
            grown = grown.split_right('A').unwrap().0;
        }
        assert_eq!(grown.count(), 12);
        assert!(grown.split_right('A').unwrap_err().contains("at most 12"));
        assert!(grown.split_down('A').unwrap_err().contains("at most 12"));

        // Controls: what cannot be done is refused in words, and the mosaic is not changed.
        let refused = start.close('A').unwrap_err();
        assert!(refused.contains("no neighbour shares a whole edge"), "{refused}");
        assert!(Mosaic::parse("AB;CC").unwrap().close('C').unwrap_err().contains("whole edge"));
        assert!(Mosaic::single().close('A').unwrap_err().contains("last viewport"));
        assert!(start.close('Z').unwrap_err().contains("no viewport `Z`"));
        assert!(start.swap('A', 'Z').unwrap_err().contains("no viewport `Z`"));
        assert!(start.split_right('Z').unwrap_err().contains("no viewport `Z`"));
        assert!(start.resize(Boundary::Column(2), 0.1).unwrap_err().contains("no column boundary 3"));
        assert!(start.resize(Boundary::Row(0), 3.0).unwrap_err().contains("nothing"));
        assert!(start.resize(Boundary::Row(0), f64::NAN).is_err());
    }
}
