//! The text form of the engine API: what the console, `cav exec` and the MCP `command` tool speak.
//!
//! A line is parsed into the same [`Command`] or [`Query`] a button would issue, so text is not a
//! second path into the engine, only another way to write the first. [`describe`] is the inverse:
//! the line that would issue a given command, which is also what the log shows.

use cav_sim::{DrawStyle, LIBRARY, NamedView, Projection, SceneSpec, Target};

use crate::command::{ChannelName, Command, ExportKind, Query, Reply, Stage, ViewCommand};
use crate::engine::Engine;
use crate::log::{Level, LogFilter, Subsystem};
use cav_sim::PaneContent;

/// What a console line asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Change the engine's state.
    Do(Command),
    /// Read the engine's state.
    Ask(Query),
    /// Explain the commands, or one of them.
    Help(Option<String>),
    /// List the scene library.
    Scenes,
    /// Start the journal again from the session as it stands ([`Engine::restart_journal`]).
    Restart,
}

/// One console command, for `help`, completion and the menus.
#[derive(Clone, Copy, Debug)]
pub struct CommandDoc {
    /// The first word.
    pub name: &'static str,
    /// How it is written.
    pub usage: &'static str,
    /// What it does.
    pub about: &'static str,
    /// A line that parses on a fresh engine.
    pub example: &'static str,
}

/// Every console command. View commands act on the active pane unless the line ends in `@N`.
pub const COMMANDS: &[CommandDoc] = &[
    CommandDoc {
        name: "help",
        usage: "help [command]",
        about: "list the commands, or explain one",
        example: "help view",
    },
    CommandDoc { name: "scenes", usage: "scenes", about: "list the scene library", example: "scenes" },
    CommandDoc {
        name: "load",
        usage: "load <scene|file.ron>",
        about: "replace the scene with a library scene or a file; quote a path with spaces (an edit: undo takes it back)",
        example: "load void",
    },
    CommandDoc {
        name: "set",
        usage: "set <path> <value>",
        about: "set a value of the scene by its path, as JSON: a number, true, [x, y, z] (an edit; a drag is a run of these)",
        example: "set duration 20",
    },
    CommandDoc {
        name: "add",
        usage: "add <list> <value> | add <list> <index> <value>",
        about: "put an item into a list of the scene (bodies, array.towers), as JSON: at the end, or before item <index> (an edit)",
        example: "add array.towers {\"label\": \"tower 6\", \"position\": [6, 0, 0]}",
    },
    CommandDoc {
        name: "remove",
        usage: "remove <path[index]>",
        about: "take an item out of a list of the scene; the items after it move down (an edit)",
        example: "remove array.towers[4]",
    },
    CommandDoc {
        name: "save",
        usage: "save <file.ron>",
        about: "write the scene to a file that `load` reads back; quote a path with spaces",
        example: "save saved-scene.ron",
    },
    CommandDoc {
        name: "export",
        usage: "export bundle <directory>",
        about: "write the bundle: .npy arrays as v1's StateBundle, each tower's readout times, the numerics, the per-tower error estimate and a manifest; only from a refined signal",
        example: "export bundle a-bundle",
    },
    CommandDoc {
        name: "figure",
        usage: "figure <signal|periodogram|cadence|flight|view> [tower <N>] [gradiometer|lower|upper] [from <a> to <b>] [as <paper-column|paper-full|slide|poster|screen>] [size <W>x<H>] [dpi <N>] [untitled] to <file.svg|.pdf|.png> [@N]",
        about: "write a figure: a plot or a pane's view as SVG, PDF or PNG in a preset's style and size (size in mm, dpi for a PNG; untitled: no title, the axes take the room), with its data (.csv or .npy) and a .json of axes, units and provenance beside it",
        example: "figure periodogram tower 2 as paper-column to fig.pdf",
    },
    CommandDoc {
        name: "mass",
        usage: "mass <kg> at <x> <y> <z>",
        about: "add a point mass at rest (an edit)",
        example: "mass 10 at 3 0 1",
    },
    CommandDoc {
        name: "preset",
        usage: "preset <aion10|aice|file.ron>",
        about: "set the instrument to a preset (AICE's launch is derived, not kept), or to one of your own from a file of an instrument (an edit)",
        example: "preset aice",
    },
    CommandDoc {
        name: "launch",
        usage: "launch symmetric",
        about: "set the launch velocity to the one that brings the atoms back to where they started at 2T, g·T − v_kick/2, from the instrument's gravity, T, atom and kick number as they are now (an edit)",
        example: "launch symmetric",
    },
    CommandDoc {
        name: "compare",
        usage: "compare <aion10|aice|file.ron|off>",
        about: "read the same scene on another instrument as well: the plots draw both, and `signal … compared` answers for it",
        example: "compare aice",
    },
    CommandDoc {
        name: "run",
        usage: "run",
        about: "compute the signal again from nothing, dropping what was kept",
        example: "run",
    },
    CommandDoc {
        name: "signal",
        usage: "signal [tower] [gradiometer|lower|upper] [<from> <to>] [compared]",
        about: "a tower's signal, one line a shot: readout time, seconds, and phase, radians; all of the timeline, or the shots read out from one time to another",
        example: "signal 2 lower",
    },
    CommandDoc {
        name: "reading",
        usage: "reading [seconds]",
        about: "what every tower would read from a shot read out at a time (the playhead if none is given)",
        example: "reading 3.5",
    },
    CommandDoc { name: "undo", usage: "undo", about: "take back the last edit", example: "undo" },
    CommandDoc { name: "redo", usage: "redo", about: "reapply the edit last taken back", example: "redo" },
    CommandDoc {
        name: "select",
        usage: "select <tower N|body N|room|floor|none>",
        about: "select something, or nothing",
        example: "select tower 3",
    },
    CommandDoc { name: "seek", usage: "seek <seconds>", about: "move the playhead", example: "seek 4.2" },
    CommandDoc {
        name: "view",
        usage: "view <iso|top|bottom|north|south|east|west|home> [@N]",
        about: "look from a named direction, or return to the pane's home camera",
        example: "view top",
    },
    CommandDoc {
        name: "proj",
        usage: "proj <persp|ortho|toggle> [@N]",
        about: "perspective or orthographic",
        example: "proj ortho",
    },
    CommandDoc {
        name: "style",
        usage: "style <shaded|flat|xray|wireframe|paper> [@N]",
        about: "the draw style",
        example: "style paper",
    },
    CommandDoc {
        name: "show",
        usage: "show <scene|signal|periodogram|flight|cadence> [@N]",
        about: "what a pane shows: the scene, every tower's signal, the periodogram with the combs, the atoms' flights on absolute time, or the cadence's spectral window",
        example: "show signal",
    },
    CommandDoc {
        name: "span",
        usage: "span <from> <to>|all [@N]",
        about: "the stretch of a plot's horizontal axis a pane shows: seconds for the signal, hertz for the periodogram",
        example: "span 0 200",
    },
    CommandDoc {
        name: "periodogram",
        usage: "periodogram [tower] [gradiometer|lower|upper] [compared]",
        about: "the periodogram of a tower's signal: its largest peaks, and where each body's harmonics fold to",
        example: "periodogram 1",
    },
    CommandDoc {
        name: "cadence",
        usage: "cadence",
        about: "what the schedule is predicted to do: mean cycle, Nyquist, shots kept, each alias line and its height, the floor",
        example: "cadence",
    },
    CommandDoc {
        name: "window",
        usage: "window [tower] [to <hertz>]",
        about: "the spectral window of a tower's shots, |W|: its largest lines, one realisation for the scene's seed",
        example: "window 1 to 1.0",
    },
    CommandDoc { name: "panes", usage: "panes <1|2|4>", about: "split the figure", example: "panes 4" },
    CommandDoc { name: "pane", usage: "pane <N>", about: "make pane N the active one", example: "pane 1" },
    CommandDoc {
        name: "orbit",
        usage: "orbit <yaw> <pitch> [@N]",
        about: "camera direction, degrees: yaw anticlockwise from east, pitch above the horizontal",
        example: "orbit -60 25",
    },
    CommandDoc {
        name: "zoom",
        usage: "zoom <metres> [@N]",
        about: "distance from the target to the camera",
        example: "zoom 18",
    },
    CommandDoc {
        name: "pan",
        usage: "pan <x> <y> <z> [@N]",
        about: "the point the camera looks at, metres",
        example: "pan 0 0 2.5",
    },
    CommandDoc {
        name: "get",
        usage: "get [path]",
        about: "the scene spec, or the part at a path such as array.towers[2].position",
        example: "get array.instrument",
    },
    CommandDoc {
        name: "ls",
        usage: "ls [path]",
        about: "what is inside a path: each field with its value, unit, kind, range and what it is",
        example: "ls array.instrument",
    },
    CommandDoc {
        name: "schema",
        usage: "schema [path]",
        about: "the JSON Schema of the scene spec, or of the field at a path",
        example: "schema bodies",
    },
    CommandDoc { name: "stats", usage: "stats", about: "the engine's state at a glance", example: "stats" },
    CommandDoc { name: "camera", usage: "camera [@N]", about: "a pane's view as data", example: "camera" },
    CommandDoc {
        name: "log",
        usage: "log [count] [debug|info|warn|error] [text]",
        about: "the last entries of the event log",
        example: "log 5 info",
    },
    CommandDoc {
        name: "journal",
        usage: "journal [save <file.json> | restart]",
        about: "the commands applied so far, as JSON: a session that can be replayed (`cav replay <file>`); with `save`, written to a file; `restart` starts a new journal from the session as it stands (the way on once a journal has reached its size; the undo history starts again)",
        example: "journal",
    },
];

