//! The GUI as data (`plan/REBUILD.md` principle 11): the big scopes, the menus and every action
//! of the window, read from `ui/actions.ron`, which is compiled in (the page has no files to
//! read). The menu bar, the right-click menus, the palette, the tool strip's tooltips and the `?`
//! sheet are all views of this one table, and every action is a console line, so none of them is
//! a second path into the engine.
//!
//! What is data and what is code: the table says *what* exists, where, under which keys and when
//! it can run; the code says how a line is run ([`crate::Shell::run`]), how a key is routed
//! ([`crate::keyboard`]) and how a menu is drawn. A file that names something that does not exist
//! fails `the_data_files_are_valid`, with its line.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::keyboard::{Chord, GLOBAL, Shortcut};

/// The built-in table, as compiled in.
pub const ACTIONS_RON: &str = include_str!("../ui/actions.ron");

/// One big scope of the keyboard model.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScopeData {
    /// Its id: the first part of every focus path under it.
    pub id: String,
    /// What the breadcrumb and the `?` sheet call it.
    pub name: String,
}

/// When an action can run: a closed set of rules the code knows.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Default)]
pub enum Needs {
    /// Always.
    #[default]
    Always,
    /// There is an edit to undo.
    Undo,
    /// There is an edit to redo.
    Redo,
    /// Something is selected.
    Selection,
    /// A body or a tower is selected: an item of a list, which can be duplicated, renamed and
    /// taken out.
    Item,
    /// A body is selected.
    Body,
    /// The scene has a tower n (from 1).
    Tower(usize),
    /// A body of the scene is a voxel shape.
    Shapes,
}

/// One action.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Action {
    /// Unique; what keys, tests and the palette's history use.
    pub id: String,
    /// What a menu or the palette says.
    pub label: String,
    /// Where it sits in the menu bar (`View/Look from`), or empty for none.
    #[serde(default)]
    pub menu: String,
    /// The console line it runs; may hold `{selection}`, `{body}`, `{tower}`.
    pub line: String,
    /// Where its keys work: a scope's id or [`GLOBAL`].
    #[serde(default = "global")]
    pub scope: String,
    /// Its key chords.
    #[serde(default)]
    pub keys: Vec<String>,
    /// When it can run.
    #[serde(default)]
    pub needs: Needs,
    /// A rule above it in its menu.
    #[serde(default)]
    pub rule: bool,
    /// On the right-click menu of a row of the scene tree (for that row's item).
    #[serde(default)]
    pub context: bool,
    /// One line for the palette and the `?` sheet.
    #[serde(default)]
    pub about: String,
}

fn global() -> String {
    GLOBAL.to_owned()
}

impl Action {
    /// The text of its first key, for a menu's right-hand side; empty for none.
    #[must_use]
    pub fn shortcut_text(&self) -> String {
        self.keys.first().cloned().unwrap_or_default()
    }

    /// Its menu path as segments: `View/Look from` is `["View", "Look from"]`.
    #[must_use]
    pub fn menu_path(&self) -> Vec<&str> {
        if self.menu.is_empty() { Vec::new() } else { self.menu.split('/').collect() }
    }
}

/// The whole table.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Table {
    /// The big scopes, in Tab order.
    pub scopes: Vec<ScopeData>,
    /// The menu bar's menus, left to right.
    pub menus: Vec<String>,
    /// Every action.
    pub actions: Vec<Action>,
}

impl Table {
    /// Reads a table from RON and checks it: unique ids, known scopes, chords that parse, menus
    /// that exist, no two actions on one chord in one scope chain.
    ///
    /// # Errors
    /// The first problem, with the line of the file it is on where there is one.
    pub fn parse(text: &str) -> Result<Self, String> {
        let table: Self = ron::from_str(text).map_err(|e| format!("ui/actions.ron: {e}"))?;
        table.check(text)?;
        Ok(table)
    }

