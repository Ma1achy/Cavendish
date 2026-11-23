//! The recipes in `docs/extending/` are held to the code: every file, function, test and
//! validation case a recipe names in backticks must exist, so a rename fails `quick` until the
//! recipe follows it.
//!
//! What counts as a name (anything else in backticks is prose or an example and is not checked):
//! - a path under `crates/`, `scenes/`, `docs/`, `xtask/`, `oracle/` or `plan/`: the file exists;
//! - `name()`: a function `fn name` is declared somewhere in the source;
//! - `A::b` (also `a::tests::b`): the last segment is a word of the source;
//! - a snake-case word with two or more underscores (a test's name), or an upper-case constant
//!   with an underscore: a word of the source;
//! - `subsystem.case_name`: a validation case's name, quoted in the source.
//!
//! A recipe may mark a made-up example with `my` or `your` in it (`validation_my_case`,
//! `YourKind`): those are not checked.

use std::path::Path;

/// The names a recipe's text makes, as `(kind, name)`; see the module's rules.
fn named(text: &str) -> Vec<(Kind, String)> {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        for (index, token) in line.split('`').enumerate() {
            // Odd pieces are inside backticks.
            if index % 2 == 0 || token.is_empty() {
                continue;
            }
            let lower = token.to_lowercase();
            if lower.contains("my") || lower.contains("your") || token.contains('<') || token.contains('…')
            {
                continue;
            }
            let word =
                |text: &str| !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            let roots = ["crates/", "scenes/", "docs/", "xtask/", "oracle/", "plan/"];
            if roots.iter().any(|root| token.starts_with(root)) {
                if !token.contains('*') && !token.contains(' ') {
                    out.push((Kind::File, token.trim_end_matches('/').to_owned()));
                }
            } else if let Some(name) = token.strip_suffix("()") {
                let name = name.rsplit("::").next().unwrap_or(name);
                if word(name) {
                    out.push((Kind::Function, name.to_owned()));
                }
            } else if token.contains("::") {
                let last = token.rsplit("::").next().unwrap_or(token);
                let last = last.split('(').next().unwrap_or(last);
                if word(last) && token.split("::").all(|part| word(part.split('(').next().unwrap_or(part))) {
                    out.push((Kind::Word, last.to_owned()));
                }
            } else if word(token) {
                let underscores = token.matches('_').count();
                let constant =
                    token.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
                let snake = token.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
                if (snake && underscores >= 2) || (constant && underscores >= 1) {
                    out.push((Kind::Word, token.to_owned()));
                }
            } else if let Some((subsystem, case)) = token.split_once('.') {
                let lower = |text: &str| word(text) && !text.chars().any(|c| c.is_ascii_uppercase());
                if lower(subsystem) && lower(case) && case.contains('_') {
                    out.push((Kind::Case, token.to_owned()));
                }
            }
        }
    }
    out
}

/// What kind of thing a recipe names, which decides how it is looked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    File,
    Function,
    Word,
    Case,
}

/// Whether `name` is in `source` as a whole word.
fn has_word(source: &str, name: &str) -> bool {
    let edge = |c: Option<char>| c.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
    source.match_indices(name).any(|(at, _)| {
        edge(source[..at].chars().next_back()) && edge(source[at + name.len()..].chars().next())
    })
}

/// The names of `recipe` that are not in the repository at `root` with source text `source`.
fn missing(root: &Path, source: &str, recipe: &str) -> Vec<String> {
    named(recipe)
        .into_iter()
        .filter(|(kind, name)| match kind {
            Kind::File => !root.join(name).exists(),
            Kind::Function => !has_word(source, &format!("fn {name}")),
            Kind::Word => !has_word(source, name),
            Kind::Case => !source.contains(&format!("\"{name}\"")),
        })
        .map(|(kind, name)| format!("{kind:?} `{name}`"))
        .collect()
}

/// Every `.rs` file under `dir`, read into one text. The tree is the repository's own source.
fn source_under(dir: &Path, into: &mut String) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if name != "target" && name != ".git" {
                source_under(&path, into);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            into.push_str(&text);
            into.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recipes_name_things_that_exist() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut source = String::new();
        for dir in ["crates", "xtask/src", "oracle/tests", "oracle/src"] {
            source_under(&root.join(dir), &mut source);
        }
        // This file's own examples must not be what satisfies a recipe.
        let own = std::fs::read_to_string(root.join("xtask/src/recipes.rs")).unwrap();
        let source = source.replace(&own, "fn the_recipes_name_things_that_exist() {}\n");
        let dir = root.join("docs").join("extending");
        let mut recipes = 0;
        let mut names = 0;
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "md") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let lost = missing(root, &source, &text);
            assert!(lost.is_empty(), "{} names what is not in the code: {lost:?}", path.display());
            recipes += 1;
            names += named(&text).len();
            // Every recipe but the index names an example to read.
            if entry.file_name() != "README.md" {
                assert!(text.contains("**Example to read:**"), "{} names no example", path.display());
            }
        }
        // The index and a recipe for each of the six kinds; and the check is not empty.
        assert!(recipes >= 7, "{recipes} recipes");
        assert!(names >= 100, "only {names} names were checked");

        // Controls: each kind of name, wrong, is found; right, is not.
        let wrong = "Run `no_such_test_at_all`, call `nowhere_fn()`, read `crates/cav-core/src/absent.rs`, \
                     see `Engine::not_a_method`, case `timing.no_such_case`, and `NOT_A_CONSTANT`.";
        assert_eq!(missing(root, &source, wrong).len(), 6, "{:?}", missing(root, &source, wrong));
        let right = "Run `the_recipes_name_things_that_exist`? No: `every_documented_command_parses`, call \
                     `sources()`, read `crates/cav-core/src/forward.rs`, see `Engine::prepare`, case \
                     `timing.offset_shifts_one_tower`, and `S2_SCENES`.";
        assert_eq!(missing(root, &source, right), Vec::<String>::new());
        // A word inside a longer word is not that word.
        assert!(has_word("fn key(", "key") && !has_word("fn monkey(", "key") && !has_word("keyed", "key"));
        // Made-up examples and code blocks are not checked.
        assert!(
            named("`validation_my_case`, `YourKind::field`\n```\n`not_checked_in_a_block`\n```").is_empty()
        );
    }
}
