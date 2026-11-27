//! The `?` sheet: every key, by where it works. Made from the action table
//! ([`crate::actions`]) and the keyboard model's own keys ([`MODEL`]), so a key added to the
//! table is on the sheet with no line here (`no_shortcut_clashes_and_every_one_is_listed`).

use crate::actions::{Action, Table};

/// One row of the sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Where it works: a scope's name, "Anywhere", or a mode of the model.
    pub scope: String,
    /// Its keys, as data writes them.
    pub keys: Vec<String>,
    /// What it does, short: the action's name as a menu would say it.
    pub label: String,
    /// The longer sentence, for a tooltip.
    pub about: String,
    /// The actions the row stands for (one, or a run such as `select-tower-1` … `-9` on the keys
    /// `1` … `9`); empty for a key of the model itself.
    pub actions: Vec<String>,
}

impl Row {
    /// The keys as the sheet prints them: a run of single digits as `1–9`, others joined by ` · `.
    #[must_use]
    pub fn keys_text(&self) -> String {
        let digits: Vec<u32> = self
            .keys
            .iter()
            .filter_map(|k| (k.len() == 1).then(|| k.chars().next()?.to_digit(10)).flatten())
            .collect();
        if digits.len() == self.keys.len() && digits.len() > 2 && digits.windows(2).all(|w| w[1] == w[0] + 1)
        {
            return format!("{}–{}", digits[0], digits[digits.len() - 1]);
        }
        self.keys.join(" · ")
    }
}

/// The sheet's row of the sprint keys, `(keys, what)`, where Ctrl sprints (`ctrl_sprints`) or not
/// (in a browser: [`crate::platform::CTRL_SPRINTS`]).
#[must_use]
pub const fn sprint_row(ctrl_sprints: bool) -> (&'static str, &'static str) {
    if ctrl_sprints {
        ("Ctrl · W twice", "sprint while held · until W is let go")
    } else {
        ("W twice", "sprint until W is let go")
    }
}

/// This platform's [`sprint_row`].
const SPRINT: (&str, &str) = sprint_row(crate::platform::CTRL_SPRINTS);

/// The keys of the keyboard model itself (`docs/design/keyboard.md` §3, §6, §7 and Cavendish's
/// sessions), which are mechanisms, not actions: `(where, keys, what)`.
pub const MODEL: &[(&str, &str, &str)] = &[
    ("Moving around", "Tab · Shift+Tab", "next or previous big scope (in a window: its next section)"),
    ("Moving around", "Enter", "go in; on a button, press it; on a number or the view, start adjusting"),
    ("Moving around", "Esc", "back out one step: stop adjusting, up a scope, close a window"),
    ("Moving around", "arrows", "move between neighbours; while adjusting, adjust"),
    ("Moving around", "Shift · Alt", "with an adjustment: ×10 · ×0.1"),
    ("Moving around", "held key", "repeats after 500 ms, then every 40 ms"),
    ("Moving around", "Page Up · Page Down", "in a list being scrolled: a page"),
    ("Typing", "Enter · Esc", "stop typing, keeping the text"),
    ("Typing", "Tab", "leave the field and move on"),
    ("The view, adjusting", "arrows", "orbit by 1° (Ctrl: pan)"),
    ("The view, adjusting", "+ · −", "zoom"),
    ("The view, with the mouse", "right button held", "fly while it is held: the mouse looks, the keys move"),
    ("The view, with the mouse", "middle drag · wheel", "pan · zoom toward the pointer"),
    (
        "Moving the selection",
        "arrows · Page Up, Page Down",
        "move the way they point on screen · up and down, by the snap step",
    ),
    ("Moving the selection", "X · Y · Z", "lock to an axis"),
    ("Moving the selection", "0–9 - .", "type a distance, metres"),
    ("Moving the selection", "Tab", "type the next axis's distance"),
    ("Moving the selection", "Enter · Esc", "keep the move · cancel it"),
    ("Flying and walking", "mouse · arrows", "look (the pointer is captured and hidden)"),
    ("Flying and walking", "W A S D", "forward, left, back, right, level whatever the look"),
    ("Flying and walking", SPRINT.0, SPRINT.1),
    ("Flying and walking", "Space twice", "switch between flying and walking"),
    ("Flying and walking", "Space · Shift", "flying: up · down (E · Q too); walking: jump · sneak"),
    ("Flying and walking", "wheel", "the speed in use, walking or flying, shown for a moment"),
    ("Flying and walking", "N", "noclip: pass through matter (it flies), or not"),
    (
        "Flying and walking",
        "Esc · Enter",
        "stop, the camera where it is (so do Shift+F flying, Shift+G walking)",
    ),
    ("Flying and walking", "F3 · F11", "the only other keys that act: hide the panels · fill the screen"),
];