/// The console line that issues `command`. Pane-addressed commands carry `@N` unless they address
/// the active pane.
#[must_use]
pub fn describe(command: &Command, active_pane: usize) -> String {
    let at = |pane: usize| if pane == active_pane { String::new() } else { format!(" @{}", pane + 1) };
    match command {
        Command::Load(spec) => {
            // Only a library scene has a line that loads it again. A scene from a file is known
            // here by its contents alone, so the log says what it is rather than a line that
            // would load something else.
            let library =
                LIBRARY.iter().find(|s| s.name == spec.name).and_then(|s| SceneSpec::from_ron(s.ron).ok());
            if library.as_ref() == Some(&**spec) {
                format!("load {}", spec.name)
            } else {
                format!("load <the scene `{}`, not a library scene: load it from its file>", spec.name)
            }
        }
        Command::Set { path, value, .. } => format!("set {path} {value}"),
        Command::Insert { path, index: None, value } => format!("add {path} {value}"),
        Command::Insert { path, index: Some(at), value } => format!("add {path} {at} {value}"),
        Command::Remove { path } => format!("remove {path}"),
        Command::Export { what: ExportKind::Scene, to } => format!("save {}", quoted(to)),
        Command::Export { what: ExportKind::Journal, to } => format!("journal save {}", quoted(to)),
        Command::Export { what: ExportKind::Bundle, to } => format!("export bundle {}", quoted(to)),
        Command::Export { what: ExportKind::Figure(figure), to } => {
            let mut line = format!("figure {}", figure.what.name());
            if let Some(tower) = figure.tower {
                line.push_str(&format!(" tower {tower}"));
            }
            if figure.channel != ChannelName::default() {
                line.push_str(&format!(" {}", figure.channel.name()));
            }
            if let Some([from, to]) = figure.span {
                line.push_str(&format!(" from {from} to {to}"));
            }
            line.push_str(&format!(" as {}", figure.preset));
            if let Some([width, height]) = figure.size_mm {
                line.push_str(&format!(" size {width}x{height}"));
            }
            if let Some(dpi) = figure.dpi {
                line.push_str(&format!(" dpi {dpi}"));
            }
            if !figure.title {
                line.push_str(" untitled");
            }
            line.push_str(&format!(" to {}", quoted(to)));
            if figure.what == crate::FigureKind::View {
                line.push_str(&at(figure.pane));
            }
            line
        }
        Command::Run { from: Stage::Forward } => "run".into(),
        Command::Compare(None) => "compare off".into(),
        Command::Compare(Some(instrument)) => {
            match cav_sim::presets::built_in().into_iter().find(|(_, _, preset)| preset == &**instrument) {
                Some((name, _, _)) => format!("compare {name}"),
                None => "compare <an instrument of your own: compare it from its file>".into(),
            }
        }
        Command::Seek(t) => format!("seek {t}"),
        Command::Select(None) => "select none".into(),
        Command::Select(Some(Target::Tower(n))) => format!("select tower {n}"),
        Command::Select(Some(Target::Room)) => "select room".into(),
        Command::Select(Some(Target::Floor)) => "select floor".into(),
        Command::Select(Some(Target::Body(n))) => format!("select body {n}"),
        Command::Undo => "undo".into(),
        Command::Redo => "redo".into(),
        Command::View(view) => match *view {
            ViewCommand::Panes(n) => format!("panes {n}"),
            ViewCommand::Activate(pane) => format!("pane {}", pane + 1),
            ViewCommand::Look { pane, view } => format!("view {}{}", view.name(), at(pane)),
            ViewCommand::Home { pane } => format!("view home{}", at(pane)),
            ViewCommand::Project { pane, projection } => {
                let word = if projection == Projection::Perspective { "persp" } else { "ortho" };
                format!("proj {word}{}", at(pane))
            }
            ViewCommand::Style { pane, style } => format!("style {}{}", style.name(), at(pane)),
            ViewCommand::Show { pane, content } => format!("show {}{}", content.name(), at(pane)),
            ViewCommand::Span { pane, span: None } => format!("span all{}", at(pane)),
            ViewCommand::Span { pane, span: Some([from, to]) } => format!("span {from} {to}{}", at(pane)),
            ViewCommand::Orbit { pane, yaw, pitch } => format!("orbit {yaw} {pitch}{}", at(pane)),
            ViewCommand::Zoom { pane, distance } => format!("zoom {distance}{}", at(pane)),
            ViewCommand::Pan { pane, target: [x, y, z] } => format!("pan {x} {y} {z}{}", at(pane)),
        },
    }
}

/// A path as a console word: in double quotes when it has a space in it.
fn quoted(path: &str) -> String {
    if path.contains(char::is_whitespace) { format!("\"{path}\"") } else { path.to_owned() }
}

/// Splits a console line into words. Double quotes keep what is between them together, so a path
/// with a space is one word: `load "C:/my scenes/pass.ron"`.
///
/// # Errors
/// A quote that is opened and not closed.
pub fn words(line: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut started) = (false, false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // Inside quotes `\"` is a quote in the word, not the end of it. Nothing else is
            // escaped: a backslash is otherwise itself, as in a Windows path.
            '\\' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                word.push('"');
            }
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if std::mem::take(&mut started) {
                    out.push(std::mem::take(&mut word));
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err(format!("`{line}`: a `\"` is opened and never closed"));
    }
    if started {
        out.push(word);
    }
    Ok(out)
}

/// Splits a script into its statements: at `;` and at line breaks, except inside double quotes.
/// A statement that starts with `#` is a comment: it runs to the end of its line whatever it
/// holds (a `;`, a lone `"`), and is left out.
#[must_use]
pub fn statements(script: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut quoted) = (0, false);
    // Whether the statement being read has begun, and whether it began with `#`.
    let (mut begun, mut comment) = (false, false);
    // Whether the character before was a backslash that escapes this one.
    let mut escaped = false;
    for (at, c) in script.char_indices() {
        if !begun && !c.is_whitespace() {
            begun = true;
            comment = c == '#';
        }
        // Inside quotes a backslash takes the next character with it, as in JSON: `\"` does not
        // end the string and `\\` is one backslash. (It once did end it: over MCP,
        // `set name "a\"; get duration; set name \"b"` ran as three statements.)
        if quoted && !comment {
            if std::mem::take(&mut escaped) {
                continue;
            }
            if c == '\\' {
                escaped = true;
                continue;
            }
        }
        match c {
            '\n' if comment || !quoted => {
                if !comment {
                    out.push(&script[start..at]);
                }
                (start, quoted, begun, comment) = (at + 1, false, false, false);
            }
            _ if comment => {}
            '"' => quoted = !quoted,
            ';' if !quoted => {
                out.push(&script[start..at]);
                (start, begun) = (at + 1, false);
            }
            _ => {}
        }
    }
    if !comment {
        out.push(&script[start..]);
    }
    out.into_iter().map(str::trim).filter(|line| !line.is_empty()).collect()
}

