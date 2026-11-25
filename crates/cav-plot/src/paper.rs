//! The scene as a technical drawing on paper: a plan from above or a vertical section, ink on
//! cream, orthographic, at a stated scale 1:N.
//!
//! What is drawn ([`drawing`]):
//! - the world's matter as ink outlines of its occupancy at the drawing's resolution: a brick
//!   level ([`cav_core::voxel::world::Brick`]) whose cells are at least [`CELL_MM`] on paper, so
//!   a square kilometre at 5 cm is drawn from a few hundred coarse bricks and never from its
//!   10¹⁰ cells. In plan, the outline of each height band of the top surface (the edge of the
//!   matter heavier, a change of band lighter); in section, the cut matter outlined and hatched
//!   at 45°, the hatching clipped to it;
//! - the towers (in plan a ring each, in section a line from base to top) with their labels, the
//!   bodies as dots where they are at the playhead;
//! - a scale bar of a round length, a north arrow (in a section, the direction north lies in),
//!   dimension lines (the world's extent; the towers' spacing), and a title block (the scene, the
//!   view, the scale, the cell drawn, the engine's version and revision; never the time of day,
//!   so the same command writes the same bytes).
//!
//! Bounds: at most [`MAX_PAPER_BRICKS`] bricks are read, the display list holds at most
//! [`MAX_PAPER_ITEMS`] items and the world's lines at most [`MAX_PAPER_POINTS`] points. Past any
//! of them, the drawing is made from a coarser brick level, and says so; were the coarsest still
//! over, the world's lines past the cap are left out, and it says that too. (A bound by
//! construction: [`drawing_within`] holds the list to its cap whatever the world.)
//!
//! Frames: positions are metres in the world's frame (x east, y north, z up). On the page, `u`
//! runs right and `v` up: a plan has `(u, v) = (x, y)`; a section at x = m looks west, `(u, v) =
//! (y, z)`; a section at y = m looks north, `(u, v) = (x, z)`. A drawing is in points, y down.

use std::collections::BTreeMap;
use std::ops::Range;

use cav_core::Vec3;
use cav_core::voxel::world::{BRICK_SIDE, Brick, ChunkKey};
use cav_core::voxel::{Aabb, Cover};
use cav_engine::PaperView;
use cav_sim::SceneSpec;
use cav_sim::world::World;

use crate::drawing::{Anchor, Colour, Drawing, Face, Item, Measure, text_box};
use crate::style::{MM_PER_POINT, Style};

/// The most items a paper drawing's display list holds.
pub const MAX_PAPER_ITEMS: usize = 20_000;

/// The most points the world's lines in a paper drawing hold, all told (an SVG of them is a few
/// megabytes).
pub const MAX_PAPER_POINTS: usize = 200_000;

/// The most bricks a paper drawing reads from the world.
pub const MAX_PAPER_BRICKS: usize = 256;

/// The coarsest brick level a paper drawing goes to: cells of `2^24` finest cells.
pub const MAX_PAPER_LEVEL: u8 = 24;

/// The smallest cell drawn, millimetres on paper: the finest level whose cells are at least this.
pub const CELL_MM: f64 = 1.0;

/// The most towers labelled; past it, the towers are drawn and not named.
pub const MAX_TOWER_LABELS: usize = 64;

/// The paper: the renderer's paper style's cream.
pub const PAPER: Colour = Colour(cav_render::PAPER_SRGB);

/// The ink: the renderer's paper style's.
pub const INK: Colour = Colour::hex(0x2b_2620);

/// What a paper drawing drew, for its record and its tests.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    /// The view drawn.
    pub view: PaperView,
    /// The scale's N: one metre on paper is `1/N` metres of the scene.
    pub scale: f64,
    /// Points on the page a metre of the scene.
    pub points_per_metre: f64,
    /// The point of the scene, `(u, v)` metres, at [`Self::centre_page`].
    pub centre: [f64; 2],
    /// Where [`Self::centre`] is on the page, points.
    pub centre_page: [f64; 2],
    /// The brick level the world is drawn at, if there is matter in the world.
    pub level: Option<u8>,
    /// The side of a cell at that level, metres.
    pub cell: Option<f64>,
    /// The level whose cells are [`CELL_MM`] on paper, before any coarsening for the bounds.
    pub finest_level: Option<u8>,
    /// Bricks read at the level drawn.
    pub bricks: usize,
    /// Which of the drawing's items are the world's outlines.
    pub outline_items: Range<usize>,
    /// Which are its hatching.
    pub hatch_items: Range<usize>,
    /// Each dimension line drawn: its label and its length, metres.
    pub dimensions: Vec<(String, f64)>,
    /// The scale bar's length, metres.
    pub scale_bar: f64,
    /// The north marker: `arrow up` (plan), `arrow right` (a section looking west) or `into the
    /// page` (a section looking north).
    pub north: &'static str,
    /// The title block's lines, top to bottom.
    pub title_block: Vec<String>,
    /// What the drawing left out or changed, in sentences.
    pub notes: Vec<String>,
}

/// The engine's build as the title block writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Revision {
    /// The engine's version.
    pub version: String,
    /// The commit it was built from.
    pub revision: String,
    /// Whether it had changes not committed, if known.
    pub dirty: Option<bool>,
}

/// The page's frame: where a scene point `(u, v)` goes, points.
#[derive(Clone, Copy)]
struct Frame {
    centre: [f64; 2],
    page: [f64; 2],
    scale: f64,
}

impl Frame {
    fn at(&self, u: f64, v: f64) -> [f32; 2] {
        [
            (self.page[0] + (u - self.centre[0]) * self.scale) as f32,
            (self.page[1] - (v - self.centre[1]) * self.scale) as f32,
        ]
    }
}

/// The scene's world, its matter's box and its brick grid's keys along the drawing's axes.
struct Matter {
    world: World,
    extent: Aabb,
}

/// The drawing's two axes in the world: which coordinates `u` and `v` are (0 x, 1 y, 2 z).
const fn axes(view: PaperView) -> (usize, usize) {
    match view {
        PaperView::Plan => (0, 1),
        PaperView::SectionX(_) => (1, 2),
        PaperView::SectionY(_) => (0, 2),
    }
}

