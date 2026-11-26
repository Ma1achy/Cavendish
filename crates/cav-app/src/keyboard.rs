//! The keyboard model of `docs/design/keyboard.md` (Principia's, adopted in full), as pure logic:
//! a tree of scopes walked in two modes, a focus path, a ring that shows only while keyboard
//! navigation is on, and one router that turns each key into [`Effect`]s in a fixed priority.
//!
//! Nothing here touches the engine or egui's widgets. The frame registers what it draws into a
//! [`FocusTree`] (visual order = registration order), hands each key to [`Keyboard::route`], and
//! carries out the effects that come back: a click, an adjustment, an action's console line. The
//! state is UI only: never journalled, never undone, never seen by the engine (the spec's §2).
//!
//! Units: an [`Effect::Adjust`] carries *steps*, already multiplied (Shift ×10, Alt ×0.1); what a
//! step is in metres, seconds or degrees is the widget's ([`step_for`] for numbers).

use std::collections::HashMap;

use egui::{Key, Modifiers, Rect};

/// A scope's or an element's identity, stable from frame to frame: `tree`, `tree/bodies`,
/// `inspector/bodies[0].mass`. The big scopes' ids are the first part of every path under them.
pub type Id = String;

/// The two modes of the spec's §1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// Moving between things: arrows move, Enter goes in. Orange ring.
    #[default]
    Navigation,
    /// Operating one thing: arrows adjust it. Blue ring.
    Interaction,
}

/// What a node of the tree is, which decides what Enter and the arrows do on it (the spec's §4, §6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A region with elements or sub-scopes inside: Enter goes in, onto the first.
    Scope,
    /// A button, a tick box, a row that selects: Enter is its click and stays in navigation mode.
    Button,
    /// A number or a slider: Enter starts adjusting it with the arrows.
    Adjust,
    /// A line of text: Enter starts typing in it.
    Text,
    /// A drop-down of `count` choices, `current` shown: Enter opens it, ↑ ↓ choose, Enter picks,
    /// Esc closes it unchanged.
    Choice {
        /// How many choices it has.
        count: usize,
        /// Which is chosen now, from 0.
        current: usize,
    },
    /// A canvas that is itself the interactive thing (the view): Enter starts interaction with it.
    Canvas,
    /// The selection's handles in the viewport: Enter starts a transform session (arrows move it
    /// by the snap step, X/Y/Z lock an axis, digits type a distance, Enter keeps, Esc cancels:
    /// Cavendish's one local exception, the spec's §7).
    Session,
    /// A read-only leaf: Enter does nothing.
    ReadOnly,
}

/// One scope or element as the frame registered it.
#[derive(Clone, Debug)]
pub struct Node {
    /// Its identity.
    pub id: Id,
    /// What the breadcrumb calls it.
    pub name: String,
    /// What it is.
    pub kind: Kind,
    /// A disabled node stays in the order (the spec's §9) and does nothing on Enter.
    pub enabled: bool,
    /// Where it was drawn, points; `None` when it is not on screen (a hidden or collapsed panel).
    pub rect: Option<Rect>,
    parent: Option<usize>,
    children: Vec<usize>,
}

/// The tree of scopes and elements, made again each frame as the frame draws. The big scopes
/// come first, from data, in their fixed order, whether or not their panel is shown: order never
/// depends on state. Windows are roots of their own, after them.
#[derive(Clone, Debug, Default)]
pub struct FocusTree {
    nodes: Vec<Node>,
    index: HashMap<Id, usize>,
    big: Vec<usize>,
    windows: Vec<usize>,
    /// The egui layer each window was drawn in, for which of two overlapping windows is on top.
    layers: HashMap<Id, egui::LayerId>,
}

impl FocusTree {
    /// A tree holding the big scopes, `(id, name)` in Tab order, and nothing else yet.
    #[must_use]
    pub fn new(big_scopes: &[(&str, &str)]) -> Self {
        let mut tree = Self::default();
        for (id, name) in big_scopes {
            let at = tree.push(None, id, name, Kind::Scope);
            tree.big.push(at);
        }
        tree
    }

    fn push(&mut self, parent: Option<usize>, id: &str, name: &str, kind: Kind) -> usize {
        let at = self.nodes.len();
        self.nodes.push(Node {
            id: id.to_owned(),
            name: name.to_owned(),
            kind,
            enabled: true,
            rect: None,
            parent,
            children: Vec::new(),
        });
        self.index.insert(id.to_owned(), at);
        if let Some(parent) = parent {
            self.nodes[parent].children.push(at);
        }
        at
    }

    /// Registers `id` under `parent` (a big scope, a window or a scope already registered), as the
    /// next in order. Registering an id twice in a frame (egui may lay a frame out twice) only
    /// refreshes its kind; an unknown parent is ignored, so a widget drawn outside any scope is
    /// simply not reachable rather than wrongly placed.
    pub fn add(&mut self, parent: &str, id: &str, name: &str, kind: Kind) {
        if let Some(&at) = self.index.get(id) {
            self.nodes[at].kind = kind;
            self.nodes[at].name = name.to_owned();
            return;
        }
        if let Some(&parent) = self.index.get(parent) {
            self.push(Some(parent), id, name, kind);
        }
    }

    /// Registers a window: a root of its own, whose children are its sections.
    pub fn window(&mut self, id: &str, name: &str) {
        if !self.index.contains_key(id) {
            let at = self.push(None, id, name, Kind::Scope);
            self.windows.push(at);
        }
    }

    /// Says where `id` was drawn.
    pub fn place(&mut self, id: &str, rect: Rect) {
        if let Some(&at) = self.index.get(id) {
            self.nodes[at].rect = Some(rect);
        }
    }

    /// Says where a window was drawn and in which egui layer (its response's).
    pub fn place_window(&mut self, id: &str, rect: Rect, layer: egui::LayerId) {
        self.place(id, rect);
        self.layers.insert(id.to_owned(), layer);
    }

    /// Marks `id` as disabled: it keeps its place and does nothing.
    pub fn disable(&mut self, id: &str) {
        if let Some(&at) = self.index.get(id) {
            self.nodes[at].enabled = false;
        }
    }

    /// How many scopes and elements are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether nothing is registered (never: the big scopes always are).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The node called `id`, if it was registered.
    #[must_use]
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.index.get(id).map(|&at| &self.nodes[at])
    }

    /// The innermost node of an open window drawn under `pos` (points), if there is one: where a
    /// click on a window's control moves the keyboard's focus (the spec's §8). Nodes outside
    /// windows are never answered: a click on a panel or the canvas leaves the focus alone.
    /// `top` is the layer egui has on top at `pos`: where two windows overlap, only the one in it
    /// is looked in (S18's second review, minor 5). A click on a window's own ground (no control
    /// under it) answers the window's first section, never the bare window (minor 7).
    #[must_use]
    pub fn window_node_at(&self, pos: egui::Pos2, top: Option<egui::LayerId>) -> Option<&str> {
        let root = |mut at: usize| {
            while let Some(parent) = self.nodes[at].parent {
                at = parent;
            }
            at
        };
        let on_top = |window: usize| match (top, self.layers.get(&self.nodes[window].id)) {
            (Some(top), Some(layer)) => *layer == top,
            _ => true,
        };
        let found = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(at, node)| {
                let window = root(*at);
                node.rect.is_some_and(|r| r.contains(pos)) && self.windows.contains(&window) && on_top(window)
            })
            .min_by(|(_, a), (_, b)| {
                let area = |n: &Node| n.rect.map_or(f32::INFINITY, |r| r.area());
                area(a).total_cmp(&area(b))
            })
            .map(|(at, _)| at)?;
        match self.nodes[found].children.first() {
            Some(&first) if self.windows.contains(&found) => Some(self.nodes[first].id.as_str()),
            _ => Some(self.nodes[found].id.as_str()),
        }
    }

    /// The ids of `id`'s children, in order.
    #[must_use]
    pub fn children(&self, id: &str) -> Vec<&str> {
        self.index
            .get(id)
            .map(|&at| self.nodes[at].children.iter().map(|&c| self.nodes[c].id.as_str()).collect())
            .unwrap_or_default()
    }

    /// The big scopes' ids, in Tab order.
    #[must_use]
    pub fn big_scopes(&self) -> Vec<&str> {
        self.big.iter().map(|&at| self.nodes[at].id.as_str()).collect()
    }

    /// Whether `id` is a window's root.
    #[must_use]
    pub fn is_window(&self, id: &str) -> bool {
        self.index.get(id).is_some_and(|at| self.windows.contains(at))
    }

    /// The path from a root (a big scope or a window) down to `id`, if it is registered.
    #[must_use]
    pub fn path_to(&self, id: &str) -> Option<Vec<Id>> {
        let mut at = *self.index.get(id)?;
        let mut path = vec![self.nodes[at].id.clone()];
        while let Some(parent) = self.nodes[at].parent {
            path.push(self.nodes[parent].id.clone());
            at = parent;
        }
        path.reverse();
        Some(path)
    }

    /// The breadcrumb for a focus path: its names joined by ` › `.
    #[must_use]
    pub fn breadcrumb(&self, path: &[Id]) -> String {
        path.iter().filter_map(|id| self.node(id)).map(|n| n.name.as_str()).collect::<Vec<_>>().join(" › ")
    }

    /// The siblings of `id`: its parent's children, or the big scopes for a big scope, or the
    /// windows for a window.
    fn siblings(&self, id: &str) -> Vec<&str> {
        let Some(&at) = self.index.get(id) else { return Vec::new() };
        match self.nodes[at].parent {
            Some(parent) => self.nodes[parent].children.iter().map(|&c| self.nodes[c].id.as_str()).collect(),
            None if self.windows.contains(&at) => vec![self.nodes[at].id.as_str()],
            None => self.big_scopes(),
        }
    }
}

