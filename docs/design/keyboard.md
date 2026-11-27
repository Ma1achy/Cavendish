# Keyboard navigation and interaction model

> **Status: requirement for Cavendish** (owner, 25 Nov 2026). This is the keyboard model of the
> owner's Principia app, given as a reusable spec. Cavendish adopts it in full in the editor slice
> (see `plan/REBUILD.md` §6 and `docs/research/editor-ux.md`). Principia's own layout is the running
> example below; Cavendish's big scopes, Tab order and scope shortcuts are set in the editor slice's
> brief and must keep every rule here. Where a rule here and the viewer mock-up disagree, this wins.

A complete keyboard model for a dense, panel-based desktop app (Principia: an egui app with a
central figure, side panels, a timeline, floating windows). Every control is reachable and
operable from the keyboard; the mouse and keyboard coexist without fighting; nothing happens
"unseen".

## 1. The core idea: a tree of scopes, walked in two modes

The UI is a TREE OF SCOPES.
- A scope is a region (a panel, a section, a window, a toolbar). Scopes contain elements
  (buttons, sliders, checkboxes, text fields, dropdowns, custom widgets) and/or sub-scopes.
- The top level is a fixed, numbered list of BIG SCOPES in Tab order. Principia's:
    1 top bar · 2 Manifold view (left panel) · 3 Figure (the main canvas) · 4 Trajectory
    · 5 Compass · 6 Time · 7 Legend · 8 footer
  Tab from 8 wraps to 1.
- Big scopes can contain sub-scopes (Manifold view → Chart, Navigate, Slice centre, Tilt),
  reached with Enter.

At any moment the keyboard is in exactly one of TWO MODES:

  NAVIGATION mode (orange ring): you are MOVING between things.
    Arrows move between siblings. Enter goes in / starts interacting / clicks.
  INTERACTION mode (blue ring): you are OPERATING one thing.
    Arrows adjust the focused element (slider value, pan the canvas, scroll a list...).

This split is the whole trick: arrows always mean "move" or "adjust", never ambiguously both,
and the ring colour tells you which.

## 2. Focus vs "keyboard navigation is on" — two separate pieces of state

- FOCUS: where keys go. Always exists.
- KEYBOARD NAVIGATION ON/OFF: whether the focus ring and the two modes are shown and active.

Rules:
- The app starts with keyboard navigation OFF (no ring).
- ANY key press turns it ON and shows the ring where the focus is, AFTER that key has acted.
  A modifier pressed alone (Shift, Ctrl, Alt, Cmd) does not count.
- Exception that keeps it safe: Enter in navigation mode while navigation is OFF only turns it
  on and shows the ring — it does NOT press whatever was focused. Nothing unseen is ever pressed.
- ANY mouse click or drag turns it OFF and hides the ring (including a click-off to close
  something). The click moves the focus only where a specific rule says it does (below).
  The next key press shows the ring again where the focus already is.
- Scroll and hover are neither click nor drag: they leave the ring as it is.
- This state is pure UI: never undoable, never seen by the engine/model layer.

Result: mouse users never see rings; keyboard users always know where they are; switching between
the two never teleports the focus.

## 3. The key table