fn component(p: [f64; 3], axis: usize) -> f64 {
    p[axis]
}

fn lo_hi(a: &Aabb, axis: usize) -> (f64, f64) {
    let (lo, hi) = (a.lo.to_array(), a.hi.to_array());
    (lo[axis], hi[axis])
}

/// The smallest scale's N, from 1, 1.25, 1.5, 2, 2.5, 3, 4, 5, 6 and 7.5 times a power of ten
/// (and a whole number), that is at least `least`.
#[must_use]
pub fn round_scale(least: f64) -> f64 {
    let least = least.max(1.0);
    let mut power = 10f64.powf(least.log10().floor() - 1.0).max(1.0);
    loop {
        for m in [1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 7.5] {
            let n = m * power;
            if n.fract() == 0.0 && n >= least * (1.0 - 1e-12) {
                return n;
            }
        }
        power *= 10.0;
    }
}

/// A length as a drawing writes it: metres, to three figures or so, `1000 m`, `12.5 m`, `0.25 m`.
#[must_use]
pub fn length_text(metres: f64) -> String {
    let size = metres.abs();
    let text = if size >= 100.0 {
        format!("{metres:.0}")
    } else if size >= 10.0 {
        format!("{metres:.1}")
    } else {
        format!("{metres:.2}")
    };
    let text =
        if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_owned() } else { text };
    format!("{text} m")
}

/// The brick level whose cells are at least `cell` metres, for a world of finest cells `pitch`.
fn level_for(pitch: f64, cell: f64) -> u8 {
    (0..=MAX_PAPER_LEVEL)
        .find(|l| pitch * f64::from(1u32 << l) >= cell * (1.0 - 1e-12))
        .unwrap_or(MAX_PAPER_LEVEL)
}

/// The keys of the bricks of `level` the drawing reads: the world's box, and for a section only
/// those the cut plane passes through. `None` when the cut misses the world.
fn keys_of(matter: &Matter, view: PaperView, level: u8) -> Option<(ChunkKey, ChunkKey)> {
    let grid = &matter.world.grid;
    let (mut lo, mut hi) = grid.bricks_of(level, &matter.extent);
    match view {
        PaperView::Plan => {}
        PaperView::SectionX(at) | PaperView::SectionY(at) => {
            let axis = if matches!(view, PaperView::SectionX(_)) { 0 } else { 1 };
            let (a, b) = lo_hi(&matter.extent, axis);
            if !(at >= a && at < b) {
                return None;
            }
            let side = grid.brick_side(level);
            let key = ((at - grid.origin.to_array()[axis]) / side).floor() as i32;
            if axis == 0 {
                (lo.x, hi.x) = (key, key);
            } else {
                (lo.y, hi.y) = (key, key);
            }
        }
    }
    Some((lo, hi))
}

fn count(lo: ChunkKey, hi: ChunkKey) -> usize {
    let side = |a: i32, b: i32| usize::try_from(i64::from(b) - i64::from(a) + 1).unwrap_or(0);
    side(lo.x, hi.x).saturating_mul(side(lo.y, hi.y)).saturating_mul(side(lo.z, hi.z))
}

/// The bricks of `level` from `lo` to `hi`, the empty ones left out.
///
/// Level 0 is each chunk voxelised; an error there (a chunk past its work) is handed back.
fn bricks(world: &World, level: u8, lo: ChunkKey, hi: ChunkKey) -> Result<Vec<Brick>, String> {
    let mut out = Vec::new();
    for z in lo.z..=hi.z {
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let key = ChunkKey::new(x, y, z);
                let brick = if level == 0 {
                    world.chunk(key)?.map(|chunk| Brick::of_chunk(&chunk))
                } else {
                    world.brick(level, key)
                };
                out.extend(brick);
            }
        }
    }
    Ok(out)
}

/// A grid of cells of the drawing: `n[0] × n[1]` cells from cell index `first` (global, along
/// `u` and `v`), each with a class (`None` empty).
struct Cells {
    first: [i64; 2],
    n: [usize; 2],
    class: Vec<Option<i64>>,
}

impl Cells {
    fn get(&self, i: i64, j: i64) -> Option<i64> {
        let (a, b) = (i - self.first[0], j - self.first[1]);
        if a < 0 || b < 0 || a >= self.n[0] as i64 || b >= self.n[1] as i64 {
            return None;
        }
        self.class[a as usize + self.n[0] * b as usize]
    }
}