/// What an action is called on the sheet: its menu label, with the submenu it sits in when the
/// label alone does not say (`toggle` of View › Projection is "Projection: toggle").
fn sheet_label(action: &Action) -> String {
    let path = action.menu_path();
    match path.as_slice() {
        [_, sub, ..] => format!("{sub}: {}", action.label),
        _ => action.label.clone(),
    }
}

/// The rows of the sheet: the table's actions that have keys, grouped by where they work in the
/// order of the big scopes (anywhere first), then the model's own. Neighbouring actions of one
/// scope on the keys `1`, `2`, … with the same line but for the digit are one row (`1–9`).
#[must_use]
pub fn rows(table: &Table) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    let mut scopes: Vec<&str> = vec![crate::keyboard::GLOBAL];
    scopes.extend(table.scopes.iter().map(|s| s.id.as_str()));
    // Scopes below a big one (`tree/bodies`) follow their big scope.
    let mut rest: Vec<&str> =
        table.actions.iter().map(|a| a.scope.as_str()).filter(|s| !scopes.contains(s)).collect();
    rest.sort_unstable();
    rest.dedup();
    // The line with its digits taken out: `select tower 3` and `select tower 4` are one kind.
    let kind = |line: &str| line.chars().filter(|c| !c.is_ascii_digit()).collect::<String>();
    for scope in scopes.into_iter().chain(rest) {
        for action in table.actions.iter().filter(|a| a.scope == scope && !a.keys.is_empty()) {
            let digit = action.keys.len() == 1 && action.keys[0].chars().all(|c| c.is_ascii_digit());
            if digit
                && let Some(last) = out.last_mut()
                && last.scope == table.scope_name(scope)
                && last.keys.iter().all(|k| k.chars().all(|c| c.is_ascii_digit()))
                && last
                    .actions
                    .first()
                    .and_then(|id| table.action(id))
                    .is_some_and(|first| kind(&first.line) == kind(&action.line))
            {
                last.keys.push(action.keys[0].clone());
                last.actions.push(action.id.clone());
                if last.actions.len() == 2 {
                    // A run: said once, for any of its numbers.
                    last.label = format!("{} (that number)", kind(&action.line).trim());
                    last.about = format!(
                        "{}: the key's number",
                        action.about.trim_end_matches(char::is_numeric).trim()
                    );
                }
                continue;
            }
            out.push(Row {
                scope: table.scope_name(scope).to_owned(),
                keys: action.keys.clone(),
                label: sheet_label(action),
                about: action.about.clone(),
                actions: vec![action.id.clone()],
            });
        }
    }
    for (scope, keys, label) in MODEL {
        out.push(Row {
            scope: (*scope).to_owned(),
            keys: vec![(*keys).to_owned()],
            label: (*label).to_owned(),
            about: String::new(),
            actions: Vec::new(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_of_numbered_keys_is_one_row() {
        let rows = rows(crate::actions::table());
        let towers: Vec<&Row> =
            rows.iter().filter(|r| r.actions.iter().any(|a| a.starts_with("select-tower-"))).collect();
        assert_eq!(towers.len(), 1, "the viewport's 1–9 are one row");
        assert_eq!(towers[0].keys_text(), "1–9");
        assert_eq!(towers[0].actions.len(), 9);
        // Control: keys that are not a run are listed as they are.
        let undo = rows.iter().find(|r| r.actions == ["redo"]).unwrap();
        assert_eq!(undo.keys_text(), "Ctrl+Shift+Z · Ctrl+Y");
        // A label that needs its submenu has it.
        let toggle = rows.iter().find(|r| r.actions == ["proj-toggle"]).unwrap();
        assert_eq!(toggle.label, "Projection: toggle");
    }
}
