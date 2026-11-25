//! The command palette (`Ctrl+K`): one box that searches every action, every library scene,
//! every console command and every field of the selection, and runs what is chosen through the
//! same console line a menu or a key would. It is a window of two sections, the query and the
//! results (`docs/research/editor-ux.md` §3.10.6).
//!
//! Its one exception to the keyboard model, written down: while the query is being typed in,
//! ↑ ↓ choose a result and Enter runs it (as an open drop-down takes them), and Esc closes the
//! palette at once rather than first leaving the field.

use crate::actions::Table;
use crate::shell::Shell;
use crate::theme;

/// What kind of thing a result is; results are grouped in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    /// An action whose keys work where the focus is now.
    Here,
    /// Any other action.
    Action,
    /// Something to add to the scene.
    Add,
    /// A library scene to open.
    Scene,
    /// A console command.
    Command,
    /// A field of the selection, in the inspector.
    Field,
}

impl Group {
    /// The word the results show beside each.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Here => "here",
            Self::Action => "action",
            Self::Add => "add",
            Self::Scene => "scene",
            Self::Command => "command",
            Self::Field => "field",
        }
    }
}

/// One result.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Its group.
    pub group: Group,
    /// What it says.
    pub label: String,
    /// Where it lives or what it does, after the label.
    pub detail: String,
    /// Its key, if it has one.
    pub keys: String,
    /// The action it runs, when it is an action.
    pub action: Option<String>,
    /// The console line it runs, when it is not an action: a scene's `load`, a command with no
    /// arguments; a command that needs arguments opens the console with its first word typed.
    pub line: Option<String>,
    /// The field it goes to, when it is a field: its path.
    pub field: Option<String>,
}

/// Every result there is now, before filtering: the actions (those that can run), the library,
/// the console's commands and the selection's fields.
#[must_use]
pub fn entries(table: &Table, shell: &Shell) -> Vec<Entry> {
    let mut out = Vec::new();
    for action in &table.actions {
        if !shell.can_run(action) {
            continue;
        }
        let here = action.scope != crate::keyboard::GLOBAL && shell.keyboard.within(&action.scope);
        let menu = action.menu.replace('/', " › ");
        out.push(Entry {
            group: if here { Group::Here } else { Group::Action },
            label: if menu.is_empty() {
                action.label.clone()
            } else {
                format!("{menu} › {}", action.label)
            },
            detail: action.about.clone(),
            keys: action.shortcut_text(),
            action: Some(action.id.clone()),
            line: None,
            field: None,
        });
    }
    for starter in cav_sim::starters::starters() {
        out.push(Entry {
            group: Group::Add,
            label: format!("Add › {}", starter.label),
            detail: starter.about.clone(),
            keys: String::new(),
            action: None,
            line: Some(format!("add {}", starter.name)),
            field: None,
        });
    }
    for scene in cav_sim::LIBRARY {
        out.push(Entry {
            group: Group::Scene,
            label: format!("Open › {}", scene.name),
            detail: scene.path.to_owned(),
            keys: String::new(),
            action: None,
            line: Some(format!("load {}", scene.name)),
            field: None,
        });
    }
    for command in cav_engine::console::COMMANDS {
        out.push(Entry {
            group: Group::Command,
            label: command.usage.to_owned(),
            detail: command.about.to_owned(),
            keys: String::new(),
            action: None,
            line: Some(command.name.to_owned()),
            field: None,
        });
    }
    for path in shell.inspected_fields() {
        out.push(Entry {
            group: Group::Field,
            label: path.clone(),
            detail: "go to the field in the inspector".to_owned(),
            keys: String::new(),
            action: None,
            line: None,
            field: Some(path),
        });
    }
    out
}

/// How well `query` matches `text`: `None` when its letters are not all in `text` in order
/// (case aside); otherwise higher is better. A match at the start of a word and a run of
/// consecutive letters score more; a shorter text scores a little more.
#[must_use]
pub fn score(query: &str, text: &str) -> Option<i32> {
    let query: Vec<char> = query.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    if query.is_empty() {
        return Some(0);
    }
    let text: Vec<char> = text.to_lowercase().chars().collect();
    let (mut at, mut score, mut run) = (0usize, 0i32, 0i32);
    let mut last: Option<usize> = None;
    for q in query {
        let found = (at..text.len()).find(|&i| text[i] == q)?;
        let word_start = found == 0 || !text[found - 1].is_alphanumeric();
        run = if last == Some(found.wrapping_sub(1)) { run + 1 } else { 0 };
        score += 10 + 6 * run + if word_start { 12 } else { 0 };
        last = Some(found);
        at = found + 1;
    }
    Some(score - i32::try_from(text.len()).unwrap_or(i32::MAX) / 8)
}