/// The cells of the drawing at `level`: in plan, each column's height band (`band` metres) by
/// its highest occupied cell; in section, the cut's occupied cells (class 0).
///
/// A brick's cell is occupied when any piece touches it (conservative), which would draw a box
/// a cell larger on every side. So each such cell is asked again along its centre line: the
/// vertical segment through the middle of its footprint (in plan) or through its middle along
/// the drawing and the cut plane (in section), its whole height. An edge on a cell boundary is
/// then drawn where it is, any other within half a cell across the page, and a layer thinner
/// than a cell (the ground) is still drawn, a cell thick.
fn cells_of(
    view: PaperView,
    world: &World,
    level: u8,
    lo: ChunkKey,
    hi: ChunkKey,
    found: &[Brick],
    band: f64,
) -> Cells {
    let grid = &world.grid;
    let pieces = world.pieces();
    let side = i64::from(BRICK_SIDE);
    let cell = grid.brick_side(level) / f64::from(BRICK_SIDE);
    let origin = grid.origin.to_array();
    let lo = lo.to_array().map(i64::from);
    let hi = hi.to_array().map(i64::from);
    let (ua, va) = axes(view);
    let first = [lo[ua] * side, lo[va] * side];
    let n = [((hi[ua] - lo[ua] + 1) * side) as usize, ((hi[va] - lo[va] + 1) * side) as usize];
    let mut class = vec![None; n[0] * n[1]];
    let index = |i: i64, j: i64| (i - first[0]) as usize + n[0] * (j - first[1]) as usize;
    // The middle of global cell index `i` along `axis`, and its two faces.
    let middle = |axis: usize, i: i64| origin[axis] + (i as f64 + 0.5) * cell;
    let face = |axis: usize, i: i64| origin[axis] + i as f64 * cell;
    let touched = |near: &[cav_core::voxel::world::Piece<'_>], x: f64, y: f64, z: i64| {
        let segment = Aabb::new(Vec3::new(x, y, face(2, z)), Vec3::new(x, y, face(2, z + 1)));
        near.iter().any(|p| p.solid.cover(&segment) != Cover::Outside)
    };
    for brick in found {
        let key = brick.key.to_array().map(i64::from);
        let boxed = grid.brick_box(level, brick.key);
        let near: Vec<_> = pieces.iter().copied().filter(|p| p.solid.bounds().overlaps(&boxed)).collect();
        let global = |c: u32, axis: usize| key[axis] * side + i64::from(c);
        match view {
            PaperView::Plan => {
                for y in 0..BRICK_SIDE {
                    for x in 0..BRICK_SIDE {
                        let (gx, gy) = (global(x, 0), global(y, 1));
                        let (mx, my) = (middle(0, gx), middle(1, gy));
                        let Some(z) = (0..BRICK_SIDE)
                            .rev()
                            .find(|z| brick.get([x, y, *z]) && touched(&near, mx, my, global(*z, 2)))
                        else {
                            continue;
                        };
                        let top = origin[2] + (global(z, 2) + 1) as f64 * cell;
                        let band = (top / band).floor() as i64;
                        let at = &mut class[index(gx, gy)];
                        *at = Some(at.map_or(band, |b: i64| b.max(band)));
                    }
                }
            }
            PaperView::SectionX(at) | PaperView::SectionY(at) => {
                let across = if matches!(view, PaperView::SectionX(_)) { 0 } else { 1 };
                let k = (((at - face(across, key[across] * side)) / cell).floor() as i64).clamp(0, side - 1)
                    as u32;
                for a in 0..BRICK_SIDE {
                    for z in 0..BRICK_SIDE {
                        let c = if across == 0 { [k, a, z] } else { [a, k, z] };
                        let u = global(a, ua);
                        let (x, y) = if across == 0 { (at, middle(1, u)) } else { (middle(0, u), at) };
                        if brick.get(c) && touched(&near, x, y, global(z, 2)) {
                            class[index(u, global(z, 2))] = Some(0);
                        }
                    }
                }
            }
        }
    }
    Cells { first, n, class }
}

/// The edges between cells of different classes, merged into straight runs and chained into
/// polylines, in grid points (global cell corners): first the edges of the matter (empty on one
/// side), then the edges between two bands.
fn outlines(cells: &Cells) -> [Vec<Vec<[i64; 2]>>; 2] {
    let [i0, j0] = cells.first;
    let (i1, j1) = (i0 + cells.n[0] as i64, j0 + cells.n[1] as i64);
    let mut segments: [Vec<([i64; 2], [i64; 2])>; 2] = [Vec::new(), Vec::new()];
    let kind = |a: Option<i64>, b: Option<i64>| match (a, b) {
        (None, None) => None,
        (Some(a), Some(b)) if a == b => None,
        (Some(_), Some(_)) => Some(1),
        _ => Some(0),
    };
    // Along u: the edge at row line j between cells (i, j-1) and (i, j).
    for j in j0..=j1 {
        let mut run: Option<(usize, i64)> = None;
        for i in i0..=i1 {
            let here = if i < i1 { kind(cells.get(i, j - 1), cells.get(i, j)) } else { None };
            match (run, here) {
                (Some((k, start)), h) if h != Some(k) => {
                    segments[k].push(([start, j], [i, j]));
                    run = h.map(|h| (h, i));
                }
                (None, Some(h)) => run = Some((h, i)),
                _ => {}
            }
        }
    }
    // Along v: the edge at column line i between cells (i-1, j) and (i, j).
    for i in i0..=i1 {
        let mut run: Option<(usize, i64)> = None;
        for j in j0..=j1 {
            let here = if j < j1 { kind(cells.get(i - 1, j), cells.get(i, j)) } else { None };
            match (run, here) {
                (Some((k, start)), h) if h != Some(k) => {
                    segments[k].push(([i, start], [i, j]));
                    run = h.map(|h| (h, j));
                }
                (None, Some(h)) => run = Some((h, j)),
                _ => {}
            }
        }
    }
    segments.map(|s| chain(&s))
}

/// Segments joined end to end into polylines, in a fixed order.
fn chain(segments: &[([i64; 2], [i64; 2])]) -> Vec<Vec<[i64; 2]>> {
    let mut at: BTreeMap<[i64; 2], Vec<usize>> = BTreeMap::new();
    for (n, (a, b)) in segments.iter().enumerate() {
        at.entry(*a).or_default().push(n);
        at.entry(*b).or_default().push(n);
    }
    let mut used = vec![false; segments.len()];
    let next = |point: [i64; 2], used: &mut Vec<bool>| -> Option<[i64; 2]> {
        let n = *at.get(&point)?.iter().find(|n| !used[**n])?;
        used[n] = true;
        let (a, b) = segments[n];
        Some(if a == point { b } else { a })
    };
    let mut lines = Vec::new();
    for n in 0..segments.len() {
        if used[n] {
            continue;
        }
        used[n] = true;
        let (a, b) = segments[n];
        let mut line = vec![a, b];
        while let Some(p) = next(*line.last().expect("two points"), &mut used) {
            line.push(p);
        }
        let mut back = Vec::new();
        while let Some(p) = next(*back.last().unwrap_or(&a), &mut used) {
            back.push(p);
        }
        back.reverse();
        back.extend(line);
        lines.push(back);
    }
    lines
}

