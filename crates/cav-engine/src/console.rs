//! The text form of the engine API: what the console, `cav exec` and the MCP `command` tool speak.
//!
//! A line is parsed into the same [`Command`] or [`Query`] a button would issue, so text is not a
//! second path into the engine, only another way to write the first. [`describe`] is the inverse:
//! the line that would issue a given command, which is also what the log shows.

use cav_sim::{DrawStyle, LIBRARY, NamedView, Projection, SceneSpec, Target};

use crate::command::{ChannelName, Command, PaneContent, Query, Reply, Stage, ViewCommand};
use crate::engine::Engine;
use crate::log::{Level, LogFilter, Subsystem};

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
        name: "mass",
        usage: "mass <kg> at <x> <y> <z>",
        about: "add a point mass at rest (an edit)",
        example: "mass 10 at 3 0 1",
    },
    CommandDoc {
        name: "run",
        usage: "run",
        about: "compute the signal again from nothing, dropping what was kept",
        example: "run",
    },
    CommandDoc {
        name: "signal",
        usage: "signal [tower] [gradiometer|lower|upper]",
        about: "a tower's signal over the timeline, one line a shot: readout time, seconds, and phase, radians",
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
        usage: "show <scene|signal|periodogram> [@N]",
        about: "what a pane shows: the scene, every tower's signal, or the periodogram with the combs",
        example: "show signal",
    },
    CommandDoc {
        name: "periodogram",
        usage: "periodogram [tower] [gradiometer|lower|upper]",
        about: "the periodogram of a tower's signal: its largest peaks, and where each body's harmonics fold to",
        example: "periodogram 1",
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
        name: "schema",
        usage: "schema",
        about: "the JSON Schema of the scene spec",
        example: "schema",
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
        usage: "journal",
        about: "the commands applied so far, as JSON: a session that can be replayed",
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
        Command::Set { path, value } => format!("set {path} {value}"),
        Command::Run { from: Stage::Forward } => "run".into(),
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
            ViewCommand::Orbit { pane, yaw, pitch } => format!("orbit {yaw} {pitch}{}", at(pane)),
            ViewCommand::Zoom { pane, distance } => format!("zoom {distance}{}", at(pane)),
            ViewCommand::Pan { pane, target: [x, y, z] } => format!("pan {x} {y} {z}{}", at(pane)),
        },
    }
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
    for c in line.chars() {
        match c {
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
    for (at, c) in script.char_indices() {
        if !begun && !c.is_whitespace() {
            begun = true;
            comment = c == '#';
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
            ("set", [path, value @ ..]) if !value.is_empty() => {
                // The value is JSON; a word that is not JSON is taken as a string, so that
                // `set name pass` needs no quotes within quotes.
                let text = value.join(" ");
                let value = serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text));
                Ok(Request::Do(Command::Set { path: (*path).to_owned(), value }))
            }
            ("mass", [kg, "at", x, y, z]) => {
                let position =
                    [number(x, "metres", name)?, number(y, "metres", name)?, number(z, "metres", name)?];
                // The first label of the form `mass N` that no body has yet.
                let taken = |label: &str| self.spec().bodies.iter().any(|b| b.label == label);
                let label = (1..).map(|n| format!("mass {n}")).find(|l| !taken(l)).expect("a free label");
                let body = cav_sim::BodySpec::point(&label, number(kg, "kilograms", name)?, position);
                let mut bodies = self.spec().bodies.clone();
                bodies.push(body);
                let value = serde_json::to_value(bodies).expect("bodies are JSON");
                Ok(Request::Do(Command::Set { path: "bodies".into(), value }))
            }
            ("run", []) => Ok(Request::Do(Command::Run { from: Stage::Forward })),
            ("signal", args) if args.len() <= 2 => {
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
                Ok(Request::Ask(Query::Signal { tower, channel }))
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
                Ok(Request::Ask(Query::Periodogram { tower, channel }))
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
            ("get", []) => Ok(Request::Ask(Query::Spec(None))),
            ("get", [path]) => Ok(Request::Ask(Query::Spec(Some((*path).to_owned())))),
            ("schema", []) => Ok(Request::Ask(Query::Schema)),
            ("stats", []) => Ok(Request::Ask(Query::Stats)),
            ("camera", []) => Ok(Request::Ask(Query::View(pane))),
            ("journal", []) => Ok(Request::Ask(Query::Journal)),
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
            Request::Ask(query) => {
                // A client that prints numbers waits for them (`cav exec`, MCP); the application's
                // patience is zero and it is told `stale` instead.
                match query {
                    Query::Signal { .. } | Query::Periodogram { .. } | Query::Reading(None) => {
                        self.settle(self.patience());
                        self.answer(&query)
                    }
                    Query::Reading(Some(time)) => {
                        // A reading away from the playhead: ask for it, answer, and ask for the
                        // playhead's again, which is what the panels show.
                        self.ask_reading(time);
                        self.settle(self.patience());
                        let answer = self.answer(&query);
                        self.ask_reading(self.time());
                        answer
                    }
                    _ => self.answer(&query),
                }
            }
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

    /// Answers a query as the console prints it; a refusal goes in the log.
    fn answer(&mut self, query: &Query) -> Result<String, String> {
        self.query(query).map(|reply| render(&reply)).inspect_err(|why| {
            self.note(Level::Error, Subsystem::Engine, why.clone());
        })
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

/// A reply as the console prints it.
#[must_use]
pub fn render(reply: &Reply) -> String {
    let json = |value: &dyn erased::Json| value.pretty();
    match reply {
        Reply::Spec(value) | Reply::Schema(value) => json(value),
        Reply::Stats(stats) => json(stats),
        Reply::View(view) => json(view),
        Reply::Journal(journal) => serde_json::to_string(journal).expect("commands are JSON"),
        Reply::Signal(signal) => {
            let mut lines = vec![format!(
                "# tower {} · {} · {} · {} shots · quadrature {} · t [s], phase [rad]",
                signal.tower,
                signal.channel.name(),
                signal.state,
                signal.times.len(),
                crate::estimate_text(signal.estimate)
            )];
            lines.extend(signal.warnings.iter().map(|warning| format!("# warning: {warning}")));
            lines.extend(signal.times.iter().zip(&signal.values).map(|(t, v)| format!("{t}  {v:e}")));
            lines.join("\n")
        }
        Reply::Periodogram(spectrum) => {
            let mut lines = vec![format!(
                "# tower {} · {} · {} · {} bins to the Nyquist frequency, {} Hz · f [Hz], √S [rad/√Hz]",
                spectrum.tower,
                spectrum.channel.name(),
                spectrum.state,
                spectrum.frequencies.len(),
                spectrum.nyquist
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
        Reply::Reading(reading) => {
            let towers = &reading.towers;
            let width = towers.iter().map(|t| t.label.len()).max().unwrap_or(5).max(5);
            let mut lines = vec![format!(
                "# a shot read out at {} s · {} · quadrature {}",
                reading.time,
                reading.state,
                crate::estimate_text(reading.estimate)
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
        Reply::Log(entries) => entries
            .iter()
            .map(|e| {
                format!("{:>9.3}  {:<5}  {:<6}  {}", e.at, e.level.name(), e.subsystem.name(), e.message)
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

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
            assert!(out.is_ok(), "`{}` failed: {out:?}", doc.example);
        }
        assert!(help().lines().count() == COMMANDS.len());
        // Control: what is not documented is not a command, and says so.
        let why = engine.run_line("frobnicate 3").unwrap_err();
        assert!(why.contains("not a command"), "{why}");
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
