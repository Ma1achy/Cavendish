//! A small argument reader: positional words, `--name value` options and `--name` flags. The
//! CLI has a dozen subcommands, which does not earn a parsing dependency.

/// The arguments of one subcommand.
#[derive(Debug)]
pub struct Args {
    positional: Vec<String>,
    options: Vec<(String, String)>,
    flags: Vec<String>,
}

impl Args {
    /// Splits `raw` into positional words and the options named in `known` (without the dashes).
    ///
    /// # Errors
    /// An option that is not known, is given twice or has no value.
    pub fn parse(raw: &[String], known: &[&str]) -> Result<Self, String> {
        Self::parse_with_flags(raw, known, &[])
    }

    /// As [`Args::parse`], and also the flags named in `flags`: `--name` with no value.
    ///
    /// # Errors
    /// An option or flag that is not known or is given twice, an option with no value, a flag with one.
    pub fn parse_with_flags(raw: &[String], known: &[&str], flags: &[&str]) -> Result<Self, String> {
        let mut args = Self { positional: Vec::new(), options: Vec::new(), flags: Vec::new() };
        let mut words = raw.iter();
        while let Some(word) = words.next() {
            let Some(name) = word.strip_prefix("--") else {
                args.positional.push(word.clone());
                continue;
            };
            // Both `--out file` and `--out=file`.
            let (name, inline) = name.split_once('=').map_or((name, None), |(n, v)| (n, Some(v.to_owned())));
            if flags.contains(&name) {
                if inline.is_some() {
                    return Err(format!("`--{name}` takes no value"));
                }
                if args.flags.iter().any(|n| n == name) {
                    return Err(format!("`--{name}` is given twice"));
                }
                args.flags.push(name.to_owned());
                continue;
            }
            if !known.contains(&name) {
                let all: Vec<&str> = known.iter().chain(flags).copied().collect();
                let list = if all.is_empty() { "none".to_owned() } else { format!("--{}", all.join(", --")) };
                return Err(format!("`--{name}` is not an option here (options: {list})"));
            }
            if args.options.iter().any(|(n, _)| n == name) {
                return Err(format!("`--{name}` is given twice"));
            }
            let value = match inline {
                Some(value) => value,
                None => words.next().cloned().ok_or_else(|| format!("`--{name}` needs a value"))?,
            };
            args.options.push((name.to_owned(), value));
        }
        Ok(args)
    }

    /// Takes the value of an option, if it was given.
    pub fn option(&mut self, name: &str) -> Option<String> {
        let at = self.options.iter().position(|(n, _)| n == name)?;
        Some(self.options.remove(at).1)
    }

    /// Whether a flag was given.
    pub fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|n| n == name)
    }

    /// The positional words, which must number exactly `count`.
    ///
    /// # Errors
    /// Too few or too many words.
    pub fn finish(self, count: usize) -> Result<Vec<String>, String> {
        if self.positional.len() == count {
            Ok(self.positional)
        } else {
            Err(format!(
                "expected {count} argument{}, found {}: {:?}",
                if count == 1 { "" } else { "s" },
                self.positional.len(),
                self.positional
            ))
        }
    }
}

impl Args {
    /// The positional words, which must number at most `most`.
    ///
    /// # Errors
    /// Too many words.
    pub fn finish_up_to(self, most: usize) -> Result<Vec<String>, String> {
        if self.positional.len() <= most {
            Ok(self.positional)
        } else {
            Err(format!(
                "expected at most {most} argument{}, found {}: {:?}",
                if most == 1 { "" } else { "s" },
                self.positional.len(),
                self.positional
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn options_and_positionals_are_separated() {
        let mut args =
            Args::parse(&words("room --out a.png --size=640x480 extra"), &["out", "size", "view"]).unwrap();
        assert_eq!(args.option("out").as_deref(), Some("a.png"));
        assert_eq!(args.option("size").as_deref(), Some("640x480"));
        assert_eq!(args.option("view"), None);
        assert_eq!(args.finish(2).unwrap(), ["room", "extra"]);
    }

    #[test]
    fn mistakes_are_refused_with_the_reason() {
        let known = ["out", "view"];
        let refused = |text: &str| Args::parse(&words(text), &known).unwrap_err();
        assert!(
            refused("room --colour red")
                .contains("`--colour` is not an option here (options: --out, --view)")
        );
        assert!(refused("room --out").contains("needs a value"));
        assert!(refused("--out a --out b").contains("given twice"));
        assert!(
            Args::parse(&words("a b"), &known)
                .unwrap()
                .finish(1)
                .unwrap_err()
                .contains("expected 1 argument, found 2")
        );
        assert!(Args::parse(&words("--x 1"), &[]).unwrap_err().contains("options: none"));
        // A flag takes no value and stands anywhere; a word after it is positional.
        let flagged = |text: &str| Args::parse_with_flags(&words(text), &known, &["quiet"]);
        let args = flagged("room --quiet --out a extra").unwrap();
        assert!(args.flag("quiet") && !args.flag("out"));
        assert_eq!(args.finish(2).unwrap(), ["room", "extra"]);
        assert!(flagged("room --quiet=yes").unwrap_err().contains("takes no value"));
        assert!(flagged("--quiet --quiet").unwrap_err().contains("given twice"));
        assert!(flagged("--loud").unwrap_err().contains("options: --out, --view, --quiet"));
        assert!(!flagged("room").unwrap().flag("quiet"), "a flag not given is not set");
        // Control: the same shapes, written correctly, are accepted.
        assert!(Args::parse(&words("room --out a --view top"), &known).unwrap().finish(1).is_ok());
    }
}