/// The 45° hatching of a section's occupied cells, `spacing` cells apart: each line `u − v = c`
/// (cell units, `c` a whole number of spacings) clipped to the cells, in pieces that join where
/// the line runs on through occupied cells. Each piece as its two ends, cell units.
fn hatching(cells: &Cells, spacing: f64) -> Vec<[[f64; 2]; 2]> {
    let mut done: Vec<[[f64; 2]; 2]> = Vec::new();
    // The pieces still open at the top of the last row, by line: (end, index in `done`).
    let mut open: BTreeMap<i64, (f64, usize)> = BTreeMap::new();
    let [i0, j0] = cells.first;
    for j in j0..j0 + cells.n[1] as i64 {
        let mut next: BTreeMap<i64, (f64, usize)> = BTreeMap::new();
        let mut i = i0;
        let i1 = i0 + cells.n[0] as i64;
        while i < i1 {
            if cells.get(i, j).is_none() {
                i += 1;
                continue;
            }
            let start = i;
            while i < i1 && cells.get(i, j).is_some() {
                i += 1;
            }
            let (a, b) = (start as f64, i as f64);
            // Lines crossing the strip of row j over [a, b): c from a − j − 1 to b − j.
            let k0 = ((a - (j + 1) as f64) / spacing).ceil() as i64;
            let k1 = ((b - j as f64) / spacing).floor() as i64;
            for k in k0..=k1 {
                let c = k as f64 * spacing;
                let (bottom, top) = (c + j as f64, c + (j + 1) as f64);
                let (xs, xe) = (bottom.max(a), top.min(b));
                if xe <= xs {
                    continue;
                }
                let zs = if xs == bottom { j as f64 } else { xs - c };
                let ze = if xe == top { (j + 1) as f64 } else { xe - c };
                let joined = (xs == bottom).then(|| open.get(&k)).flatten().filter(|(end, _)| *end == xs);
                let n = if let Some((_, n)) = joined {
                    done[*n][1] = [xe, ze];
                    *n
                } else {
                    done.push([[xs, zs], [xe, ze]]);
                    done.len() - 1
                };
                if xe == top {
                    next.insert(k, (xe, n));
                }
            }
        }
        open = next;
    }
    done
}

/// What the world contributes to a drawing at one level.
struct WorldLines {
    outlines: Vec<Item>,
    hatches: Vec<Item>,
    bricks: usize,
}

/// The points of a line item; none for any other.
fn points_of(item: &Item) -> usize {
    match item {
        Item::Line { points, .. } => points.len(),
        _ => 0,
    }
}

impl WorldLines {
    /// Whether they fit in `items` items and [`MAX_PAPER_POINTS`] points.
    fn within(&self, items: usize) -> bool {
        let points: usize = self.outlines.iter().chain(&self.hatches).map(points_of).sum();
        self.outlines.len() + self.hatches.len() <= items && points <= MAX_PAPER_POINTS
    }

    /// Keeps the lines, outlines first, while they fit in `items` items and
    /// [`MAX_PAPER_POINTS`] points; answers how many were left out.
    fn cut_to(&mut self, items: usize) -> usize {
        let (mut kept, mut points, mut left_out) = (0, 0, 0);
        for list in [&mut self.outlines, &mut self.hatches] {
            list.retain(|item| {
                let fits = kept < items && points + points_of(item) <= MAX_PAPER_POINTS;
                if fits {
                    kept += 1;
                    points += points_of(item);
                } else {
                    left_out += 1;
                }
                fits
            });
        }
        left_out
    }
}

fn world_lines(
    matter: &Matter,
    view: PaperView,
    level: u8,
    frame: &Frame,
    style: &Style,
    band: f64,
) -> Result<WorldLines, String> {
    let grid = &matter.world.grid;
    let Some((lo, hi)) = keys_of(matter, view, level) else {
        return Ok(WorldLines { outlines: Vec::new(), hatches: Vec::new(), bricks: 0 });
    };
    let found = bricks(&matter.world, level, lo, hi)?;
    let cells = cells_of(view, &matter.world, level, lo, hi, &found, band);
    let cell = grid.brick_side(level) / f64::from(BRICK_SIDE);
    let origin = grid.origin.to_array();
    let (ua, va) = axes(view);
    let to_page = |p: [f64; 2]| frame.at(origin[ua] + p[0] * cell, origin[va] + p[1] * cell);
    let [edges, bands] = outlines(&cells);
    let mut out = Vec::new();
    for (lines, width) in [(edges, 1.3 * style.line), (bands, 0.8 * style.line)] {
        for line in lines {
            out.push(Item::Line {
                points: line.iter().map(|p| to_page([p[0] as f64, p[1] as f64])).collect(),
                width,
                colour: INK,
                dash: Vec::new(),
            });
        }
    }
    let mut hatches = Vec::new();
    if view != PaperView::Plan {
        let spacing = f64::from(0.6 * style.text_size) / (cell * frame.scale);
        for [a, b] in hatching(&cells, spacing) {
            hatches.push(Item::Line {
                points: vec![to_page(a), to_page(b)],
                width: 0.45 * style.line,
                colour: INK,
                dash: Vec::new(),
            });
        }
    }
    Ok(WorldLines { outlines: out, hatches, bricks: count(lo, hi) })
}

/// A dimension line from `a` to `b` on the page (horizontal or vertical), its extension lines
/// from the feature each end measures (`from`: the page's y of each for a horizontal line, x for
/// a vertical one) to a little past the line, arrowheads, and its label beside it (above, or to
/// the left turned).
fn dimension(items: &mut Vec<Item>, a: [f32; 2], b: [f32; 2], from: [f32; 2], label: &str, style: &Style) {
    let w = 0.6 * style.line;
    let line = |points: Vec<[f32; 2]>| Item::Line { points, width: w, colour: INK, dash: Vec::new() };
    let vertical = (a[0] - b[0]).abs() < (a[1] - b[1]).abs();
    let t = style.text_size;
    let (clear, past) = (0.3 * t, 0.45 * t);
    // Extension lines: from just off the feature to a little past the dimension line.
    for (p, feature) in [(a, from[0]), (b, from[1])] {
        let across = if vertical { p[0] } else { p[1] };
        let toward = if across >= feature { 1.0 } else { -1.0 };
        let (start, end) = (feature + toward * clear, across + toward * past);
        let ext = if vertical { vec![[start, p[1]], [end, p[1]]] } else { vec![[p[0], start], [p[0], end]] };
        items.push(line(ext));
    }
    items.push(line(vec![a, b]));
    // Arrowheads at both ends, pointing out to the extension lines.
    let head = 0.55 * t;
    let half = 0.18 * t;
    for (tip, toward) in [(a, b), (b, a)] {
        let (dx, dy) = (toward[0] - tip[0], toward[1] - tip[1]);
        let length = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (ux, uy) = (dx / length, dy / length);
        let base = [tip[0] + ux * head.min(0.45 * length), tip[1] + uy * head.min(0.45 * length)];
        items.push(Item::Polygon {
            points: vec![
                tip,
                [base[0] - uy * half, base[1] + ux * half],
                [base[0] + uy * half, base[1] - ux * half],
            ],
            fill: INK,
        });
    }
    let middle = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
    items.push(Item::Text {
        at: if vertical { [middle[0] - 0.25 * t, middle[1]] } else { [middle[0], middle[1] - 0.25 * t] },
        anchor: Anchor(0.5, 1.0),
        text: label.to_owned(),
        face: Face::Mono,
        size: 0.9 * t,
        colour: INK,
        turned: vertical,
    });
}

