//! A small JSON reader, for what cargo says about the workspace (`cargo metadata`). xtask has no
//! dependencies, and must not gain one to read a file format this plain.
//!
//! Tests only. Numbers are kept as `f64`; `\u` escapes of the basic plane are read, surrogate
//! pairs become the replacement character (cargo's metadata has neither).

/// A JSON value.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    List(Vec<Json>),
    /// The members in the order written.
    Table(Vec<(String, Json)>),
}

impl Json {
    /// The member called `key` of a table; `Null` for anything else.
    pub fn get(&self, key: &str) -> &Json {
        match self {
            Json::Table(members) => {
                members.iter().find(|(name, _)| name == key).map_or(&Json::Null, |(_, v)| v)
            }
            _ => &Json::Null,
        }
    }

    /// The items of a list; none for anything else.
    pub fn items(&self) -> &[Json] {
        match self {
            Json::List(items) => items,
            _ => &[],
        }
    }

    /// The text of a string.
    pub fn text(&self) -> Option<&str> {
        match self {
            Json::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// Reads one JSON value that is the whole of `text`.
///
/// # Errors
/// Where the text stops being JSON, as a byte offset.
pub fn parse(text: &str) -> Result<Json, String> {
    let bytes = text.as_bytes();
    let mut at = 0;
    let value = value(bytes, &mut at, 0)?;
    space(bytes, &mut at);
    if at == bytes.len() { Ok(value) } else { Err(format!("something follows the value at byte {at}")) }
}

fn space(bytes: &[u8], at: &mut usize) {
    while bytes.get(*at).is_some_and(|b| b" \t\r\n".contains(b)) {
        *at += 1;
    }
}

/// How deep a value may nest: cargo's metadata is five or six deep; this stops a file of brackets.
const MAX_DEPTH: usize = 64;

fn value(bytes: &[u8], at: &mut usize, depth: usize) -> Result<Json, String> {
    if depth > MAX_DEPTH {
        return Err(format!("nested more than {MAX_DEPTH} deep at byte {at}"));
    }
    space(bytes, at);
    let literal = |at: &mut usize, word: &[u8], value: Json| {
        if bytes[*at..].starts_with(word) {
            *at += word.len();
            Ok(value)
        } else {
            Err(format!("not JSON at byte {at}"))
        }
    };
    match bytes.get(*at) {
        Some(b'n') => literal(at, b"null", Json::Null),
        Some(b't') => literal(at, b"true", Json::Bool(true)),
        Some(b'f') => literal(at, b"false", Json::Bool(false)),
        Some(b'"') => string(bytes, at).map(Json::Text),
        Some(b'[') => {
            *at += 1;
            let mut items = Vec::new();
            loop {
                space(bytes, at);
                if bytes.get(*at) == Some(&b']') {
                    *at += 1;
                    return Ok(Json::List(items));
                }
                if !items.is_empty() {
                    if bytes.get(*at) != Some(&b',') {
                        return Err(format!("a comma or `]` was expected at byte {at}"));
                    }
                    *at += 1;
                }
                items.push(value(bytes, at, depth + 1)?);
            }
        }
        Some(b'{') => {
            *at += 1;
            let mut members = Vec::new();
            loop {
                space(bytes, at);
                if bytes.get(*at) == Some(&b'}') {
                    *at += 1;
                    return Ok(Json::Table(members));
                }
                if !members.is_empty() {
                    if bytes.get(*at) != Some(&b',') {
                        return Err(format!("a comma or `}}` was expected at byte {at}"));
                    }
                    *at += 1;
                    space(bytes, at);
                }
                let key = string(bytes, at)?;
                space(bytes, at);
                if bytes.get(*at) != Some(&b':') {
                    return Err(format!("a colon was expected at byte {at}"));
                }
                *at += 1;
                members.push((key, value(bytes, at, depth + 1)?));
            }
        }
        Some(b'-' | b'0'..=b'9') => {
            let start = *at;
            while bytes.get(*at).is_some_and(|b| b"+-.eE0123456789".contains(b)) {
                *at += 1;
            }
            std::str::from_utf8(&bytes[start..*at])
                .ok()
                .and_then(|number| number.parse().ok())
                .map(Json::Number)
                .ok_or_else(|| format!("not a number at byte {start}"))
        }
        _ => Err(format!("not JSON at byte {at}")),
    }
}

fn string(bytes: &[u8], at: &mut usize) -> Result<String, String> {
    if bytes.get(*at) != Some(&b'"') {
        return Err(format!("a string was expected at byte {at}"));
    }
    *at += 1;
    let mut out = Vec::new();
    loop {
        match bytes.get(*at) {
            None => return Err("a string does not end".into()),
            Some(b'"') => {
                *at += 1;
                return String::from_utf8(out).map_err(|_| "a string is not UTF-8".to_owned());
            }
            Some(b'\\') => {
                let escaped = *bytes.get(*at + 1).ok_or("a string does not end")?;
                *at += 2;
                match escaped {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'r' => out.push(b'\r'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'u' => {
                        let code = bytes
                            .get(*at..*at + 4)
                            .and_then(|hex| std::str::from_utf8(hex).ok())
                            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                            .ok_or_else(|| format!("a bad \\u escape at byte {at}"))?;
                        *at += 4;
                        let c = char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER);
                        out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                    }
                    other => out.push(other),
                }
            }
            Some(byte) => {
                out.push(*byte);
                *at += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_is_read_and_what_is_not_json_is_refused() {
        let read =
            parse(r#" {"a": [1, -2.5e3, true, null, "x\"y\\zé\n"], "b": {"c": {}}, "d": []} "#).unwrap();
        assert_eq!(
            read.get("a").items(),
            [
                Json::Number(1.0),
                Json::Number(-2500.0),
                Json::Bool(true),
                Json::Null,
                Json::Text("x\"y\\zé\n".into())
            ]
        );
        assert_eq!(read.get("b").get("c"), &Json::Table(vec![]));
        assert_eq!(read.get("d").items().len(), 0);
        assert_eq!(read.get("absent"), &Json::Null);
        assert_eq!(read.get("a").get("of a list"), &Json::Null);
        assert_eq!(read.get("a").items()[4].text(), Some("x\"y\\zé\n"));
        // Controls: each of these is not JSON and says so.
        for bad in ["", "{", r#"{"a" 1}"#, "[1 2]", r#"{"a": 1} x"#, r#""open"#, "nul", "[1,]x", "-"] {
            assert!(parse(bad).is_err(), "`{bad}` was read");
        }
        // Brackets without end are refused, not followed down.
        assert!(parse(&"[".repeat(100_000)).unwrap_err().contains("deep"));
    }
}
