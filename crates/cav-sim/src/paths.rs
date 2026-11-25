//! Paths into a scene spec: the one implementation of `get`, `set`, `insert`, `remove`, `ls` and
//! the schema at a path. The engine, `cav get|set|ls`, the MCP tools and the application's
//! property editors all come here, so a field is addressed, described and edited the same way
//! from every one of them.
//!
//! A path is the field names from the top of the spec, joined by dots, with `[n]` for the `n`-th
//! item of a list (from 0): `array.towers[2].position[0]`. An enum that carries data is addressed
//! through its variant, as it is written in a file: `bodies[0].motion.Static.position`. The empty
//! path is the whole spec.
//!
//! How it works: the spec is turned into a `serde_json::Value` (exact for every finite `f64`:
//! the workspace builds `serde_json` with `float_roundtrip`), the value is walked, and an edited
//! value is turned back into a [`SceneSpec`] and validated. What a field *is* (its kind, unit,
//! range, doc string, and the stage an edit of it invalidates) is read from the spec's JSON
//! Schema, which `schemars` derives from the types: units and stages are attributes on the fields
//! (`x-unit`, `x-stage`), so there is no second table to forget.
//!
//! Not for hot loops: every edit costs a trip of the whole spec through JSON (measured in
//! `docs/slices/S2.md` §7). Indices are positions: removing `bodies[2]` renumbers `bodies[3]`.

use serde_json::Value;

use crate::SceneSpec;

/// A field's or a variant's name in words, capitalised: `tower_height` is "Tower height",
/// `LowerMinusUpper` "Lower minus upper", `HalfSpace` "Half space". Display only: what a file or
/// a console line writes is the name itself.
#[must_use]
pub fn words(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    let mut previous: Option<char> = None;
    for c in name.chars() {
        if c == '_' {
            out.push(' ');
        } else if c.is_uppercase() && previous.is_some_and(|p| p.is_lowercase() || p.is_ascii_digit()) {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else if out.is_empty() {
            out.extend(c.to_uppercase());
        } else if c.is_uppercase() && out.ends_with(' ') {
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
        previous = Some(c);
    }
    out
}

/// One step of a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seg {
    /// A field of a struct, or the variant of an enum that carries data.
    Field(String),
    /// An item of a list or a component of a vector, from 0.
    Index(usize),
}

/// Parses a path into its steps. The empty path (or blanks) is no steps: the whole spec.
///
/// # Errors
/// A `[` with no `]`, an index that is not a whole number, an empty field name.
pub fn parse(path: &str) -> Result<Vec<Seg>, String> {
    let path = path.trim();
    let mut segs = Vec::new();
    if path.is_empty() {
        return Ok(segs);
    }
    for part in path.split('.') {
        let (name, mut rest) = part.split_at(part.find('[').unwrap_or(part.len()));
        if name.is_empty() && rest.is_empty() {
            return Err(format!("`{path}`: a path is field names joined by single dots"));
        }
        if !name.is_empty() {
            segs.push(Seg::Field(name.to_owned()));
        }
        while let Some(open) = rest.strip_prefix('[') {
            let Some(close) = open.find(']') else {
                return Err(format!("`{part}`: a `[` needs a `]`"));
            };
            let number = &open[..close];
            let index = number.parse().map_err(|_| format!("`{part}`: `{number}` is not an index"))?;
            segs.push(Seg::Index(index));
            rest = &open[close + 1..];
        }
        if !rest.is_empty() {
            return Err(format!("`{part}`: `{rest}` is not part of a path"));
        }
    }
    Ok(segs)
}

/// The text of a path: the inverse of [`parse`].
#[must_use]
pub fn text(segs: &[Seg]) -> String {
    let mut out = String::new();
    for seg in segs {
        match seg {
            Seg::Field(name) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(name);
            }
            Seg::Index(n) => out.push_str(&format!("[{n}]")),
        }
    }
    out
}

/// `path` with one more step: `join("bodies[0]", "mass")` is `bodies[0].mass`.
#[must_use]
pub fn join(path: &str, field: &str) -> String {
    if path.is_empty() { field.to_owned() } else { format!("{path}.{field}") }
}

fn here(walked: &[Seg]) -> String {
    if walked.is_empty() { "the scene".to_owned() } else { format!("`{}`", text(walked)) }
}

/// Why a step cannot be taken from `value`, said with what is there instead.
fn no_step(value: &Value, walked: &[Seg], seg: &Seg) -> String {
    match (value, seg) {
        (Value::Null, _) => format!(
            "{} is absent (none), so it has nothing inside it; `set {} <value>` adds one, and `schema {}` \
             shows its shape",
            here(walked),
            text(walked),
            text(walked)
        ),
        (Value::Object(map), Seg::Field(name)) => {
            let fields: Vec<&str> = map.keys().map(String::as_str).collect();
            format!("{} has no field `{name}`; it has: {}", here(walked), fields.join(", "))
        }
        (Value::Array(items), Seg::Index(n)) => {
            format!("{} has {} items; there is no [{n}]", here(walked), items.len())
        }
        (_, Seg::Index(_)) => format!("{} is not a list", here(walked)),
        (_, Seg::Field(name)) => format!("{} has no field `{name}`: it is a single value", here(walked)),
    }
}

fn step<'a>(value: &'a Value, seg: &Seg) -> Option<&'a Value> {
    match seg {
        Seg::Field(name) => value.as_object()?.get(name),
        Seg::Index(n) => value.as_array()?.get(*n),
    }
}

/// The value at `path` inside `root`.
///
/// # Errors
/// When the path does not parse or leads nowhere; the message says what is there instead.
pub fn get<'a>(root: &'a Value, path: &str) -> Result<&'a Value, String> {
    let segs = parse(path)?;
    let mut value = root;
    for (n, seg) in segs.iter().enumerate() {
        value = step(value, seg).ok_or_else(|| no_step(value, &segs[..n], seg))?;
    }
    Ok(value)
}

fn get_mut<'a>(root: &'a mut Value, segs: &[Seg]) -> Result<&'a mut Value, String> {
    // Walked read-only first, for the refusals; then again to borrow mutably.
    let mut probe = &*root;
    for (n, seg) in segs.iter().enumerate() {
        probe = step(probe, seg).ok_or_else(|| no_step(probe, &segs[..n], seg))?;
    }
    let mut value = root;
    for seg in segs {
        value = match seg {
            Seg::Field(name) => value.get_mut(name.as_str()).expect("walked"),
            Seg::Index(n) => value.get_mut(*n).expect("walked"),
        };
    }
    Ok(value)
}

/// Replaces the value at `path` and returns what was there. Nothing is created: the place must
/// exist (an absent option is a place: it holds `null`).
///
/// # Errors
/// As [`get`].
pub fn set(root: &mut Value, path: &str, value: Value) -> Result<Value, String> {
    let segs = parse(path)?;
    Ok(std::mem::replace(get_mut(root, &segs)?, value))
}

