//! The workspace held to the plan (REBUILD §2 principle 10 and §3.1, `docs/slices/S2.md` §2
//! "Crates after S2"): which crate may depend on which, and that every library crate warns of
//! undocumented public items.
//!
//! Tests only. The manifests are read as text, since xtask has no dependencies: a manifest's
//! name and the keys of its dependency tables are all that is wanted, and `Cargo.toml` writes
//! those one to a line.

use std::path::Path;

/// What one crate's `Cargo.toml` says, as far as the plan cares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// The package's name.
    pub name: String,
    /// The names in `[dependencies]` and `[build-dependencies]`, under any target.
    pub normal: Vec<String>,
    /// The names in `[dev-dependencies]`, under any target.
    pub dev: Vec<String>,
}

/// Which table of a manifest a line is in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Table {
    Package,
    Normal,
    Dev,
    Other,
}

/// A line of TOML with its comment taken off: a `#` inside a quoted string is not a comment.
fn uncommented(line: &str) -> &str {
    let mut quote = None;
    for (at, c) in line.char_indices() {
        match (quote, c) {
            (None, '#') => return &line[..at],
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            _ => {}
        }
    }
    line
}

/// How many brackets and braces a line opens, less those it closes, outside quoted strings: above
/// zero while a value runs on over several lines.
fn depth_change(line: &str) -> i32 {
    let (mut quote, mut depth) = (None, 0);
    for c in line.chars() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '[' | '{') => depth += 1,
            (None, ']' | '}') => depth -= 1,
            _ => {}
        }
    }
    depth
}

/// The table a header names, and the dependency it names when it is one dependency's own table
/// (`[dependencies.serde]`). A build dependency counts as a normal one; `[target.….dependencies]`
/// counts as `[dependencies]`.
fn table_of(header: &str) -> (Table, Option<String>) {
    let (table, own) = match header.rfind("dependencies.") {
        Some(at) => {
            let end = at + "dependencies".len();
            (&header[..end], Some(header[end + 1..].trim().trim_matches(['"', '\'']).to_owned()))
        }
        None => (header, None),
    };
    let kind = match table.rsplit('.').next().unwrap_or(table).trim() {
        "package" if own.is_none() => Table::Package,
        "dependencies" | "build-dependencies" => Table::Normal,
        "dev-dependencies" => Table::Dev,
        _ => Table::Other,
    };
    // `[workspace.dependencies]` lists versions; it makes nothing depend on anything.
    if header.starts_with("workspace") { (Table::Other, None) } else { (kind, own) }
}

/// The string a `key = "value"` pair gives `key`, wherever in the line it is.
fn quoted_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let mut rest = line;
    while let Some(at) = rest.find(key) {
        let before_ok = rest[..at].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '-'));
        let after = rest[at + key.len()..].trim_start();
        if before_ok && let Some(value) = after.strip_prefix('=') {
            let value = value.trim_start().strip_prefix('"')?;
            return value.split('"').next();
        }
        rest = &rest[at + key.len()..];
    }
    None
}

/// Reads a manifest.
///
/// # Errors
/// When it names no package.
pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let (mut name, mut normal, mut dev) = (None, Vec::new(), Vec::new());
    let (mut table, mut depth) = (Table::Other, 0);
    for line in text.lines() {
        let line = uncommented(line).trim();
        if depth > 0 {
            depth += depth_change(line);
            continue;
        }
        if let Some(header) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            let (kind, own) = table_of(header.trim());
            table = kind;
            if let Some(dependency) = own {
                match kind {
                    Table::Normal => normal.push(dependency),
                    Table::Dev => dev.push(dependency),
                    _ => {}
                }
                // The keys under it are that dependency's own (`version`, `features`).
                table = Table::Other;
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        depth = depth_change(value).max(0);
        match table {
            Table::Package if key.trim() == "name" => {
                name = Some(value.trim().trim_matches('"').to_owned());
            }
            Table::Normal | Table::Dev => {
                // `cav-core.workspace = true` and `cav-core = { … }` both name `cav-core`; a
                // renamed dependency is the package it says it is.
                let written = key.trim().split('.').next().unwrap_or("").trim().trim_matches(['"', '\'']);
                let dependency = quoted_value(value, "package").unwrap_or(written).to_owned();
                if table == Table::Normal { normal.push(dependency) } else { dev.push(dependency) }
            }
            _ => {}
        }
    }
    let name = name.ok_or("the manifest has no [package] name")?;
    Ok(Manifest { name, normal, dev })
}