/// A key as the router sees it: the key and the modifiers held with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Press {
    /// The key.
    pub key: Key,
    /// What was held with it.
    pub modifiers: Modifiers,
}

impl Press {
    /// A key with no modifiers.
    #[must_use]
    pub const fn plain(key: Key) -> Self {
        Self { key, modifiers: Modifiers::NONE }
    }

    /// A key with modifiers.
    #[must_use]
    pub const fn with(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    /// The step multiplier the spec gives every adjustment: Shift ×10, Alt ×0.1, both ×1.
    #[must_use]
    pub fn multiplier(self) -> f64 {
        let mut m = 1.0;
        if self.modifiers.shift {
            m *= 10.0;
        }
        if self.modifiers.alt {
            m *= 0.1;
        }
        m
    }
}

/// A key chord as data writes it (`Ctrl+Shift+Z`, `F3`, `?`, `Delete`), matched with the spec's
/// modifier hygiene: Ctrl (Cmd on a Mac) must match; Shift and Alt must match when a binding of
/// the same key with them exists, and are otherwise ignored, so Shift+Home is still Home.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    /// The key.
    pub key: Key,
    /// Ctrl or Cmd.
    pub command: bool,
    /// Shift.
    pub shift: bool,
    /// Alt.
    pub alt: bool,
}

impl Chord {
    /// Reads a chord: modifiers `Ctrl+`, `Shift+`, `Alt+` then one key, by egui's name for it or
    /// one of `? ` + - = , . / [ ] { }`.
    ///
    /// # Errors
    /// What is not a chord.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut chord = Self { key: Key::Escape, command: false, shift: false, alt: false };
        let mut rest = text.trim();
        loop {
            if let Some(after) = rest.strip_prefix("Ctrl+") {
                chord.command = true;
                rest = after;
            } else if let Some(after) = rest.strip_prefix("Shift+") {
                chord.shift = true;
                rest = after;
            } else if let Some(after) = rest.strip_prefix("Alt+") {
                chord.alt = true;
                rest = after;
            } else {
                break;
            }
        }
        chord.key = match rest {
            "?" => Key::Questionmark,
            "`" => Key::Backtick,
            "+" => Key::Plus,
            "-" => Key::Minus,
            "=" => Key::Equals,
            "," => Key::Comma,
            "." => Key::Period,
            "/" => Key::Slash,
            "[" => Key::OpenBracket,
            "]" => Key::CloseBracket,
            "{" => Key::OpenCurlyBracket,
            "}" => Key::CloseCurlyBracket,
            "Del" => Key::Delete,
            name => Key::from_name(name).ok_or_else(|| format!("`{text}`: `{name}` is not a key"))?,
        };
        Ok(chord)
    }

    /// How data writes it: the inverse of [`Self::parse`], for the `?` sheet and the menus.
    #[must_use]
    pub fn text(self) -> String {
        let key = match self.key {
            Key::Questionmark => "?".to_owned(),
            Key::Backtick => "`".to_owned(),
            Key::Plus => "+".to_owned(),
            Key::Minus => "-".to_owned(),
            Key::Equals => "=".to_owned(),
            Key::Comma => ",".to_owned(),
            Key::Period => ".".to_owned(),
            Key::Slash => "/".to_owned(),
            Key::OpenBracket => "[".to_owned(),
            Key::CloseBracket => "]".to_owned(),
            Key::OpenCurlyBracket => "{".to_owned(),
            Key::CloseCurlyBracket => "}".to_owned(),
            key => key.name().to_owned(),
        };
        format!(
            "{}{}{}{key}",
            if self.command { "Ctrl+" } else { "" },
            if self.shift { "Shift+" } else { "" },
            if self.alt { "Alt+" } else { "" }
        )
    }

    /// Whether `press` is exactly this chord.
    #[must_use]
    pub fn exactly(self, press: Press) -> bool {
        press.key == self.key
            && press.modifiers.command == self.command
            && press.modifiers.shift == self.shift
            && press.modifiers.alt == self.alt
    }

    /// Whether `press` is this chord with Shift and Alt not counted.
    #[must_use]
    pub fn loosely(self, press: Press) -> bool {
        press.key == self.key && press.modifiers.command == self.command && !self.shift && !self.alt
    }
}

/// A shortcut: in `scope` (a node's id, or [`GLOBAL`]), `chord` runs the action `action`.
#[derive(Clone, Debug, PartialEq)]
pub struct Shortcut {
    /// Where it works: a scope's id (anywhere inside it, in either mode), or [`GLOBAL`].
    pub scope: String,
    /// The keys.
    pub chord: Chord,
    /// The action's id.
    pub action: String,
}

/// The scope of a key that works wherever the focus is (the spec's "global keys").
pub const GLOBAL: &str = "global";

/// What the frame is asked to do after a key. The router decides; the frame carries it out.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Press this element as a click would.
    Click(Id),
    /// Adjust this element by `steps` (already multiplied) along `axis` (0 x, 1 y, 2 z): an
    /// arrow in interaction mode. For a number only `axis` 0 or 1 come, both meaning "more or
    /// less"; a canvas or a session reads the axis.
    Adjust {
        /// What is adjusted.
        id: Id,
        /// Which way: 0 for ← →, 1 for ↑ ↓, 2 for Page Up / Page Down.
        axis: usize,
        /// How far, in the widget's steps: ±1, ±10 with Shift, ±0.1 with Alt.
        steps: f64,
        /// Ctrl was held: on a canvas the arrows pan rather than orbit.
        ctrl: bool,
    },
    /// Zoom a canvas by `steps` fixed log₂ steps (`+` and `-` in interaction mode).
    Zoom {
        /// The canvas.
        id: Id,
        /// How many steps, signed.
        steps: f64,
    },
    /// Interaction with this element has begun (a canvas, a number, a session).
    Interact(Id),
    /// Interaction with this element has ended; `keep` is false when a transform session was
    /// cancelled with Esc.
    Release {
        /// The element.
        id: Id,
        /// Whether what the interaction did is kept.
        keep: bool,
    },
    /// Start typing in this text field.
    Type(Id),
    /// Leave the text field being typed in (Tab: the router has already moved on).
    StopTyping,
    /// A choice of this drop-down was picked, from 0.
    Pick {
        /// The drop-down.
        id: Id,
        /// The choice.
        index: usize,
    },
    /// Run the action with this id (a shortcut or a global key).
    Action(String),
    /// A key for an open transform session: a character typed into its distance, an axis lock,
    /// or Tab to the next axis.
    Session(SessionKey),
    /// Close this window; the focus has gone back to where it was before it opened.
    Close(Id),
    /// Esc at a big scope's top level in navigation mode: the frame's own ladder (leave the
    /// hidden GUI, clear the selection).
    Escape,
    /// The focus has moved onto this big scope by Tab: if it is hidden (a maximised pane, a
    /// collapsed panel) the frame shows it first, so that nothing happens unseen.
    Reveal(Id),
}