/// The value `set` is given, as it was written: JSON, or a bare word taken as text, so that
/// `set name pass` needs no quotes. What begins as JSON's strings, lists and objects do (`"`,
/// `[`, `{`) must be JSON: a string with a quote missing is a mistake to be told about, not a
/// name with a quote in it.
///
/// # Errors
/// What is wrong with text that set out to be JSON.
pub fn value_of(text: &str) -> Result<serde_json::Value, String> {
    let text = text.trim();
    match serde_json::from_str(text) {
        Ok(value) => Ok(value),
        Err(why) if text.starts_with(['"', '[', '{']) => {
            Err(format!("`{text}` begins as JSON and is not JSON ({why}); a bare word needs no quotes"))
        }
        Err(_) => Ok(serde_json::Value::String(text.to_owned())),
    }
}

/// The usage line of a command, for error messages.
fn usage(name: &str) -> String {
    COMMANDS.iter().find(|c| c.name == name).map_or_else(String::new, |c| format!("usage: {}", c.usage))
}

fn number(word: &str, what: &str, command: &str) -> Result<f64, String> {
    word.parse::<f64>().map_err(|_| format!("`{word}` is not a number ({what}); {}", usage(command)))
}

impl Engine {
    /// Parses one console line against the current state (the active pane, `proj toggle`).
    /// Loading reads the library or the file here, so the command carries the scene itself and a
    /// journal replays without the file.
    ///
    /// # Errors
    /// What was expected instead.
    pub fn parse(&self, line: &str) -> Result<Request, String> {
        // `set` takes its value as it was written, quotes and all: the value is JSON, and a
        // body's motion or a label has strings in it. Every other line is words.
        if let Some(rest) = line.trim_start().strip_prefix("set")
            && rest.starts_with(char::is_whitespace)
            && let Some((path, value)) = rest.trim().split_once(char::is_whitespace)
        {
            return Ok(Request::Do(Command::set(path, value_of(value)?)));
        }
        // `add` too: a list, an index if there is one, and the item as it was written.
        if let Some(rest) = line.trim_start().strip_prefix("add")
            && rest.starts_with(char::is_whitespace)
            && let Some((list, rest)) = rest.trim().split_once(char::is_whitespace)
        {
            let rest = rest.trim();
            let (index, text) = match rest.split_once(char::is_whitespace) {
                Some((first, item)) if first.parse::<usize>().is_ok() => (first.parse().ok(), item.trim()),
                _ => (None, rest),
            };
            let value = serde_json::from_str(text)
                .map_err(|e| format!("`{text}` is not JSON ({e}); {}", usage("add")))?;
            return Ok(Request::Do(Command::Insert { path: list.to_owned(), index, value }));
        }
        let owned = words(line)?;
        let mut words: Vec<&str> = owned.iter().map(String::as_str).collect();
        let Some(&name) = words.first() else {
            return Err("an empty line; try `help`".into());
        };
        // A trailing `@N` addresses pane N (from 1); otherwise the active pane.
        let mut pane = self.active_pane();
        if let Some(n) = words.last().and_then(|w| w.strip_prefix('@')) {
            let n: usize = n.parse().map_err(|_| format!("`@{n}`: a pane is addressed as @1, @2, …"))?;
            pane = n.checked_sub(1).ok_or("panes are counted from 1")?;
            words.pop();
        }
        // `compared` at the end of a signal or a periodogram asks for the comparison's.
        let compared = matches!(name, "signal" | "periodogram") && words.last() == Some(&"compared");
        if compared {
            words.pop();
        }
        let args = &words[1..];
        let wrong = || format!("`{line}`: {}", usage(name));
        let view = |change: ViewCommand| Ok(Request::Do(Command::View(change)));
        match (name, args) {
            ("help", []) => Ok(Request::Help(None)),
            ("help", [topic]) => Ok(Request::Help(Some((*topic).to_owned()))),
            ("scenes", []) => Ok(Request::Scenes),
            ("load", [scene]) => {
                cav_sim::load(scene).map(|spec| Request::Do(Command::load(spec))).map_err(|e| e.0)
            }
            ("mass", [kg, "at", x, y, z]) => {
                let position =
                    [number(x, "metres", name)?, number(y, "metres", name)?, number(z, "metres", name)?];
                // The first label of the form `mass N` that no body has yet.
                let taken = |label: &str| self.spec().bodies.iter().any(|b| b.label == label);
                let label = (1..).map(|n| format!("mass {n}")).find(|l| !taken(l)).expect("a free label");
                let body = cav_sim::BodySpec::point(&label, number(kg, "kilograms", name)?, position);
                let value = serde_json::to_value(body).expect("a body is JSON");
                Ok(Request::Do(Command::Insert { path: "bodies".into(), index: None, value }))
            }
            ("remove", [path]) => Ok(Request::Do(Command::Remove { path: (*path).to_owned() })),
            ("save", [file]) => {
                Ok(Request::Do(Command::Export { what: ExportKind::Scene, to: (*file).to_owned() }))
            }
            ("export", ["bundle", directory]) => {
                Ok(Request::Do(Command::Export { what: ExportKind::Bundle, to: (*directory).to_owned() }))
            }
            ("figure", [kind, options @ ..]) => {
                let kind = crate::FigureKind::parse(kind)
                    .ok_or_else(|| format!("`{kind}` is not something a figure shows; {}", usage(name)))?;
                self.parse_figure(kind, options, pane).map(|(figure, to)| {
                    Request::Do(Command::Export { what: ExportKind::Figure(Box::new(figure)), to })
                })
            }
            ("journal", ["save", file]) => {
                Ok(Request::Do(Command::Export { what: ExportKind::Journal, to: (*file).to_owned() }))
            }
            ("run", []) => Ok(Request::Do(Command::Run { from: Stage::Forward })),
            ("preset", [which]) => {
                let instrument = cav_sim::presets::load(which).map_err(|e| e.0)?;
                let value = serde_json::to_value(instrument).expect("an instrument is JSON");
                Ok(Request::Do(Command::set("array.instrument", value)))
            }
            ("launch", ["symmetric"]) => {
                let launch = cav_sim::presets::symmetric_launch(&self.spec().array.instrument);
                Ok(Request::Do(Command::set("array.instrument.launch_velocity", serde_json::json!(launch))))
            }
            ("compare", ["off"]) => Ok(Request::Do(Command::Compare(None))),
            ("compare", [which]) => {
                let instrument = cav_sim::presets::load(which).map_err(|e| e.0)?;
                Ok(Request::Do(Command::Compare(Some(Box::new(instrument)))))
            }
            ("signal", args) if args.len() <= 4 => {
                let (mut tower, mut channel) = (self.selected_tower().unwrap_or(1), ChannelName::default());
                // The words before a span are a tower and a channel, in either order; a span is
                // the last two words when there are more words than those can be, or when the
                // last word is not a whole number.
                let spanned = args.len() >= 2
                    && args[args.len() - 2..].iter().all(|word| word.parse::<f64>().is_ok())
                    && (args.len() > 2 || args[1].parse::<usize>().is_err());
                let (names, span) = if spanned {
                    let (names, ends) = args.split_at(args.len() - 2);
                    (names, Some([number(ends[0], "seconds", name)?, number(ends[1], "seconds", name)?]))
                } else {
                    (args, None)
                };
                for &arg in names {
                    match (arg.parse::<usize>(), ChannelName::parse(arg)) {
                        (Ok(n), _) => tower = n,
                        (_, Some(named)) => channel = named,
                        _ => {
                            return Err(format!(
                                "`{arg}` is neither a tower number nor a channel; {}",
                                usage(name)
                            ));
                        }
                    }
                }
                Ok(Request::Ask(Query::Signal { tower, channel, span, compared }))
            }
            ("reading", []) => Ok(Request::Ask(Query::Reading(None))),
            ("reading", [t]) => Ok(Request::Ask(Query::Reading(Some(number(t, "seconds", name)?)))),
            ("undo", []) => Ok(Request::Do(Command::Undo)),
            ("redo", []) => Ok(Request::Do(Command::Redo)),
            ("select", ["none"]) => Ok(Request::Do(Command::Select(None))),
            ("select", ["room"]) => Ok(Request::Do(Command::Select(Some(Target::Room)))),
            ("select", ["floor"]) => Ok(Request::Do(Command::Select(Some(Target::Floor)))),
            ("select", ["body", n]) => {
                let n = n.parse().map_err(|_| format!("`{n}` is not a body number; {}", usage(name)))?;
                Ok(Request::Do(Command::Select(Some(Target::Body(n)))))
            }
            ("select", ["tower", n]) => {
                let n = n.parse().map_err(|_| format!("`{n}` is not a tower number; {}", usage(name)))?;
                Ok(Request::Do(Command::Select(Some(Target::Tower(n)))))
            }
            ("seek", [t]) => Ok(Request::Do(Command::Seek(number(t, "seconds", name)?))),
            ("view", ["home"]) => view(ViewCommand::Home { pane }),
            ("view", [direction]) => match NamedView::parse(direction) {
                Some(named) => view(ViewCommand::Look { pane, view: named }),
                None => Err(format!("`{direction}` is not a view; {}", usage(name))),
            },
            ("proj", [word]) => {
                let projection = match *word {
                    "persp" | "perspective" => Projection::Perspective,
                    "ortho" | "orthographic" => Projection::Orthographic,
                    "toggle" => match self.view(pane.min(crate::MAX_PANES - 1)).projection {
                        Projection::Perspective => Projection::Orthographic,
                        Projection::Orthographic => Projection::Perspective,
                    },
                    other => return Err(format!("`{other}` is not a projection; {}", usage(name))),
                };
                view(ViewCommand::Project { pane, projection })
            }
            ("style", [word]) => match DrawStyle::parse(word) {
                Some(style) => view(ViewCommand::Style { pane, style }),
                None => Err(format!("`{word}` is not a draw style; {}", usage(name))),
            },
            ("span", ["all"]) => view(ViewCommand::Span { pane, span: None }),
            ("span", [from, to]) => {
                let span = [number(from, "the start", name)?, number(to, "the end", name)?];
                view(ViewCommand::Span { pane, span: Some(span) })
            }
            ("show", [word]) => match PaneContent::parse(word) {
                Some(content) => view(ViewCommand::Show { pane, content }),
                None => Err(format!("`{word}` is not something a pane shows; {}", usage(name))),
            },
            ("periodogram", args) if args.len() <= 2 => {
                let (mut tower, mut channel) = (self.selected_tower().unwrap_or(1), ChannelName::default());
                for &arg in args {
                    match (arg.parse::<usize>(), ChannelName::parse(arg)) {
                        (Ok(n), _) => tower = n,
                        (_, Some(named)) => channel = named,
                        _ => {
                            return Err(format!(
                                "`{arg}` is neither a tower number nor a channel; {}",
                                usage(name)
                            ));
                        }
                    }
                }
                Ok(Request::Ask(Query::Periodogram { tower, channel, compared }))
            }
            ("panes", [n]) => match n.parse() {
                Ok(n) => view(ViewCommand::Panes(n)),
                Err(_) => Err(wrong()),
            },
            ("pane", [n]) => match n.parse::<usize>().ok().and_then(|n| n.checked_sub(1)) {
                Some(pane) => view(ViewCommand::Activate(pane)),
                None => Err(wrong()),
            },
            ("orbit", [yaw, pitch]) => view(ViewCommand::Orbit {
                pane,
                yaw: number(yaw, "degrees", name)?,
                pitch: number(pitch, "degrees", name)?,
            }),
            ("zoom", [distance]) => {
                view(ViewCommand::Zoom { pane, distance: number(distance, "metres", name)? })
            }
            ("pan", [x, y, z]) => {
                let target =
                    [number(x, "metres", name)?, number(y, "metres", name)?, number(z, "metres", name)?];
                view(ViewCommand::Pan { pane, target })
            }
            ("cadence", []) => Ok(Request::Ask(Query::Cadence)),
            ("window", args) if args.len() <= 3 => {
                let (tower, to) = match args {
                    [] => (self.selected_tower().unwrap_or(1), None),
                    [tower] => (tower.parse().map_err(|_| wrong())?, None),
                    ["to", hertz] => {
                        (self.selected_tower().unwrap_or(1), Some(number(hertz, "hertz", name)?))
                    }
                    [tower, "to", hertz] => {
                        (tower.parse().map_err(|_| wrong())?, Some(number(hertz, "hertz", name)?))
                    }
                    _ => return Err(wrong()),
                };
                Ok(Request::Ask(Query::Window { tower, to }))
            }
            ("get", []) => Ok(Request::Ask(Query::Spec(None))),
            ("get", [path]) => Ok(Request::Ask(Query::Spec(Some((*path).to_owned())))),
            ("schema", []) => Ok(Request::Ask(Query::Schema(None))),
            ("schema", [path]) => Ok(Request::Ask(Query::Schema(Some((*path).to_owned())))),
            ("ls", []) => Ok(Request::Ask(Query::Ls(None))),
            ("ls", [path]) => Ok(Request::Ask(Query::Ls(Some((*path).to_owned())))),
            ("stats", []) => Ok(Request::Ask(Query::Stats)),
            ("camera", []) => Ok(Request::Ask(Query::View(pane))),
            ("journal", []) => Ok(Request::Ask(Query::Journal)),
            ("journal", ["restart"]) => Ok(Request::Restart),
            ("log", args) => {
                let mut filter = LogFilter { last: Some(20), ..LogFilter::default() };
                for &arg in args {
                    match arg {
                        "debug" => filter.min_level = Some(Level::Debug),
                        "info" => filter.min_level = Some(Level::Info),
                        "warn" => filter.min_level = Some(Level::Warn),
                        "error" => filter.min_level = Some(Level::Error),
                        other => match other.parse::<usize>() {
                            Ok(count) => filter.last = Some(count),
                            Err(_) => filter.text = Some(other.to_owned()),
                        },
                    }
                }
                Ok(Request::Ask(Query::Log(filter)))
            }
            _ if COMMANDS.iter().any(|c| c.name == name) => Err(wrong()),
            _ => Err(format!("`{name}` is not a command; try `help`")),
        }
    }