/// Which workspace crates each crate may have as normal dependencies: the plan's table
/// (`docs/slices/S2.md` §2 "Crates after S2", with the test-support crates and xtask). Every
/// workspace crate has a row, so that a new crate is added here on purpose.
pub const PLAN: &[(&str, &[&str])] = &[
    ("cav-core", &[]),
    ("cav-sim", &["cav-core"]),
    ("cav-gpu", &["cav-core"]),
    ("cav-engine", &["cav-core", "cav-sim"]),
    ("cav-render", &["cav-core", "cav-sim", "cav-gpu"]),
    ("cav-plot", &["cav-core", "cav-sim", "cav-engine", "cav-gpu", "cav-render"]),
    ("cav-app", &["cav-core", "cav-sim", "cav-engine", "cav-gpu", "cav-render", "cav-plot"]),
    ("cav", &["cav-core", "cav-sim", "cav-engine", "cav-gpu", "cav-render", "cav-plot"]),
    ("cav-validate", &[]),
    ("cav-memtest", &[]),
    ("xtask", &[]),
];

/// The test-support crates: any crate's tests may use them.
pub const DEV_ANYWHERE: &[&str] = &["cav-validate", "cav-memtest"];

/// What else a crate's tests may depend on, beyond [`DEV_ANYWHERE`] and what the crate itself
/// may depend on: `cav-memtest`'s own tests measure the memory of reads and of engine commands.
pub const DEV_ALSO: &[(&str, &[&str])] = &[("cav-memtest", &["cav-core", "cav-sim", "cav-engine"])];

/// Where the workspace is allowed to differ from [`PLAN`] for the time being.
#[derive(Clone, Copy, Debug)]
pub struct Allowances {
    /// Crates of the plan that need not exist yet.
    pub absent: &'static [&'static str],
    /// Normal dependencies outside the plan that are let through: `(crate, dependency)`.
    pub edges: &'static [(&'static str, &'static str)],
}

impl Allowances {
    /// No allowance: the plan as written.
    pub const NONE: Self = Self { absent: &[], edges: &[] };
}

/// TEMPORARY, until TASK-S2-07 merges: `cav-plot` is not in the workspace yet, and `cav` still
/// takes its plots from `cav-app`. Delete this constant when that task lands and hold the real
/// workspace to [`Allowances::NONE`]; nothing else may be added to it.
pub const UNTIL_CAV_PLOT_LANDS: Allowances =
    Allowances { absent: &["cav-plot"], edges: &[("cav", "cav-app")] };

/// TEMPORARY: the library crates that do not yet carry `#![warn(missing_docs)]`. `cav-engine` and
/// `cav-app` are being restructured by TASK-S2-07, which also adds `cav-plot`; empty this list
/// when they are documented.
pub const NOT_YET_DOCUMENTED: &[&str] = &["cav-app", "cav-plot"];