/// A key inside a transform session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionKey {
    /// A character of the typed distance: a digit, `-`, `.`; `\u{8}` is Backspace.
    Char(char),
    /// Lock to an axis, 0 x, 1 y, 2 z (again to unlock).
    Lock(usize),
    /// Tab: type the next axis's distance.
    NextAxis,
}

/// A drop-down opened from the keyboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Popup {
    /// The drop-down's id.
    pub id: Id,
    /// How many choices.
    pub count: usize,
    /// The highlighted one, from 0.
    pub at: usize,
}

/// What a session in interaction mode on the viewport is doing, which decides who owns the keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Session {
    /// Moving the selection (`Kind::Session` in interaction mode).
    Transform,
    /// Flying the camera: W A S D, E, Q are the session's (held, read by the frame), and every
    /// other letter does nothing (no tool changes mid-flight).
    Fly,
}

/// The keyboard's state: UI only (the spec's §11).
#[derive(Clone, Debug, PartialEq)]
pub struct Keyboard {
    /// Whether keyboard navigation is on: the ring and the modes are shown and active.
    pub nav_on: bool,
    /// Which mode.
    pub mode: Mode,
    /// Where the keys go: from a root (a big scope or an open window) down to the focused node.
    pub focus: Vec<Id>,
    /// Whether a text field is being typed in (the frame keeps this true to egui's own focus).
    pub typing: bool,
    /// An open drop-down.
    pub popup: Option<Popup>,
    /// A session that owns the viewport's keys.
    pub session: Option<Session>,
    /// Where the focus goes back to when the open window closes.
    pub before_window: Option<Vec<Id>>,
}

/// Where the focus starts: the viewport (so `1`–`9` select a tower, as before the model).
pub const START: &str = "viewport";

impl Default for Keyboard {
    fn default() -> Self {
        Self {
            nav_on: false,
            mode: Mode::Navigation,
            focus: vec![START.to_owned()],
            typing: false,
            popup: None,
            session: None,
            before_window: None,
        }
    }
}

/// Keys that do not by themselves count as a key press (the spec's §2).
fn is_modifier_only(_key: Key) -> bool {
    // egui sends no key events for modifiers on their own; kept as the spec's rule in one place.
    false
}

impl Keyboard {
    /// The focused node's id.
    #[must_use]
    pub fn focused(&self) -> &str {
        self.focus.last().map_or(START, String::as_str)
    }

    /// Whether `id` is on the focus path (it or something inside it has the focus).
    #[must_use]
    pub fn within(&self, id: &str) -> bool {
        self.focus.iter().any(|f| f == id)
    }

    /// A mouse click or drag: keyboard navigation goes off and the ring hides. The focus stays
    /// where it was (the spec's §2, §8); a click that ends typing is the frame's to see.
    pub fn pointer(&mut self) {
        self.nav_on = false;
    }

    /// Moves the focus onto `id` (a click on a control in a window, the palette choosing a field),
    /// in navigation mode.
    pub fn focus_on(&mut self, tree: &FocusTree, id: &str) {
        if let Some(path) = tree.path_to(id) {
            self.focus = path;
            self.mode = Mode::Navigation;
            self.popup = None;
            self.session = None;
        }
    }

    /// A window has opened: it takes the focus at its first section `first` (its normal
    /// landing), remembering where the focus was. `keyboard`: opened from the keyboard, which
    /// turns navigation on (the spec's §8). The window need not have been drawn yet: it is
    /// registered when it is, before the next key.
    pub fn open_window(&mut self, id: &str, first: &str, keyboard: bool) {
        // One window at a time holds the focus; the first's return point is kept when another
        // opens over it.
        if self.before_window.is_none() {
            self.before_window = Some(self.focus.clone());
        }
        self.focus = vec![id.to_owned(), first.to_owned()];
        self.mode = Mode::Navigation;
        self.popup = None;
        self.session = None;
        if keyboard {
            self.nav_on = true;
        }
    }

    /// The focused window has closed (by Esc, its ×, or its own action): the focus goes back.
    pub fn close_window(&mut self) {
        self.focus = self.before_window.take().unwrap_or_else(|| vec![START.to_owned()]);
        self.mode = Mode::Navigation;
        self.popup = None;
        self.typing = false;
    }

    /// The focus as far as `tree` still has it: a node that is gone (a deleted body's row)
    /// gives way to its nearest ancestor that is there, and a window that is gone to where the
    /// focus was before it. Called before each key.
    pub fn repair(&mut self, tree: &FocusTree) {
        if let Some(root) = self.focus.first()
            && tree.node(root).is_none()
            && self.before_window.is_some()
        {
            self.close_window();
        }
        while self.focus.len() > 1 && tree.node(self.focused()).is_none() {
            self.focus.pop();
            self.mode = Mode::Navigation;
            self.session = None;
        }
        if self.focus.is_empty() || tree.node(self.focused()).is_none() {
            self.focus = vec![START.to_owned()];
            self.mode = Mode::Navigation;
        }
    }

    /// Routes one key. Returns what the frame should do, and whether the key was used (a key
    /// that was not is left to egui: what a text field types).
    ///
    /// The priority is the spec's §11: (1) a text field being typed in; (2) an open drop-down;
    /// (3) global keys; (4) the scope shortcuts along the focus path, innermost first; (5) the
    /// modes (Tab, Enter, Esc, arrows). Navigation turns on *after* the key has acted, and Enter
    /// with navigation off only turns it on.
    pub fn route(&mut self, tree: &FocusTree, shortcuts: &[Shortcut], press: Press) -> (Vec<Effect>, bool) {
        self.repair(tree);
        let was_on = self.nav_on;
        let outcome = if !was_on && press.key == Key::Enter && !self.typing && self.mode == Mode::Navigation {
            // Nothing unseen is ever pressed: this Enter only shows where the focus is.
            (Vec::new(), true)
        } else {
            self.route_on(tree, shortcuts, press)
        };
        if !is_modifier_only(press.key) {
            self.nav_on = true;
        }
        outcome
    }

    fn route_on(&mut self, tree: &FocusTree, shortcuts: &[Shortcut], press: Press) -> (Vec<Effect>, bool) {
        // (1) Typing: every key is the field's but Esc, Enter and Tab.
        if self.typing {
            return match press.key {
                Key::Escape | Key::Enter => {
                    // The field keeps its text (egui sees the key and lets the field go).
                    self.typing = false;
                    self.mode = Mode::Navigation;
                    (Vec::new(), false)
                }
                Key::Tab => {
                    self.typing = false;
                    self.mode = Mode::Navigation;
                    let mut effects = vec![Effect::StopTyping];
                    effects.extend(self.tab(tree, press.modifiers.shift));
                    (effects, true)
                }
                _ => (Vec::new(), false),
            };
        }
        // (2) An open drop-down.
        if let Some(popup) = &mut self.popup {
            let effects = match press.key {
                Key::ArrowUp | Key::ArrowLeft => {
                    popup.at = popup.at.saturating_sub(1);
                    Vec::new()
                }
                Key::ArrowDown | Key::ArrowRight => {
                    popup.at = (popup.at + 1).min(popup.count.saturating_sub(1));
                    Vec::new()
                }
                Key::Enter => {
                    let picked = Effect::Pick { id: popup.id.clone(), index: popup.at };
                    self.popup = None;
                    self.mode = Mode::Navigation;
                    vec![picked]
                }
                Key::Escape => {
                    self.popup = None;
                    self.mode = Mode::Navigation;
                    Vec::new()
                }
                _ => Vec::new(),
            };
            return (effects, true);
        }
        // A session owns its keys before anything but the globals.
        let global = find(shortcuts, GLOBAL, press);
        if let Some(session) = self.session
            && self.mode == Mode::Interaction
            && global.is_none()
            && let Some(effects) = self.session_key(session, press)
        {
            return (effects, true);
        }
        // (3) Global keys.
        if let Some(action) = global {
            return (vec![Effect::Action(action)], true);
        }
        // (4) Scope shortcuts, innermost scope first.
        for scope in self.focus.iter().rev() {
            if let Some(action) = find(shortcuts, scope, press) {
                return (vec![Effect::Action(action)], true);
            }
        }
        // (5) The modes.
        self.mode_key(tree, press)
    }