    /// Runs one console line: parses it, applies or answers it, and returns what the console prints.
    /// A line that does not parse is logged as an error, like a command that is refused.
    ///
    /// # Errors
    /// Why the line was not understood or the command was refused. Nothing has changed.
    pub fn run_line(&mut self, line: &str) -> Result<String, String> {
        let request = match self.parse(line) {
            Ok(request) => request,
            Err(why) => {
                self.note(Level::Error, Subsystem::Engine, why.clone());
                return Err(why);
            }
        };
        match request {
            Request::Do(command) => {
                let described = describe(&command, self.active_pane());
                self.apply(command).map(|()| described)
            }
            // A client that prints numbers waits for the ones it asked for and no others
            // (`cav exec`, MCP); the application's patience is a tenth of a second, after which
            // it is told `stale`.
            Request::Ask(query) => self.ask(&query).map(|reply| render(&reply)).inspect_err(|why| {
                self.note(Level::Error, Subsystem::Engine, why.clone());
            }),
            Request::Restart => self
                .restart_journal()
                .map(|commands| {
                    format!(
                        "the journal starts again: {commands} commands that make the session as it stands. \
                         The undo history starts again too"
                    )
                })
                .inspect_err(|why| self.note(Level::Error, Subsystem::Engine, why.clone())),
            Request::Help(None) => Ok(help()),

            Request::Help(Some(topic)) => COMMANDS
                .iter()
                .find(|c| c.name == topic)
                .map(|c| format!("{}\n  {}\n  e.g. {}", c.usage, c.about, c.example))
                .ok_or_else(|| {
                    let why = format!("`{topic}` is not a command; try `help`");
                    self.note(Level::Error, Subsystem::Engine, why.clone());
                    why
                }),
            Request::Scenes => {
                // Two columns, the first as wide as the longest name and two spaces more.
                let width = LIBRARY.iter().map(|s| s.name.len()).max().unwrap_or(0) + 2;
                let lines: Vec<String> =
                    LIBRARY.iter().map(|s| format!("{:<width$}{}", s.name, s.path)).collect();
                Ok(lines.join("\n"))
            }
        }
    }

