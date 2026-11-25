//! The window's layouts as data (`ui/layouts.ron`): each a name and the console lines that
//! arrange the figure for a task (Scene, Instrument, Analysis). Choosing one runs its lines, so a
//! layout can do nothing a command cannot; the big scopes and their Tab order are the same in
//! every layout (`every_layout_keeps_every_scope_and_the_figure_uncovered`).

use std::sync::OnceLock;

use serde::Deserialize;

/// The built-in layouts, as compiled in.
pub const LAYOUTS_RON: &str = include_str!("../ui/layouts.ron");

/// One layout.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LayoutData {
    /// What `layout <id>` names.
    pub id: String,
    /// What the top bar's tab says.
    pub label: String,
    /// The console lines it runs, in order.
    pub lines: Vec<String>,
    /// The inspector's folding sections it opens (`inspector/instrument`), if any: UI only.
    #[serde(default)]
    pub unfold: Vec<String>,
    /// One line for the tooltip and the palette.
    pub about: String,
}

/// Every layout.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Layouts {
    /// The layouts, in the top bar's order; the first is the window's at the start.
    pub layouts: Vec<LayoutData>,
}

impl Layouts {
    /// Reads and checks the layouts: ids unique, every line a command, each chosen by an action
    /// of the table (`layout <id>`).
    ///
    /// # Errors
    /// The first problem.
    pub fn parse(text: &str) -> Result<Self, String> {
        let layouts: Self = ron::from_str(text).map_err(|e| format!("ui/layouts.ron: {e}"))?;
        let engine = cav_engine::Engine::new();
        let mut ids = std::collections::BTreeSet::new();
        for layout in &layouts.layouts {
            if !ids.insert(layout.id.as_str()) {
                return Err(format!("ui/layouts.ron: the layout `{}` is listed twice", layout.id));
            }
            for line in &layout.lines {
                // Only what arranges the figure: a view command (journalled, never an edit, never
                // the selection). S18's review, minor 7: an edit, a load or a `select` was let
                // through, and switching layout lost the selection.
                match engine.parse(line) {
                    Ok(cav_engine::Request::Do(cav_engine::Command::View(view))) if !view.is_edit() => {}
                    Ok(_) => {
                        return Err(format!(
                            "ui/layouts.ron: `{}` runs `{line}`, which is not a view command",
                            layout.id
                        ));
                    }
                    Err(_) => {
                        return Err(format!(
                            "ui/layouts.ron: `{}` runs `{line}`, which is not a command",
                            layout.id
                        ));
                    }
                }
            }
            let line = format!("layout {}", layout.id);
            if !crate::actions::table().actions.iter().any(|a| a.line == line) {
                return Err(format!("ui/layouts.ron: no action of ui/actions.ron runs `{line}`"));
            }
        }
        if layouts.layouts.is_empty() {
            return Err("ui/layouts.ron: no layouts".into());
        }
        Ok(layouts)
    }

    /// The layout called `id`.
    #[must_use]
    pub fn layout(&self, id: &str) -> Option<&LayoutData> {
        self.layouts.iter().find(|l| l.id == id)
    }
}

/// The layouts compiled in, read once.
///
/// # Panics
/// When the built-in file does not parse or check: `the_layouts_are_valid` fails first.
pub fn layouts() -> &'static Layouts {
    static LAYOUTS: OnceLock<Layouts> = OnceLock::new();
    LAYOUTS.get_or_init(|| Layouts::parse(LAYOUTS_RON).expect("ui/layouts.ron is valid (a test holds it)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layouts_are_valid() {
        let all = Layouts::parse(LAYOUTS_RON).unwrap();
        assert_eq!(
            all.layouts.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
            ["scene", "instrument", "analysis"]
        );
        // Control: a line that is not a command, a layout no action chooses, a twice-listed one.
        assert!(
            Layouts::parse(&LAYOUTS_RON.replace("\"panes 1\"", "\"panes many\""))
                .unwrap_err()
                .contains("panes many")
        );
        // An edit, a selection or Home (an undoable step) is not a layout's to run.
        for line in ["set duration 5", "select none", "load void", "view home", "tool move"] {
            let with = LAYOUTS_RON.replace("\"pane 1\"]", &format!("\"pane 1\", \"{line}\"]"));
            assert!(Layouts::parse(&with).unwrap_err().contains(line), "{line}");
        }
        assert!(Layouts::parse(&LAYOUTS_RON.replace("id: \"analysis\"", "id: \"other\"")).is_err());
        assert!(Layouts::parse(&LAYOUTS_RON.replace("id: \"analysis\"", "id: \"scene\"")).is_err());
    }
}