/// Puts `value` into the list at `list`, before item `index` (at the end if `None`). Returns the
/// index it now has.
///
/// # Errors
/// When `list` is not a list, or `index` is past its end.
pub fn insert(root: &mut Value, list: &str, index: Option<usize>, value: Value) -> Result<usize, String> {
    let segs = parse(list)?;
    let Value::Array(items) = get_mut(root, &segs)? else {
        return Err(format!("{} is not a list: nothing can be added to it", here(&segs)));
    };
    let at = index.unwrap_or(items.len());
    if at > items.len() {
        return Err(format!(
            "{} has {} items; an item can be put at [0] to [{}]",
            here(&segs),
            items.len(),
            items.len()
        ));
    }
    items.insert(at, value);
    Ok(at)
}

/// Takes the item at `path` (which ends in an index) out of its list and returns it. The items
/// after it move down by one.
///
/// # Errors
/// When `path` does not end in an index of a list.
pub fn remove(root: &mut Value, path: &str) -> Result<Value, String> {
    let segs = parse(path)?;
    let Some((Seg::Index(n), list)) = segs.split_last() else {
        return Err(format!(
            "`{path}` is not an item of a list: `remove` takes a path that ends in an index, such as bodies[1] \
             (to take an optional part away, set it to null)"
        ));
    };
    let Value::Array(items) = get_mut(root, list)? else {
        return Err(format!("{} is not a list", here(list)));
    };
    if *n >= items.len() {
        return Err(format!("{} has {} items; there is no [{n}]", here(list), items.len()));
    }
    Ok(items.remove(*n))
}

/// What an edit of a field invalidates: the earliest thing that must be done again.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum Stage {
    /// Nothing is computed from it (a name, a description).
    Nothing,
    /// Only what is drawn (the room).
    View,
    /// The forward model: every tower's signal.
    Forward,
}

impl Stage {
    /// The word used in the log and in `ls`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nothing => "nothing",
            Self::View => "view",
            Self::Forward => "forward",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        [Self::Nothing, Self::View, Self::Forward].into_iter().find(|stage| stage.name() == word)
    }
}

/// What kind of thing a field is: which editor it gets, and how `ls` describes it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum Kind {
    /// A real number.
    Number,
    /// A whole number.
    Integer,
    /// Text.
    Text,
    /// True or false.
    Flag,
    /// A fixed number of real numbers: a position, a size.
    Vector(usize),
    /// A list that items can be added to and removed from.
    List,
    /// A struct: its fields are its children.
    Group,
    /// One of several named variants. A variant that carries data has that data as its child,
    /// under the variant's name.
    Choice(Vec<String>),
}

impl Kind {
    /// The word `ls` prints.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Number => "number".into(),
            Self::Integer => "integer".into(),
            Self::Text => "text".into(),
            Self::Flag => "flag".into(),
            Self::Vector(n) => format!("vector of {n}"),
            Self::List => "list".into(),
            Self::Group => "group".into(),
            Self::Choice(names) => format!("one of {}", names.join(" | ")),
        }
    }

    /// Whether a field of this kind holds one value and has no children.
    #[must_use]
    pub const fn is_leaf(&self) -> bool {
        matches!(self, Self::Number | Self::Integer | Self::Text | Self::Flag | Self::Vector(_))
    }
}

/// One field of a spec, as `ls`, the editors and the schema reference describe it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Entry {
    /// Its path from the top of the spec.
    pub path: String,
    /// Its own name: the last step of the path (`[2]` for an item of a list).
    pub name: String,
    /// What kind of thing it is.
    pub kind: Kind,
    /// Whether it may be absent (`null`).
    pub optional: bool,
    /// Its doc string.
    pub doc: String,
    /// What an editor calls it, when the schema gives it a title (`#[schemars(title = …)]`);
    /// otherwise [`Entry::label`] makes one from its name.
    pub title: Option<String>,
    /// Its unit, as written in the schema (`m`, `s`, `kg/m³`, `1` for a pure number); `None` for
    /// things that have none (text, flags, groups).
    pub unit: Option<String>,
    /// The least value it may take, and whether that value itself is excluded.
    pub minimum: Option<(f64, bool)>,
    /// The greatest value it may take.
    pub maximum: Option<f64>,
    /// The longest it may be, for text.
    pub max_length: Option<usize>,
    /// What an edit of it invalidates.
    pub stage: Stage,
    /// Its value now.
    pub value: Value,
    /// For a choice: the variant it is now.
    pub variant: Option<String>,
}

impl Entry {
    /// What a person reads for it: the schema's title, else its name in words (`tower_height`
    /// is "Tower height", `[2]` is "Item 3"). For editors and breadcrumbs; the path stays the
    /// name a console line uses.
    #[must_use]
    pub fn label(&self) -> String {
        if let Some(title) = &self.title {
            return title.clone();
        }
        if let Some(n) = self
            .name
            .strip_prefix('[')
            .and_then(|n| n.strip_suffix(']'))
            .and_then(|n| n.parse::<usize>().ok())
        {
            return format!("Item {}", n + 1);
        }
        words(&self.name)
    }

    /// One line for `ls`: the name, the value (shortened), the unit, the kind and the first
    /// sentence of the doc string.
    #[must_use]
    pub fn line(&self) -> String {
        let value = match (&self.kind, &self.value) {
            (_, Value::Null) => "none".to_owned(),
            (Kind::List, Value::Array(items)) => format!("{} item(s)", items.len()),
            (Kind::Group, _) => "…".to_owned(),
            (Kind::Choice(_), _) => self.variant.clone().unwrap_or_default(),
            (_, value) => value.to_string(),
        };
        let unit =
            self.unit.as_deref().filter(|unit| *unit != "1").map_or_else(String::new, |u| format!(" {u}"));
        let range = match (self.minimum, self.maximum) {
            (None, None) => String::new(),
            (low, high) => format!(
                " · {}{}",
                low.map_or_else(String::new, |(v, open)| format!(
                    "{} {v}",
                    if open { "above" } else { "from" }
                )),
                high.map_or_else(String::new, |v| format!(" to {v}")),
            ),
        };
        let about = self.doc.split(". ").next().unwrap_or_default().replace('\n', " ");
        let optional = if self.optional { ", optional" } else { "" };
        format!(
            "{}  =  {value}{unit}  ({}{optional}{range})  {}",
            self.name,
            self.kind.name(),
            about.trim_end_matches('.')
        )
    }
}

/// The schema a `$ref` points at, or the schema itself.
fn deref<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
    match schema.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix("#/$defs/")) {
        Some(name) => &root["$defs"][name],
        None => schema,
    }
}

/// A field's schema, looked through: where the field's own keywords are (`site`: the doc string,
/// the unit, the stage), and the schema of what it holds (`body`), with `Option` taken off.
struct Looked<'a> {
    site: &'a Value,
    body: &'a Value,
    optional: bool,
}

fn look<'a>(root: &'a Value, site: &'a Value) -> Looked<'a> {
    // `Option<T>` is `anyOf: [T, null]` for a struct and `type: [T, "null"]` for a plain value.
    if let Some(any) = site.get("anyOf").and_then(Value::as_array) {
        let is_null = |s: &&Value| s.get("type").and_then(Value::as_str) == Some("null");
        if let (Some(inner), true) = (any.iter().find(|s| !is_null(s)), any.iter().any(|s| is_null(&s))) {
            return Looked { site, body: deref(root, inner), optional: true };
        }
    }
    let nullable =
        site.get("type").and_then(Value::as_array).is_some_and(|types| types.iter().any(|t| t == "null"));
    Looked { site, body: deref(root, site), optional: nullable }
}