    /// The words of a `figure` line after what it shows: options in any order, then `to <file>`.
    fn parse_figure(
        &self,
        what: crate::FigureKind,
        mut words: &[&str],
        pane: usize,
    ) -> Result<(crate::Figure, String), String> {
        use crate::FigureKind;
        let wrong = |why: String| format!("{why}; {}", usage("figure"));
        let mut figure = crate::Figure::new(what, crate::FigureFormat::Svg);
        figure.pane = if what == FigureKind::View { pane } else { 0 };
        // A periodogram and a window are one tower's: the selected one, or the first.
        if matches!(what, FigureKind::Periodogram | FigureKind::Cadence) {
            figure.tower = Some(self.selected_tower().unwrap_or(1));
        }
        loop {
            match words {
                ["to", file] => {
                    figure.format = crate::FigureFormat::of_file(file).ok_or_else(|| {
                        wrong(format!("`{file}` does not end in .svg, .pdf or .png, so it names no format"))
                    })?;
                    return Ok((figure, (*file).to_owned()));
                }
                ["tower", n, rest @ ..] => {
                    figure.tower =
                        Some(n.parse().map_err(|_| wrong(format!("`{n}` is not a tower number")))?);
                    words = rest;
                }
                ["from", from, "to", to, rest @ ..] if !rest.is_empty() => {
                    figure.span =
                        Some([number(from, "the start", "figure")?, number(to, "the end", "figure")?]);
                    words = rest;
                }
                ["as", preset, rest @ ..] => {
                    figure.preset = (*preset).to_owned();
                    words = rest;
                }
                ["size", size, rest @ ..] => {
                    let sides = size
                        .split_once(['x', '×'])
                        .and_then(|(w, h)| Some([w.parse::<f64>().ok()?, h.parse::<f64>().ok()?]))
                        .ok_or_else(|| {
                            wrong(format!("`{size}` is not a size; write it as 85x60 (millimetres)"))
                        })?;
                    figure.size_mm = Some(sides);
                    words = rest;
                }
                ["dpi", dpi, rest @ ..] => {
                    figure.dpi = Some(number(dpi, "pixels per inch", "figure")?);
                    words = rest;
                }
                ["untitled", rest @ ..] => {
                    figure.title = false;
                    words = rest;
                }
                [channel, rest @ ..] if ChannelName::parse(channel).is_some() => {
                    figure.channel = ChannelName::parse(channel).expect("just checked");
                    words = rest;
                }
                [] | ["to"] => return Err(wrong("a figure needs `to <file>` at the end".into())),
                [other, ..] => return Err(wrong(format!("`{other}` is not an option of a figure"))),
            }
        }
    }

    /// The selected tower, from 1, if a tower is selected.
    fn selected_tower(&self) -> Option<usize> {
        match self.selection() {
            Some(Target::Tower(n)) => Some(n),
            _ => None,
        }
    }

    /// Runs several lines, separated by `;` or line breaks, stopping at the first that fails.
    ///
    /// # Errors
    /// The failing line and why; the lines before it have been applied.
    pub fn run_script(&mut self, script: &str) -> Result<Vec<String>, String> {
        statements(script)
            .into_iter()
            .map(|line| self.run_line(line).map_err(|why| format!("`{line}`: {why}")))
            .collect()
    }
}

/// The list of commands, one line each.
#[must_use]
pub fn help() -> String {
    let width = COMMANDS.iter().map(|c| c.usage.len()).max().unwrap_or(0);
    COMMANDS.iter().map(|c| format!("{:<width$}  {}", c.usage, c.about)).collect::<Vec<_>>().join("\n")
}

/// The most shots a signal's answer prints, one a line. A signal may hold two million; the text
/// of all of them would be a hundred megabytes in the log and over the wire.
pub const MAX_LINES: usize = 10_000;

/// A reply as the console prints it.
#[must_use]
pub fn render(reply: &Reply) -> String {
    let json = |value: &dyn erased::Json| value.pretty();
    match reply {
        Reply::Spec(value) | Reply::Schema(value) => json(value),
        Reply::Entries(entries) => {
            entries.iter().map(cav_sim::paths::Entry::line).collect::<Vec<_>>().join("\n")
        }
        Reply::Stats(stats) => json(stats),
        Reply::View(view) => json(view),
        Reply::Journal(journal) => {
            // The list as JSON, command by command, for as long as it fits. A journal that
            // fits is printed whole and is a journal file (`journal::parse` reads a bare list);
            // one that does not ends in a line that is not JSON, so it cannot pass for whole.
            let mut text = String::from("[");
            let mut printed = 0;
            for command in journal {
                let json = serde_json::to_string(command).expect("commands are JSON");
                if text.len() + json.len() + 2 > MAX_ANSWER_BYTES {
                    break;
                }
                if printed > 0 {
                    text.push(',');
                }
                text.push_str(&json);
                printed += 1;
            }
            text.push(']');
            if printed < journal.len() {
                text.push_str(&format!(
                    "\n# … {} more commands of {} are not printed (an answer is held to {MAX_ANSWER_BYTES} bytes): \
                     `journal save <file>` writes the whole journal",
                    journal.len() - printed,
                    journal.len()
                ));
            }
            text
        }
        Reply::Signal(signal) => {
            let mut lines = vec![format!(
                "# tower {} · {} · {} · {} shots · quadrature {} · t [s], phase [rad]",
                signal.tower,
                signal.channel.name(),
                signal.state,
                signal.times.len(),
                crate::estimate_text(signal.estimate, signal.settled)
            )];
            lines.extend(signal.warnings.iter().map(|warning| format!("# warning: {warning}")));
            let shots = signal.times.iter().zip(&signal.values).enumerate();
            lines.extend(shots.take(MAX_LINES).map(|(k, (t, v))| {
                // A shot that was not taken has no value to print.
                if signal.taken.get(k).copied().unwrap_or(true) {
                    format!("{t}  {v:e}")
                } else {
                    format!("{t}  not taken")
                }
            }));
            if signal.times.len() > MAX_LINES {
                lines.push(format!(
                    "# … {} more shots are not printed: ask for a stretch, `signal {} {} <from> <to>`",
                    signal.times.len() - MAX_LINES,
                    signal.tower,
                    signal.channel.name()
                ));
            }
            lines.join("\n")
        }
        Reply::Periodogram(spectrum) => {
            let mut lines = vec![format!(
                "# tower {} · {} · {} · {} bins to the Nyquist frequency, {} Hz · {} of {} shots · f [Hz], √S [rad/√Hz]",
                spectrum.tower,
                spectrum.channel.name(),
                spectrum.state,
                spectrum.frequencies.len(),
                spectrum.nyquist,
                spectrum.method,
                spectrum.shots
            )];
            // The eight largest bins, largest first.
            let mut bins: Vec<usize> = (0..spectrum.power.len()).collect();
            bins.sort_by(|&a, &b| spectrum.power[b].total_cmp(&spectrum.power[a]));
            lines.extend(
                bins.iter()
                    .take(8)
                    .map(|&k| format!("{}  {:e}", spectrum.frequencies[k], spectrum.power[k].sqrt())),
            );
            for comb in &spectrum.combs {
                let places: Vec<String> = comb
                    .harmonics
                    .iter()
                    .map(|h| {
                        format!(
                            "{}f₀ at {:.6} Hz{}",
                            h.n,
                            h.appears_at,
                            if h.folded { " (folded)" } else { "" }
                        )
                    })
                    .collect();
                lines.push(format!(
                    "# comb of `{}`, f₀ = {:.6} Hz: {}",
                    comb.label,
                    comb.fundamental,
                    places.join(", ")
                ));
            }
            lines.join("\n")
        }
        Reply::Cadence(cadence) => {
            let fewest = cadence.kept.iter().min().copied().unwrap_or(0);
            let most = cadence.kept.iter().max().copied().unwrap_or(0);
            let mut lines = vec![
                format!(
                    "# mean cycle {} s · Nyquist {:.6} Hz · {} shots planned · {fewest} to {most} taken a tower",
                    cadence.mean_cycle, cadence.nyquist, cadence.shots
                ),
                // Each is said only where it is true: no floor outside the closed form's regime,
                // no factor for a tower that takes nothing (the notes below say why).
                format!(
                    "# {} · {}",
                    cadence.floor.map_or_else(
                        || "floor under a tone: not given here".to_owned(),
                        |floor| format!("floor under a tone {floor:.3e} of its power")
                    ),
                    cadence.noise_factor.map_or_else(
                        || "a tower takes no shot: no tone can be detected there".to_owned(),
                        |factor| format!("smallest detectable tone × {factor:.4} for the shots not taken")
                    )
                ),
                "# line [Hz]  height |W| (1 = as tall as the tone)  what".to_owned(),
            ];
            lines.extend(
                cadence
                    .lines
                    .iter()
                    .map(|line| format!("{:.6}  {:.4}  {}", line.frequency, line.height, line.what)),
            );
            lines.extend(cadence.notes.iter().map(|note| format!("# note: {note}")));
            lines.join("\n")
        }
        Reply::Window(window) => {
            let mut lines = vec![format!(
                "# tower {} · spectral window |W| at {} frequencies to {} Hz · its largest lines: f [Hz], |W|",
                window.tower,
                window.frequencies.len(),
                window.frequencies.last().copied().unwrap_or(0.0)
            )];
            // The local maxima away from zero frequency, largest first.
            let w = &window.window;
            let mut peaks: Vec<usize> =
                (1..w.len().saturating_sub(1)).filter(|&k| w[k] > w[k - 1] && w[k] >= w[k + 1]).collect();
            peaks.sort_by(|&a, &b| w[b].total_cmp(&w[a]));
            lines.extend(peaks.iter().take(8).map(|&k| format!("{:.6}  {:.4}", window.frequencies[k], w[k])));
            lines.join("\n")
        }
        Reply::Reading(reading) => {
            let towers = &reading.towers;
            let width = towers.iter().map(|t| t.label.len()).max().unwrap_or(5).max(5);
            let mut lines = vec![format!(
                "# a shot read out at {} s · {} · quadrature {}",
                reading.time,
                reading.state,
                crate::estimate_text(reading.estimate, reading.settled)
            )];
            lines.extend(reading.warnings.iter().map(|warning| format!("# warning: {warning}")));
            lines.push(format!("# {:<width$}  ΔΦ [rad]  δφ₁ lower [rad]  δφ₂ upper [rad]", "tower"));
            lines.extend(
                towers.iter().map(|t| {
                    format!("{:<width$}  {:e}  {:e}  {:e}", t.label, t.gradiometer, t.lower, t.upper)
                }),
            );
            lines.join("\n")
        }
        Reply::Log(entries) => {
            // The newest entries that fit: the log is asked for what has just happened.
            let (mut lines, mut bytes) = (Vec::new(), 0);
            for e in entries.iter().rev() {
                let line =
                    format!("{:>9.3}  {:<5}  {:<6}  {}", e.at, e.level.name(), e.subsystem.name(), e.message);
                bytes += line.len() + 1;
                if bytes > MAX_ANSWER_BYTES {
                    break;
                }
                lines.push(line);
            }
            if lines.len() < entries.len() {
                lines.push(format!(
                    "# … {} earlier entries of {} are not printed (an answer is held to {MAX_ANSWER_BYTES} bytes): \
                     ask for fewer (`log 5`), or for a level or a word",
                    entries.len() - lines.len(),
                    entries.len()
                ));
            }
            lines.reverse();
            lines.join("\n")
        }
    }
}

