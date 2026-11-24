//! The menu bar as data: every enabled item is a console command (M13-R18), so a menu is a list of
//! lines, and a test can check that each one parses.

/// One entry of a menu.
#[derive(Clone, Copy, Debug)]
pub enum Item {
    /// Runs a console line. `shortcut` is shown beside the label.
    Run {
        /// What the entry says.
        label: &'static str,
        /// The console line it runs.
        line: &'static str,
        /// The key that does the same, as shown; empty for none.
        shortcut: &'static str,
    },
    /// Something a later slice brings: shown greyed out, with the slice that brings it.
    Later {
        /// What the entry says.
        label: &'static str,
        /// The slice that brings it, such as `S3`.
        slice: &'static str,
    },
    /// A rule between groups.
    Separator,
    /// A nested menu.
    Sub {
        /// What the entry says.
        label: &'static str,
        /// The entries of the nested menu.
        items: &'static [Item],
    },
}

const fn run(label: &'static str, line: &'static str) -> Item {
    Item::Run { label, line, shortcut: "" }
}

const fn key(label: &'static str, line: &'static str, shortcut: &'static str) -> Item {
    Item::Run { label, line, shortcut }
}

const fn later(label: &'static str, slice: &'static str) -> Item {
    Item::Later { label, slice }
}

/// The menus, left to right.
pub const MENUS: &[(&str, &[Item])] = &[
    (
        "File",
        &[
            Item::Sub {
                label: "New scene from library",
                items: &[
                    run("room", "load room"),
                    run("void", "load void"),
                    run("infinite-floor", "load infinite-floor"),
                ],
            },
            later("Open scene…", "S2"),
            later("Save scene", "S2"),
            later("Export…", "S2"),
            Item::Separator,
            run("Quit", "quit"),
        ],
    ),
    (
        "Edit",
        &[
            key("Undo", "undo", "Ctrl+Z"),
            key("Redo", "redo", "Ctrl+Shift+Z"),
            Item::Separator,
            later("Edit history…", "S2"),
            later("Preferences…", "S2"),
        ],
    ),
    (
        "View",
        &[
            Item::Sub {
                label: "Look from",
                items: &[
                    run("iso", "view iso"),
                    run("top", "view top"),
                    run("bottom", "view bottom"),
                    run("north", "view north"),
                    run("south", "view south"),
                    run("east", "view east"),
                    run("west", "view west"),
                ],
            },
            key("Home", "view home", "Home"),
            Item::Sub {
                label: "Projection",
                items: &[
                    run("perspective", "proj persp"),
                    run("orthographic", "proj ortho"),
                    key("toggle", "proj toggle", "O"),
                ],
            },
            Item::Sub {
                label: "Panes",
                items: &[run("1", "panes 1"), run("2", "panes 2"), run("4", "panes 4")],
            },
            Item::Sub {
                label: "Draw style",
                items: &[
                    run("shaded", "style shaded"),
                    run("flat", "style flat"),
                    run("x-ray", "style xray"),
                    run("wireframe", "style wireframe"),
                    run("paper", "style paper"),
                ],
            },
            Item::Separator,
            key("Hide the GUI", "gui", "F3"),
            later("Colour by field…", "S3"),
            later("Saved views…", "S2"),
        ],
    ),
    (
        "Scene",
        &[
            later("Add source", "S1"),
            later("Air…", "S8"),
            later("Noise stack…", "S12"),
            Item::Separator,
            key("Play / pause", "play", "Space"),
            run("Rewind", "seek 0"),
            later("Run", "S2"),
        ],
    ),
    (
        "Array",
        &[
            Item::Sub {
                label: "Select tower",
                items: &[
                    key("tower 1", "select tower 1", "1"),
                    key("tower 2", "select tower 2", "2"),
                    key("tower 3", "select tower 3", "3"),
                    key("tower 4", "select tower 4", "4"),
                    key("tower 5", "select tower 5", "5"),
                ],
            },
            key("Select nothing", "select none", "Esc"),
            Item::Separator,
            later("Add tower", "S2"),
            later("Layout", "S2"),
            later("Timing…", "S2"),
        ],
    ),
    ("Dataset", &[later("Builder…", "S11"), later("Run batch", "S11")]),
    ("Windows", &[key("Console", "console", "`"), key("Keyboard", "keys", "?"), run("About", "about")]),
    (
        "Help",
        &[
            key("Keyboard reference", "keys", "?"),
            run("Console commands", "help"),
            run("Scene spec reference", "schema"),
            run("About Cavendish", "about"),
        ],
    ),
];

/// Whether a menu's `line` can be run on a scene of `towers` towers: `select tower n` only when
/// there is a tower n (a scene may have fewer than five, or none). Every other line can.
#[must_use]
pub fn possible(line: &str, towers: usize) -> bool {
    match line.strip_prefix("select tower ").map(str::parse::<usize>) {
        Some(Ok(n)) => n >= 1 && n <= towers,
        _ => true,
    }
}

/// Every line a menu can run, in menu order.
#[must_use]
pub fn lines() -> Vec<&'static str> {
    fn collect(items: &'static [Item], out: &mut Vec<&'static str>) {
        for item in items {
            match item {
                Item::Run { line, .. } => out.push(line),
                Item::Sub { items, .. } => collect(items, out),
                Item::Later { .. } | Item::Separator => {}
            }
        }
    }
    let mut out = Vec::new();
    for (_, items) in MENUS {
        collect(items, &mut out);
    }
    out
}