/// The names of an enum's variants, in the schema's order, and for each the schema of the data it
/// carries (none for a plain variant).
fn variants(body: &Value) -> Vec<(String, Option<&Value>)> {
    let Some(options) = body.get("oneOf").and_then(Value::as_array) else { return Vec::new() };
    let mut out = Vec::new();
    for option in options {
        if let Some(name) = option.get("const").and_then(Value::as_str) {
            out.push((name.to_owned(), None));
        } else if let Some(names) = option.get("enum").and_then(Value::as_array) {
            out.extend(names.iter().filter_map(Value::as_str).map(|name| (name.to_owned(), None)));
        } else if let Some(properties) = option.get("properties").and_then(Value::as_object) {
            out.extend(properties.iter().map(|(name, data)| (name.clone(), Some(data))));
        }
    }
    out
}

/// The schema of the field one step down from `body`.
fn child_schema<'a>(root: &'a Value, body: &'a Value, seg: &Seg) -> Option<&'a Value> {
    match seg {
        Seg::Field(name) => {
            body.get("properties").and_then(|properties| properties.get(name.as_str())).or_else(|| {
                variants(body).into_iter().find(|(variant, _)| variant == name).and_then(|(_, data)| data)
            })
        }
        Seg::Index(_) => body.get("items").map(|items| deref(root, items)).or(body.get("items")),
    }
}

/// The field's own schema at `path`: the site where its keywords are.
fn site_at<'a>(root: &'a Value, segs: &[Seg]) -> Result<&'a Value, String> {
    let mut site = root;
    for (n, seg) in segs.iter().enumerate() {
        let body = look(root, site).body;
        site = child_schema(root, body, seg).ok_or_else(|| match seg {
            Seg::Field(name) => {
                let mut fields: Vec<String> = body
                    .get("properties")
                    .and_then(Value::as_object)
                    .map(|p| p.keys().cloned().collect())
                    .unwrap_or_default();
                fields.extend(variants(body).into_iter().map(|(name, _)| name));
                format!("{} has no field `{name}`; it has: {}", here(&segs[..n]), fields.join(", "))
            }
            Seg::Index(_) => format!("{} is not a list", here(&segs[..n])),
        })?;
    }
    Ok(site)
}

/// The JSON Schema of the field at `path`, with references followed one level so that it stands
/// on its own: its type, its range, its doc string, its unit and its children's names.
///
/// # Errors
/// When there is no such field in any scene (this does not look at a scene's values, so
/// `room.size` has a schema whether or not the scene has a room).
pub fn schema_at(path: &str) -> Result<Value, String> {
    let root = SceneSpec::schema_ref();
    let segs = parse(path)?;
    if segs.is_empty() {
        return Ok(root.clone());
    }
    let site = site_at(root, &segs)?;
    let looked = look(root, site);
    let mut out = looked.body.clone();
    if let (Value::Object(out), Value::Object(site)) = (&mut out, site) {
        for (key, value) in site {
            if key != "$ref" && key != "anyOf" {
                out.insert(key.clone(), value.clone());
            }
        }
        if looked.optional {
            out.insert("x-optional".into(), Value::Bool(true));
        }
    }
    Ok(out)
}