/// Whether every word of `query` begins a word of `text` (case aside): "add sph" in "Add ›
/// Sphere", not in "View › Show in the pane › the scene", where its letters are only scattered.
#[must_use]
pub fn word_match(query: &str, text: &str) -> bool {
    let text = text.to_lowercase();
    let words: Vec<&str> = text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let query = query.to_lowercase();
    let mut wanted = query.split_whitespace().peekable();
    wanted.peek().is_some() && wanted.all(|q| words.iter().any(|w| w.starts_with(q)))
}

/// The results for `query`, best first: each entry's index in `entries` and its score. Entries
/// of an earlier group win ties. At most `limit`. When some entries match by the starts of
/// their words, only those are given: letters merely scattered through a long label are noise
/// beside them (S18's second review, UX 6: "sph" was padded with "Show in the pane" rows);
/// otherwise the scattered matches are what there is.
#[must_use]
pub fn filter(entries: &[Entry], query: &str, limit: usize) -> Vec<(usize, i32)> {
    let strong = |e: &Entry| word_match(query, &e.label) || word_match(query, &e.detail);
    let any_strong = !query.trim().is_empty() && entries.iter().any(strong);
    let mut scored: Vec<(usize, i32)> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !any_strong || strong(e))
        .filter_map(|(n, e)| {
            let best = score(query, &e.label).max(score(query, &e.detail).map(|s| s / 3));
            best.map(|s| (n, s))
        })
        .collect();
    scored
        .sort_by(|a, b| b.1.cmp(&a.1).then(entries[a.0].group.cmp(&entries[b.0].group)).then(a.0.cmp(&b.0)));
    scored.truncate(limit);
    scored
}

/// The palette's state: UI only.
#[derive(Clone, Debug, Default)]
pub struct Palette {
    /// Whether it is open.
    pub open: bool,
    /// What has been typed.
    pub query: String,
    /// The highlighted result, from 0.
    pub at: usize,
    /// Ask egui to put the caret in the query on the next frame.
    pub focus_query: bool,
    /// Only the actions of this menu (a menu opened from the keyboard is the palette, filtered).
    pub menu: Option<String>,
}

/// How many results are shown.
pub const SHOWN: usize = 12;

