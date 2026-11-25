//! The `?` sheet: every key, by where it works. Made from the action table
//! ([`crate::actions`]) and the keyboard model's own keys ([`MODEL`]), so a key added to the
//! table is on the sheet with no line here (`no_shortcut_clashes_and_every_one_is_listed`).

use crate::actions::Table;

/// One row of the sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Where it works: a scope's name, "Anywhere", or a mode of the model.
    pub scope: String,
    /// Its keys, as data writes them.
    pub keys: Vec<String>,
    /// What it does.
    pub label: String,
    /// The action's id; empty for a key of the model itself.
    pub action: String,
}

/// The keys of the keyboard model itself (`docs/design/keyboard.md` §3, §6, §7 and Cavendish's
/// sessions), which are mechanisms, not actions: `(where, keys, what)`.
pub const MODEL: &[(&str, &str, &str)] = &[
    ("Moving around", "Tab · Shift+Tab", "next or previous big scope (in a window: its next section)"),
    ("Moving around", "Enter", "go in; on a button, press it; on a number or the view, start adjusting"),
    ("Moving around", "Esc", "back out one step: stop adjusting, up a scope, close a window"),
    ("Moving around", "arrows", "move between neighbours; while adjusting, adjust"),
    ("Moving around", "Shift · Alt", "with an adjustment: ×10 · ×0.1"),
    ("Moving around", "held key", "repeats after 500 ms, then every 40 ms"),
    ("Typing", "Enter · Esc", "stop typing, keeping the text"),
    ("Typing", "Tab", "leave the field and move on"),
    ("The view, adjusting", "arrows", "orbit by 1° (Ctrl: pan)"),
    ("The view, adjusting", "+ · −", "zoom"),
    ("Moving the selection", "arrows · Page Up, Page Down", "move along x, y, z by the snap step"),
    ("Moving the selection", "X · Y · Z", "lock to an axis"),
    ("Moving the selection", "0–9 - .", "type a distance, metres"),
    ("Moving the selection", "Enter · Esc", "keep the move · cancel it"),
    ("Flying", "W A S D · E Q", "forward, left, back, right · up, down"),
    ("Flying", "arrows", "look"),
    ("Flying", "Enter · Esc", "stop flying, the camera where it is"),
];

/// The rows of the sheet: the table's actions that have keys, grouped by where they work in the
/// order of the big scopes (anywhere first), then the model's own.
#[must_use]
pub fn rows(table: &Table) -> Vec<Row> {
    let mut out = Vec::new();
    let mut scopes: Vec<&str> = vec![crate::keyboard::GLOBAL];
    scopes.extend(table.scopes.iter().map(|s| s.id.as_str()));
    // Scopes below a big one (`tree/bodies`) follow their big scope.
    let mut rest: Vec<&str> =
        table.actions.iter().map(|a| a.scope.as_str()).filter(|s| !scopes.contains(s)).collect();
    rest.sort_unstable();
    rest.dedup();
    for scope in scopes.into_iter().chain(rest) {
        for action in table.actions.iter().filter(|a| a.scope == scope && !a.keys.is_empty()) {
            out.push(Row {
                scope: table.scope_name(scope).to_owned(),
                keys: action.keys.clone(),
                label: if action.about.is_empty() { action.label.clone() } else { action.about.clone() },
                action: action.id.clone(),
            });
        }
    }
    for (scope, keys, label) in MODEL {
        out.push(Row {
            scope: (*scope).to_owned(),
            keys: vec![(*keys).to_owned()],
            label: (*label).to_owned(),
            action: String::new(),
        });
    }
    out
}