/// Every way the manifests break the plan, in words; empty when they keep it.
///
/// A crate that is not in the plan, a crate of the plan that is not in the workspace, a normal
/// dependency on a workspace crate the plan does not give it, and a dev-dependency on a
/// workspace crate that is neither test support nor something the crate may depend on anyway.
/// Dependencies on crates outside the workspace are not this test's business.
pub fn dependency_faults(manifests: &[Manifest], allowances: &Allowances) -> Vec<String> {
    let in_workspace = |name: &str| manifests.iter().any(|m| m.name == name);
    let mut faults = Vec::new();
    for (planned, _) in PLAN {
        if !in_workspace(planned) && !allowances.absent.contains(planned) {
            faults.push(format!("{planned} is in the plan's table and not in the workspace"));
        }
    }
    for manifest in manifests {
        let name = manifest.name.as_str();
        let Some((_, allowed)) = PLAN.iter().find(|(planned, _)| *planned == name) else {
            faults.push(format!(
                "{name} is in the workspace and not in the plan's table: add its row (and say so in the plan)"
            ));
            continue;
        };
        for dependency in manifest.normal.iter().filter(|d| in_workspace(d)) {
            let dependency = dependency.as_str();
            if !allowed.contains(&dependency) && !allowances.edges.contains(&(name, dependency)) {
                faults.push(format!("{name} depends on {dependency}; the plan allows it {allowed:?}"));
            }
        }
        let also = DEV_ALSO.iter().find(|(of, _)| *of == name).map_or(&[][..], |(_, also)| *also);
        for dependency in manifest.dev.iter().filter(|d| in_workspace(d)) {
            let dependency = dependency.as_str();
            if !(allowed.contains(&dependency)
                || DEV_ANYWHERE.contains(&dependency)
                || also.contains(&dependency))
            {
                faults.push(format!(
                    "{name}'s tests depend on {dependency}; test support is {DEV_ANYWHERE:?}, and its own list is {also:?}"
                ));
            }
        }
    }
    faults
}

/// Whether a crate's `lib.rs` turns the `missing_docs` lint on for the whole crate: the inner
/// attribute on a line of its own (so not in a comment), as a warning or stronger.
pub fn warns_of_missing_docs(lib: &str) -> bool {
    lib.lines().map(str::trim).any(|line| {
        ["warn", "deny", "forbid"].iter().any(|level| line == format!("#![{level}(missing_docs)]"))
    })
}

/// The library crates that do not warn of missing docs, apart from those in `excused`.
/// `libraries` is each library crate's name with the text of its `lib.rs`.
pub fn undocumented<'a>(libraries: &[(&'a str, &str)], excused: &[&str]) -> Vec<&'a str> {
    libraries
        .iter()
        .filter(|(name, lib)| !warns_of_missing_docs(lib) && !excused.contains(name))
        .map(|(name, _)| *name)
        .collect()
}