    /// A key in a session, or `None` when the session leaves it to the rest of the router: only
    /// the arrows, which the mode handling turns into adjustments of the session. Every other key
    /// that is not a global is the session's, used or eaten, so that nothing else acts while it
    /// is open (S18's review, R1: Delete inside a transform session removed the body, and Esc
    /// then moved another).
    fn session_key(&mut self, session: Session, press: Press) -> Option<Vec<Effect>> {
        let id = self.focused().to_owned();
        let arrow = matches!(press.key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown);
        match session {
            Session::Transform => {
                let typed = match press.key {
                    Key::Num0 => Some('0'),
                    Key::Num1 => Some('1'),
                    Key::Num2 => Some('2'),
                    Key::Num3 => Some('3'),
                    Key::Num4 => Some('4'),
                    Key::Num5 => Some('5'),
                    Key::Num6 => Some('6'),
                    Key::Num7 => Some('7'),
                    Key::Num8 => Some('8'),
                    Key::Num9 => Some('9'),
                    Key::Minus => Some('-'),
                    Key::Period => Some('.'),
                    Key::Backspace => Some('\u{8}'),
                    _ => None,
                };
                if let Some(c) = typed {
                    return Some(vec![Effect::Session(SessionKey::Char(c))]);
                }
                match press.key {
                    Key::X => Some(vec![Effect::Session(SessionKey::Lock(0))]),
                    Key::Y => Some(vec![Effect::Session(SessionKey::Lock(1))]),
                    Key::Z if !press.modifiers.command => Some(vec![Effect::Session(SessionKey::Lock(2))]),
                    Key::Tab => Some(vec![Effect::Session(SessionKey::NextAxis)]),
                    Key::Enter | Key::Escape => {
                        self.mode = Mode::Navigation;
                        self.session = None;
                        Some(vec![Effect::Release { id, keep: press.key == Key::Enter }])
                    }
                    Key::PageUp | Key::PageDown => {
                        let sign = if press.key == Key::PageUp { 1.0 } else { -1.0 };
                        Some(vec![Effect::Adjust {
                            id,
                            axis: 2,
                            steps: sign * press.multiplier(),
                            ctrl: false,
                        }])
                    }
                    _ if arrow => None,
                    // Every other key is the session's too: nothing changes tool, deletes or
                    // duplicates mid-move.
                    _ => Some(Vec::new()),
                }
            }
            Session::Fly => match press.key {
                // Held keys: the frame reads them each frame; the press itself does nothing more.
                Key::W | Key::A | Key::S | Key::D | Key::E | Key::Q => Some(Vec::new()),
                Key::Enter | Key::Escape => {
                    // Both keep the camera where it was flown (the spec's Esc rule; Home or the
                    // history's Home step bring it back).
                    self.mode = Mode::Navigation;
                    self.session = None;
                    Some(vec![Effect::Release { id, keep: true }])
                }
                _ if arrow => None,
                _ => Some(Vec::new()),
            },
        }
    }

    fn mode_key(&mut self, tree: &FocusTree, press: Press) -> (Vec<Effect>, bool) {
        let id = self.focused().to_owned();
        let node = tree.node(&id).cloned();
        let kind = node.as_ref().map_or(Kind::Scope, |n| n.kind);
        let enabled = node.as_ref().is_none_or(|n| n.enabled);
        match press.key {
            Key::Tab => (self.tab(tree, press.modifiers.shift), true),
            Key::Enter => match self.mode {
                Mode::Interaction => {
                    self.mode = Mode::Navigation;
                    self.session = None;
                    (vec![Effect::Release { id, keep: true }], true)
                }
                Mode::Navigation => (self.enter(tree, &id, kind, enabled), true),
            },
            Key::Escape => match self.mode {
                Mode::Interaction => {
                    self.mode = Mode::Navigation;
                    let keep = kind != Kind::Session;
                    self.session = None;
                    (vec![Effect::Release { id, keep }], true)
                }
                Mode::Navigation => {
                    let in_window = self.focus.first().is_some_and(|root| tree.is_window(root));
                    if in_window && self.focus.len() <= 2 {
                        let window = self.focus[0].clone();
                        self.close_window();
                        (vec![Effect::Close(window)], true)
                    } else if self.focus.len() > 1 {
                        self.focus.pop();
                        (Vec::new(), true)
                    } else {
                        (vec![Effect::Escape], true)
                    }
                }
            },
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown => {
                let back = matches!(press.key, Key::ArrowLeft | Key::ArrowUp);
                match self.mode {
                    Mode::Navigation => {
                        self.step(tree, back);
                        (Vec::new(), true)
                    }
                    Mode::Interaction => {
                        let axis = usize::from(matches!(press.key, Key::ArrowUp | Key::ArrowDown));
                        // Right and up are "more".
                        let sign =
                            if matches!(press.key, Key::ArrowRight | Key::ArrowUp) { 1.0 } else { -1.0 };
                        let ctrl = press.modifiers.command;
                        (vec![Effect::Adjust { id, axis, steps: sign * press.multiplier(), ctrl }], true)
                    }
                }
            }
            Key::PageUp | Key::PageDown if self.mode == Mode::Interaction => {
                let sign = if press.key == Key::PageUp { 1.0 } else { -1.0 };
                (
                    vec![Effect::Adjust {
                        id,
                        axis: 2,
                        steps: sign * 10.0 * press.multiplier(),
                        ctrl: false,
                    }],
                    true,
                )
            }
            Key::Plus | Key::Equals | Key::Minus
                if self.mode == Mode::Interaction && kind == Kind::Canvas =>
            {
                let sign = if press.key == Key::Minus { -1.0 } else { 1.0 };
                (vec![Effect::Zoom { id, steps: sign * press.multiplier() }], true)
            }
            _ => (Vec::new(), false),
        }
    }

    /// Enter in navigation mode on the focused node (the spec's §3 and §4).
    fn enter(&mut self, tree: &FocusTree, id: &str, kind: Kind, enabled: bool) -> Vec<Effect> {
        if !enabled {
            return Vec::new();
        }
        match kind {
            Kind::Scope => {
                if let Some(first) = tree.children(id).first() {
                    self.focus.push((*first).to_owned());
                }
                Vec::new()
            }
            Kind::Button => vec![Effect::Click(id.to_owned())],
            Kind::Adjust | Kind::Canvas => {
                self.mode = Mode::Interaction;
                vec![Effect::Interact(id.to_owned())]
            }
            Kind::Session => {
                self.mode = Mode::Interaction;
                self.session = Some(Session::Transform);
                vec![Effect::Interact(id.to_owned())]
            }
            Kind::Text => {
                self.mode = Mode::Interaction;
                self.typing = true;
                vec![Effect::Type(id.to_owned())]
            }
            Kind::Choice { count, current } => {
                self.mode = Mode::Interaction;
                self.popup =
                    Some(Popup { id: id.to_owned(), count, at: current.min(count.saturating_sub(1)) });
                Vec::new()
            }
            Kind::ReadOnly => Vec::new(),
        }
    }

    /// Tab or Shift+Tab: the next big scope (wrapping), or inside a window its next section.
    fn tab(&mut self, tree: &FocusTree, back: bool) -> Vec<Effect> {
        self.mode = Mode::Navigation;
        self.popup = None;
        self.session = None;
        let root = self.focus.first().cloned().unwrap_or_else(|| START.to_owned());
        if tree.is_window(&root) {
            let sections = tree.children(&root);
            if sections.is_empty() {
                self.focus = vec![root];
                return Vec::new();
            }
            let now = self.focus.get(1).and_then(|s| sections.iter().position(|x| x == s));
            let next = cycle(now, sections.len(), back);
            self.focus = vec![root, sections[next].to_owned()];
            return Vec::new();
        }
        let big = tree.big_scopes();
        let now = big.iter().position(|b| *b == root);
        let next = cycle(now, big.len(), back);
        let id = big[next].to_owned();
        self.focus = vec![id.clone()];
        vec![Effect::Reveal(id)]
    }

