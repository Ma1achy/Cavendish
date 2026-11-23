//! GPU tier: the whole application drawn with no window, checked by pixel.

use cav_app::Shell;
use cav_app::headless::Headless;
use cav_core::Vec3;
use cav_render::{BACKGROUND_SRGB, Camera, tower_srgb};
use egui::Rect;

const SIZE: [u32; 2] = [1440, 900];

struct Picture {
    rgba: Vec<u8>,
}

impl Picture {
    fn at(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * SIZE[0] + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }

    fn has_near(&self, at: [f64; 2], reach: i64, colour: [u8; 3], tolerance: u8) -> bool {
        let (cx, cy) = (at[0].round() as i64, at[1].round() as i64);
        (-reach..=reach).any(|dy| {
            (-reach..=reach).any(|dx| {
                let (x, y) = (cx + dx, cy + dy);
                x >= 0
                    && y >= 0
                    && x < i64::from(SIZE[0])
                    && y < i64::from(SIZE[1])
                    && close(self.at(x as u32, y as u32), colour, tolerance)
            })
        })
    }

    /// Whether every pixel of the rectangle is within `tolerance` of `colour`.
    fn all_close(&self, rect: Rect, colour: [u8; 3], tolerance: u8) -> bool {
        (rect.top().ceil() as u32..rect.bottom().floor() as u32).all(|y| {
            (rect.left().ceil() as u32..rect.right().floor() as u32)
                .all(|x| close(self.at(x, y), colour, tolerance))
        })
    }

    /// How many different colours the rectangle holds.
    fn colours_in(&self, rect: Rect) -> usize {
        let mut seen = std::collections::HashSet::new();
        for y in rect.top().ceil() as u32..rect.bottom().floor() as u32 {
            for x in rect.left().ceil() as u32..rect.right().floor() as u32 {
                seen.insert(self.at(x, y));
            }
        }
        seen.len()
    }
}

fn close(a: [u8; 3], b: [u8; 3], tolerance: u8) -> bool {
    (0..3).all(|i| a[i].abs_diff(b[i]) <= tolerance)
}

fn render(headless: &mut Headless, shell: &mut Shell) -> Picture {
    Picture { rgba: headless.render(shell, SIZE, vec![Vec::new(); 3]).expect("render") }
}

/// Where a point 7.3 m up tower `n` should appear on screen, through pane `pane`'s camera.
fn tower_on_screen(shell: &Shell, pane: usize, camera_of: usize, n: usize) -> [f64; 2] {
    let rect = shell.layout().panes[pane];
    let size = [rect.width().round() as u32, rect.height().round() as u32];
    let camera = Camera::new(shell.engine.view(camera_of), f64::from(size[0]) / f64::from(size[1]));
    let point = Vec3::from_array(shell.engine.spec().array.towers[n].position) + Vec3::Z * 7.3;
    let [x, y] = camera.pixel(point, size).expect("in front of the camera");
    [f64::from(rect.left()) + x, f64::from(rect.top()) + y]
}

#[test]
fn gpu_ui_draws_every_panel_and_the_scene() {
    let mut headless = Headless::new().expect("the GPU tier needs an adapter");
    let mut shell = Shell::new();
    shell.run("style flat").unwrap();
    shell.run("console").unwrap();
    let picture = render(&mut headless, &mut shell);

    // Every panel has something drawn in it: text and widgets make many colours.
    for name in ["menu", "footer", "timeline", "outliner", "properties", "header", "console"] {
        let rect = shell.layout().panel(name).unwrap_or_else(|| panic!("no {name} panel"));
        let colours = picture.colours_in(rect);
        assert!(colours >= 12, "the {name} panel is drawn with only {colours} colours");
    }
    // The pane shows the scene: each tower where the pane's camera projects it, and nobody else's.
    let pane = shell.layout().panes[0];
    for n in 0..5 {
        let at = tower_on_screen(&shell, 0, 0, n);
        assert!(picture.has_near(at, 2, tower_srgb(n), 12), "tower {} is not at {at:?} in the window", n + 1);
        assert!(!picture.has_near(at, 2, tower_srgb((n + 1) % 5), 12));
    }
    // The pane's colours arrive unchanged: its corner is the renderer's background exactly, which
    // fails if the UI decodes or encodes the picture's sRGB bytes a second time.
    let corner = picture.at(pane.left() as u32 + 3, pane.bottom() as u32 - 4);
    assert!(close(corner, BACKGROUND_SRGB, 1), "the pane's background arrives as {corner:?}");

    // Control: with the GUI hidden the same places hold the scene, not panels.
    shell.run("gui").unwrap();
    let hidden = render(&mut headless, &mut shell);
    assert!(shell.layout().panels.is_empty());
    assert_eq!(shell.layout().panes[0], shell.layout().screen);
    let menu = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(SIZE[0] as f32, 28.0));
    // (Within one level: egui dithers its output by half a level, as it does in the window.)
    assert!(hidden.all_close(menu, BACKGROUND_SRGB, 1), "where the menu bar was is plain background");
    assert!(!picture.all_close(menu, BACKGROUND_SRGB, 1), "and with the GUI shown it was not");
    assert!(close(hidden.at(4, 4), BACKGROUND_SRGB, 1));
    for n in 0..5 {
        let at = tower_on_screen(&shell, 0, 0, n);
        assert!(hidden.has_near(at, 2, tower_srgb(n), 12), "F3: tower {} is not at {at:?}", n + 1);
    }
}