| key            | action |
|----------------|--------|
| Tab / Shift+Tab| Next / previous BIG scope, landing on the scope itself, in navigation mode, from either mode. Inside a window: next / previous SECTION of the window, staying in the window. |
| Enter          | Navigation mode: on a scope → go in, onto its first element. On an element → start interaction with it. On a button → its click (stays in navigation mode). Interaction mode: back to navigation mode on the same element. |
| Esc            | Interaction mode: back to navigation mode on the same element. Navigation mode: up one scope (at a window's top level: close the window / act as Cancel). |
| Arrows         | Navigation mode: move between siblings (on a big scope itself, siblings are the big scopes, in Tab order). Interaction mode: adjust the focused element. |
| Shift / Alt    | Step multipliers everywhere: Shift ×10, Alt ×0.1. Same meaning on every widget, no exceptions. |
| Letters, Space | The current scope's SHORTCUTS (see §5). Active whenever the focus is in that scope (on it or anything inside), in EITHER mode — except while typing in a text field. |
| Held keys      | Delay then repeat: the DAS/ARR model from Tetris. 500 ms delay, then a repeat every 40 ms. |
| Ctrl+Z         | Undo (app-level command history), in either mode. While typing in a text field it undoes the text edit instead. |
| Ctrl+C / Ctrl+V| Context copy/paste where a scope defines it (Principia: copy/paste an initial-condition record in the Inspector). In a text field: ordinary text copy/paste. |
| ?              | The shortcuts sheet, over everything, in either mode. In a text field, types "?". |
| Global keys    | A small set that work wherever the focus is, in either mode (Principia: Home = reset the view to the preset, one undoable command; Alt+L = show/hide all overlay lines). Still yield to text fields: in a text field Home moves the caret, Alt+L types what the platform types. |

Modifier hygiene for global keys: Shift/Alt don't change them; Ctrl/Cmd+key is a different key
(so Ctrl+Home is not Home). Global "reset" keys do not auto-repeat.

## 4. Landing rules — where focus goes when you arrive

Every keyboard landing is in NAVIGATION mode:
- Tab onto a big scope → lands on the scope itself, the WHOLE scope highlighted.
- Enter into a scope → lands on its FIRST element.
- Opening a window → lands on its first section (e.g. the console's header bar).
- A text field reached by keyboard → lands in navigation mode on the field (not typing yet);
  Enter starts typing.

Exceptions to "Enter goes in":
- A scope with nothing inside that IS the interactive thing (the Figure/canvas): Enter starts
  interaction with it directly.
- A read-only scope with nothing inside (the Legend): Enter does nothing.

Resets: a section's "reset" button is its LAST element, so Enter into the section still lands on
the first real control. Enter on a reset = its click; on a disabled reset, nothing — and it stays
in the order (see §9).

## 5. Scope shortcuts (letters and Space)

Each scope owns a small set of single-key shortcuts, active anywhere inside it, in either mode.
Arrow actions for a scope need Enter (interaction mode) first; letter shortcuts don't.

Principia's examples:
- Figure (canvas): arrows pan, + / − zoom (in interaction mode); P pin the sample under the
  pointer; K lock; T show/hide the trace under the cursor; Shift+T kept traces; Y syzygies;
  L listen (sonification on/off); Space keep the current trajectory.
- Time: Space play/pause; ← → step frames (interaction mode, on the scrubber).
- Compass: arrows tilt (tilt mode) or move the slice along the normal (slice mode).
- Trajectory views: arrows pan / orbit, + / − zoom, per view.

Every shortcut is listed on the "?" sheet. Avoid reusing a letter for two meanings across scopes
that are commonly adjacent; if a spec text conflicts with an existing binding, keep the existing
binding and move the new action (Principia moved "keep trajectory" from K to Space because K was
already Lock).

## 6. Interaction mode per widget type

- Slider / number: arrows ±step, Shift ×10, Alt ×0.1. Held = DAS/ARR repeat.
- Canvas (figure, views): arrows pan by a fixed base step; + / − zoom by a fixed log2 step;
  spherical views: arrows orbit by 1°.
- Dropdown / choice: Enter opens it, ↑ ↓ choose, Enter picks, Esc closes it UNCHANGED.
- Checkbox / button: Enter acts as its click (stays in navigation mode; no interaction mode needed).
- Grid widget (e.g. a matrix of cells): arrows move a highlighted cell; Space activates it as a
  click would; Esc/Enter returns to navigation mode.
- Lists/logs: ↑ ↓ one entry, Page Up/Down a page, Home/End first/last, Shift ×10. The WHOLE list is
  reachable, not just rendered rows (virtualised lists must still navigate the full data).
- Compound widgets (the compass): in navigation mode the parts are siblings in a fixed order
  (the widget, then its mode buttons, then its view buttons, then its axis dropdowns);
  Enter on the widget itself starts interacting with it.

## 7. Text fields — the one place keys mean "type"

- Reached by keyboard: navigation mode on the field. Enter starts typing (= interaction mode).
- While typing, EVERY key goes to the field except:
    Esc and Enter → stop typing, back to navigation mode on the field, text KEPT;
    Tab / Shift+Tab → leave the field and move on (next section inside a window).
  Ctrl+Z undoes the text edit; "?" types a "?"; global keys type what the platform types.
- **Cavendish's one local exception** (owner, 25 Nov 2026): in a viewport *transform session*
  (moving the selection with arrows or a typed distance, `docs/slices/S18.md`) **Esc cancels**
  the session, putting the selection back where it was, and **Enter keeps** it. Everywhere else
  Esc keeps what was typed, as above. Both end in navigation mode on the same element.
  Within a session **Tab is "the next axis"** of the typed distance (Blender's), not the next
  big scope, and the session holds **every key that is not a global**, used or not: nothing else
  acts on a body half moved. A global key (Ctrl+Z, Space, ?, …) first ends the session,
  cancelled, then acts; Ctrl+Z after a move is that cancel and nothing more. (Recorded after S18's
  review, R1 and minor 3.)
- **Cavendish's second local exception** (owner, 25 Nov 2026: "make it work like a fps game
  camera"; "literally copy minecraft creative mode"): while the camera is in *first person*
  (the right button held over a scene viewport, or a session started by `fly`, Shift+F, or
  `walk`, Shift+G) the session holds **every key, the globals too**. W A S D, Space, Shift and
  Ctrl are held to move, so none of them may also act: Space does not play (it rises, or jumps;
  twice, it switches flying and walking), Ctrl+S (sprinting backwards) does not save, Ctrl+Z
  does not undo. In a browser Ctrl is not the sprint (Ctrl+W closes the tab, and no page can
  stop it): there the double tap of W is the only sprint (`platform::CTRL_SPRINTS`; S20's
  review, finding 3). Only Esc or Enter (stop, the camera kept), the session's own keys (Shift+F,
  Shift+G, N for noclip), F3 and F11 still act (`first_person::WHILE_FLYING`; none of them is a
  chord of a key held to move, which a test holds). A Shift still down from the chord that began
  the session is not "down" until it has been let go. The session ends in navigation mode on
  the canvas; it is never an edit.
- A mouse click on a text field starts typing immediately (interaction mode), caret at the click,
  ring hidden until the next key press (the first character typed shows the ring blue).
- A click anywhere else ends typing and leaves the keyboard in navigation mode.

## 8. Mouse interplay — clicks move focus only by rule

- A click on a control in a window (e.g. a console button) moves the keyboard focus onto it, in
  navigation mode (ring hidden until the next key).
- A click on the canvas that does a canvas action (e.g. pins a sample) does NOT steal focus from
  wherever it was; it just ends any typing.
- Opening a window or menu with the mouse gives it focus at its normal landing, with NO ring and
  keyboard navigation off. Opening it from the keyboard turns navigation on and shows the ring.
- **Cavendish:** anything that moves the focus out of a viewport transform session (a click on a
  window's control, a window opened with the mouse), a drag in the view, and any primary click
  outside the tool's options row (under the viewport's header while a session is under way;
  so: a row of the scene tree, the canvas, a menu) **ends the
  session, keeping its move**, as a click outside a move keeps it in Blender. Of the mouse,
  nothing cancels; of the keys, Esc and any global key do (Ctrl+Z being that cancel, §7). A
  click on a window's bare ground lands on its first section, and where windows overlap the one
  on top takes the click. (S18's second review, I2 and minors 5, 7; its third review, minors 1
  and 2.)

## 9. Windows, dialogs and overlays

- A window is NOT a big scope; the big-scope order never changes when windows open.
- A window is a list of SECTIONS. Inside it, Tab / Shift+Tab move between sections and stay in
  the window. Esc at the window's top level closes it (or acts as Cancel in a dialog).
- Closing returns focus to whatever opened it (the footer button, the menu entry), sometimes with
  that menu left open so you can continue.
- Re-invoking "open X" on an already-open window gives it focus again at its landing.
- ORDER NEVER DEPENDS ON STATE: a disabled button stays in the order (disabled, not removed); an
  empty list is still a stop; how the window was opened doesn't change the order. Predictability
  beats "skipping dead controls".
- A window opening by itself (e.g. the console on an error) does NOT take focus.

Principia's console, as a worked example: two scopes, header bar (filters, search field, Copy,
Clear, ×) and log. ↑ ↓ or Tab move between the two; Enter on the header goes in onto the first
filter; ← → walk the header row in visual order and stop at the ends; Enter on the log starts
interaction (scroll by entry/page, Home/End); Esc from either top-level scope closes the console.

## 10. What the user sees

While keyboard navigation is on:
- A focus RING on the current scope/element: ORANGE in navigation mode, BLUE in interaction mode.
  A whole-scope highlight when landed on a scope itself.
- A BREADCRUMB in the top bar: "Manifold view › Navigate › zoom", "Console › Header › Copy".
- Nothing else changes on screen. No layout shift, no extra chrome.
"?" shows the shortcut sheet as an overlay over everything.

## 11. Implementation notes (how Principia structures it)

- Keep the scope tree as DATA (an ordered tree of scope/element ids), separate from rendering.
  Each widget registers its id under its scope each frame; navigation walks the tree, not screen
  geometry. Visual order = tree order, declared once.
- Keyboard state is a small struct beside the focus: { nav_on: bool, mode: Navigation|Interaction,
  focus_path: Vec<Id> }. It's UI-only, never in the undo history, never read by the engine.
- Route each key in a fixed priority:
    1. text field being typed in (it eats everything but Esc/Enter/Tab);
    2. open dropdown/popup;
    3. global keys (Home, Alt+L, ?, Ctrl+Z);
    4. scope shortcuts (walk focus_path from innermost to outermost scope);
    5. mode handling (arrows/Enter/Esc/Tab per §3).
- Steps live in one table (pan step, zoom log2 step, orbit angle, slider steps) so Shift ×10 /
  Alt ×0.1 are applied uniformly.
- DAS/ARR: track key-down time per key; fire once on press, then after 500 ms every 40 ms.
- The canvas "Home" button is NOT a focusable element; the Home key does its job. Don't duplicate
  a global key as a tab stop.
- Test it like logic, not like pixels: drive key sequences into the state machine and assert the
  focus path, mode and ring. Include control arms (e.g. a click must hide the ring; Enter with nav
  off must not press the focused button).