/// The most bytes of a journal or of log entries an answer prints: 1 MiB. A journal may hold
/// 64 MiB and the log 2000 entries of 64 KiB; `journal save` still writes the whole journal.
pub const MAX_ANSWER_BYTES: usize = 1 << 20;

/// Pretty JSON for anything serialisable, without a generic at every call site.
mod erased {
    pub trait Json {
        fn pretty(&self) -> String;
    }

    impl<T: serde::Serialize> Json for T {
        fn pretty(&self) -> String {
            serde_json::to_string_pretty(self).expect("replies are JSON")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FigureFormat, FigureKind};
    use cav_sim::dev;

    #[test]
    fn console_lines_parse_to_the_same_commands() {
        // Each row: the line a person types, and the command a button issues.
        let rows: Vec<(&str, Command)> = vec![
            ("load void", Command::load(dev::void())),
            ("load infinite-floor", Command::load(dev::infinite_floor())),
            ("undo", Command::Undo),
            ("redo", Command::Redo),
            ("select tower 3", Command::Select(Some(Target::Tower(3)))),
            ("select room", Command::Select(Some(Target::Room))),
            ("select floor", Command::Select(Some(Target::Floor))),
            ("select body 2", Command::Select(Some(Target::Body(2)))),
            ("select none", Command::Select(None)),
            ("seek 4.2", Command::Seek(4.2)),
            ("view top", Command::View(ViewCommand::Look { pane: 0, view: NamedView::Top })),
            ("view west @3", Command::View(ViewCommand::Look { pane: 2, view: NamedView::West })),
            ("view home", Command::View(ViewCommand::Home { pane: 0 })),
            (
                "proj ortho",
                Command::View(ViewCommand::Project { pane: 0, projection: Projection::Orthographic }),
            ),
            (
                "proj persp @2",
                Command::View(ViewCommand::Project { pane: 1, projection: Projection::Perspective }),
            ),
            ("style xray", Command::View(ViewCommand::Style { pane: 0, style: DrawStyle::XRay })),
            ("panes 4", Command::View(ViewCommand::Panes(4))),
            ("pane 2", Command::View(ViewCommand::Activate(1))),
            ("orbit -60 25", Command::View(ViewCommand::Orbit { pane: 0, yaw: -60.0, pitch: 25.0 })),
            ("zoom 0.30000000000000004", Command::View(ViewCommand::Zoom { pane: 0, distance: 0.1 + 0.2 })),
            ("pan 1 -2 0.5 @4", Command::View(ViewCommand::Pan { pane: 3, target: [1.0, -2.0, 0.5] })),
            (
                "figure periodogram tower 2 as paper-column to fig.pdf",
                figure(FigureKind::Periodogram, FigureFormat::Pdf, "fig.pdf", |f| f.tower = Some(2)),
            ),
            (
                "figure signal lower from 0.5 to 6 as slide size 200x100 dpi 96 to \"my figures/s.png\"",
                figure(FigureKind::Signal, FigureFormat::Png, "my figures/s.png", |f| {
                    f.channel = ChannelName::Lower;
                    f.span = Some([0.5, 6.0]);
                    f.preset = "slide".into();
                    f.size_mm = Some([200.0, 100.0]);
                    f.dpi = Some(96.0);
                }),
            ),
            (
                "figure cadence tower 1 as paper-column to window.svg",
                figure(FigureKind::Cadence, FigureFormat::Svg, "window.svg", |f| f.tower = Some(1)),
            ),
            (
                "figure flight as paper-full untitled to flight.pdf",
                figure(FigureKind::Flight, FigureFormat::Pdf, "flight.pdf", |f| {
                    f.preset = "paper-full".into();
                    f.title = false;
                }),
            ),
            (
                "figure view as screen to plan.png @3",
                figure(FigureKind::View, FigureFormat::Png, "plan.png", |f| {
                    f.preset = "screen".into();
                    f.pane = 2;
                }),
            ),
        ];
        let engine = Engine::new();
        for (line, command) in &rows {
            assert_eq!(engine.parse(line), Ok(Request::Do(command.clone())), "{line}");
            // And back: the log's description of the command is the line that issues it.
            assert_eq!(describe(command, 0), *line, "{command:?}");
        }
        // Control: lines that differ give commands that differ.
        assert_ne!(engine.parse("view top"), engine.parse("view bottom"));
        assert_ne!(engine.parse("view top"), engine.parse("view top @2"));
        assert_ne!(engine.parse("seek 4.2"), engine.parse("seek 4.200000000000001"));
    }

    /// The export of a figure of `what` to `to`, as `with` leaves it.
    fn figure(
        what: FigureKind,
        format: FigureFormat,
        to: &str,
        with: impl Fn(&mut crate::Figure),
    ) -> Command {
        let mut figure = crate::Figure::new(what, format);
        with(&mut figure);
        Command::Export { what: ExportKind::Figure(Box::new(figure)), to: to.to_owned() }
    }