#[test]
fn gpu_pane_count_changes_the_figure() {
    let mut headless = Headless::new().expect("the GPU tier needs an adapter");
    let mut shell = Shell::new();
    shell.run("panes 4").unwrap();
    for pane in 1..=4 {
        shell.run(&format!("style flat @{pane}")).unwrap();
    }
    let picture = render(&mut headless, &mut shell);
    assert_eq!(shell.layout().panes.len(), 4);
    // Tower 2 stands apart in every one of the four views; each pane shows it through its own camera.
    for pane in 0..4 {
        let at = tower_on_screen(&shell, pane, pane, 1);
        assert!(picture.has_near(at, 2, tower_srgb(1), 12), "pane {}: tower 2 is not at {at:?}", pane + 1);
    }
    // Control: the panes really have different cameras. Through the first pane's camera, tower 2
    // would be somewhere else in the second pane, and it is not drawn there.
    let wrong = tower_on_screen(&shell, 1, 0, 1);
    assert!(!picture.has_near(wrong, 2, tower_srgb(1), 12), "pane 2 seems to use pane 1's camera");

    // A frame in which nothing changed costs no pane redraws; a change redraws.
    let draws = headless.pane_draws();
    assert!(draws >= 4, "four panes were drawn");
    let again = render(&mut headless, &mut shell);
    assert_eq!(headless.pane_draws(), draws, "an unchanged frame redrew a pane");
    assert!(again.rgba == picture.rgba, "an unchanged frame is the same picture");
    shell.run("view north @2").unwrap();
    let moved = render(&mut headless, &mut shell);
    assert!(headless.pane_draws() > draws);
    assert!(moved.rgba != picture.rgba);

    // Back to one pane: it fills the figure, and tower 2 is where the single camera puts it.
    shell.run("panes 1").unwrap();
    let single = render(&mut headless, &mut shell);
    assert_eq!(shell.layout().panes, vec![shell.layout().figure]);
    assert!(single.has_near(tower_on_screen(&shell, 0, 0, 1), 2, tower_srgb(1), 12));
}

#[test]
fn gpu_ui_draws_the_signal_and_the_periodogram() {
    use cav_app::plots::place;
    let mut headless = Headless::new().expect("the GPU tier needs an adapter");
    let mut shell = Shell::new();
    for line in ["load pass", "panes 2", "show signal @2", "seek 5.46"] {
        shell.run(line).unwrap();
    }
    assert!(shell.engine.settle(std::time::Duration::from_secs(60)));
    let picture = render(&mut headless, &mut shell);
    let drawn = shell.layout().plots[0].clone();
    let signal = shell.engine.signal().1.unwrap().clone();
    // Each tower's value at each shot is a dot of that tower's colour where the axes put it.
    let channel = cav_core::forward::Channel::Gradiometer;
    for tower in [0, 1, 2] {
        for shot in [0, 3, 5] {
            let value = signal.channel(tower, channel)[shot];
            let at = place(drawn.area, &drawn.x, &drawn.y, (signal.times[shot], value));
            assert!(
                picture.has_near([f64::from(at.x), f64::from(at.y)], 1, tower_srgb(tower), 12),
                "tower {} shot {shot}: no dot at {at:?}",
                tower + 1
            );
        }
    }
    // The playhead is a line down the plot at 5.46 s.
    let playhead = drawn.playhead.unwrap();
    // One colour in every plot and in the timeline, and no series' (`the_playhead_is_one_colour`).
    let c = cav_app::theme::PLAYHEAD;
    let link = [c.r(), c.g(), c.b()];
    let on_line = (drawn.area.top() as u32 + 4..drawn.area.bottom() as u32 - 4)
        .filter(|&y| picture.has_near([f64::from(playhead), f64::from(y)], 1, link, 40))
        .count();
    assert!(on_line as f32 > 0.8 * drawn.area.height(), "the playhead covers {on_line} rows");
    // Control: the scene pane beside it holds no such dot; the plot is in its own pane.
    let scene_pane = shell.layout().panes[0];
    assert!(!scene_pane.contains(drawn.area.center()));

    // The periodogram of the zoo: a stem of the first comb's colour at the linear oscillator's f₀.
    for line in ["load oscillator-zoo", "set duration 400", "panes 2", "show periodogram @2"] {
        shell.run(line).unwrap();
    }
    assert!(shell.engine.settle(std::time::Duration::from_secs(60)));
    let picture = render(&mut headless, &mut shell);
    let drawn = shell.layout().plots[0].clone();
    let (_, _, x, folded) = drawn.stems.iter().find(|s| s.0 == "linear" && s.1 == 1).unwrap().clone();
    assert!(!folded);
    // The first comb's colour: not the grey of the spectrum's own line.
    let blue = [0x5a, 0xb4, 0xff];
    let low = f64::from(drawn.area.bottom()) - 6.0;
    assert!(picture.has_near([f64::from(x), low], 1, blue, 30), "no stem at x = {x}");
    // Control: a little to the side of the stem, at the same height, there is none.
    assert!(!picture.has_near([f64::from(x) - 8.0, low], 1, blue, 30));
}
