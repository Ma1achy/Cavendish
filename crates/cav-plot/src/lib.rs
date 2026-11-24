//! Plots as display lists, and figures.
//!
//! ## What this crate is for
//! The plots a pane can show in place of the scene (signal, periodogram, cadence window, flight
//! diagram), each built once as a [`drawing::Drawing`] in points from the engine, a
//! [`style::Style`] and a measure of text ([`plots`]); and every way such a drawing is shown:
//! painted by egui in the application ([`paint`]), written as SVG ([`svg`]) or PDF ([`pdf`]),
//! or painted to a PNG with no window ([`headless`]). The application's dark theme and the
//! presets for papers, slides and posters are the same drawing code with different styles.
//! [`figure`] is the export: a figure at a stated size, the data behind it and a JSON of its
//! axes, units and provenance, installed into an engine as its figure exporter so that the
//! console, `cav figure` and the MCP server all issue one command. The application (`cav-app`)
//! and the command line (`cav`) both draw through this crate, so the command line does not link
//! the application.
//!
//! ```text
//! let mut engine = cav_engine::Engine::new();
//! cav_plot::figure::install(&mut engine);
//! engine.run_line("load comb")?;
//! engine.run_line("figure periodogram tower 2 as paper-column to fig.pdf")?;
//! // fig.pdf, fig.csv, fig.json
//! ```
//!
//! ## What it may depend on
//! `cav-core`, `cav-sim`, `cav-engine`, `cav-gpu`, `cav-render`; `egui` and `egui-wgpu` (never
//! `eframe` or `winit`: nothing here opens a window); `serde_json`; `skrifa`, which egui already
//! compiles, for glyph outlines. The PDF writer is in-house.
//!
//! ## Units
//! A drawing is in points (1/72 inch), y down. A figure's size is stated in millimetres, a
//! PNG's resolution in pixels per inch and its size in pixels. Data keeps the engine's units:
//! seconds, hertz, radians, rad/√Hz.
//!
//! ## The fonts, and what a figure holds of them
//! Text is set in two faces, both under the Ubuntu Font Licence 1.0: Ubuntu Mono (the file in
//! `assets/fonts/`, with the licence beside it and built into the binary) and Ubuntu Light (the
//! copy egui carries in `epaint_default_fonts`, which ships the same licence text and declares
//! `Ubuntu-font-1.0` in its manifest). In SVG and PDF, text is written as the outlines of its
//! glyphs, so the files embed no font. The licence's preamble says that it "does not require
//! any document created using the fonts or their derivatives to be published under this
//! licence, as long as the primary purpose of the document is not to be a vehicle for the
//! distribution of the fonts", and its clause 4 that the requirement to stay under the licence
//! "does not affect any document created using the Font Software". A figure is such a
//! document. This is our reading of the licence's text, not legal advice.
//! Every character the plots write is in both faces; were one missing from the face asked for,
//! it would be drawn from the other ([`fonts::face_of`]).

#![warn(missing_docs)]

pub mod drawing;
pub mod figure;
pub mod fonts;
pub mod headless;
pub mod paint;
pub mod pdf;
pub mod plots;
pub mod style;
pub mod svg;
pub mod theme;
pub mod voxels;