    /// An arrow in navigation mode: the previous or next sibling, stopping at the ends.
    fn step(&mut self, tree: &FocusTree, back: bool) {
        let id = self.focused().to_owned();
        let siblings = tree.siblings(&id);
        let Some(at) = siblings.iter().position(|s| *s == id) else { return };
        let next = if back { at.checked_sub(1) } else { (at + 1 < siblings.len()).then_some(at + 1) };
        if let Some(next) = next {
            let next = siblings[next].to_owned();
            *self.focus.last_mut().expect("the focus is never empty") = next;
        }
    }
}

/// The next index round a ring of `len`, from `now` (or the start), forwards or back.
fn cycle(now: Option<usize>, len: usize, back: bool) -> usize {
    match (now, back) {
        (None, false) => 0,
        (None, true) => len - 1,
        (Some(at), false) => (at + 1) % len,
        (Some(at), true) => (at + len - 1) % len,
    }
}

/// The action bound to `press` in `scope`: an exact chord first, then one that ignores Shift and
/// Alt (modifier hygiene: Shift+Home is Home, Shift+F is fly when bound).
fn find(shortcuts: &[Shortcut], scope: &str, press: Press) -> Option<String> {
    let here = || shortcuts.iter().filter(|s| s.scope == scope);
    here()
        .find(|s| s.chord.exactly(press))
        .or_else(|| here().find(|s| s.chord.loosely(press)))
        .map(|s| s.action.clone())
}

/// Whether `press` is a global key of `shortcuts` (one that works wherever the focus is).
#[must_use]
pub fn is_global(shortcuts: &[Shortcut], press: Press) -> bool {
    find(shortcuts, GLOBAL, press).is_some()
}

/// Keys that repeat when held (the spec's DAS/ARR): the arrows, Page Up/Down, `+`, `-`,
/// Backspace, and the brackets (the transport's steps: a held `[` or `]` scrubs shot by shot).
/// A reset key (Home) and the other shortcuts fire once.
#[must_use]
pub fn repeats(key: Key) -> bool {
    matches!(
        key,
        Key::ArrowLeft
            | Key::ArrowRight
            | Key::ArrowUp
            | Key::ArrowDown
            | Key::PageUp
            | Key::PageDown
            | Key::Plus
            | Key::Minus
            | Key::Equals
            | Key::Backspace
            | Key::OpenBracket
            | Key::CloseBracket
            | Key::OpenCurlyBracket
            | Key::CloseCurlyBracket
    )
}

/// The delay before a held key repeats, seconds (the spec: 500 ms).
pub const REPEAT_DELAY: f64 = 0.5;
/// The time between repeats after it, seconds (the spec: 40 ms).
pub const REPEAT_EVERY: f64 = 0.04;

/// Held keys and when each next fires: Tetris's DAS/ARR on the frame's clock (seconds), so a
/// test can hold a key without a real clock.
#[derive(Clone, Debug)]
pub struct Repeat {
    held: Vec<(Press, f64, u32)>,
    /// The delay before the first repeat, seconds ([`REPEAT_DELAY`]).
    delay: f64,
    /// The time between repeats after it, seconds ([`REPEAT_EVERY`]).
    every: f64,
}

impl Default for Repeat {
    fn default() -> Self {
        Self::with_clock(REPEAT_DELAY, REPEAT_EVERY)
    }
}

impl Repeat {
    /// A repeat on another clock: `delay` seconds before the first repeat, then one every
    /// `every` seconds. The application uses [`Repeat::default`] (the spec's 500 ms and 40 ms);
    /// this is for a test that needs a broken one as its control.
    #[must_use]
    pub const fn with_clock(delay: f64, every: f64) -> Self {
        Self { held: Vec::new(), delay, every }
    }

    /// A key went down at `t`: it fires now (the caller routes it) and, if it repeats, is held.
    pub fn press(&mut self, press: Press, t: f64) {
        self.release(press.key);
        if repeats(press.key) {
            self.held.push((press, t, 1));
        }
    }

    /// A key came up.
    pub fn release(&mut self, key: Key) {
        self.held.retain(|(p, _, _)| p.key != key);
    }

    /// Lets go of everything (the window lost focus, a click).
    pub fn clear(&mut self) {
        self.held.clear();
    }

    /// Whether any key is held.
    #[must_use]
    pub fn any(&self) -> bool {
        !self.held.is_empty()
    }

    /// The repeats due by `t`, each to be routed as a press: one at `REPEAT_DELAY` after the key
    /// went down, then one every `REPEAT_EVERY`.
    pub fn due(&mut self, t: f64) -> Vec<Press> {
        let mut out = Vec::new();
        let (delay, every) = (self.delay, self.every);
        for (press, down, fired) in &mut self.held {
            let held = t - *down;
            let total = if held < delay {
                1
            } else {
                // A small allowance so that a frame landing on a repeat's instant counts it.
                2 + ((held - delay + 1e-9) / every).floor() as u32
            };
            while *fired < total {
                out.push(*press);
                *fired += 1;
            }
        }
        out
    }
}

