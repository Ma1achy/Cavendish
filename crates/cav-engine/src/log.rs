//! The event log: a bounded ring of what the engine did and refused, read by the console.

use std::collections::VecDeque;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How much an entry matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub enum Level {
    /// Routine: view changes, scrubbing.
    Debug,
    /// Something happened.
    Info,
    /// Something looks wrong but was done.
    Warn,
    /// Something was refused.
    Error,
}

impl Level {
    /// The word shown in the console.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// Which part of the system an entry is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Subsystem {
    /// The application shell.
    App,
    /// The engine itself.
    Engine,
    /// Changes to the scene spec.
    Edit,
    /// Cameras, panes and styles.
    View,
    /// The renderer.
    Render,
    /// The MCP server.
    Mcp,
}

impl Subsystem {
    /// The word shown in the console.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Engine => "engine",
            Self::Edit => "edit",
            Self::View => "view",
            Self::Render => "render",
            Self::Mcp => "mcp",
        }
    }
}

/// One line of the log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LogEntry {
    /// Position in the log since the engine started; never reused.
    pub seq: u64,
    /// Seconds since the engine started, by the wall clock.
    pub at: f64,
    /// How much it matters.
    pub level: Level,
    /// What it is about.
    pub subsystem: Subsystem,
    /// What happened.
    pub message: String,
}

/// Which entries a log query returns. Every condition given must hold.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LogFilter {
    /// Only entries at this level or above.
    pub min_level: Option<Level>,
    /// Only entries about this subsystem.
    pub subsystem: Option<Subsystem>,
    /// Only entries whose message contains this text, ignoring case.
    pub text: Option<String>,
    /// Only the last this-many of the entries that match.
    pub last: Option<usize>,
}

impl LogFilter {
    /// Whether an entry passes the level, subsystem and text conditions.
    #[must_use]
    pub fn admits(&self, entry: &LogEntry) -> bool {
        self.min_level.is_none_or(|l| entry.level >= l)
            && self.subsystem.is_none_or(|s| entry.subsystem == s)
            && self.text.as_ref().is_none_or(|t| entry.message.to_lowercase().contains(&t.to_lowercase()))
    }
}

/// The most bytes one message holds. A longer one is cut there and says how much was left out:
/// the ring bounds how many entries there are, and this bounds each, so the log has a size
/// whatever is written to it (an answer of two million lines was once noted whole).
pub const MAX_MESSAGE_BYTES: usize = 64 << 10;

/// `message`, cut to [`MAX_MESSAGE_BYTES`] at a character's edge if it is longer.
fn capped(mut message: String) -> String {
    if message.len() <= MAX_MESSAGE_BYTES {
        return message;
    }
    let left_out = message.len();
    let mut cut = MAX_MESSAGE_BYTES;
    while !message.is_char_boundary(cut) {
        cut -= 1;
    }
    message.truncate(cut);
    message.push_str(&format!("\n… cut here: {cut} of {left_out} bytes kept"));
    // The long message's own buffer is given back: what is kept is the cut text and no more.
    message.shrink_to_fit();
    message
}

/// A ring of the most recent entries.
#[derive(Debug)]
pub struct Log {
    entries: VecDeque<LogEntry>,
    capacity: usize,
    next_seq: u64,
    started: web_time::Instant,
}

impl Log {
    /// An empty log that keeps the last `capacity` entries.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: capacity.max(1),
            next_seq: 0,
            started: web_time::Instant::now(),
        }
    }

    /// Appends an entry, dropping the oldest when the ring is full.
    pub fn push(&mut self, level: Level, subsystem: Subsystem, message: impl Into<String>) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        let at = self.started.elapsed().as_secs_f64();
        self.entries.push_back(LogEntry {
            seq: self.next_seq,
            at,
            level,
            subsystem,
            message: capped(message.into()),
        });
        self.next_seq += 1;
    }

    /// Replaces the newest entry's message, for a run of commands that overwrite one another.
    pub fn amend_last(&mut self, message: impl Into<String>) {
        if let Some(last) = self.entries.back_mut() {
            last.message = capped(message.into());
            last.at = self.started.elapsed().as_secs_f64();
        }
    }

    /// The entries that pass the filter, oldest first.
    #[must_use]
    pub fn select(&self, filter: &LogFilter) -> Vec<LogEntry> {
        let matching: Vec<&LogEntry> = self.entries.iter().filter(|e| filter.admits(e)).collect();
        let skip = filter.last.map_or(0, |n| matching.len().saturating_sub(n));
        matching.into_iter().skip(skip).cloned().collect()
    }

    /// Every entry held, oldest first.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &LogEntry> {
        self.entries.iter()
    }

    /// How many entries are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many entries have ever been pushed.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.next_seq
    }

    /// Drops every entry; sequence numbers carry on.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_is_a_bounded_ring_and_filters_compose() {
        let mut log = Log::new(4);
        for n in 0..10 {
            let level = if n % 2 == 0 { Level::Info } else { Level::Error };
            let subsystem = if n < 8 { Subsystem::Edit } else { Subsystem::View };
            log.push(level, subsystem, format!("Entry {n}"));
        }
        // Capacity respected: the last four survive, in order, with their original numbers.
        assert_eq!(log.len(), 4);
        assert_eq!(log.total(), 10);
        let seqs: Vec<u64> = log.iter().map(|e| e.seq).collect();
        assert_eq!(seqs, [6, 7, 8, 9]);

        let all = LogFilter::default();
        assert_eq!(log.select(&all).len(), 4);
        let errors = LogFilter { min_level: Some(Level::Error), ..all.clone() };
        assert_eq!(log.select(&errors).iter().map(|e| e.seq).collect::<Vec<_>>(), [7, 9]);
        let edits = LogFilter { subsystem: Some(Subsystem::Edit), ..all.clone() };
        assert_eq!(log.select(&edits).iter().map(|e| e.seq).collect::<Vec<_>>(), [6, 7]);
        // Filters compose: an error about an edit.
        let both =
            LogFilter { min_level: Some(Level::Error), subsystem: Some(Subsystem::Edit), ..all.clone() };
        assert_eq!(log.select(&both).iter().map(|e| e.seq).collect::<Vec<_>>(), [7]);
        // Text is matched without regard to case; `last` counts matching entries.
        let text = LogFilter { text: Some("entry 8".into()), ..all.clone() };
        assert_eq!(log.select(&text).len(), 1);
        let last = LogFilter { last: Some(1), min_level: Some(Level::Error), ..all.clone() };
        assert_eq!(log.select(&last)[0].seq, 9);
        // Control: a filter nothing matches returns nothing, so the filters above are not vacuous.
        let none = LogFilter { text: Some("entry 3".into()), ..all };
        assert!(log.select(&none).is_empty(), "entry 3 was dropped by the ring");
    }
}
