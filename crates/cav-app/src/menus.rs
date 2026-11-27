//! The menu bar, as a view of the action table (`ui/actions.ron`, [`crate::actions`]): every item
//! is an action, so every item is a console line (M13-R18) and its key is the one the router
//! knows. Nothing is greyed out that works; what cannot run now is disabled, with the reason in
//! its `needs`.

use egui::Button;

use crate::actions::{Action, table};
use crate::shell::Shell;

/// The menus, left to right.
#[must_use]
pub fn titles() -> Vec<String> {
    table().menus.clone()
}

/// Every line a menu can run, in menu order.
#[must_use]
pub fn lines() -> Vec<String> {
    let table = table();
    table
        .menus
        .iter()
        .flat_map(|menu| {
            table
                .actions
                .iter()
                .filter(move |a| a.menu_path().first() == Some(&menu.as_str()))
                .map(|a| a.line.clone())
        })
        .collect()
}

/// The actions under the menu path `prefix` (`View`, `View/Look from`), in the table's order,
/// as items and the submenus among them (each once, where its first action is).
enum Item<'a> {
    Action(&'a Action),
    Sub(String),
}

fn items(prefix: &str) -> Vec<Item<'static>> {
    let mut out: Vec<Item<'static>> = Vec::new();
    let mut subs: Vec<String> = Vec::new();
    for action in &table().actions {
        if action.menu == prefix {
            out.push(Item::Action(action));
        } else if let Some(rest) = action.menu.strip_prefix(prefix).and_then(|r| r.strip_prefix('/')) {
            let sub = rest.split('/').next().unwrap_or(rest).to_owned();
            if !subs.contains(&sub) {
                subs.push(sub.clone());
                out.push(Item::Sub(sub));
            }
        }
    }
    out
}

/// The Add menu: the starters (`scenes/starters.ron`), bodies first, voxel shapes in a submenu,
/// then the tower. Each adds its starter where the camera looks and selects it. Last, a new
/// folder of the scene tree.
pub(crate) fn add_menu(shell: &mut Shell, ui: &mut egui::Ui) {
    let starters = cav_sim::starters::starters();
    let is_shape = |s: &cav_sim::starters::Starter| s.body.as_ref().is_some_and(|b| b.shape.is_some());
    let mut chosen: Option<&str> = None;
    for starter in starters.iter().filter(|s| s.body.is_some() && !is_shape(s)) {
        if ui.button(&starter.label).on_hover_text(&starter.about).clicked() {
            chosen = Some(&starter.name);
        }
    }
    ui.menu_button("Voxel shape", |ui| {
        for starter in starters.iter().filter(|s| is_shape(s)) {
            if ui.button(&starter.label).on_hover_text(&starter.about).clicked() {
                chosen = Some(&starter.name);
            }
        }
    });
    ui.separator();
    for starter in starters.iter().filter(|s| s.tower.is_some()) {
        if ui.button(&starter.label).on_hover_text(&starter.about).clicked() {
            chosen = Some(&starter.name);
        }
    }
    if let Some(name) = chosen {
        let _ = shell.add_starter(name);
        ui.close();
    }
    // A folder of the scene tree: the `new-folder` action, here beside what it will hold.
    if let Some(action) = table().action("new-folder") {
        ui.separator();
        let response = ui.button(&action.label).on_hover_text(&action.about);
        shell.layout.buttons.push(("new-folder".to_owned(), response.rect));
        if response.clicked() {
            let _ = shell.run_action(&action.id);
            ui.close();
        }
    }
}

/// Draws the items of the menu `prefix` into an open menu, running what is clicked.
pub(crate) fn draw_items(shell: &mut Shell, ui: &mut egui::Ui, prefix: &str) {
    if prefix == "Add" {
        add_menu(shell, ui);
        return;
    }
    for item in items(prefix) {
        match item {
            Item::Action(action) => {
                if action.rule {
                    ui.separator();
                }
                let enabled = shell.can_run(action);
                let button = Button::new(&action.label).shortcut_text(action.shortcut_text());
                let response = ui.add_enabled(enabled, button).on_hover_text(&action.about);
                if response.clicked() {
                    let _ = shell.run_action(&action.id);
                    ui.close();
                }
            }
            Item::Sub(sub) => {
                let path = format!("{prefix}/{sub}");
                ui.menu_button(sub, |ui| draw_items(shell, ui, &path));
            }
        }
    }
}
