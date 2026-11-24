// Where the build's revision and its dirty flag come from. This file is `include!`d by `build.rs`,
// which asks git once, when the engine is compiled, and by a test in `src/lib.rs`, which holds
// the decision to what it says. Nothing here runs when a bundle or a figure is written.

use std::path::{Path, PathBuf};

/// What the engine is compiled from, relative to the workspace root: the three crates that make
/// the numbers, the scenes built into the library, and what decides how they are compiled.
/// `dirty` is about these and nothing else; a change to any of them makes the stamp again.
pub const ENGINE_SOURCES: &[&str] = &[
    "crates/cav-core/src",
    "crates/cav-core/Cargo.toml",
    "crates/cav-sim/src",
    "crates/cav-sim/Cargo.toml",
    "crates/cav-engine/src",
    "crates/cav-engine/Cargo.toml",
    "crates/cav-engine/build.rs",
    "crates/cav-engine/provenance.rs",
    "scenes",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo",
];

/// What the build records of where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    /// The commit that was checked out, forty (or sixty-four) hexadecimal digits; or `unknown`.
    pub revision: String,
    /// `true`, `false` or `unknown`: whether [`ENGINE_SOURCES`] then differed from that commit.
    pub dirty: &'static str,
    /// The files and directories whose change means the stamp must be made again.
    pub watch: Vec<PathBuf>,
}

/// What `program` prints when run in `directory` with `arguments`; `None` when it cannot be run
/// (git is not installed) or says it failed (there is no repository, or no commit).
///
/// The repository is the one `directory` is in, found from there: the variables git sets for a
/// hook it runs (`GIT_DIR` and its kin) are not passed on, or a build inside a hook (the
/// pre-push hook runs `cargo xtask quick`) would ask that repository whatever the directory.
pub fn ask(program: &str, directory: &Path, arguments: &[&str]) -> Option<String> {
    let mut command = std::process::Command::new(program);
    for name in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR", "GIT_OBJECT_DIRECTORY", "GIT_PREFIX"]
    {
        command.env_remove(name);
    }
    let output = command.args(arguments).current_dir(directory).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The revision and the dirty flag, from what git answered.
///
/// `head` is the answer to `git rev-parse --show-prefix HEAD` asked in the workspace root: a
/// line that is empty when that root is the top of a work tree, then the commit. `status` is the
/// answer to `git status --porcelain -- <ENGINE_SOURCES>`: nothing when they are as committed.
///
/// Anything else is `unknown`, never a guess: no answer (no git, no repository, no commit yet);
/// a workspace that sits inside someone else's repository (the commit would be theirs); an
/// answer that is not a commit. Without a revision there is nothing for `dirty` to be about.
pub fn decide(head: Option<&str>, status: Option<&str>) -> (String, &'static str) {
    let revision = head.and_then(|answer| {
        let mut lines = answer.lines();
        let (prefix, commit) = (lines.next()?, lines.next()?.trim());
        let is_commit = matches!(commit.len(), 40 | 64) && commit.bytes().all(|b| b.is_ascii_hexdigit());
        (prefix.trim().is_empty() && is_commit && lines.next().is_none()).then(|| commit.to_owned())
    });
    match (revision, status) {
        (None, _) => ("unknown".to_owned(), "unknown"),
        (Some(revision), None) => (revision, "unknown"),
        (Some(revision), Some(changes)) => (revision, if changes.trim().is_empty() { "false" } else { "true" }),
    }
}

/// The stamp of the workspace at `root`, asked of `program` (git).
pub fn stamp(program: &str, root: &Path) -> Stamp {
    let head = ask(program, root, &["rev-parse", "--show-prefix", "HEAD"]);
    // `--no-optional-locks`: asking must not rewrite the index (git's own state, left alone).
    let mut status = vec!["--no-optional-locks", "status", "--porcelain", "--"];
    status.extend_from_slice(ENGINE_SOURCES);
    let (revision, dirty) = decide(head.as_deref(), ask(program, root, &status).as_deref());

    // The sources, and what in git moves when the commit does: HEAD and its log (every commit,
    // checkout, merge and reset), the branch it names and the packed refs; with `.gitignore`,
    // which decides what counts as untracked. Not the index: `git add` rewrites it without
    // changing the commit or whether the sources differ from it (a staged change is as dirty as
    // an unstaged one), and watching it compiled the engine and everything above it again after
    // every `git add` (S2's second review, item 6). The sources themselves are watched, so an
    // edit, a new file or a deleted one is seen without it.
    let mut watch: Vec<PathBuf> = ENGINE_SOURCES.iter().chain(&[".gitignore"]).map(|path| root.join(path)).collect();
    let mut inside = vec!["HEAD".to_owned(), "logs/HEAD".to_owned(), "packed-refs".to_owned()];
    inside.extend(ask(program, root, &["symbolic-ref", "-q", "HEAD"]).map(|branch| branch.trim().to_owned()));
    let mut arguments = vec!["rev-parse"];
    for name in &inside {
        arguments.extend(["--git-path", name.as_str()]);
    }
    if let Some(paths) = ask(program, root, &arguments) {
        watch.extend(paths.lines().map(|path| root.join(path.trim())));
    }
    // Cargo runs a build script every time if a file it was told to watch is not there.
    watch.retain(|path| path.exists());
    Stamp { revision, dirty, watch }
}
