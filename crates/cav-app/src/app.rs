//! The application inside eframe: the [`Shell`], its panes on the GPU and the frame clock. The
//! native window (`main.rs`) and the browser page ([`crate::web`]) run this same value.

use web_time::Instant;

use crate::{GpuPanes, Shell, theme};

/// The shell and its panes, as eframe runs them.
pub struct App {
    shell: Shell,
    panes: GpuPanes,
    last_frame: Instant,
    /// Minimise the window on its first frame (`--minimised`; never in a browser).
    minimise: bool,
}

impl App {
    /// The application around `shell`, drawing its panes with `panes`. `minimise` minimises the
    /// window on the first frame.
    #[must_use]
    pub fn new(shell: Shell, panes: GpuPanes, minimise: bool) -> Self {
        Self { shell, panes, last_frame: Instant::now(), minimise }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if std::mem::take(&mut self.minimise) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        let now = Instant::now();
        self.shell.frame_ms = Some(now.duration_since(self.last_frame).as_secs_f32() * 1e3);
        self.last_frame = now;
        self.shell.ui(ui, &mut self.panes);
        // In a browser, the files this frame "wrote" (an export, a figure) are handed to it as
        // downloads. Natively they are on disc already.
        #[cfg(target_arch = "wasm32")]
        crate::web::offer_downloads();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::BACKGROUND.to_normalized_gamma_f32()
    }
}