/// The directories of the workspace's crates: every directory of `crates/` with a manifest, and
/// `xtask`. That is what the root manifest's `members` says, and this checks that it still does.
///
/// # Errors
/// When the root manifest lists its members another way, or a directory cannot be read.
pub fn member_directories(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let manifest =
        std::fs::read_to_string(root.join("Cargo.toml")).map_err(|e| format!("Cargo.toml: {e}"))?;
    if !manifest.lines().any(|line| line.trim() == r#"members = ["crates/*", "xtask"]"#) {
        return Err("the root manifest's members are no longer `crates/*` and `xtask`: \
                    teach xtask/src/workspace.rs where the crates are"
            .into());
    }
    let mut directories = vec![root.join("xtask")];
    for entry in std::fs::read_dir(root.join("crates")).map_err(|e| format!("crates/: {e}"))? {
        let path = entry.map_err(|e| format!("crates/: {e}"))?.path();
        if path.join("Cargo.toml").is_file() {
            directories.push(path);
        }
    }
    directories.sort();
    Ok(directories)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask is one level down").to_path_buf()
    }

    fn manifest(name: &str, normal: &[&str], dev: &[&str]) -> Manifest {
        let owned = |names: &[&str]| names.iter().map(|n| (*n).to_owned()).collect();
        Manifest { name: name.to_owned(), normal: owned(normal), dev: owned(dev) }
    }

    /// A workspace that keeps the plan to the letter, `cav-plot` included.
    fn as_planned() -> Vec<Manifest> {
        PLAN.iter().map(|(name, allowed)| manifest(name, allowed, &["cav-validate"])).collect()
    }

    fn real_manifests() -> Vec<Manifest> {
        member_directories(&root())
            .unwrap()
            .iter()
            .map(|directory| {
                let path = directory.join("Cargo.toml");
                let text =
                    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                parse_manifest(&text).unwrap_or_else(|why| panic!("{}: {why}", path.display()))
            })
            .collect()
    }

    #[test]
    fn a_manifest_gives_its_name_and_its_dependencies_by_table() {
        let text = r#"
[package]
name = "cav-example" # not "cav-other"
description = "name = \"cav-wrong\""

[dependencies]
cav-core.workspace = true
serde = { version = "1.0", features = [
    "derive", # cav-sim = "not a dependency"
] }
core = { package = "cav-gpu", path = "../cav-gpu" }

[dependencies.cav-sim]
path = "../cav-sim"

[target.'cfg(windows)'.dependencies]
cav-render = { workspace = true }

[build-dependencies]
cav-engine.workspace = true

[dev-dependencies]
cav-validate.workspace = true

[lints]
workspace = true

[workspace.dependencies]
cav-app = { path = "crates/cav-app" }
"#;
        let read = parse_manifest(text).unwrap();
        assert_eq!(read.name, "cav-example");
        assert_eq!(read.normal, ["cav-core", "serde", "cav-gpu", "cav-sim", "cav-render", "cav-engine"]);
        assert_eq!(read.dev, ["cav-validate"]);
        // Control: a manifest with no package is refused, not read as a crate with no name.
        assert!(parse_manifest("[dependencies]\ncav-core.workspace = true\n").is_err());
    }

    #[test]
    fn the_crates_depend_as_the_plan_says() {
        let real = real_manifests();
        let faults = dependency_faults(&real, &UNTIL_CAV_PLOT_LANDS);
        assert!(faults.is_empty(), "the workspace breaks the plan's table:\n{}", faults.join("\n"));
        // The reading itself is not vacuous: the real workspace has the edges it is known to have.
        let of = |name: &str| real.iter().find(|m| m.name == name).unwrap_or_else(|| panic!("no {name}"));
        assert!(of("cav-sim").normal.iter().any(|d| d == "cav-core"));
        assert!(of("cav-render").normal.iter().any(|d| d == "cav-gpu"));
        assert!(of("cav-core").dev.iter().any(|d| d == "cav-validate"));
        assert!(of("cav-core").normal.iter().all(|d| !d.starts_with("cav")));
        assert!(of("xtask").normal.is_empty() && of("xtask").dev.is_empty(), "xtask depends on nothing");

        // Control arms, on made-up workspaces. The plan kept to the letter passes with no allowance.
        assert_eq!(dependency_faults(&as_planned(), &Allowances::NONE), Vec::<String>::new());
        // The core depending on the simulation is refused.
        let mut upside_down = as_planned();
        upside_down[0].normal.push("cav-sim".into());
        let faults = dependency_faults(&upside_down, &Allowances::NONE);
        assert_eq!(faults.len(), 1, "{faults:?}");
        assert!(faults[0].starts_with("cav-core depends on cav-sim"), "{faults:?}");
        // A crate the table does not know is refused, whatever it depends on.
        let mut one_more = as_planned();
        one_more.push(manifest("cav-new", &[], &[]));
        let faults = dependency_faults(&one_more, &Allowances::NONE);
        assert_eq!(faults.len(), 1, "{faults:?}");
        assert!(
            faults[0].starts_with("cav-new is in the workspace and not in the plan's table"),
            "{faults:?}"
        );
        // A crate of the plan that is not there is refused unless it is allowed to be absent.
        let without_plot: Vec<Manifest> = as_planned().into_iter().filter(|m| m.name != "cav-plot").collect();
        let without_plot: Vec<Manifest> = without_plot
            .into_iter()
            .map(|mut m| {
                m.normal.retain(|d| d != "cav-plot");
                m
            })
            .collect();
        assert_eq!(dependency_faults(&without_plot, &Allowances::NONE).len(), 1);
        assert!(dependency_faults(&without_plot, &UNTIL_CAV_PLOT_LANDS).is_empty());
        // The command line depending on the application is refused without today's allowance, and
        // the allowance lets through that edge and no other.
        let mut cli_on_app = as_planned();
        let cav = cli_on_app.iter_mut().find(|m| m.name == "cav").unwrap();
        cav.normal.push("cav-app".into());
        assert_eq!(dependency_faults(&cli_on_app, &Allowances::NONE).len(), 1);
        assert!(dependency_faults(&cli_on_app, &UNTIL_CAV_PLOT_LANDS).is_empty());
        let engine = cli_on_app.iter_mut().find(|m| m.name == "cav-engine").unwrap();
        engine.normal.push("cav-app".into());
        assert_eq!(dependency_faults(&cli_on_app, &UNTIL_CAV_PLOT_LANDS).len(), 1);
        // Dev-dependencies are judged apart: test support anywhere, the engine in the memory
        // tests, but not the engine in the core's tests.
        let mut tests = as_planned();
        tests.iter_mut().find(|m| m.name == "cav-memtest").unwrap().dev.push("cav-engine".into());
        tests.iter_mut().find(|m| m.name == "cav-gpu").unwrap().dev.push("cav-memtest".into());
        assert!(dependency_faults(&tests, &Allowances::NONE).is_empty());
        tests[0].dev.push("cav-engine".into());
        let faults = dependency_faults(&tests, &Allowances::NONE);
        assert_eq!(faults.len(), 1, "{faults:?}");
        assert!(faults[0].starts_with("cav-core's tests depend on cav-engine"), "{faults:?}");
        // A dependency from outside the workspace is nobody's fault here.
        let mut outside = as_planned();
        outside[0].normal.push("serde".into());
        assert!(dependency_faults(&outside, &Allowances::NONE).is_empty());
    }

    #[test]
    fn every_library_crate_warns_of_missing_docs() {
        let mut libraries = Vec::new();
        for directory in member_directories(&root()).unwrap() {
            let Ok(lib) = std::fs::read_to_string(directory.join("src").join("lib.rs")) else { continue };
            let manifest = std::fs::read_to_string(directory.join("Cargo.toml")).unwrap();
            libraries.push((parse_manifest(&manifest).unwrap().name, lib));
        }
        let libraries: Vec<(&str, &str)> = libraries.iter().map(|(n, l)| (n.as_str(), l.as_str())).collect();
        // Not vacuous: the library crates were found, and the ones this rule already holds are
        // among them.
        for held in ["cav-core", "cav-sim", "cav-gpu", "cav-render", "cav-validate", "cav-memtest"] {
            assert!(libraries.iter().any(|(name, _)| *name == held), "{held}'s lib.rs was not found");
            assert!(!NOT_YET_DOCUMENTED.contains(&held), "{held} is documented and stays so");
        }
        let without = undocumented(&libraries, NOT_YET_DOCUMENTED);
        assert!(
            without.is_empty(),
            "these library crates lack `#![warn(missing_docs)]` in lib.rs: {without:?}"
        );

        // Control arms. A crate without the attribute is named; one excused is not.
        let with = "//! A crate.\n\n#![warn(missing_docs)]\n\npub mod a;\n";
        let made_up = [("with", with), ("without", "//! A crate.\n\npub mod a;\n")];
        assert_eq!(undocumented(&made_up, &[]), ["without"]);
        assert_eq!(undocumented(&made_up, &["without"]), Vec::<&str>::new());
        // The attribute in a comment, allowed rather than warned of, or for another lint, is not it.
        for not_it in [
            "// #![warn(missing_docs)]\n",
            "//! Put `#![warn(missing_docs)]` here.\n",
            "#![allow(missing_docs)]\n",
            "#![warn(missing_debug_implementations)]\n",
            "#[warn(missing_docs)]\npub mod a;\n",
        ] {
            assert!(!warns_of_missing_docs(not_it), "{not_it:?} was taken for the attribute");
        }
        assert!(warns_of_missing_docs("#![deny(missing_docs)]\n"), "stronger than a warning is fine");
        assert!(warns_of_missing_docs("    #![warn(missing_docs)]\r\n"), "indentation and CRLF are fine");
    }
}