    fn check(&self, text: &str) -> Result<(), String> {
        // The line an action's id is written on, for the messages.
        let line_of = |id: &str| {
            let needle = format!("id: \"{id}\"");
            text.lines().position(|l| l.contains(&needle)).map_or(0, |n| n + 1)
        };
        let mut seen = std::collections::BTreeSet::new();
        for scope in &self.scopes {
            if !seen.insert(scope.id.as_str()) {
                return Err(format!("ui/actions.ron: the scope `{}` is listed twice", scope.id));
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        for action in &self.actions {
            let at = line_of(&action.id);
            if !ids.insert(action.id.as_str()) {
                return Err(format!("ui/actions.ron:{at}: the id `{}` is used twice", action.id));
            }
            if action.scope != GLOBAL && !self.knows_scope(&action.scope) {
                return Err(format!(
                    "ui/actions.ron:{at}: `{}` names the scope `{}`, which is not a scope",
                    action.id, action.scope
                ));
            }
            if let Some(menu) = action.menu_path().first()
                && !self.menus.iter().any(|m| m == menu)
            {
                return Err(format!(
                    "ui/actions.ron:{at}: `{}` is in the menu `{menu}`, which is not in `menus`",
                    action.id
                ));
            }
            for key in &action.keys {
                Chord::parse(key).map_err(|why| format!("ui/actions.ron:{at}: `{}`: {why}", action.id))?;
            }
            if action.line.trim().is_empty() {
                return Err(format!("ui/actions.ron:{at}: `{}` runs no line", action.id));
            }
        }
        if let Some((a, b, chord)) =
            clash(&self.shortcuts(), |scope| self.chain(scope), |id| self.action(id).map(|a| a.line.clone()))
        {
            return Err(format!(
                "ui/actions.ron:{}: `{a}` and `{b}` are both on {} in one scope chain",
                line_of(&b),
                chord.text()
            ));
        }
        Ok(())
    }

    /// Whether `id` is a scope the keyboard knows: a big scope, a window, or one of the scopes
    /// under them that the frame registers ([`SUB_SCOPES`]). The whole id is checked, so a
    /// misspelt `tree/sorces` is refused (S18's review, minor 6: only its first part was).
    fn knows_scope(&self, id: &str) -> bool {
        self.scopes.iter().any(|s| s.id == id) || WINDOWS.contains(&id) || SUB_SCOPES.contains(&id)
    }

    /// The scopes a shortcut in `scope` can be heard from, innermost first: `tree/bodies` is
    /// reached from itself and from `tree`; a big scope only from itself.
    fn chain(&self, scope: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut at = scope.to_owned();
        loop {
            out.push(at.clone());
            match at.rsplit_once('/') {
                Some((parent, _)) => at = parent.to_owned(),
                None => break,
            }
        }
        out
    }

    /// Every key binding, as the router takes them.
    #[must_use]
    pub fn shortcuts(&self) -> Vec<Shortcut> {
        self.actions
            .iter()
            .flat_map(|action| {
                action.keys.iter().filter_map(|key| {
                    Some(Shortcut {
                        scope: action.scope.clone(),
                        chord: Chord::parse(key).ok()?,
                        action: action.id.clone(),
                    })
                })
            })
            .collect()
    }

    /// The action called `id`.
    #[must_use]
    pub fn action(&self, id: &str) -> Option<&Action> {
        self.actions.iter().find(|a| a.id == id)
    }

    /// The big scopes as the keyboard's tree takes them.
    #[must_use]
    pub fn big_scopes(&self) -> Vec<(&str, &str)> {
        self.scopes.iter().map(|s| (s.id.as_str(), s.name.as_str())).collect()
    }

    /// What the scope called `id` is called, for the `?` sheet; the id itself if it is not a big
    /// scope.
    #[must_use]
    pub fn scope_name<'a>(&'a self, id: &'a str) -> &'a str {
        if id == GLOBAL {
            return "Anywhere";
        }
        self.scopes.iter().find(|s| s.id == id).map_or(id, |s| s.name.as_str())
    }
}

/// The scopes inside the big scopes that the frame registers every frame, whatever is shown, and
/// that may own shortcuts. `every_sub_scope_is_registered` holds this list to what is drawn.
pub const SUB_SCOPES: &[&str] = &[
    "tree/sources",
    "tree/array",
    "tree/surroundings",
    "viewport/canvas",
    "viewport/selection",
    "timeline/scrubber",
];

/// The windows the keyboard model knows (each a list of sections): their ids may own shortcuts.
pub const WINDOWS: &[&str] = &["palette", "sheet", "about", "console", "history", "wizard", "open", "save"];

/// Two actions on one chord where both can be heard at once: in one scope, or a global and any
/// scope, or a scope and one inside it. `None` when there is no clash.
fn clash(
    shortcuts: &[Shortcut],
    chain: impl Fn(&str) -> Vec<String>,
    line: impl Fn(&str) -> Option<String>,
) -> Option<(String, String, Chord)> {
    for (n, a) in shortcuts.iter().enumerate() {
        for b in &shortcuts[n + 1..] {
            if a.chord != b.chord || a.action == b.action {
                continue;
            }
            let heard_together = a.scope == b.scope
                || a.scope == GLOBAL
                || b.scope == GLOBAL
                || chain(&a.scope).contains(&b.scope)
                || chain(&b.scope).contains(&a.scope);
            // The same line bound in two places is one thing, not a clash: the viewport's `1`
            // and the tree's `1` both select tower 1.
            let same = line(&a.action).is_some() && line(&a.action) == line(&b.action);
            if heard_together && !same {
                return Some((a.action.clone(), b.action.clone(), a.chord));
            }
        }
    }
    None
}

/// The table compiled in, read once.
///
/// # Panics
/// When the built-in file does not parse or check: `the_data_files_are_valid` fails first.
pub fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| Table::parse(ACTIONS_RON).expect("ui/actions.ron is valid (a test holds it)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_files_are_valid() {
        let table = Table::parse(ACTIONS_RON).unwrap();
        assert_eq!(
            table.scopes.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["top-bar", "toolbar", "tree", "viewport", "inspector", "timeline", "footer"],
            "the big scopes and their order (docs/slices/S18.md §2)"
        );
        assert!(table.actions.len() >= 60, "{}", table.actions.len());
        // Every line parses: the window's own, or one the engine's console reads.
        let shell = crate::Shell::new();
        for action in &table.actions {
            // The default scene has towers and no bodies: a tower is what is selected.
            let line = crate::Shell::resolve(&action.line, Some(cav_engine::Target::Tower(1)));
            assert!(
                crate::Shell::is_app_command(&line) || shell.engine.parse(&line).is_ok(),
                "`{}` runs `{line}`, which is not a command",
                action.id
            );
        }
        // Control: a corrupted id fails, naming its line.
        let corrupt =
            ACTIONS_RON.replace("scope: \"viewport\", keys: [\"O\"]", "scope: \"nowhere\", keys: [\"O\"]");
        let why = Table::parse(&corrupt).unwrap_err();
        assert!(why.contains("nowhere") && why.starts_with("ui/actions.ron:"), "{why}");
        let line: usize = why.split(':').nth(1).unwrap().parse().unwrap();
        assert!(corrupt.lines().nth(line - 1).unwrap().contains("proj-toggle"), "{why}");
        // A scope under a big one is checked whole: `tree/sorces` is not `tree/sources`.
        let misspelt = ACTIONS_RON
            .replace("scope: \"viewport\", keys: [\"O\"]", "scope: \"tree/sorces\", keys: [\"O\"]");
        assert!(Table::parse(&misspelt).unwrap_err().contains("tree/sorces"));
        let right = ACTIONS_RON
            .replace("scope: \"viewport\", keys: [\"O\"]", "scope: \"tree/sources\", keys: [\"O\"]");
        assert!(Table::parse(&right).is_ok(), "the control: the scope as it is spelt");
        // And a key that is not a key, and a menu that is not a menu.
        assert!(Table::parse(&ACTIONS_RON.replace("keys: [\"F3\"]", "keys: [\"F99\"]")).is_err());
        assert!(
            Table::parse(
                &ACTIONS_RON.replace("menu: \"Scene\", line: \"run\"", "menu: \"Nope\", line: \"run\"")
            )
            .is_err()
        );
    }

    #[test]
    fn no_shortcut_clashes_and_every_one_is_listed() {
        let table = table();
        let shortcuts = table.shortcuts();
        let lines = |id: &str| table.action(id).map(|a| a.line.clone());
        assert!(clash(&shortcuts, |s| table.chain(s), lines).is_none());
        // Every shortcut is on the `?` sheet and reachable from the palette (both are made from
        // the actions that have keys).
        let sheet = crate::sheet::rows(table);
        for shortcut in &shortcuts {
            assert!(
                sheet
                    .iter()
                    .any(|row| row.keys.contains(&shortcut.chord.text())
                        && row.actions.contains(&shortcut.action)),
                "{} is not on the sheet",
                shortcut.action
            );
        }
        // The palette offers what can run now: every keyed action that can, on a shell with an
        // edit to undo and one to redo.
        let mut shell = crate::Shell::new();
        shell.run("load void").unwrap();
        shell.run("load point").unwrap();
        shell.run("undo").unwrap();
        shell.run("select tower 1").unwrap();
        let entries = crate::palette::entries(table, &shell);
        for action in table.actions.iter().filter(|a| !a.keys.is_empty() && shell.can_run(a)) {
            assert!(entries.iter().any(|e| e.action.as_deref() == Some(action.id.as_str())), "{}", action.id);
        }
        assert!(["undo", "redo"].iter().all(|id| entries.iter().any(|e| e.action.as_deref() == Some(id))));
        // Control: what cannot run is not offered (no tower 9 in the default array).
        assert!(!entries.iter().any(|e| e.action.as_deref() == Some("select-tower-9")));
        // Control: a duplicate letter in one chain is a clash, whichever way round.
        let mut doubled = shortcuts.clone();
        doubled.push(Shortcut {
            scope: "viewport".into(),
            chord: Chord::parse("O").unwrap(),
            action: "other".into(),
        });
        assert!(clash(&doubled, |s| table.chain(s), lines).is_some());
        let mut nested = shortcuts.clone();
        nested.push(Shortcut {
            scope: "viewport/canvas".into(),
            chord: Chord::parse("O").unwrap(),
            action: "other".into(),
        });
        assert!(
            clash(&nested, |s| table.chain(s), lines).is_some(),
            "a scope and one inside it are heard together"
        );
        let mut global = shortcuts;
        global.push(Shortcut {
            scope: "tree".into(),
            chord: Chord::parse("Ctrl+Z").unwrap(),
            action: "other".into(),
        });
        assert!(clash(&global, |s| table.chain(s), lines).is_some(), "a global is heard everywhere");
    }
}