/// The longest of 1, 2 or 5 times a power of ten that is at most `most` metres.
fn bar_length(most: f64) -> f64 {
    crate::voxels::scale_length(most.max(1e-9))
}

/// A paper drawing of `scene` (its world, towers and bodies at `time` seconds) of `points`
/// (width, height) in `style`'s sizes, ink on cream; titled when `title` is. At most
/// [`MAX_PAPER_ITEMS`] items.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn drawing(
    scene: &SceneSpec,
    time: f64,
    view: PaperView,
    style: &Style,
    points: [f32; 2],
    title: bool,
    revision: &Revision,
    measure: &dyn Measure,
) -> (Drawing, Drawn) {
    drawing_within(scene, time, view, style, points, title, revision, measure, MAX_PAPER_ITEMS)
}

/// [`drawing`] with its display list held to `cap` items instead of [`MAX_PAPER_ITEMS`]: for
/// showing the bound at work. With a cap smaller than the drawing's frame (the title block, the
/// scale bar and so on), the world is left out and the drawing says so.
#[must_use]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn drawing_within(
    scene: &SceneSpec,
    time: f64,
    view: PaperView,
    style: &Style,
    points: [f32; 2],
    title: bool,
    revision: &Revision,
    measure: &dyn Measure,
    cap: usize,
) -> (Drawing, Drawn) {
    let (ua, va) = axes(view);
    let mut notes = Vec::new();
    let matter = scene.world.as_ref().and_then(|spec| {
        let world = World::new(spec);
        world.extent().map(|extent| Matter { world, extent })
    });
    match (&scene.world, &matter) {
        (None, _) => notes.push("the scene has no world: its towers and bodies are drawn alone".to_owned()),
        (Some(_), None) => notes.push("the scene's world holds no matter".to_owned()),
        _ => {}
    }
    let gravity = scene.array.instrument.gravity;
    let towers: Vec<([f64; 3], f64, &str)> = (0..scene.array.towers.len())
        .map(|n| {
            let t = &scene.array.towers[n];
            (t.position, scene.array.instrument_of(n).tower_height, t.label.as_str())
        })
        .collect();
    let bodies: Vec<[f64; 3]> = scene
        .bodies
        .iter()
        .filter(|b| b.appears.is_none_or(|t| time >= t) && b.vanishes.is_none_or(|t| time < t))
        .map(|b| b.motion.position(time, gravity))
        .collect();

    // What the drawing must hold, in (u, v) metres.
    let mut extent = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    let mut include = |u: f64, v: f64| {
        if u.is_finite() && v.is_finite() {
            extent = (extent.0.min(u), extent.1.max(u), extent.2.min(v), extent.3.max(v));
        }
    };
    if let Some(m) = &matter {
        let (u0, u1) = lo_hi(&m.extent, ua);
        let (v0, v1) = lo_hi(&m.extent, va);
        include(u0, v0);
        include(u1, v1);
    }
    for (p, height, _) in &towers {
        include(component(*p, ua), component(*p, va));
        if view != PaperView::Plan {
            include(component(*p, ua), p[2] + height);
        }
    }
    for p in &bodies {
        include(component(*p, ua), component(*p, va));
    }
    if !extent.0.is_finite() {
        extent = (-1.0, 1.0, -1.0, 1.0);
    }
    let least = 1.0;
    let grow = |a: f64, b: f64| {
        let span = (b - a).max(least);
        let mid = 0.5 * (a + b);
        (mid - 0.52 * span, mid + 0.52 * span)
    };
    let (u0, u1) = grow(extent.0, extent.1);
    let (v0, v1) = grow(extent.2, extent.3);

    // The title block's lines, given the scale and what the matter row says.
    let view_text = match view {
        PaperView::Plan => "Plan, from above".to_owned(),
        PaperView::SectionX(at) => format!("Section at x = {}, looking west", length_text(at)),
        PaperView::SectionY(at) => format!("Section at y = {}, looking north", length_text(at)),
    };
    let (w, h) = (f64::from(points[0]), f64::from(points[1]));
    let short: String = revision.revision.chars().take(12).collect();
    let block_lines = |scale: f64, matter_row: String| {
        vec![
            scene.name.clone(),
            view_text.clone(),
            format!("Scale 1:{scale} on {:.0} × {:.0} mm", w * MM_PER_POINT, h * MM_PER_POINT),
            matter_row,
            format!(
                "Cavendish {} · {short}{}",
                revision.version,
                if revision.dirty == Some(true) { " (modified)" } else { "" }
            ),
        ]
    };
    let t = style.text_size;
    let block_sizes = |n: usize| if n == 0 { style.title_size } else { 0.9 * t };
    let widest = |lines: &[String]| {
        lines
            .iter()
            .enumerate()
            .map(|(n, text)| measure.size(text, Face::Sans, block_sizes(n))[0])
            .fold(0.0f32, f32::max)
    };

    // The page: the title at the top; the title block at the bottom right and the scale bar at
    // the bottom left; room for dimension lines round the drawing and for the north arrow. The
    // drawing goes above the title block or beside it, whichever gives it the larger scale.
    let margin = 1.2 * t;
    let top = if title { margin + 2.0 * style.title_size } else { margin };
    let row = 1.55 * t;
    let rows = 5.0;
    let block_height = rows * row + 0.4 * t;
    let dim_room = 2.8 * t;
    let bar_room = 3.2 * t;
    let north_room = 3.2 * t;
    let fit_of = |left: f64, right: f64, top: f64, bottom: f64| {
        let (aw, ah) = ((right - left).max(1.0), (bottom - top).max(1.0));
        ((aw / (u1 - u0)).min(ah / (v1 - v0)), [left, top, aw, ah])
    };
    let left = f64::from(margin + dim_room);
    let area_top = f64::from(top + dim_room);
    let points_per_mm = 1.0 / MM_PER_POINT;
    // Above the block: as wide as the page less the north arrow's room.
    let above = fit_of(
        left,
        w - f64::from(margin + north_room),
        area_top,
        h - f64::from(margin + block_height.max(bar_room) + 0.6 * t + dim_room),
    );
    // Beside it: as tall as the page less the scale bar's room, as wide as the block leaves.
    let estimate = widest(&block_lines(
        round_scale(1000.0 * points_per_mm / above.0),
        "Matter in cells of 000.00 m".into(),
    )) + 1.8 * t;
    let beside = fit_of(
        left,
        w - f64::from(margin + estimate + 0.6 * t),
        area_top,
        h - f64::from(margin + bar_room + dim_room),
    );
    let (fit, [area_left, area_top, aw, ah]) = if beside.0 > above.0 { beside } else { above };
    // A metre at 1:N is 1000/N mm: N at least what fits.
    let scale = round_scale(1000.0 * points_per_mm / fit);
    let ppm = 1000.0 * points_per_mm / scale;
    let frame = Frame {
        centre: [0.5 * (u0 + u1), 0.5 * (v0 + v1)],
        page: [area_left + 0.5 * aw, area_top + 0.5 * ah],
        scale: ppm,
    };

    // The frame of the drawing first, so its count is known when the world's level is chosen.
    let mut chrome: Vec<Item> = Vec::new();
    let mut dimensions = Vec::new();
    let line =
        |points: Vec<[f32; 2]>, width: f32| Item::Line { points, width, colour: INK, dash: Vec::new() };
    let (a, b) = (frame.at(extent.0, extent.2), frame.at(extent.1, extent.3));
    let (left_edge, top_edge, bottom_edge) = (a[0], b[1], a[1]);
    let gap = 1.6 * t;
    let shortest = 2.5 * t;
    let mut dimension_of =
        |chrome: &mut Vec<Item>, what: &str, a: [f32; 2], b: [f32; 2], from: [f32; 2], metres: f64| {
            let label = length_text(metres);
            if (a[0] - b[0]).abs().max((a[1] - b[1]).abs()) < shortest {
                notes.push(format!("{what} ({label}) is too short to dimension at 1:{scale}"));
                return;
            }
            dimension(chrome, a, b, from, &label, style);
            dimensions.push((label, metres));
        };
    // The world's extent along u, below; along v at the left (in plan).
    if let Some(m) = &matter {
        let (a, b) = lo_hi(&m.extent, ua);
        let (v_lo, _) = lo_hi(&m.extent, va);
        let y = bottom_edge + gap;
        let from = frame.at(0.0, v_lo)[1];
        let (pa, pb) = ([frame.at(a, 0.0)[0], y], [frame.at(b, 0.0)[0], y]);
        dimension_of(&mut chrome, "the world's extent", pa, pb, [from, from], b - a);
        if view == PaperView::Plan {
            let (a, b) = lo_hi(&m.extent, va);
            let (u_lo, _) = lo_hi(&m.extent, ua);
            let x = left_edge - gap;
            let from = frame.at(u_lo, 0.0)[0];
            let (pa, pb) = ([x, frame.at(0.0, b)[1]], [x, frame.at(0.0, a)[1]]);
            dimension_of(&mut chrome, "the world's extent", pa, pb, [from, from], b - a);
        }
    }
    // The towers' spacing along u, above: a chain when its labels fit, else the whole run.
    let ring = (0.7 * t).min(((ppm * 0.4) as f32).max(0.35 * t));
    let mut across: Vec<(f64, f32)> = towers
        .iter()
        .map(|(p, height, _)| {
            let top = if view == PaperView::Plan {
                frame.at(0.0, p[va])[1] - ring
            } else {
                frame.at(0.0, p[2] + height)[1]
            };
            (component(*p, ua), top)
        })
        .collect();
    across.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    across.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9);
    if across.len() >= 2 {
        let y = top_edge - gap;
        let fits = across.len() <= 12
            && across.windows(2).all(|pair| {
                let label = length_text(pair[1].0 - pair[0].0);
                let width = f64::from(measure.size(&label, Face::Mono, 0.9 * t)[0]);
                (pair[1].0 - pair[0].0) * ppm > width + f64::from(1.4 * t)
            });
        let pairs: Vec<((f64, f32), (f64, f32))> = if fits {
            across.windows(2).map(|pair| (pair[0], pair[1])).collect()
        } else {
            vec![(across[0], across[across.len() - 1])]
        };
        for ((a, from_a), (b, from_b)) in pairs {
            let (pa, pb) = ([frame.at(a, 0.0)[0], y], [frame.at(b, 0.0)[0], y]);
            dimension_of(&mut chrome, "the towers' spacing", pa, pb, [from_a, from_b], b - a);
        }
    }
    // Towers: a ring each in plan, a line base to top in a section; labels where they fit.
    let mut placed: Vec<([f32; 2], [f32; 2])> = Vec::new();
    let mut unlabelled = 0;
    for (n, (p, height, label)) in towers.iter().enumerate() {
        let at = frame.at(component(*p, ua), component(*p, va));
        let label_at = if view == PaperView::Plan {
            chrome.push(Item::Circle { centre: at, radius: ring, fill: INK });
            chrome.push(Item::Circle { centre: at, radius: ring - style.line, fill: PAPER });
            [at[0] + ring + 0.2 * t, at[1] - ring]
        } else {
            let top = frame.at(component(*p, ua), p[2] + height);
            chrome.push(line(vec![at, top], 1.6 * style.line));
            chrome.push(Item::Circle { centre: top, radius: 0.5 * ring, fill: INK });
            [top[0] + 0.3 * t, top[1] - 0.2 * t]
        };
        if n >= MAX_TOWER_LABELS {
            unlabelled += 1;
            continue;
        }
        let size = measure.size(label, Face::Sans, 0.85 * t);
        let boxed = text_box(label_at, Anchor::LEFT_BOTTOM, size, false);
        let overlaps = |(a, b): &([f32; 2], [f32; 2])| {
            a[0] < boxed.1[0] && boxed.0[0] < b[0] && a[1] < boxed.1[1] && boxed.0[1] < b[1]
        };
        if placed.iter().any(overlaps) {
            unlabelled += 1;
            continue;
        }
        placed.push(boxed);
        chrome.push(Item::Text {
            at: label_at,
            anchor: Anchor::LEFT_BOTTOM,
            text: (*label).to_owned(),
            face: Face::Sans,
            size: 0.85 * t,
            colour: INK,
            turned: false,
        });
    }
    if unlabelled > 0 {
        notes.push(format!(
            "{unlabelled} towers are drawn without their labels, which would overlap or are past {MAX_TOWER_LABELS}"
        ));
    }
    for p in &bodies {
        let centre = frame.at(component(*p, ua), component(*p, va));
        chrome.push(Item::Circle { centre, radius: style.dot, fill: INK });
    }
    // The north marker at the top right of the page, under the title's line.
    let nx = (w - f64::from(margin) - f64::from(1.1 * t)) as f32;
    let ny = top;
    let north = match view {
        PaperView::Plan => {
            let tip = [nx, ny + 1.6 * t];
            let base = ny + 4.2 * t;
            let notch = ny + 3.5 * t;
            chrome.push(Item::Polygon { points: vec![tip, [nx - 0.6 * t, base], [nx, notch]], fill: INK });
            chrome.push(line(vec![tip, [nx + 0.6 * t, base], [nx, notch], tip], 0.6 * style.line));
            chrome.push(Item::Text {
                at: [nx, ny + 1.4 * t],
                anchor: Anchor(0.5, 1.0),
                text: "N".into(),
                face: Face::Sans,
                size: style.title_size,
                colour: INK,
                turned: false,
            });
            "arrow up"
        }
        PaperView::SectionX(_) => {
            let y = ny + 2.6 * t;
            let (a, b) = (nx - 1.6 * t, nx + 0.2 * t);
            chrome.push(line(vec![[a, y], [b - 0.5 * t, y]], style.line));
            chrome.push(Item::Polygon {
                points: vec![[b, y], [b - 0.7 * t, y - 0.35 * t], [b - 0.7 * t, y + 0.35 * t]],
                fill: INK,
            });
            chrome.push(Item::Text {
                at: [nx - 0.7 * t, y - 0.5 * t],
                anchor: Anchor(0.5, 1.0),
                text: "N".into(),
                face: Face::Sans,
                size: style.title_size,
                colour: INK,
                turned: false,
            });
            "arrow right"
        }
        PaperView::SectionY(_) => {
            let c = [nx, ny + 2.8 * t];
            let r = 0.6 * t;
            chrome.push(Item::Circle { centre: c, radius: r, fill: INK });
            chrome.push(Item::Circle { centre: c, radius: r - style.line, fill: PAPER });
            let d = 0.7 * (r - style.line);
            chrome.push(line(vec![[c[0] - d, c[1] - d], [c[0] + d, c[1] + d]], style.line));
            chrome.push(line(vec![[c[0] - d, c[1] + d], [c[0] + d, c[1] - d]], style.line));
            chrome.push(Item::Text {
                at: [nx, c[1] - r - 0.2 * t],
                anchor: Anchor(0.5, 1.0),
                text: "N".into(),
                face: Face::Sans,
                size: style.title_size,
                colour: INK,
                turned: false,
            });
            "into the page"
        }
    };
    // The scale bar at the bottom left: four blocks, ink and paper in turn.
    let bar = bar_length(0.3 * aw / ppm);
    let bar_points = (bar * ppm) as f32;
    let bx = margin;
    let by = (h as f32) - margin - 1.6 * t;
    let bh = 0.45 * t;
    for k in 0..4 {
        let x = bx + bar_points * k as f32 / 4.0;
        chrome.push(Item::Rect {
            min: [x, by],
            max: [x + bar_points / 4.0, by + bh],
            fill: Some(if k % 2 == 0 { INK } else { PAPER }),
            stroke: Some((0.5 * style.line, INK)),
        });
    }
    for (x, text) in [(bx, "0".to_owned()), (bx + bar_points, length_text(bar))] {
        chrome.push(Item::Text {
            at: [x, by - 0.2 * t],
            anchor: Anchor(0.5, 1.0),
            text,
            face: Face::Mono,
            size: 0.9 * t,
            colour: INK,
            turned: false,
        });
    }

    // The world, at the finest level whose cells are a millimetre on paper, coarser while the
    // bricks, the items or the points are past their caps.
    let frame_items = chrome.len() + 2 + usize::from(title) + 4 + rows as usize * 2;
    let budget = cap.saturating_sub(frame_items);
    let mut world = WorldLines { outlines: Vec::new(), hatches: Vec::new(), bricks: 0 };
    let (mut level, mut finest) = (None, None);
    if let Some(m) = &matter {
        let pitch = m.world.grid.pitch;
        let first = level_for(pitch, CELL_MM * scale / 1000.0);
        finest = Some(first);
        let (z0, z1) = lo_hi(&m.extent, 2);
        let largest = (0..3)
            .map(|axis| {
                let (a, b) = lo_hi(&m.extent, axis);
                b - a
            })
            .fold(0.0, f64::max);
        let mut l = first;
        loop {
            let cell = pitch * f64::from(1u32 << l);
            let band = bar_length(((z1 - z0) / 6.0).max(cell) * 1.999).max(cell);
            let fits_bricks = keys_of(m, view, l).is_none_or(|(lo, hi)| count(lo, hi) <= MAX_PAPER_BRICKS);
            // No coarser than an eighth of the world: past that a cell's centre line may miss its matter.
            let last = l >= MAX_PAPER_LEVEL || cell * 8.0 >= largest;
            if fits_bricks {
                match world_lines(m, view, l, &frame, style, band) {
                    Ok(lines) if lines.within(budget) || last => {
                        world = lines;
                        level = Some(l);
                        break;
                    }
                    Ok(_) => {}
                    Err(why) => notes.push(format!("level {l} could not be made ({why})")),
                }
            }
            if last {
                break;
            }
            l += 1;
        }
        match level {
            None => notes.push(format!("the world could not be drawn within {MAX_PAPER_BRICKS} bricks at any level")),
            Some(l) if l > first => notes.push(format!(
                "drawn from cells of {} (level {l}) in place of {} (level {first}), to stay within {MAX_PAPER_BRICKS} bricks, {cap} items and {MAX_PAPER_POINTS} points",
                length_text(pitch * f64::from(1u32 << l)),
                length_text(pitch * f64::from(1u32 << first)),
            )),
            Some(_) => {}
        }
        let left_out = world.cut_to(budget);
        if left_out > 0 {
            notes.push(format!(
                "{left_out} of the world's lines are left out, past the cap of {cap} items or {MAX_PAPER_POINTS} points"
            ));
        }
        if level.is_some() && keys_of(m, view, level.unwrap_or(0)).is_none() {
            notes.push("the cut misses the world".to_owned());
        }
    }
    let cell = level.zip(matter.as_ref()).map(|(l, m)| m.world.grid.pitch * f64::from(1u32 << l));

    // The title block at the bottom right.
    let block = block_lines(
        scale,
        match (&scene.world, cell) {
            (None, _) => "No world: towers and bodies only".to_owned(),
            (Some(_), None) => "The world holds no matter".to_owned(),
            (Some(_), Some(c)) => format!("Matter in cells of {}", length_text(c)),
        },
    );
    let block_width = widest(&block) + 1.2 * t;
    let bmax = [w as f32 - margin, h as f32 - margin];
    let bmin = [bmax[0] - block_width, bmax[1] - block_height];
    let mut frame_list =
        vec![Item::Rect { min: bmin, max: bmax, fill: Some(PAPER), stroke: Some((style.line, INK)) }];
    for (n, text) in block.iter().enumerate() {
        let y = bmin[1] + 0.2 * t + row * n as f32;
        if n > 0 {
            frame_list.push(line(vec![[bmin[0], y], [bmax[0], y]], 0.5 * style.line));
        }
        frame_list.push(Item::Text {
            at: [bmin[0] + 0.6 * t, y + 0.5 * row],
            anchor: Anchor::LEFT_CENTRE,
            text: text.clone(),
            face: Face::Sans,
            size: block_sizes(n),
            colour: INK,
            turned: false,
        });
    }

    let name = match view {
        PaperView::Plan => format!("Plan · {}", scene.name),
        _ => format!("Section · {}", scene.name),
    };
    let mut drawing = Drawing::new(points, name.clone());
    drawing.items.push(Item::Rect { min: [0.0, 0.0], max: points, fill: Some(PAPER), stroke: None });
    if title {
        drawing.items.push(Item::Text {
            at: [margin, margin],
            anchor: Anchor::LEFT_TOP,
            text: name,
            face: Face::Sans,
            size: style.title_size,
            colour: INK,
            turned: false,
        });
    }
    let first_world = drawing.items.len();
    let hatch_items = first_world..first_world + world.hatches.len();
    drawing.items.extend(world.hatches);
    let outline_items = hatch_items.end..hatch_items.end + world.outlines.len();
    drawing.items.extend(world.outlines);
    drawing.items.extend(chrome);
    drawing.items.extend(frame_list);
    if drawing.items.len() > cap {
        // Only when the cap is smaller than the drawing's own frame.
        notes.push(format!("the drawing's frame alone is past the cap of {cap} items"));
    }
    let drawn = Drawn {
        view,
        scale,
        points_per_metre: ppm,
        centre: frame.centre,
        centre_page: frame.page,
        level,
        cell,
        finest_level: finest,
        bricks: world.bricks,
        outline_items,
        hatch_items,
        dimensions,
        scale_bar: bar,
        north,
        title_block: block,
        notes,
    };
    (drawing, drawn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_and_lengths_are_round() {
        assert_eq!(round_scale(1.0), 1.0);
        assert_eq!(round_scale(1.1), 2.0);
        assert_eq!(round_scale(110.0), 125.0);
        assert_eq!(round_scale(5001.0), 6000.0);
        assert_eq!(round_scale(0.2), 1.0);
        assert_eq!(length_text(1000.0), "1000 m");
        assert_eq!(length_text(12.5), "12.5 m");
        assert_eq!(length_text(4.0), "4 m");
        assert_eq!(length_text(0.25), "0.25 m");
    }

    #[test]
    fn outlines_chain_round_a_square_and_hatching_stays_inside() {
        // A 3 × 2 block of cells in a 5 × 4 grid.
        let mut class = vec![None; 20];
        for j in 1..3 {
            for i in 1..4 {
                class[i + 5 * j] = Some(0);
            }
        }
        let cells = Cells { first: [0, 0], n: [5, 4], class };
        let [edges, bands] = outlines(&cells);
        assert!(bands.is_empty());
        assert_eq!(edges.len(), 1, "{edges:?}");
        let ring = &edges[0];
        assert_eq!(ring.first(), ring.last(), "closed: {ring:?}");
        assert_eq!(ring.len(), 5, "four corners and back: {ring:?}");
        let hatch = hatching(&cells, 0.5);
        assert!(!hatch.is_empty());
        for [a, b] in &hatch {
            for p in [a, b] {
                assert!(p[0] >= 1.0 && p[0] <= 4.0 && p[1] >= 1.0 && p[1] <= 3.0, "{p:?} outside");
            }
            // At 45°.
            assert!(((b[0] - a[0]) - (b[1] - a[1])).abs() < 1e-12);
        }
        // Each line is one piece across the block: none broken at the row between.
        let mut lines: Vec<i64> = hatch.iter().map(|[a, _]| ((a[0] - a[1]) / 0.5).round() as i64).collect();
        lines.dedup();
        assert_eq!(lines.len(), hatch.len());
        // Control: an empty grid has no outline and no hatching.
        let empty = Cells { first: [0, 0], n: [5, 4], class: vec![None; 20] };
        assert!(outlines(&empty).iter().all(Vec::is_empty) && hatching(&empty, 0.5).is_empty());
    }
}