/// Draws the palette, if it is open, and runs what is chosen.
pub fn draw(shell: &mut Shell, ctx: &egui::Context) {
    if !shell.palette.open {
        return;
    }
    let table = crate::actions::table();
    let mut all = entries(table, shell);
    if let Some(menu) = &shell.palette.menu {
        if menu == "Add" {
            // The Add menu is the starters.
            all.retain(|e| e.group == Group::Add);
        } else {
            let in_menu =
                |id: &String| table.action(id).is_some_and(|a| a.menu_path().first() == Some(&menu.as_str()));
            all.retain(|e| e.action.as_ref().is_some_and(in_menu));
        }
    }
    let found = filter(&all, &shell.palette.query, SHOWN);
    shell.palette.at = shell.palette.at.min(found.len().saturating_sub(1));
    let mut chosen: Option<usize> = None;
    let mut close = false;
    shell.building.window("palette", "Palette");
    shell.building.add("palette", "palette/query", "Search", crate::keyboard::Kind::Text);
    shell.building.add("palette", "palette/results", "Results", crate::keyboard::Kind::Scope);
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::unique("palette"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(screen.center().x - 300.0, screen.top() + 40.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(8.0).show(ui, |ui| {
                ui.set_width(600.0);
                let hint = match &shell.palette.menu {
                    Some(menu) => format!("the {menu} menu: type to narrow"),
                    None => "search actions, things to add, scenes, commands, fields".to_owned(),
                };
                let edit = egui::TextEdit::singleline(&mut shell.palette.query)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY);
                let response = ui.add(edit);
                shell.building.place("palette/query", response.rect);
                if shell.palette.focus_query || shell.take_type("palette/query") {
                    response.request_focus();
                    shell.palette.focus_query = false;
                }
                if response.changed() {
                    shell.palette.at = 0;
                }
                if response.has_focus() {
                    // The palette's exception: ↑ ↓ choose and Enter runs while typing.
                    let (up, down) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown)));
                    if up {
                        shell.palette.at = shell.palette.at.saturating_sub(1);
                    }
                    if down && shell.palette.at + 1 < found.len() {
                        shell.palette.at += 1;
                    }
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    chosen = Some(shell.palette.at);
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
                ui.add_space(4.0);
                let results_top = ui.cursor().min;
                for (row, (n, _)) in found.iter().enumerate() {
                    let entry = &all[*n];
                    let id = format!("palette/result/{row}");
                    shell.building.add("palette/results", &id, &entry.label, crate::keyboard::Kind::Button);
                    let lit = row == shell.palette.at;
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), egui::Sense::click());
                    shell.building.place(&id, rect);
                    if lit {
                        ui.painter().rect_filled(rect, 2.0, theme::SELECTION);
                    } else if response.hovered() {
                        ui.painter().rect_filled(rect, 2.0, theme::HOVER);
                    }
                    let painter = ui.painter();
                    let mid = rect.center().y;
                    painter.text(
                        egui::pos2(rect.left() + 6.0, mid),
                        egui::Align2::LEFT_CENTER,
                        entry.group.name(),
                        egui::FontId::proportional(10.5),
                        theme::WEAK,
                    );
                    let galley = painter.layout_no_wrap(entry.label.clone(), egui::FontId::proportional(13.0), theme::STRONG);
                    let label_width = galley.size().x;
                    painter.galley(egui::pos2(rect.left() + 58.0, mid - galley.size().y / 2.0), galley, theme::STRONG);
                    if !entry.keys.is_empty() {
                        painter.text(
                            egui::pos2(rect.right() - 6.0, mid),
                            egui::Align2::RIGHT_CENTER,
                            &entry.keys,
                            egui::FontId::monospace(11.0),
                            theme::GOLD,
                        );
                    }
                    let room = rect.width() - 58.0 - label_width - 90.0;
                    if room > 40.0 && !entry.detail.is_empty() {
                        let detail = painter.layout(
                            entry.detail.clone(),
                            egui::FontId::proportional(11.0),
                            theme::WEAK,
                            room,
                        );
                        let first = detail.rows.first().map_or(0.0, |r| r.size.y);
                        let clipped = painter.with_clip_rect(rect);
                        clipped.galley(egui::pos2(rect.left() + 70.0 + label_width, mid - first / 2.0), detail, theme::WEAK);
                    }
                    if response.clicked() || shell.take_click(&id) {
                        chosen = Some(row);
                    }
                }
                if found.is_empty() {
                    ui.label(egui::RichText::new("nothing matches").color(theme::WEAK));
                }
                let results = egui::Rect::from_min_max(results_top, ui.min_rect().max);
                shell.building.place("palette/results", results);
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Up, Down choose · Enter runs · Esc closes · what runs is a console line, echoed in the log")
                        .size(11.0)
                        .color(theme::WEAK),
                );
            });
        });
    if let Some(row) = chosen
        && let Some((n, _)) = found.get(row)
    {
        let entry = all[*n].clone();
        shell.palette_close();
        shell.palette_run(&entry);
    } else if close {
        shell.palette_close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_finds_every_action_and_command() {
        let shell = Shell::new();
        let table = crate::actions::table();
        let all = entries(table, &shell);
        // Every action that can run is a result for its own label.
        for action in table.actions.iter().filter(|a| shell.can_run(a)) {
            let found = filter(&all, &action.label, all.len());
            assert!(
                found.iter().any(|(n, _)| all[*n].action.as_deref() == Some(action.id.as_str())),
                "`{}` is not found by its label",
                action.id
            );
        }
        // Every console command is a result for its own name.
        for command in cav_engine::console::COMMANDS {
            let found = filter(&all, command.name, all.len());
            assert!(
                found.iter().any(|(n, _)| all[*n].line.as_deref() == Some(command.name)),
                "{}",
                command.name
            );
        }
        // Fuzzy: "sph" finds the sphere first among the things to add.
        let found = filter(&all, "add sph", 3);
        assert_eq!(all[found[0].0].line.as_deref(), Some("add sphere"), "{:?}", all[found[0].0]);
        // Only matches by the starts of words when there are any (S18's second review, UX 6):
        // "sph" gives no "Show in the pane" rows; control: a query that begins no word
        // ("shwpn") still finds them by their scattered letters.
        let sph = filter(&all, "sph", all.len());
        assert!(
            !sph.is_empty()
                && sph
                    .iter()
                    .all(|(n, _)| word_match("sph", &all[*n].label) || word_match("sph", &all[*n].detail))
        );
        assert!(
            !sph.iter().any(|(n, _)| all[*n].label.contains("Show in the pane")),
            "{:?}",
            sph.iter().map(|(n, _)| &all[*n].label).collect::<Vec<_>>()
        );
        let scattered = filter(&all, "shwpn", all.len());
        assert!(scattered.iter().any(|(n, _)| all[*n].label.contains("Show in the pane")));
        // Control: letters out of order do not match.
        assert!(score("hps", "sphere").is_none());
        assert!(score("sph", "sphere").unwrap() > score("sph", "a shape here").unwrap());
    }
}