    #[test]
    fn a_figure_line_is_checked_before_anything_is_drawn() {
        let mut engine = Engine::new();
        // Words that are not a figure's are refused with the usage.
        for (line, expected) in [
            ("figure spectrum to a.svg", "`spectrum` is not something a figure shows"),
            ("figure signal", "a figure needs `to <file>` at the end"),
            ("figure signal to a.jpg", "`a.jpg` does not end in .svg, .pdf or .png"),
            ("figure signal tower two to a.svg", "`two` is not a tower number"),
            ("figure signal size big to a.svg", "`big` is not a size; write it as 85x60"),
            ("figure signal sideways to a.svg", "`sideways` is not an option of a figure"),
            ("figure signal to a.svg b.svg", "is not an option of a figure"),
        ] {
            let why = engine.run_line(line).unwrap_err();
            assert!(why.contains(expected) && why.contains("usage: figure <signal|"), "`{line}`: {why}");
        }
        // A figure the engine can see is impossible is refused for that, before the exporter
        // (there is none here) is asked for anything.
        for (line, expected) in [
            ("figure signal tower 9 to a.svg", "there is no tower 9; the scene has towers 1 to 5"),
            ("figure signal size 5x60 to a.svg", "a figure of 5 × 60 mm cannot be made"),
            ("figure signal size 85x1e9 to a.svg", "each side is from 10 to 2000 mm"),
            ("figure signal dpi 0 to a.png", "0 is not a resolution"),
            ("figure signal dpi 1e9 to a.png", "from 10 to 2400"),
            ("figure signal from 6 to 2 to a.svg", "not from 6 to 2"),
            ("figure view to a.png @3", "there is no pane 3; the figure has 1"),
        ] {
            let why = engine.run_line(line).unwrap_err();
            assert!(why.contains(expected), "`{line}`: {why}");
        }
        // A command whose format is not its file's is refused too: a journal cannot say .pdf
        // and write a PNG.
        let lying = figure(FigureKind::Signal, FigureFormat::Png, "a.pdf", |_| {});
        assert!(engine.apply(lying).unwrap_err().contains("`a.pdf` is not the name of a .png file"));
        // Nothing of all that changed the engine or reached its journal.
        assert!(engine.journal().is_empty() && !engine.draws_figures());
        // Control: with an exporter the same kind of line is applied, journalled, and the
        // exporter is handed the figure and the path; a replay that skips exports calls nothing.
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let record = std::sync::Arc::clone(&seen);
        engine.set_figure_exporter(Box::new(move |_, figure, path| {
            record.lock().unwrap().push((figure.what, path.to_path_buf()));
            Ok(vec![path.to_path_buf()])
        }));
        engine.run_line("figure flight as slide to somewhere/f.svg").unwrap();
        assert_eq!(
            *seen.lock().unwrap(),
            [(FigureKind::Flight, std::path::PathBuf::from("somewhere/f.svg"))]
        );
        assert_eq!(engine.journal().len(), 1);
        assert!(engine.log().iter().next_back().unwrap().message.ends_with("f.svg (1 files)"));
        let replayed = Engine::replay(engine.journal()).unwrap();
        assert_eq!(replayed.digest(), engine.digest());
        assert_eq!(seen.lock().unwrap().len(), 1, "a replay with nowhere for exports to go draws nothing");
        // An exporter's own refusal is the command's.
        engine.set_figure_exporter(Box::new(|_, _, _| Err("no ink".into())));
        assert_eq!(engine.run_line("figure flight to f.svg").unwrap_err(), "no ink");
        assert_eq!(engine.journal().len(), 1);
    }

    #[test]
    fn unaddressed_view_commands_go_to_the_active_pane() {
        let mut engine = Engine::new();
        engine.run_script("panes 4; pane 3; view north; proj toggle").unwrap();
        assert_eq!(engine.view(2).named(), Some(NamedView::North));
        assert_eq!(engine.view(2).projection, Projection::Perspective, "pane 3 starts orthographic; toggled");
        // Control: the first pane was not touched.
        assert_eq!(engine.view(0).named(), Some(NamedView::Iso));
        // The log describes commands relative to the active pane.
        engine.run_line("style paper @1").unwrap();
        assert_eq!(engine.log().iter().next_back().unwrap().message, "style paper @1");
    }

    #[test]
    fn every_documented_command_parses() {
        let mut engine = Engine::new();
        // The examples that write a file write it under the build's own directory.
        let scratch =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/console examples");
        std::fs::create_dir_all(&scratch).unwrap();
        engine.set_exports(crate::Exports::Into(scratch));
        for doc in COMMANDS {
            assert!(doc.example.starts_with(doc.name), "{}: the example shows the command", doc.name);
            assert!(doc.usage.starts_with(doc.name));
            // `redo` has nothing to redo on a fresh engine; give it something.
            if doc.name == "redo" {
                engine.run_script("load void; undo").unwrap();
            }
            if doc.name == "undo" {
                engine.run_line("load void").unwrap();
            }
            let out = engine.run_line(doc.example);
            if doc.name == "figure" {
                // The engine does not draw, and no client has given this one an exporter: the
                // example parses to a figure and is refused for that reason and no other.
                // (`cav-plot`'s tests run the same line with the exporter installed.)
                assert!(matches!(
                    engine.parse(doc.example),
                    Ok(Request::Do(Command::Export { what: ExportKind::Figure(_), .. }))
                ));
                let why = out.unwrap_err();
                assert!(why.starts_with("no figure exporter is installed in this client"), "{why}");
                continue;
            }
            assert!(out.is_ok(), "`{}` failed: {out:?}", doc.example);
        }
        assert!(help().lines().count() == COMMANDS.len());
        // Control: what is not documented is not a command, and says so.
        let why = engine.run_line("frobnicate 3").unwrap_err();
        assert!(why.contains("not a command"), "{why}");
    }

    #[test]
    fn the_usage_lines_name_every_figure_and_every_pane_content() {
        // `show` and `figure` parse their word from `PaneContent::ALL` and `FigureKind::ALL`, so a
        // new plot works with no line here; their usage lines are written by hand and did not
        // follow it (docs/extending/a-plot.md, followed end to end).
        let choices = |usage: &str| -> Vec<String> {
            let open = usage.find('<').expect("a list of choices") + 1;
            let close = open + usage[open..].find('>').expect("its end");
            usage[open..close].split('|').map(str::to_owned).collect()
        };
        let usage_of = |name: &str| COMMANDS.iter().find(|doc| doc.name == name).expect(name).usage;
        let contents: Vec<String> = PaneContent::ALL.iter().map(|c| c.name().to_owned()).collect();
        let kinds: Vec<String> = FigureKind::ALL.iter().map(|k| k.name().to_owned()).collect();
        assert_eq!(choices(usage_of("show")), contents);
        assert_eq!(choices(usage_of("figure")), kinds);
        // Control: a line that lacks one of them is found.
        assert_ne!(choices("figure <signal|periodogram|cadence|view> to <file>"), kinds);
    }

    #[test]
    fn a_bad_line_says_what_was_expected_and_changes_nothing() {
        let bad = [
            ("", "empty"),
            ("view up", "usage: view <iso|top|bottom|north|south|east|west|home>"),
            ("view", "usage: view"),
            ("seek soon", "`soon` is not a number (seconds)"),
            ("select tower three", "not a tower number"),
            ("select body one", "not a body number"),
            ("panes many", "usage: panes <1|2|4>"),
            ("style x-ray", "usage: style <shaded|flat|xray|wireframe|paper>"),
            ("proj fisheye", "usage: proj <persp|ortho|toggle>"),
            ("view top @0", "counted from 1"),
            ("view top @two", "addressed as @1"),
            ("load nowhere", "not a library scene (room, void, infinite-floor, point, pass,"),
            ("get array.nothing", "has no field `nothing`"),
            ("help frobnicate", "not a command"),
        ];
        let mut engine = Engine::new();
        let before = engine.digest();
        for (n, (line, expected)) in bad.iter().enumerate() {
            let why = engine.run_line(line).unwrap_err();
            assert!(why.contains(expected), "`{line}`: `{why}` should contain `{expected}`");
            let errors =
                engine.log().select(&LogFilter { min_level: Some(Level::Error), ..LogFilter::default() });
            assert_eq!(errors.len(), n + 1, "`{line}` is logged as an error");
        }
        assert_eq!(engine.digest(), before);
        assert!(engine.journal().is_empty());
    }

    #[test]
    fn a_script_stops_at_the_first_failure() {
        let mut engine = Engine::new();
        let why = engine.run_script("panes 2; view top\n# a comment\nview sideways; panes 4").unwrap_err();
        assert!(why.starts_with("`view sideways`"), "{why}");
        assert_eq!(engine.panes(), 2, "the lines before the failure were applied");
        assert_eq!(engine.view(0).named(), Some(NamedView::Top));
        assert_eq!(engine.journal().len(), 2, "and the one after it was not");
    }

