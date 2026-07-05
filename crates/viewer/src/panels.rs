//! The 2D panels: per-detector `signal` vs `time`, the `periodogram` when present, the `mask` as shaded
//! cycles — with a cursor at `time[ℓ]`, the SAME `ℓ` that poses the 3D scene (coherence). Which panels
//! are live is a **pure decision**: an absent optional field (its `FieldSet` flag was off) DISABLES its
//! panel — a normal state, never an error — so the decision is testable without egui.

use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, Polygon, VLine};
use state::StateBundle;

/// Whether the periodogram panel is live. Absent ⇒ disabled (normal), not an error path.
pub fn periodogram_enabled(bundle: &StateBundle) -> bool {
    bundle.periodogram.is_some()
}

/// Contiguous `[start, end]` time spans where `mask` is true — the transient-contaminated cycles drawn
/// as shaded bands. Pure, so the shading is testable without egui.
pub fn masked_spans(times: &[f64], mask: &[bool]) -> Vec<[f64; 2]> {
    let mut spans = Vec::new();
    let mut start: Option<usize> = None;
    for i in 0..times.len() {
        match (mask.get(i).copied().unwrap_or(false), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                spans.push([times[s], times[i - 1]]);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        spans.push([times[s], times[times.len() - 1]]);
    }
    spans
}

/// The per-detector signal traces vs time, the mask as shaded bands, and a vertical cursor at `time[ℓ]`
/// — the coherent cursor, the same index that poses the 3D scene.
pub fn signal_panel(ui: &mut egui::Ui, bundle: &StateBundle, ell: usize) {
    // Auto-fit by default (egui_plot); "⟲ fit" (or a double-click) refits after a manual pan/zoom.
    let reset = ui
        .horizontal(|ui| {
            ui.button("⟲ fit")
                .on_hover_text("reset the view to fit the data (or double-click the plot)")
                .clicked()
        })
        .inner;
    let cursor = bundle.time.get(ell).copied();
    let spans = masked_spans(&bundle.time, &bundle.mask);
    let d = bundle.signal.first().map_or(0, |r| r.len());
    let mut plot = Plot::new("signal")
        .height(ui.available_height())
        .legend(egui_plot::Legend::default());
    if reset {
        plot = plot.reset();
    }
    plot.show(ui, |pui| {
        // Shaded masked cycles, drawn within the current bounds so they do not blow the y-scale.
        let (lo, hi) = (pui.plot_bounds().min(), pui.plot_bounds().max());
        for s in &spans {
            pui.polygon(
                Polygon::new(PlotPoints::from(vec![
                    [s[0], lo[1]],
                    [s[1], lo[1]],
                    [s[1], hi[1]],
                    [s[0], hi[1]],
                ]))
                .fill_color(egui::Color32::from_rgba_unmultiplied(200, 80, 80, 40)),
            );
        }
        for di in 0..d {
            let pts: Vec<[f64; 2]> = bundle
                .time
                .iter()
                .zip(&bundle.signal)
                .map(|(&t, row)| [t, row[di]])
                .collect();
            pui.line(Line::new(PlotPoints::from(pts)).name(format!("det {di}")));
        }
        if let Some(t) = cursor {
            pui.vline(VLine::new(t).name("ℓ"));
        }
    });
}

/// The periodogram panel: per-detector power vs frequency when present, or a disabled note when the
/// field was not computed (`none_disables` — an absent optional is not an error).
pub fn periodogram_panel(ui: &mut egui::Ui, bundle: &StateBundle) {
    match &bundle.periodogram {
        None => {
            ui.weak("periodogram — off (enable FieldSet.periodogram)");
        }
        Some(pg) => {
            let reset = ui
                .horizontal(|ui| {
                    ui.button("⟲ fit")
                        .on_hover_text("reset the view to fit the data (or double-click the plot)")
                        .clicked()
                })
                .inner;
            let mut plot = Plot::new("periodogram")
                .height(ui.available_height())
                .legend(egui_plot::Legend::default());
            if reset {
                plot = plot.reset();
            }
            plot.show(ui, |pui| {
                for (di, power) in pg.power.iter().enumerate() {
                    let pts: Vec<[f64; 2]> =
                        pg.freqs.iter().zip(power).map(|(&f, &p)| [f, p]).collect();
                    pui.line(Line::new(PlotPoints::from(pts)).name(format!("det {di}")));
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::Periodogram;

    #[test]
    fn none_disables() {
        // An absent periodogram disables the panel and takes NO error path; present ⇒ enabled.
        let mut b = StateBundle::default();
        assert!(
            !periodogram_enabled(&b),
            "absent periodogram must disable, not error"
        );
        b.periodogram = Some(Periodogram {
            freqs: vec![0.1, 0.2],
            power: vec![vec![1.0, 0.5]],
        });
        assert!(
            periodogram_enabled(&b),
            "present periodogram enables the panel"
        );
    }

    #[test]
    fn mask_spans_contiguous() {
        let times = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let mask = vec![false, true, true, false, true, false];
        assert_eq!(masked_spans(&times, &mask), vec![[1.0, 2.0], [4.0, 4.0]]);
        // A run reaching the end closes at the last timestamp.
        let tail = vec![false, false, true, true];
        assert_eq!(masked_spans(&times[..4], &tail), vec![[2.0, 3.0]]);
    }
}