fn kind_of(body: &Value) -> Kind {
    let named = variants(body);
    if !named.is_empty() {
        return Kind::Choice(named.into_iter().map(|(name, _)| name).collect());
    }
    let types: Vec<&str> = match body.get("type") {
        Some(Value::String(one)) => vec![one.as_str()],
        Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    let first = types.into_iter().find(|t| *t != "null").unwrap_or("object");
    match first {
        "number" => Kind::Number,
        "integer" => Kind::Integer,
        "string" => Kind::Text,
        "boolean" => Kind::Flag,
        "array" => {
            match (body.get("minItems").and_then(Value::as_u64), body.get("maxItems").and_then(Value::as_u64))
            {
                (Some(least), Some(most)) if least == most => Kind::Vector(most as usize),
                _ => Kind::List,
            }
        }
        _ => Kind::Group,
    }
}

fn entry_of(root: &Value, segs: &[Seg], stage: Stage, value: &Value) -> Result<Entry, String> {
    let site = site_at(root, segs)?;
    let looked = look(root, site);
    let kind = kind_of(looked.body);
    let keyword = |key: &str| looked.site.get(key).or_else(|| looked.body.get(key));
    let number = |key: &str| keyword(key).and_then(Value::as_f64);
    let minimum =
        number("exclusiveMinimum").map(|v| (v, true)).or_else(|| number("minimum").map(|v| (v, false)));
    let variant = match (&kind, value) {
        (Kind::Choice(_), Value::String(name)) => Some(name.clone()),
        (Kind::Choice(_), Value::Object(map)) => map.keys().next().cloned(),
        _ => None,
    };
    Ok(Entry {
        path: text(segs),
        name: match segs.last() {
            Some(Seg::Field(name)) => name.clone(),
            Some(Seg::Index(n)) => format!("[{n}]"),
            None => String::new(),
        },
        // A whole number in a float's place is still the float's field.
        minimum: if matches!(kind, Kind::Number | Kind::Integer) { minimum } else { None },
        maximum: if matches!(kind, Kind::Number | Kind::Integer) { number("maximum") } else { None },
        max_length: keyword("maxLength").and_then(Value::as_u64).map(|n| n as usize),
        unit: keyword("x-unit").and_then(Value::as_str).map(str::to_owned),
        doc: keyword("description").and_then(Value::as_str).unwrap_or_default().to_owned(),
        title: looked.site.get("title").and_then(Value::as_str).map(str::to_owned),
        optional: looked.optional,
        kind,
        stage,
        value: value.clone(),
        variant,
    })
}

/// The stage of the field at `segs`: the tag of the nearest field on the way down that has one.
/// A field under no tag invalidates everything, which is the safe answer.
fn stage_at(root: &Value, segs: &[Seg]) -> Stage {
    let mut stage = Stage::Forward;
    for n in 1..=segs.len() {
        if let Ok(site) = site_at(root, &segs[..n])
            && let Some(tagged) = site.get("x-stage").and_then(Value::as_str).and_then(Stage::parse)
        {
            stage = tagged;
        }
    }
    stage
}

/// The steps one level below `segs`, given its kind and its value now.
fn children(segs: &[Seg], kind: &Kind, value: &Value) -> Vec<Vec<Seg>> {
    let below = |seg: Seg| [segs, &[seg]].concat();
    match (kind, value) {
        // In the order the fields are declared: the workspace builds `serde_json` with
        // `preserve_order`, so a value's keys are in the order the struct wrote them.
        (Kind::Group, Value::Object(map)) => map.keys().map(|name| below(Seg::Field(name.clone()))).collect(),
        (Kind::List, Value::Array(items)) => (0..items.len()).map(|n| below(Seg::Index(n))).collect(),
        // A variant with data: the data is one group, under the variant's name.
        (Kind::Choice(_), Value::Object(map)) => {
            map.keys().next().map(|name| vec![below(Seg::Field(name.clone()))]).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// The field at `path` of the scene whose value is `whole`, described.
fn entry_in(root: &Value, whole: &Value, path: &str) -> Result<Entry, String> {
    let segs = parse(path)?;
    let value = get(whole, path)?;
    entry_of(root, &segs, stage_at(root, &segs), value)
}

/// What is directly inside the field at `path` of the scene whose value is `whole`.
fn ls_in(root: &Value, whole: &Value, path: &str) -> Result<Vec<Entry>, String> {
    let segs = parse(path)?;
    let value = get(whole, path)?;
    let own = entry_of(root, &segs, stage_at(root, &segs), value)?;
    let mut below = children(&segs, &own.kind, value);
    // A choice with data lists the data's own fields (under the variant's name in their paths),
    // not the one group that holds them, which would say only "(group)" (S3's review).
    if let (Kind::Choice(_), [variant]) = (&own.kind, below.as_slice()) {
        let data = get(whole, &text(variant))?;
        let inner = entry_of(root, variant, stage_at(root, variant), data)?;
        if inner.kind == Kind::Group {
            below = children(variant, &inner.kind, data);
        }
    }
    if below.is_empty() {
        return Ok(vec![own]);
    }
    let mut out = Vec::with_capacity(below.len());
    for child in below {
        let value = get(whole, &text(&child))?;
        out.push(entry_of(root, &child, stage_at(root, &child), value)?);
    }
    Ok(out)
}

/// The field at `path` of `spec`, described.
///
/// # Errors
/// When there is no such field, or it lies inside a part the scene does not have.
pub fn entry(spec: &SceneSpec, path: &str) -> Result<Entry, String> {
    entry_in(SceneSpec::schema_ref(), &spec.to_value(), path)
}

/// What is directly inside the field at `path` of `spec`: a struct's fields in the order they are
/// declared, a list's items, the data of an enum's variant. For a field that holds one value, the
/// field itself.
///
/// # Errors
/// As [`entry`].
pub fn ls(spec: &SceneSpec, path: &str) -> Result<Vec<Entry>, String> {
    ls_in(SceneSpec::schema_ref(), &spec.to_value(), path)
}

/// Every field of `spec` under `path` that holds one value (a number, a vector, text, a flag, a
/// plain choice), depth first, in `ls` order: what the property editors show for a selection.
/// At most `limit` of them: a list of a thousand bodies is not a panel. The scene is turned
/// into a value once, however many fields there are.
///
/// # Errors
/// As [`entry`].
pub fn leaves(spec: &SceneSpec, path: &str, limit: usize) -> Result<Vec<Entry>, String> {
    let (root, whole) = (SceneSpec::schema_ref(), spec.to_value());
    let mut out = Vec::new();
    let mut stack = vec![path.to_owned()];
    while let Some(at) = stack.pop() {
        if out.len() >= limit {
            break;
        }
        let own = entry_in(root, &whole, &at)?;
        let plain_choice = matches!(own.kind, Kind::Choice(_)) && !own.value.is_object();
        if own.kind.is_leaf() || plain_choice || own.value.is_null() {
            out.push(own);
            continue;
        }
        // A choice with data shows itself (to switch the variant) and then its data.
        if matches!(own.kind, Kind::Choice(_)) {
            out.push(own);
        }
        let inside = ls_in(root, &whole, &at)?;
        stack.extend(inside.into_iter().rev().map(|child| child.path));
    }
    Ok(out)
}

/// The earliest stage that must be done again to go from scene `a` to scene `b`: the latest
/// stage among the fields that differ. `Nothing` when only names and descriptions changed (or
/// nothing did).
#[must_use]
pub fn diff(a: &SceneSpec, b: &SceneSpec) -> Stage {
    fn walk(root: &Value, segs: &mut Vec<Seg>, a: &Value, b: &Value, worst: &mut Stage) {
        if a == b {
            return;
        }
        match (a, b) {
            (Value::Object(x), Value::Object(y)) if x.len() == y.len() && x.keys().eq(y.keys()) => {
                for (key, left) in x {
                    segs.push(Seg::Field(key.clone()));
                    walk(root, segs, left, &y[key], worst);
                    segs.pop();
                }
            }
            (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
                for (n, (left, right)) in x.iter().zip(y).enumerate() {
                    segs.push(Seg::Index(n));
                    walk(root, segs, left, right, worst);
                    segs.pop();
                }
            }
            _ => *worst = (*worst).max(stage_at(root, segs)),
        }
    }
    let mut worst = Stage::Nothing;
    walk(SceneSpec::schema_ref(), &mut Vec::new(), &a.to_value(), &b.to_value(), &mut worst);
    worst
}

/// A value that fits `schema`, for adding a part that is not there: numbers at their default, or
/// the least value allowed (1 where the range starts above 0), text empty, lists empty, the first
/// variant of a choice. It has the right shape; whether it makes a valid scene (a label no other
/// body has, say) is for [`SceneSpec::validate`] to say when it is set.
#[must_use]
pub fn template(path: &str) -> Option<Value> {
    fn make(root: &Value, site: &Value) -> Value {
        if let Some(default) = site.get("default").filter(|d| !d.is_null()) {
            return default.clone();
        }
        let body = look(root, site).body;
        let number = |key: &str| site.get(key).or_else(|| body.get(key)).and_then(Value::as_f64);
        match kind_of(body) {
            Kind::Number | Kind::Integer => {
                let most = number("maximum").unwrap_or(f64::INFINITY);
                let value = match (number("exclusiveMinimum"), number("minimum")) {
                    (Some(above), _) => (above + 1.0).min(most),
                    (None, Some(least)) => least,
                    (None, None) => 0.0,
                };
                if kind_of(body) == Kind::Integer { Value::from(value as i64) } else { Value::from(value) }
            }
            Kind::Text => Value::String(String::new()),
            Kind::Flag => Value::Bool(false),
            Kind::Vector(n) => Value::Array(vec![Value::from(0.0); n]),
            Kind::List => Value::Array(Vec::new()),
            Kind::Choice(_) => match variants(body).into_iter().next() {
                Some((name, None)) => Value::String(name),
                Some((name, Some(data))) => serde_json::json!({ name: make(root, data) }),
                None => Value::Null,
            },
            Kind::Group => {
                let required: Vec<&str> = body
                    .get("required")
                    .and_then(Value::as_array)
                    .map(|names| names.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                let mut out = serde_json::Map::new();
                if let Some(properties) = body.get("properties").and_then(Value::as_object) {
                    for (name, field) in properties {
                        if required.contains(&name.as_str()) {
                            out.insert(name.clone(), make(root, field));
                        }
                    }
                }
                Value::Object(out)
            }
        }
    }
    let root = SceneSpec::schema_ref();
    let segs = parse(path).ok()?;
    let site = site_at(root, &segs).ok()?;
    // For a list, the template is of one item.
    let looked = look(root, site);
    match (kind_of(looked.body), looked.body.get("items")) {
        (Kind::List, Some(items)) => Some(make(root, items)),
        _ => Some(make(root, &Value::Object(strip_optional(site)))),
    }
}

/// A field's site without what makes it optional or defaulted, so that a template of it is the
/// thing itself and not `null`.
fn strip_optional(site: &Value) -> serde_json::Map<String, Value> {
    let mut map = site.as_object().cloned().unwrap_or_default();
    map.remove("default");
    if let Some(Value::Array(any)) = map.remove("anyOf")
        && let Some(inner) = any.into_iter().find(|s| s.get("type").and_then(Value::as_str) != Some("null"))
        && let Value::Object(inner) = inner
    {
        map.extend(inner);
    }
    if let Some(Value::Array(types)) = map.get("type").cloned() {
        let kept: Vec<Value> = types.into_iter().filter(|t| t != "null").collect();
        if let [one] = kept.as_slice() {
            map.insert("type".into(), one.clone());
        }
    }
    map
}

/// The paths of number and vector fields in the spec's schema that state no unit. Empty when
/// every one does: `1` is a unit (a pure number), silence is not.
#[must_use]
pub fn fields_without_a_unit(root: &Value) -> Vec<String> {
    fn walk(root: &Value, site: &Value, at: &str, out: &mut Vec<String>, depth: usize) {
        if depth > 12 {
            return;
        }
        let body = look(root, site).body;
        let unit = site.get("x-unit").or_else(|| body.get("x-unit"));
        match kind_of(body) {
            Kind::Number | Kind::Vector(_) if unit.is_none() => out.push(at.to_owned()),
            Kind::Group => {
                if let Some(properties) = body.get("properties").and_then(Value::as_object) {
                    for (name, field) in properties {
                        walk(root, field, &join(at, name), out, depth + 1);
                    }
                }
            }
            Kind::List => {
                if let Some(items) = body.get("items") {
                    walk(root, items, &format!("{at}[]"), out, depth + 1);
                }
            }
            Kind::Choice(_) => {
                for (name, data) in variants(body) {
                    if let Some(data) = data {
                        walk(root, data, &join(at, &name), out, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(root, root, "", &mut out, 0);
    out
}

impl SceneSpec {
    /// The spec as a JSON value: what paths walk.
    #[must_use]
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("a scene spec is JSON")
    }

    /// The scene with the value at `path` replaced, if that is a valid scene; and the value that
    /// was there.
    ///
    /// # Errors
    /// When the path leads nowhere, the value is of the wrong type, or the scene that results is
    /// refused by [`Self::validate`]; the message names the field.
    pub fn edited(&self, path: &str, value: &Value) -> Result<(Self, Value), String> {
        let mut whole = self.to_value();
        let before = set(&mut whole, path, value.clone())?;
        let spec = Self::from_edited(whole).map_err(|why| match why {
            Refused::Shape(e) => format!("`{path}` cannot be set to {value}: {e}"),
            Refused::Scene(e) => e,
        })?;
        Ok((spec, before))
    }

    /// The scene with `value` added to the list at `list` (before item `index`, or at the end),
    /// and the index it has.
    ///
    /// # Errors
    /// As [`Self::edited`].
    pub fn inserted(&self, list: &str, index: Option<usize>, value: &Value) -> Result<(Self, usize), String> {
        let mut whole = self.to_value();
        let at = insert(&mut whole, list, index, value.clone())?;
        let spec = Self::from_edited(whole).map_err(|why| match why {
            Refused::Shape(e) => format!("{value} cannot be added to `{list}`: {e}"),
            Refused::Scene(e) => e,
        })?;
        Ok((spec, at))
    }

    /// The scene with the item at `path` taken out of its list, and the item.
    ///
    /// # Errors
    /// As [`Self::edited`].
    pub fn removed(&self, path: &str) -> Result<(Self, Value), String> {
        let mut whole = self.to_value();
        let item = remove(&mut whole, path)?;
        let spec = Self::from_edited(whole).map_err(|why| match why {
            Refused::Shape(e) | Refused::Scene(e) => e,
        })?;
        Ok((spec, item))
    }

    fn from_edited(whole: Value) -> Result<Self, Refused> {
        let spec: Self = serde_json::from_value(whole).map_err(|e| Refused::Shape(e.to_string()))?;
        spec.validate().map_err(|e| Refused::Scene(e.0))?;
        Ok(spec)
    }
}

enum Refused {
    /// The value does not have the field's type.
    Shape(String),
    /// The scene it would make is not a valid one.
    Scene(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FloorKind, FloorSpec, dev};
    use serde_json::json;

    /// A scene with every kind of motion, a slab, a room and a body that comes and goes: every
    /// variant and every optional part present.
    fn everything() -> SceneSpec {
        let mut spec = dev::oscillator_zoo();
        // A world with both kinds of piece, each of the first of its two materials.
        let mut world = dev::world_box().world.unwrap();
        world.materials.push(dev::stress_1e10().world.unwrap().materials[0].clone());
        world.pieces.extend(dev::stress_1e10().world.unwrap().pieces.pop());
        world.pieces[1].material = 0;
        spec.world = Some(world);
        spec.bodies.extend(dev::pass().bodies);
        spec.bodies.extend(dev::flash().bodies);
        spec.bodies[5].label = "flash".into();
        // Bodies made of voxels: a member of George's scaffold, and a polyhedron.
        let rod = crate::ShapeSpec {
            kind: crate::ShapeKind::Cylinder { radius: 0.0338, length: 1.0, axis: crate::Axis::X },
            pitch: 0.01,
            supersample: 4,
        };
        let modes = vec![crate::ModeSpec { frequency: 2.3, scale: 1.0, phase: 0.0 }];
        spec.bodies.push(crate::BodySpec::shaped(
            "member",
            28.2,
            rod,
            crate::Motion::Modes { centre: [0.0, 0.5, 3.0], amplitude: [3e-4, 0.0, 0.0], modes },
        ));
        let hull = crate::ShapeSpec {
            kind: crate::ShapeKind::Polyhedron {
                vertices: [[0.0; 3], [0.3, 0.0, 0.0], [0.0, 0.3, 0.0], [0.0, 0.0, 0.3]]
                    .map(crate::shape::Vertex)
                    .to_vec(),
            },
            pitch: 0.05,
            supersample: 2,
        };
        spec.bodies.push(crate::BodySpec::shaped(
            "hull",
            4.0,
            hull,
            crate::Motion::Static { position: [3.0, 3.0, 1.0] },
        ));
        spec.room = SceneSpec::room().room;
        spec.floor =
            Some(FloorSpec { height: -1.0, kind: FloorKind::Slab { thickness: 0.5 }, density: 2400.0 });
        assert_eq!(spec.validate(), Ok(()));
        spec
    }

    #[test]
    fn the_walked_scene_has_every_kind_of_motion() {
        // The tests that walk every path walk `everything()`: a kind of motion that is not in
        // it is a kind whose fields no test sets, lists or draws an editor for. (Found by
        // following the motion recipe with a made-up kind: nothing failed for its absence.)
        let spec = everything();
        for kind in crate::Motion::KINDS {
            let has = spec.bodies.iter().any(|body| body.motion.kind().eq_ignore_ascii_case(kind));
            assert!(has, "no body of `everything()` moves as `{kind}`: add one");
        }
        // Control: the comparison can fail (a kind that is not one).
        assert!(!spec.bodies.iter().any(|body| body.motion.kind() == "bounce"));
    }

    /// Every path of a scene, groups and leaves alike, by walking `ls` from the top.
    fn all_paths(spec: &SceneSpec) -> Vec<Entry> {
        let mut out = Vec::new();
        let mut stack = vec![String::new()];
        while let Some(at) = stack.pop() {
            for child in ls(spec, &at).unwrap() {
                if child.path != at {
                    stack.push(child.path.clone());
                    out.push(child);
                }
            }
        }
        out
    }

    #[test]
    fn paths_parse_and_print() {
        let segs = parse("array.towers[2].position[0]").unwrap();
        assert_eq!(segs.len(), 5);
        assert_eq!(segs[2], Seg::Index(2));
        assert_eq!(text(&segs), "array.towers[2].position[0]");
        assert_eq!(parse("").unwrap(), []);
        assert_eq!(
            parse("  bodies[0][1] ").unwrap(),
            [Seg::Field("bodies".into()), Seg::Index(0), Seg::Index(1)]
        );
        for (bad, why) in [
            ("array..towers", "single dots"),
            ("towers[2", "needs a `]`"),
            ("towers[x]", "not an index"),
            ("towers[1]x", "not part of a path"),
            (".name", "single dots"),
        ] {
            assert!(parse(bad).unwrap_err().contains(why), "{bad}: {}", parse(bad).unwrap_err());
        }
        assert_eq!(join("", "name"), "name");
        assert_eq!(join("bodies[0]", "mass"), "bodies[0].mass");
    }

    #[test]
    fn every_path_gets_and_sets() {
        let spec = everything();
        let whole = spec.to_value();
        let paths = all_paths(&spec);
        assert!(paths.len() > 100, "the walk found {} paths", paths.len());
        for entry in &paths {
            // What `ls` says is there is what `get` gives, and setting it back changes nothing.
            let value = get(&whole, &entry.path).unwrap();
            assert_eq!(value, &entry.value, "{}", entry.path);
            let (same, before) =
                spec.edited(&entry.path, value).unwrap_or_else(|e| panic!("{}: {e}", entry.path));
            assert_eq!(same, spec, "{}", entry.path);
            assert_eq!(&before, value);
            // A value of the wrong type is a refusal that names the path, never a panic.
            let wrong = if entry.kind == Kind::Text { json!(12.5) } else { json!("a word") };
            let plain_choice = matches!(entry.kind, Kind::Choice(_));
            if !plain_choice {
                let why = spec.edited(&entry.path, &wrong).expect_err(&entry.path);
                assert!(why.contains("cannot be set") || why.contains(&entry.path), "{}: {why}", entry.path);
            }
            // Every number with a range refuses a value outside it, naming itself.
            if entry.kind == Kind::Number
                && let Some(most) = entry.maximum
            {
                let why = spec.edited(&entry.path, &json!(most * 4.0 + 1.0)).expect_err(&entry.path);
                assert!(why.contains(entry.path.as_str()), "{}: {why}", entry.path);
            }
        }
        // Control: a set of another value is a change, and of that field alone.
        let (changed, before) = spec.edited("bodies[1].mass", &json!(7.25)).unwrap();
        assert_eq!((changed.bodies[1].mass, before), (7.25, json!(spec.bodies[1].mass)));
        let mut expected = spec.clone();
        expected.bodies[1].mass = 7.25;
        assert_eq!(changed, expected);
        // Paths that lead nowhere say what is there.
        assert!(get(&whole, "array.tower").unwrap_err().contains("it has: instrument, towers"));
        assert!(get(&whole, "array.towers[9]").unwrap_err().contains("has 5 items"));
        assert!(get(&whole, "name[0]").unwrap_err().contains("not a list"));
        assert!(get(&whole, "name.first").unwrap_err().contains("single value"));
    }

    #[test]
    fn every_number_takes_a_changed_value_and_gives_it_back() {
        // S2's review, finding 20: the walk above sets each leaf to the value it already has,
        // and its scene has no jitter, gaps, window or alternate cycle. Here every number of a
        // scene that has them all is set to another value, read back, and undone.
        let mut spec = everything();
        spec.array.schedule.jitter = Some(crate::JitterSpec {
            model: crate::JitterModel::Walk,
            kind: crate::JitterKind::Positive,
            delta: 0.2,
        });
        spec.array.schedule.alternate = Some(2.5);
        spec.array.schedule.gaps = Some(crate::GapSpec { on: 4, period: 10 });
        spec.array.schedule.window = Some(crate::WindowSpec { start: 0.0, length: 300.0, period: 600.0 });
        spec.array.schedule.loss = 0.1;
        spec.array.towers[1].timing.offset = 0.25;
        assert_eq!(spec.validate(), Ok(()));
        // Every number: a field that is one, and each component of a vector by its own path.
        let mut numbers: Vec<Entry> = Vec::new();
        for entry in all_paths(&spec) {
            match (&entry.kind, &entry.value) {
                (Kind::Number | Kind::Integer, value) if value.is_number() => numbers.push(entry),
                (Kind::Vector(_), Value::Array(components)) => {
                    for (i, component) in components.iter().enumerate() {
                        numbers.push(Entry {
                            path: format!("{}[{i}]", entry.path),
                            name: format!("[{i}]"),
                            kind: Kind::Number,
                            value: component.clone(),
                            ..entry.clone()
                        });
                    }
                }
                _ => {}
            }
        }
        for under in
            ["schedule.jitter.delta", "schedule.alternate", "schedule.gaps.on", "schedule.window.length"]
        {
            assert!(numbers.iter().any(|entry| entry.path.ends_with(under)), "the walk has no {under}");
        }
        assert!(numbers.len() > 100, "{} numbers", numbers.len());
        let (mut taken, mut refused) = (0, Vec::new());
        for entry in &numbers {
            let old = entry.value.as_f64().unwrap();
            // Another value of the same kind, close by: one more for a count, a part in a
            // thousand for a measure (a thousandth where it is zero).
            let new = match entry.kind {
                Kind::Integer => json!(entry.value.as_u64().unwrap() + 1),
                _ if old == 0.0 => json!(1.0e-3),
                _ => json!(old * 1.001),
            };
            match spec.edited(&entry.path, &new) {
                Ok((changed, before)) => {
                    assert_eq!(get(&changed.to_value(), &entry.path).unwrap(), &new, "{}", entry.path);
                    assert_eq!(before, entry.value, "{}: what was there before", entry.path);
                    assert_ne!(changed, spec, "{}: nothing changed", entry.path);
                    // That field alone: put back, the scene is the scene, to the bit.
                    let (undone, _) = changed.edited(&entry.path, &before).unwrap();
                    assert_eq!(undone, spec, "{}: set and put back is not the same scene", entry.path);
                    assert_eq!(
                        serde_json::to_string(&undone).unwrap(),
                        serde_json::to_string(&spec).unwrap(),
                        "{}",
                        entry.path
                    );
                    taken += 1;
                }
                // A neighbouring value that another field forbids is refused, saying where.
                Err(why) => {
                    assert!(why.contains('.') || why.contains(&entry.name), "{}: {why}", entry.path);
                    refused.push(entry.path.clone());
                }
            }
        }
        assert!(refused.len() <= 4, "refused: {refused:?}");
        assert!(taken >= numbers.len() - 4);
        // Control: the comparison sees a field that is not put back.
        let (changed, _) = spec.edited("array.schedule.gaps.on", &json!(5)).unwrap();
        assert_ne!(changed, spec);
    }

    #[test]
    fn ls_lists_children_with_kind_unit_range_and_doc() {
        let spec = everything();
        let top: Vec<String> = ls(&spec, "").unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(
            top,
            ["version", "name", "description", "duration", "array", "room", "floor", "bodies", "world"]
        );
        let instrument = ls(&spec, "array.instrument").unwrap();
        let cycle = instrument.iter().find(|e| e.name == "cycle").unwrap();
        assert_eq!(cycle.kind, Kind::Number);
        assert_eq!(cycle.unit.as_deref(), Some("s"));
        assert_eq!((cycle.minimum, cycle.maximum), (Some((0.0, true)), Some(1.0e4)));
        assert_eq!((cycle.value.clone(), cycle.stage), (json!(2.0), Stage::Forward));
        assert!(cycle.doc.starts_with("Time between shots of one tower"));
        assert!(
            cycle.line().starts_with("cycle  =  2.0 s  (number · above 0 to 10000)  Time between shots"),
            "{}",
            cycle.line()
        );
        let half = instrument.iter().find(|e| e.name == "half_sequence").unwrap();
        assert_eq!(half.minimum, Some((1.0e-3, false)), "an inclusive bound is said as one");
        // What an editor calls a field: the schema's title, else its name in words (S18's review,
        // U1); control: a field with no title is its name, capitalised, its underscores spaces.
        assert_eq!(half.label(), "Half sequence T");
        let height = instrument.iter().find(|e| e.name == "tower_height").unwrap();
        assert_eq!((height.title.as_deref(), height.label().as_str()), (None, "Tower height"));
        let separation = instrument.iter().find(|e| e.name == "ifo_separation").unwrap();
        assert_eq!(separation.label(), "Interferometer separation Δr");
        assert_eq!(words("LowerMinusUpper"), "Lower minus upper");
        assert_eq!(words("HalfSpace"), "Half space");
        assert_eq!(words("launch_floor"), "Launch floor");
        assert_eq!(words("x"), "X");
        // A list's children are its items; a vector is one value; a flag and a choice say so.
        let bodies = ls(&spec, "bodies").unwrap();
        assert_eq!(bodies.len(), spec.bodies.len());
        assert_eq!((bodies[2].name.as_str(), bodies[2].path.as_str()), ("[2]", "bodies[2]"));
        let position = entry(&spec, "array.towers[1].position").unwrap();
        assert_eq!((position.kind.clone(), position.unit.as_deref()), (Kind::Vector(3), Some("m")));
        assert_eq!(ls(&spec, "array.towers[1].position").unwrap(), [position], "a leaf lists itself");
        assert_eq!(entry(&spec, "array.instrument.launch_floor").unwrap().kind, Kind::Flag);
        let sign = entry(&spec, "array.instrument.sign").unwrap();
        assert_eq!(sign.kind, Kind::Choice(vec!["LowerMinusUpper".into(), "UpperMinusLower".into()]));
        assert_eq!(sign.variant.as_deref(), Some("LowerMinusUpper"));
        // An enum with data: the choice, then its data under the variant's name.
        let motion = entry(&spec, "bodies[0].motion").unwrap();
        assert_eq!(motion.variant.as_deref(), Some("Linear"));
        let Kind::Choice(kinds) = &motion.kind else { panic!("a choice") };
        assert_eq!(kinds, &["Static", "Pass", "Linear", "Circular", "Pendulum", "Sphere", "Modes"]);
        let inside: Vec<String> =
            ls(&spec, "bodies[0].motion.Linear").unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(inside, ["centre", "amplitude", "frequency", "phase"]);
        // Listing the choice itself lists its variant's fields, not one bare group.
        let listed = ls(&spec, "bodies[0].motion").unwrap();
        let names: Vec<&str> = listed.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["centre", "amplitude", "frequency", "phase"]);
        assert_eq!(listed[0].path, "bodies[0].motion.Linear.centre");
        assert_eq!(entry(&spec, "bodies[0].motion.Linear.frequency").unwrap().unit.as_deref(), Some("Hz"));
        // The mixed enum: a plain variant and one with data.
        assert_eq!(entry(&spec, "floor.kind").unwrap().variant.as_deref(), Some("Slab"));
        assert_eq!(entry(&spec, "floor.kind.Slab.thickness").unwrap().unit.as_deref(), Some("m"));
        // Optional things say so, present or not; text has a length.
        assert!(
            entry(&spec, "room").unwrap().optional && entry(&spec, "bodies[0].appears").unwrap().optional
        );
        assert!(!entry(&spec, "duration").unwrap().optional);
        assert_eq!(entry(&spec, "name").unwrap().max_length, Some(crate::spec::limits::LABEL));
        // The leaves under a body are what its editor shows, in order.
        let shown: Vec<String> =
            leaves(&spec, "bodies[4]", 100).unwrap().into_iter().map(|e| e.path).collect();
        assert_eq!(
            shown,
            [
                "bodies[4].label",
                "bodies[4].mass",
                "bodies[4].radius",
                "bodies[4].shape",
                "bodies[4].motion",
                "bodies[4].motion.Pass.from",
                "bodies[4].motion.Pass.to",
                "bodies[4].motion.Pass.start",
                "bodies[4].motion.Pass.speed",
                "bodies[4].appears",
                "bodies[4].vanishes",
            ]
        );
        // Control: the limit is one, and the schema of a path stands on its own.
        assert_eq!(leaves(&spec, "", 7).unwrap().len(), 7);
        let schema = schema_at("array.instrument.cycle").unwrap();
        assert_eq!(
            (&schema["type"], &schema["x-unit"], &schema["maximum"]),
            (&json!("number"), &json!("s"), &json!(1.0e4))
        );
        assert_eq!(schema_at("bodies[0].motion.Pendulum.length").unwrap()["x-unit"], "m");
        assert_eq!(schema_at("room").unwrap()["x-optional"], true);
        assert!(schema_at("room.size").is_ok(), "whether or not a scene has a room");
        assert!(schema_at("room.colour").unwrap_err().contains("it has: origin, size"));
    }

    #[test]
    fn lists_take_and_lose_items() {
        let spec = dev::superposition();
        let body =
            json!({"label": "third", "mass": 2.0, "motion": {"Static": {"position": [1.0, 2.0, 3.0]}}});
        let (more, at) = spec.inserted("bodies", None, &body).unwrap();
        assert_eq!((at, more.bodies.len(), more.bodies[2].label.as_str()), (2, 3, "third"));
        let (first, at) = spec.inserted("bodies", Some(0), &body).unwrap();
        assert_eq!((at, first.bodies[0].label.as_str(), &first.bodies[1]), (0, "third", &spec.bodies[0]));
        // Taking it out again gives the scene back, and the item.
        let (back, item) = first.removed("bodies[0]").unwrap();
        assert_eq!(back, spec);
        assert_eq!(item["label"], "third");
        // Refusals: past the end, not a list, not an item, a scene that cannot be, a wrong shape.
        assert!(spec.inserted("bodies", Some(4), &body).unwrap_err().contains("at [0] to [2]"));
        assert!(spec.inserted("name", None, &body).unwrap_err().contains("not a list"));
        assert!(spec.removed("bodies").unwrap_err().contains("ends in an index"));
        assert!(spec.removed("bodies[5]").unwrap_err().contains("has 2 items"));
        let twin = json!({"label": spec.bodies[0].label, "mass": 2.0, "motion": {"Static": {"position": [0.0, 0.0, 9.0]}}});
        assert!(spec.inserted("bodies", None, &twin).unwrap_err().contains("is used twice"));
        assert!(spec.inserted("bodies", None, &json!(3.0)).unwrap_err().contains("cannot be added"));
        let mut one = spec.clone();
        one.array.towers.truncate(1);
        // Since S3 a scene may have no towers: the last one can be taken out, and put back.
        let (none, tower) = one.removed("array.towers[0]").unwrap();
        assert!(none.array.towers.is_empty());
        assert_eq!(none.inserted("array.towers", None, &tower).unwrap().0, one);
        // A component of a vector is not an item that can be taken out: the scene would not parse.
        assert!(spec.removed("array.towers[0].position[1]").is_err());
        // Control: a template of a list's item has the item's shape, and only needs a label.
        let mut made = template("bodies").unwrap();
        assert_eq!(made["motion"], json!({"Static": {"position": [0.0, 0.0, 0.0]}}));
        made["label"] = json!("new");
        assert!(spec.inserted("bodies", None, &made).is_ok(), "{made}");
    }

    #[test]
    fn an_absent_option_says_how_to_add_it() {
        let spec = dev::void();
        let whole = spec.to_value();
        assert_eq!(get(&whole, "room"), Ok(&Value::Null));
        let why = get(&whole, "room.size").unwrap_err();
        assert!(why.contains("`room` is absent") && why.contains("`set room <value>` adds one"), "{why}");
        assert!(ls(&spec, "room.size").unwrap_err().contains("is absent"));
        // `ls` of the absent thing shows it as none; its template is a room, and setting it adds one.
        assert_eq!(ls(&spec, "room").unwrap()[0].value, Value::Null);
        let room = template("room").unwrap();
        assert_eq!(room, json!({"origin": [0.0, 0.0, 0.0], "size": [0.0, 0.0, 0.0]}));
        let (with, _) = spec.edited("room", &json!({"origin": [0, 0, 0], "size": [4, 4, 3]})).unwrap();
        assert_eq!(with.room.as_ref().unwrap().size, [4.0, 4.0, 3.0]);
        assert!(get(&with.to_value(), "room.size[2]").is_ok());
        // And null takes it away again.
        let (without, before) = with.edited("room", &Value::Null).unwrap();
        assert_eq!((without, before["size"][0].clone()), (spec.clone(), json!(4.0)));
        // Control: something that is not optional cannot be set to nothing.
        assert!(spec.edited("array", &Value::Null).unwrap_err().contains("cannot be set"));
        // A variant's fields vanish with the variant: the old path says what is there now.
        let zoo = dev::oscillator_zoo();
        let (still, _) = zoo.edited("bodies[0].motion", &json!({"Static": {"position": [1, 0, 1]}})).unwrap();
        let why = get(&still.to_value(), "bodies[0].motion.Linear.frequency").unwrap_err();
        assert!(why.contains("has no field `Linear`; it has: Static"), "{why}");
    }

    #[test]
    fn every_leaf_has_a_unit_or_is_dimensionless_and_a_stage() {
        let root = SceneSpec::schema();
        assert_eq!(fields_without_a_unit(&root), Vec::<String>::new());
        // Every path of a scene with everything in it has a stage, and the stages are the stated ones.
        let spec = everything();
        for entry in all_paths(&spec) {
            let expected = match entry.path.split(['.', '[']).next().unwrap() {
                "name" | "description" | "version" => Stage::Nothing,
                "room" => Stage::View,
                _ => Stage::Forward,
            };
            assert_eq!(entry.stage, expected, "{}", entry.path);
            if matches!(entry.kind, Kind::Number | Kind::Vector(_)) {
                assert!(entry.unit.is_some(), "{} has no unit", entry.path);
            }
        }
        // Control: take one tag away and the field is named.
        let mut untagged = root.clone();
        untagged["$defs"]["InstrumentSpec"]["properties"]["cycle"].as_object_mut().unwrap().remove("x-unit");
        assert_eq!(fields_without_a_unit(&untagged), ["array.instrument.cycle"]);
        let mut untagged = root;
        untagged["$defs"]["RoomSpec"]["properties"]["size"].as_object_mut().unwrap().remove("x-unit");
        assert_eq!(fields_without_a_unit(&untagged), ["room.size"]);
    }

    #[test]
    fn diff_names_the_earliest_stage() {
        let spec = everything();
        let edit = |path: &str, value: Value| spec.edited(path, &value).unwrap().0;
        assert_eq!(diff(&spec, &spec), Stage::Nothing);
        assert_eq!(diff(&spec, &edit("name", json!("another"))), Stage::Nothing);
        assert_eq!(diff(&spec, &edit("description", json!("words"))), Stage::Nothing);
        assert_eq!(diff(&spec, &edit("room.size[0]", json!(12.0))), Stage::View);
        assert_eq!(diff(&spec, &edit("room", Value::Null)), Stage::View);
        assert_eq!(diff(&spec, &edit("bodies[2].mass", json!(3.0))), Stage::Forward);
        assert_eq!(diff(&spec, &edit("array.towers[1].position[1]", json!(0.5))), Stage::Forward);
        assert_eq!(diff(&spec, &edit("array.instrument.cycle", json!(2.5))), Stage::Forward);
        assert_eq!(diff(&spec, &edit("floor.density", json!(1000.0))), Stage::Forward);
        assert_eq!(diff(&spec, &edit("duration", json!(40.0))), Stage::Forward);
        assert_eq!(diff(&spec, &spec.removed("bodies[1]").unwrap().0), Stage::Forward);
        // Several changes: the latest stage among them.
        let both = edit("name", json!("x")).edited("room.origin[0]", &json!(-6.0)).unwrap().0;
        assert_eq!(diff(&spec, &both), Stage::View);
        let all = both.edited("bodies[0].mass", &json!(9.0)).unwrap().0;
        assert_eq!(diff(&spec, &all), Stage::Forward);
        // Control: the order of the stages is the order they are run in.
        assert!(Stage::Nothing < Stage::View && Stage::View < Stage::Forward);
    }
}