    #[test]
    fn queries_render_as_text() {
        let mut engine = Engine::new();
        assert_eq!(engine.run_line("get array.instrument.cycle").unwrap(), "2.0");
        assert!(engine.run_line("stats").unwrap().contains("\"towers\": 5"));
        assert!(engine.run_line("camera").unwrap().contains("\"projection\": \"Perspective\""));
        assert!(engine.run_line("schema").unwrap().contains("Length of the timeline, seconds."));
        let cycle = engine.run_line("schema array.instrument.cycle").unwrap();
        assert!(cycle.contains("\"x-unit\": \"s\"") && !cycle.contains("tower_height"), "{cycle}");
        let listed = engine.run_line("ls array.towers[1]").unwrap();
        assert_eq!(listed.lines().count(), 3, "{listed}");
        assert!(listed.contains("label  =  \"tower 2\"  (text)"), "{listed}");
        assert!(listed.contains("position  =  [4.0,0.0,0.0] m  (vector of 3)"), "{listed}");
        assert!(engine.run_line("ls").unwrap().lines().count() == 8);
        assert!(engine.run_line("ls room.colour").unwrap_err().contains("it has: origin, size"));
        let scenes = engine.run_line("scenes").unwrap();
        assert!(scenes.contains("void") && scenes.contains("scenes/dev/infinite-floor.ron"));
        engine.run_script("seek 1; select tower 2").unwrap();
        let log = engine.run_line("log 1 info").unwrap();
        assert!(log.ends_with("select tower 2") && log.lines().count() == 1, "{log}");
        // The journal is JSON that replays to the same state.
        let journal: Vec<Command> = serde_json::from_str(&engine.run_line("journal").unwrap()).unwrap();
        assert_eq!(Engine::replay(&journal).unwrap().digest(), engine.digest());
    }

    #[test]
    fn quotes_keep_a_path_with_spaces_in_one_word() {
        assert_eq!(words("load void").unwrap(), ["load", "void"]);
        assert_eq!(words(r#"load "C:/my scenes/a pass.ron""#).unwrap(), ["load", "C:/my scenes/a pass.ron"]);
        assert_eq!(words(r#"  view   top  @2 "#).unwrap(), ["view", "top", "@2"]);
        assert_eq!(words(r#"get """#).unwrap(), ["get", ""], "an empty quoted word is a word");
        assert!(words(r#"load "unclosed"#).unwrap_err().contains("never closed"));
        // A script is not cut inside quotes.
        assert_eq!(statements(r#"load "a; b.ron"; panes 2"#), [r#"load "a; b.ron""#, "panes 2"]);
        assert_eq!(statements("panes 2;;\n view top \n"), ["panes 2", "view top"]);
        // An escaped quote does not end a string: this is one statement, a name with a quote,
        // a semicolon and what looks like two more commands in it. Control: without the
        // backslashes the same text is three statements.
        let sly = r#"set name "a\"; get duration; set name \"b""#;
        assert_eq!(statements(sly), [sly]);
        assert_eq!(statements(&sly.replace('\\', "")).len(), 3);
        assert_eq!(
            statements(r#"set name "a\\"; panes 2"#),
            [r#"set name "a\\""#, "panes 2"],
            "an escaped backslash"
        );
        assert_eq!(words(r#"load "my \"best\" scene.ron""#).unwrap(), ["load", r#"my "best" scene.ron"#]);
        assert_eq!(
            words(r#"load "C:\scenes\pass.ron""#).unwrap(),
            ["load", r"C:\scenes\pass.ron"],
            "a path keeps its backslashes"
        );
        // What `set` takes: JSON, or a bare word; what begins as JSON must be JSON.
        assert_eq!(value_of("pass").unwrap(), serde_json::json!("pass"));
        assert_eq!(value_of(r#""a\"b""#).unwrap(), serde_json::json!("a\"b"));
        assert_eq!(value_of("[1, 2]").unwrap(), serde_json::json!([1, 2]));
        assert_eq!(value_of("3rd").unwrap(), serde_json::json!("3rd"));
        for broken in [r#""open"#, "[1, 2", r#"{"a": }"#, r#""a" b"#] {
            assert!(value_of(broken).unwrap_err().contains("is not JSON"), "{broken}");
        }

        // End to end: a scene file in a folder whose name has a space.
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/console test/my scenes");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("far void.ron");
        let mut scene = dev::void();
        scene.name = "far-void".into();
        std::fs::write(&path, scene.to_ron()).unwrap();
        let mut engine = Engine::new();
        let line = format!("load \"{}\"", path.display());
        engine.run_line(&line).unwrap();
        assert_eq!(engine.spec(), &scene);
        // Control: without the quotes the same path is several words, and is refused.
        let bare = format!("load {}", path.display());
        assert!(Engine::new().run_line(&bare).is_err());
    }

    #[test]
    fn a_comment_ends_at_its_line_whatever_it_holds() {
        // A comment with one `"` in it used to open a quote that swallowed the lines after it.
        let script = "panes 2\n# the 4\" pane; not a command\nview top\n  # last";
        assert_eq!(statements(script), ["panes 2", "view top"]);
        let mut engine = Engine::new();
        engine.run_script(script).unwrap();
        assert_eq!((engine.panes(), engine.view(0).named()), (2, Some(NamedView::Top)));
        // A `#` that does not start a statement is not a comment, and quotes still hold there.
        assert_eq!(statements("load \"a #1; b.ron\"; panes 2"), ["load \"a #1; b.ron\"", "panes 2"]);
        assert_eq!(statements("panes 2; # trailing; view top\npanes 4"), ["panes 2", "panes 4"]);
        // Control: the same line without the `#` is a statement with an unclosed quote, which
        // does take the rest of the script with it, and is refused.
        let bare = "panes 2\nthe 4\" pane\nview top";
        assert_eq!(statements(bare), ["panes 2", "the 4\" pane\nview top"]);
        assert!(Engine::new().run_script(bare).is_err());
    }

    #[test]
    fn launch_symmetric_follows_the_instrument_as_it_is_now() {
        use cav_sim::presets::{aice, aice_with_kicks, symmetric_launch};
        // The four things the rule reads. After each is changed the launch that was derived
        // for the preset is stale (a set moves nothing else: the control), and `launch
        // symmetric` derives it again from the instrument as it now is.
        for line in [
            "set array.instrument.kicks 4000",
            "set array.instrument.half_sequence 2",
            "set array.instrument.gravity 9.78",
            "set array.instrument.atom_mass 1.46e-25",
        ] {
            let mut engine = Engine::new();
            engine.run_line("preset aice").unwrap();
            engine.run_line(line).unwrap();
            let stale = engine.spec().array.instrument.clone();
            assert_eq!(stale.launch_velocity, aice().launch_velocity, "{line}");
            assert_ne!(stale.launch_velocity, symmetric_launch(&stale), "{line}");
            engine.run_line("launch symmetric").unwrap();
            let derived = engine.spec().array.instrument.clone();
            assert_eq!(derived.launch_velocity.to_bits(), symmetric_launch(&stale).to_bits(), "{line}");
            assert_eq!(
                derived,
                cav_sim::InstrumentSpec { launch_velocity: derived.launch_velocity, ..stale }
            );
            if line.contains("kicks") {
                // The preset at that kick number, by another road.
                assert_eq!(derived, aice_with_kicks(4000));
            }
        }
        // The kick number has a path, a unit and a range like any other field.
        let mut engine = Engine::new();
        assert_eq!(engine.run_line("get array.instrument.kicks").unwrap(), "1000");
        let schema = engine.run_line("schema array.instrument.kicks").unwrap();
        assert!(schema.contains("\"x-unit\": \"1\"") && schema.contains("kick number"), "{schema}");
        let why = engine.run_line("set array.instrument.kicks 0").unwrap_err();
        assert!(why.contains("array.instrument.kicks"), "{why}");
        // A whole number (George, 24 Nov 2026): a fraction is refused, never rounded.
        let why = engine.run_line("set array.instrument.kicks 1000.5").unwrap_err();
        assert!(why.contains("array.instrument.kicks") && why.contains("whole number"), "{why}");
        assert_eq!(engine.spec().array.instrument.kicks, 1000);
        // Control: a whole number is taken.
        engine.run_line("set array.instrument.kicks 1001").unwrap();
        assert_eq!(engine.spec().array.instrument.kicks, 1001);
    }

    #[test]
    fn the_log_does_not_offer_a_line_that_would_load_another_scene() {
        // A library scene: the described line loads it again.
        assert_eq!(describe(&Command::load(dev::void()), 0), "load void");
        // A scene that only shares a library name, or has none: no such line exists, and none is given.
        let mut edited = dev::void();
        edited.duration = 30.0;
        let described = describe(&Command::load(edited.clone()), 0);
        assert!(described.contains("not a library scene"), "{described}");
        assert_ne!(Engine::new().parse(&described), Ok(Request::Do(Command::load(edited))));
        let mut named = dev::void();
        named.name = "mine".into();
        assert!(describe(&Command::load(named), 0).contains("`mine`, not a library scene"));
    }
}