/// The base step of a number of `value` (the spec's "one table" of steps): a tenth of its
/// decade, so 2.5 m steps by 0.1 m and 70 kg by 1 kg; a whole number by 1; an angle by 1°; a
/// zero by 0.1 of its unit. Shift and Alt multiply it (×10, ×0.1) through [`Press::multiplier`].
#[must_use]
pub fn step_for(value: f64, unit: Option<&str>, whole: bool) -> f64 {
    if whole {
        return 1.0;
    }
    if unit == Some("°") {
        return 1.0;
    }
    if value == 0.0 || !value.is_finite() {
        return 0.1;
    }
    10f64.powf(value.abs().log10().floor() - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIG: [(&str, &str); 7] = [
        ("top-bar", "Top bar"),
        ("toolbar", "Tools"),
        ("tree", "Scene tree"),
        ("viewport", "Viewport"),
        ("inspector", "Inspector"),
        ("timeline", "Timeline"),
        ("footer", "Footer"),
    ];

    /// A tree like the application's: a few elements under each scope.
    fn tree() -> FocusTree {
        let mut t = FocusTree::new(&BIG);
        t.add("toolbar", "tool/select", "Select", Kind::Button);
        t.add("toolbar", "tool/move", "Move", Kind::Button);
        t.add("tree", "tree/bodies", "Bodies", Kind::Scope);
        t.add("tree/bodies", "row/body 1", "ball", Kind::Button);
        t.add("tree/bodies", "row/body 2", "mass", Kind::Button);
        t.add("viewport", "canvas", "Canvas", Kind::Canvas);
        t.add("viewport", "selection", "Selection", Kind::Session);
        t.add("viewport", "legend", "Legend", Kind::ReadOnly);
        t.add("inspector", "inspector/body", "Body", Kind::Scope);
        t.add("inspector/body", "field/mass", "mass", Kind::Adjust);
        t.add("inspector/body", "field/label", "label", Kind::Text);
        t.add("inspector/body", "field/motion", "motion", Kind::Choice { count: 7, current: 2 });
        t.add("inspector/body", "field/reset", "reset", Kind::Button);
        t.add("timeline", "scrubber", "Scrubber", Kind::Adjust);
        t.add("footer", "footer/console", "console", Kind::Button);
        t
    }

    fn shortcuts() -> Vec<Shortcut> {
        let s = |scope: &str, chord: &str, action: &str| Shortcut {
            scope: scope.into(),
            chord: Chord::parse(chord).unwrap(),
            action: action.into(),
        };
        vec![
            s(GLOBAL, "Ctrl+Z", "undo"),
            s(GLOBAL, "Ctrl+Shift+Z", "redo"),
            s(GLOBAL, "Home", "view-home"),
            s(GLOBAL, "?", "keys"),
            s("toolbar", "W", "tool-move"),
            s("viewport", "W", "tool-move"),
            s("viewport", "F", "frame"),
            s("viewport", "Shift+F", "fly"),
            s("tree", "Delete", "delete"),
            s("viewport", "Delete", "delete-in-view"),
            s("viewport", "Ctrl+D", "duplicate-in-view"),
            s("tree/bodies", "A", "add-body"),
            s("tree", "A", "add"),
        ]
    }

    fn key(k: Key) -> Press {
        Press::plain(k)
    }

    #[test]
    fn a_key_acts_then_shows_the_ring() {
        let (t, s) = (tree(), shortcuts());
        let mut kb = Keyboard::default();
        assert!(!kb.nav_on, "the app starts with navigation off");
        let (effects, used) = kb.route(&t, &s, key(Key::W));
        assert!(used);
        assert_eq!(effects, vec![Effect::Action("tool-move".into())], "the key acted");
        assert!(kb.nav_on, "and then the ring shows");
        // Control: the ring shows where the focus already was; the key did not move it.
        assert_eq!(kb.focus, vec!["viewport".to_owned()]);
        // Control of the order: a router that turned the ring on *before* acting would make a
        // first Enter press what is focused. The real one does not; one already on does.
        let mut first =
            Keyboard { focus: vec!["footer".into(), "footer/console".into()], ..Keyboard::default() };
        assert!(first.route(&t, &s, key(Key::Enter)).0.is_empty());
        let mut on_first = Keyboard {
            focus: vec!["footer".into(), "footer/console".into()],
            nav_on: true,
            ..Keyboard::default()
        };
        assert_eq!(on_first.route(&t, &s, key(Key::Enter)).0, vec![Effect::Click("footer/console".into())]);
        // A click hides it again; the next key shows it where the focus is.
        kb.pointer();
        assert!(!kb.nav_on);
        kb.route(&t, &s, key(Key::F));
        assert!(kb.nav_on);
    }

    #[test]
    fn enter_with_navigation_off_presses_nothing() {
        let (t, s) = (tree(), shortcuts());
        let mut kb =
            Keyboard { focus: vec!["footer".into(), "footer/console".into()], ..Keyboard::default() };
        let (effects, _) = kb.route(&t, &s, key(Key::Enter));
        assert!(effects.is_empty(), "nothing unseen is pressed: {effects:?}");
        assert!(kb.nav_on);
        // Control: the same Enter with the ring showing clicks.
        let (effects, _) = kb.route(&t, &s, key(Key::Enter));
        assert_eq!(effects, vec![Effect::Click("footer/console".into())]);
        assert_eq!(kb.mode, Mode::Navigation, "a click stays in navigation mode");
    }

    #[test]
    fn a_click_hides_the_ring_and_keeps_the_focus() {
        let (t, s) = (tree(), shortcuts());
        let mut kb = Keyboard::default();
        kb.route(&t, &s, key(Key::Tab));
        let before = kb.focus.clone();
        assert!(kb.nav_on);
        kb.pointer();
        assert!(!kb.nav_on, "a click hides the ring");
        assert_eq!(kb.focus, before, "and leaves the focus where it was");
        // A click on a window's control moves the focus there, by rule.
        kb.focus_on(&t, "field/mass");
        assert_eq!(kb.focus, ["inspector", "inspector/body", "field/mass"]);
    }

    #[test]
    fn tab_walks_the_big_scopes_in_order() {
        let (t, s) = (tree(), shortcuts());
        let mut kb = Keyboard { focus: vec!["top-bar".into()], nav_on: true, ..Keyboard::default() };
        let mut seen = Vec::new();
        for _ in 0..14 {
            let (effects, _) = kb.route(&t, &s, key(Key::Tab));
            assert_eq!(
                effects,
                vec![Effect::Reveal(kb.focused().to_owned())],
                "each landing reveals its scope"
            );
            seen.push(kb.focused().to_owned());
        }
        let order: Vec<&str> = BIG.iter().map(|(id, _)| *id).collect();
        let expected: Vec<&str> = order.iter().cycle().skip(1).take(14).copied().collect();
        assert_eq!(seen, expected, "Tab wraps from the footer to the top bar");
        // Shift+Tab goes back, and Tab leaves an element for its big scope's neighbour, from
        // either mode.
        kb.focus = vec!["inspector".into(), "inspector/body".into(), "field/mass".into()];
        kb.mode = Mode::Interaction;
        kb.route(&t, &s, Press::with(Key::Tab, Modifiers::SHIFT));
        assert_eq!((kb.focused(), kb.mode), ("viewport", Mode::Navigation));
        // Control: a big scope with nothing inside (an empty, hidden panel) is still a stop.
        let mut sparse = FocusTree::new(&BIG);
        sparse.add("tree", "row", "row", Kind::Button);
        let mut kb = Keyboard { focus: vec!["top-bar".into()], nav_on: true, ..Keyboard::default() };
        kb.route(&sparse, &s, key(Key::Tab));
        assert_eq!(kb.focused(), "toolbar", "the empty toolbar is not skipped");
    }

    #[test]
    fn enter_goes_in_and_esc_comes_out() {
        let (t, s) = (tree(), shortcuts());
        let mut kb = Keyboard { focus: vec!["inspector".into()], nav_on: true, ..Keyboard::default() };
        kb.route(&t, &s, key(Key::Enter));
        assert_eq!(kb.focused(), "inspector/body", "Enter on a scope lands on its first element");
        kb.route(&t, &s, key(Key::Enter));
        assert_eq!(kb.focused(), "field/mass");
        assert_eq!(kb.mode, Mode::Navigation, "every landing is in navigation mode");
        let (effects, _) = kb.route(&t, &s, key(Key::Enter));
        assert_eq!((effects, kb.mode), (vec![Effect::Interact("field/mass".into())], Mode::Interaction));
        let (effects, _) = kb.route(&t, &s, key(Key::Escape));
        assert_eq!(effects, vec![Effect::Release { id: "field/mass".into(), keep: true }]);
        assert_eq!((kb.focused(), kb.mode), ("field/mass", Mode::Navigation));
        kb.route(&t, &s, key(Key::Escape));
        assert_eq!(kb.focused(), "inspector/body", "Esc goes up one scope");
        kb.route(&t, &s, key(Key::Escape));
        assert_eq!(kb.focused(), "inspector");
        let (effects, _) = kb.route(&t, &s, key(Key::Escape));
        assert_eq!(effects, vec![Effect::Escape], "at a big scope Esc is the frame's ladder");
        // The canvas is itself the interactive thing: Enter starts interaction with it.
        let mut kb =
            Keyboard { focus: vec!["viewport".into(), "canvas".into()], nav_on: true, ..Keyboard::default() };
        kb.route(&t, &s, key(Key::Enter));
        assert_eq!(kb.mode, Mode::Interaction);
        // A read-only leaf: Enter does nothing.
        let mut kb =
            Keyboard { focus: vec!["viewport".into(), "legend".into()], nav_on: true, ..Keyboard::default() };
        let (effects, _) = kb.route(&t, &s, key(Key::Enter));
        assert!(effects.is_empty() && kb.mode == Mode::Navigation);
        // The reset is a section's last element, so Enter into the section lands on a real control.
        assert_eq!(t.children("inspector/body").last(), Some(&"field/reset"));
    }

    #[test]
    fn arrows_move_or_adjust_and_the_multipliers_are_exact() {
        let (t, s) = (tree(), shortcuts());
        let mut kb = Keyboard {
            focus: vec!["inspector".into(), "inspector/body".into(), "field/mass".into()],
            nav_on: true,
            ..Keyboard::default()
        };
        kb.route(&t, &s, key(Key::ArrowDown));
        assert_eq!(kb.focused(), "field/label", "navigation: arrows move between siblings");
        kb.route(&t, &s, key(Key::ArrowUp));
        kb.route(&t, &s, key(Key::ArrowUp));
        assert_eq!(kb.focused(), "field/mass", "and stop at the ends");
        kb.route(&t, &s, key(Key::Enter));
        let adjust = |kb: &mut Keyboard, modifiers: Modifiers| {
            let (effects, _) = kb.route(&t, &s, Press::with(Key::ArrowRight, modifiers));
            match effects.as_slice() {
                [Effect::Adjust { steps, axis: 0, .. }] => *steps,
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(adjust(&mut kb, Modifiers::NONE), 1.0);
        assert_eq!(adjust(&mut kb, Modifiers::SHIFT), 10.0);
        assert_eq!(adjust(&mut kb, Modifiers::ALT), 0.1);
        assert_eq!(kb.focused(), "field/mass", "interaction: arrows adjust, the focus stays");
        // The one step table: a tenth of a value's decade.
        assert_eq!(step_for(2.5, Some("m"), false), 0.1);
        assert!((step_for(70.0, Some("kg"), false) - 1.0).abs() < 1e-15);
        assert_eq!(step_for(3.0, Some("1"), true), 1.0);
        assert_eq!(step_for(45.0, Some("°"), false), 1.0);
        assert_eq!(step_for(0.0, Some("m"), false), 0.1);
        let atom = step_for(1.443e-25, Some("kg"), false);
        assert!((atom / 1e-26 - 1.0).abs() < 1e-9, "{atom}");
    }

    #[test]
    fn a_held_key_repeats_on_the_tetris_clock() {
        // Held for a second at 60 frames a second, through a `Repeat` on the given clock.
        let fires = |mut repeat: Repeat| {
            repeat.press(key(Key::ArrowRight), 0.0);
            let mut count = 1; // the press itself
            let mut t = 0.0_f64;
            while t < 1.0 - 1e-12 {
                t += 1.0 / 60.0;
                count += repeat.due(t.min(1.0)).len();
            }
            count + repeat.due(1.0).len()
        };
        assert_eq!(fires(Repeat::default()), 14, "1 + 1 at 500 ms + 12 more by 1000 ms");
        // The control: the same machinery with no delay before the repeats (S18's review, minor
        // 9: a broken `Repeat`, driven, where the first control was a closed form).
        // With no delay the first repeat comes with the press, then one every 40 ms to 1000 ms: 1 + 26
        // (the brief's closed form said 26, leaving out the repeat at 0 ms).
        assert_eq!(fires(Repeat::with_clock(0.0, REPEAT_EVERY)), 27, "no delay gives more");
        // Shortcuts and Home do not repeat.
        let mut repeat = Repeat::default();
        repeat.press(key(Key::Home), 0.0);
        assert!(repeat.due(2.0).is_empty());
        // Letting go stops it.
        repeat.press(key(Key::ArrowLeft), 0.0);
        repeat.release(Key::ArrowLeft);
        assert!(repeat.due(2.0).is_empty());
    }

    #[test]
    fn the_router_s_priorities() {
        let (t, s) = (tree(), shortcuts());
        // (1) over everything: typing keeps `?`, Home and W for the field.
        let mut kb = Keyboard { typing: true, nav_on: true, ..Keyboard::default() };
        for k in [Key::Questionmark, Key::Home, Key::W] {
            assert_eq!(kb.route(&t, &s, key(k)), (Vec::new(), false), "{k:?} goes to the field");
        }
        // (2) over (3): an open drop-down takes the arrows, Enter picks, Home is swallowed.
        let mut kb = Keyboard {
            focus: vec!["inspector".into(), "inspector/body".into(), "field/motion".into()],
            nav_on: true,
            ..Keyboard::default()
        };
        kb.route(&t, &s, key(Key::Enter));
        assert_eq!(kb.popup.as_ref().map(|p| p.at), Some(2), "it opens on the current choice");
        assert_eq!(kb.route(&t, &s, key(Key::Home)).0, Vec::new(), "the drop-down holds the keys");
        kb.route(&t, &s, key(Key::ArrowDown));
        assert_eq!(
            kb.route(&t, &s, key(Key::Enter)).0,
            vec![Effect::Pick { id: "field/motion".into(), index: 3 }]
        );
        // Esc closes a drop-down unchanged.
        kb.route(&t, &s, key(Key::Enter));
        kb.route(&t, &s, key(Key::ArrowDown));
        assert_eq!(kb.route(&t, &s, key(Key::Escape)).0, Vec::new());
        assert!(kb.popup.is_none());
        // (3) over (4): a global wins over a scope's.
        let mut kb = Keyboard { nav_on: true, ..Keyboard::default() };
        assert_eq!(kb.route(&t, &s, key(Key::Home)).0, vec![Effect::Action("view-home".into())]);
        // (4) inside out: in the tree's Bodies section A adds a body, in the tree elsewhere it adds.
        let mut kb = Keyboard {
            focus: vec!["tree".into(), "tree/bodies".into(), "row/body 1".into()],
            nav_on: true,
            ..Keyboard::default()
        };
        assert_eq!(kb.route(&t, &s, key(Key::A)).0, vec![Effect::Action("add-body".into())]);
        kb.focus = vec!["tree".into()];
        assert_eq!(kb.route(&t, &s, key(Key::A)).0, vec![Effect::Action("add".into())]);
        // (4) over (5): a shortcut, not the mode, has Shift+F; modifier hygiene keeps F apart.
        kb.focus = vec!["viewport".into()];
        assert_eq!(
            kb.route(&t, &s, Press::with(Key::F, Modifiers::SHIFT)).0,
            vec![Effect::Action("fly".into())]
        );
        assert_eq!(kb.route(&t, &s, key(Key::F)).0, vec![Effect::Action("frame".into())]);
        assert_eq!(
            kb.route(&t, &s, Press::with(Key::Z, Modifiers::COMMAND)).0,
            vec![Effect::Action("undo".into())]
        );
        assert_eq!(
            kb.route(&t, &s, Press::with(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)).0,
            vec![Effect::Action("redo".into())]
        );
        assert_eq!(
            kb.route(&t, &s, Press::with(Key::Home, Modifiers::SHIFT)).0,
            vec![Effect::Action("view-home".into())],
            "Shift does not change a global"
        );
        assert!(
            kb.route(&t, &s, Press::with(Key::Home, Modifiers::COMMAND)).0.is_empty(),
            "Ctrl+Home is not Home"
        );
        // (4) over (5): a scope shortcut on a key the modes use (an arrow, Enter) wins over the
        // mode (S18's review, minor 9); control: unbound, the arrow moves and Enter goes in.
        let mut bound = s.clone();
        for (chord, action) in [("ArrowDown", "next"), ("Enter", "open")] {
            bound.push(Shortcut {
                scope: "tree".into(),
                chord: Chord::parse(chord).unwrap(),
                action: action.into(),
            });
        }
        let mut kb = Keyboard { focus: vec!["tree".into()], nav_on: true, ..Keyboard::default() };
        assert_eq!(kb.route(&t, &bound, key(Key::ArrowDown)).0, vec![Effect::Action("next".into())]);
        assert_eq!(kb.route(&t, &bound, key(Key::Enter)).0, vec![Effect::Action("open".into())]);
        assert_eq!(kb.focus, ["tree"]);
        kb.route(&t, &s, key(Key::ArrowDown));
        assert_eq!(kb.focus, ["viewport"], "unbound, the arrow moves to the next big scope");
        kb.focus = vec!["tree".into()];
        kb.route(&t, &s, key(Key::Enter));
        assert_eq!(kb.focus, ["tree", "tree/bodies"], "unbound, Enter goes in");
        // Control: with the priorities swapped (scope before global), Home in a scope that binds
        // it would not reach the global. Built here by binding Home in the viewport and asking
        // the scope first.
        let mut swapped = s.clone();
        swapped.push(Shortcut {
            scope: "viewport".into(),
            chord: Chord::parse("Home").unwrap(),
            action: "other".into(),
        });
        assert_eq!(find(&swapped, "viewport", key(Key::Home)), Some("other".into()));
        let mut kb = Keyboard { nav_on: true, ..Keyboard::default() };
        assert_eq!(
            kb.route(&t, &swapped, key(Key::Home)).0,
            vec![Effect::Action("view-home".into())],
            "the global still wins"
        );
    }

    #[test]
    fn typing_eats_keys_but_esc_enter_and_tab() {
        let (t, s) = (tree(), shortcuts());
        let mut kb = Keyboard {
            focus: vec!["inspector".into(), "inspector/body".into(), "field/label".into()],
            nav_on: true,
            ..Keyboard::default()
        };
        assert_eq!(kb.route(&t, &s, key(Key::Enter)).0, vec![Effect::Type("field/label".into())]);
        assert!(kb.typing);
        for k in [Key::Q, Key::W, Key::Delete, Key::ArrowLeft, Key::Questionmark] {
            assert_eq!(kb.route(&t, &s, key(k)), (Vec::new(), false), "{k:?} is typed");
        }
        assert_eq!(
            kb.route(&t, &s, Press::with(Key::Z, Modifiers::COMMAND)),
            (Vec::new(), false),
            "Ctrl+Z undoes the text"
        );
        // Esc stops typing, the text kept, the focus on the field.
        kb.route(&t, &s, key(Key::Escape));
        assert_eq!((kb.typing, kb.mode, kb.focused()), (false, Mode::Navigation, "field/label"));
        // Tab leaves the field and moves on.
        kb.route(&t, &s, key(Key::Enter));
        let (effects, used) = kb.route(&t, &s, key(Key::Tab));
        assert!(used);
        assert_eq!(effects, vec![Effect::StopTyping, Effect::Reveal("timeline".into())]);
        assert!(!kb.typing);
    }

    #[test]
    fn a_session_owns_its_keys_and_esc_cancels_it() {
        let (t, s) = (tree(), shortcuts());
        let mut kb = Keyboard {
            focus: vec!["viewport".into(), "selection".into()],
            nav_on: true,
            ..Keyboard::default()
        };
        assert_eq!(kb.route(&t, &s, key(Key::Enter)).0, vec![Effect::Interact("selection".into())]);
        assert_eq!(kb.session, Some(Session::Transform));
        assert_eq!(kb.route(&t, &s, key(Key::W)).0, Vec::new(), "no tool change mid-move");
        // Nor a delete or a duplicate of the body being moved (S18's review, R1): the session
        // takes every key that is not a global, used or not.
        for press in [key(Key::Delete), Press::with(Key::D, Modifiers::COMMAND), key(Key::F2), key(Key::F)] {
            let (effects, used) = kb.route(&t, &s, press);
            assert!(
                used && !effects.iter().any(|e| matches!(e, Effect::Action(_))),
                "{press:?}: {effects:?}"
            );
        }
        // A global still reaches the frame, which decides what it does to the session.
        assert_eq!(
            kb.route(&t, &s, Press::with(Key::Z, Modifiers::COMMAND)).0,
            vec![Effect::Action("undo".into())]
        );
        assert_eq!(kb.route(&t, &s, key(Key::X)).0, vec![Effect::Session(SessionKey::Lock(0))]);
        assert_eq!(kb.route(&t, &s, key(Key::Num2)).0, vec![Effect::Session(SessionKey::Char('2'))]);
        assert_eq!(
            kb.route(&t, &s, key(Key::ArrowRight)).0,
            vec![Effect::Adjust { id: "selection".into(), axis: 0, steps: 1.0, ctrl: false }]
        );
        assert_eq!(
            kb.route(&t, &s, key(Key::Escape)).0,
            vec![Effect::Release { id: "selection".into(), keep: false }]
        );
        assert_eq!((kb.session, kb.mode), (None, Mode::Navigation));
        // Control: Enter keeps it.
        kb.route(&t, &s, key(Key::Enter));
        assert_eq!(
            kb.route(&t, &s, key(Key::Enter)).0,
            vec![Effect::Release { id: "selection".into(), keep: true }]
        );
        // Outside a session W is the Move tool's again, and Delete and Ctrl+D act (the control
        // of the arm above).
        assert_eq!(kb.route(&t, &s, key(Key::W)).0, vec![Effect::Action("tool-move".into())]);
        assert_eq!(kb.route(&t, &s, key(Key::Delete)).0, vec![Effect::Action("delete-in-view".into())]);
        assert_eq!(
            kb.route(&t, &s, Press::with(Key::D, Modifiers::COMMAND)).0,
            vec![Effect::Action("duplicate-in-view".into())]
        );
        // A fly session keeps W A S D E Q, and Esc keeps the camera.
        kb.focus = vec!["viewport".into(), "canvas".into()];
        kb.mode = Mode::Interaction;
        kb.session = Some(Session::Fly);
        for k in [Key::W, Key::A, Key::S, Key::D, Key::E, Key::Q, Key::R, Key::Delete, Key::F] {
            assert_eq!(kb.route(&t, &s, key(k)).0, Vec::new(), "{k:?} is the flight's");
        }
        assert_eq!(
            kb.route(&t, &s, key(Key::Escape)).0,
            vec![Effect::Release { id: "canvas".into(), keep: true }]
        );
    }

    #[test]
    fn windows_hold_the_focus_and_give_it_back() {
        let (mut t, s) = (tree(), shortcuts());
        t.window("palette", "Palette");
        t.add("palette", "palette/query", "Query", Kind::Text);
        t.add("palette", "palette/results", "Results", Kind::Scope);
        let mut kb = Keyboard { focus: vec!["tree".into()], ..Keyboard::default() };
        kb.open_window("palette", "palette/query", true);
        assert!(kb.nav_on, "opened from the keyboard: the ring shows");
        assert_eq!(kb.focus, ["palette", "palette/query"], "it lands on its first section");
        kb.route(&t, &s, key(Key::Tab));
        assert_eq!(kb.focused(), "palette/results", "Tab moves between its sections");
        kb.route(&t, &s, key(Key::Tab));
        assert_eq!(kb.focused(), "palette/query", "and stays in the window");
        let (effects, _) = kb.route(&t, &s, key(Key::Escape));
        assert_eq!(effects, vec![Effect::Close("palette".into())]);
        assert_eq!(kb.focus, ["tree"], "closing gives the focus back to where it was");
        // The big-scope order did not change while it was open.
        assert_eq!(t.big_scopes(), BIG.iter().map(|(id, _)| *id).collect::<Vec<_>>());
    }

    #[test]
    fn a_focus_that_has_gone_falls_back_to_its_scope() {
        let (t, s) = (tree(), shortcuts());
        let mut kb = Keyboard {
            focus: vec!["tree".into(), "tree/bodies".into(), "row/body 9".into()],
            nav_on: true,
            ..Keyboard::default()
        };
        kb.route(&t, &s, key(Key::ArrowDown));
        assert_eq!(kb.focus, ["tree", "tree/bodies"], "a deleted row gives way to its section");
        assert_eq!(t.breadcrumb(&kb.focus), "Scene tree › Bodies");
    }

    #[test]
    fn a_click_in_overlapping_windows_finds_the_top_one_and_a_section() {
        // S18's second review, minors 5 and 7.
        let mut t = tree();
        let layer = |name: &str| egui::LayerId::new(egui::Order::Middle, egui::Id::NULL.with(name));
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));
        let small = Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(20.0, 20.0));
        t.window("under", "Under");
        t.add("under", "under/list", "List", Kind::Scope);
        t.add("under/list", "under/button", "Button", Kind::Button);
        t.place_window("under", rect, layer("under"));
        t.place("under/list", rect);
        t.place("under/button", small);
        t.window("over", "Over");
        t.add("over", "over/first", "First", Kind::Scope);
        t.place_window("over", rect, layer("over"));
        t.place("over/first", Rect::from_min_size(egui::pos2(0.0, 100.0), egui::vec2(200.0, 50.0)));
        let at = egui::pos2(15.0, 15.0);
        // Control: with no layer said, the smallest node anywhere under the pointer: the button
        // of the window underneath.
        assert_eq!(t.window_node_at(at, None), Some("under/button"));
        // With the window on top said, its own: its ground, which lands on its first section.
        assert_eq!(t.window_node_at(at, Some(layer("over"))), Some("over/first"));
        assert_eq!(t.window_node_at(egui::pos2(500.0, 500.0), None), None);
    }

    #[test]
    fn chords_read_and_write_back() {
        for text in
            ["Ctrl+Z", "Ctrl+Shift+Z", "F3", "?", "`", "Delete", "Shift+F", "Ctrl+K", "Space", "1", ","]
        {
            assert_eq!(Chord::parse(text).unwrap().text(), text);
        }
        assert!(Chord::parse("Ctrl+Nothing").is_err());
    }
}
