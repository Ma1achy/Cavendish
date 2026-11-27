//! Folders of the scene tree (S20): a way to group bodies and towers under a name, saved with
//! the scene. A folder is a row of [`SceneSpec::folders`]; a body or a tower says which folder it
//! is in by that folder's label ([`crate::BodySpec::folder`], [`crate::TowerSpec::folder`]).
//!
//! Folders are for people: they have **no effect on the physics**. Nothing in [`crate::read`]
//! looks at them, and a scene reads the same bits with and without them (a test in `spec` holds
//! it). None of the three fields is written when it is empty, so a scene that uses no folder
//! serialises to the bytes it did before folders existed.
//!
//! Only the Array's towers and the Bodies are grouped: the surroundings are three single items
//! (the room, the floor, the world) and take no folder.
//!
//! The edits ([`FolderEdit`]) are functions from a scene to a scene ([`SceneSpec::folder_edit`]),
//! so they are tested without an engine; the engine makes each one step of undo.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{SceneSpec, SpecError};

/// The most folders a scene holds.
pub const MAX_FOLDERS: usize = 256;

/// The deepest a folder is nested: a folder directly in its group is at depth 1.
pub const MAX_DEPTH: usize = 8;

/// The words a folder may not be called, in any case: the console reads them as a group or as
/// "no folder" (`folder move body 1 none`, `folder new x in bodies`).
pub const RESERVED: [&str; 4] = ["none", "array", "bodies", "surroundings"];

/// Which built-in group of the scene tree a folder is in: what it may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum FolderGroup {
    /// The array: it holds towers.
    Array,
    /// The bodies: it holds bodies.
    Bodies,
}

impl FolderGroup {
    /// Both groups, in the tree's order.
    pub const ALL: [Self; 2] = [Self::Array, Self::Bodies];

    /// The group's name as the tree shows it and the console reads it (in any case).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Array => "Array",
            Self::Bodies => "Bodies",
        }
    }

    /// The group a word names, in any case.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.name().eq_ignore_ascii_case(word))
    }
}

/// A folder of the scene tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderSpec {
    /// Name shown in the scene tree; no two folders share one, and it is none of `none`,
    /// `array`, `bodies`, `surroundings`. At most 200 characters.
    #[schemars(length(min = 1, max = 200))]
    pub label: String,
    /// The group of the tree it is in: a folder of the Array holds towers, one of the Bodies
    /// holds bodies.
    pub group: FolderGroup,
    /// The label of the folder this one is inside, of the same group; absent (the default) for
    /// a folder directly in its group. Nested at most 8 deep.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// Something a folder holds, as `folder move` names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum FolderItem {
    /// A body, counted from 1 as on screen.
    Body(usize),
    /// A tower, counted from 1 as on screen.
    Tower(usize),
    /// A folder, by its label.
    Folder(String),
}

/// One edit of the scene's folders. Each is one step of undo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum FolderEdit {
    /// Make an empty folder.
    New {
        /// Its label.
        name: String,
        /// Where: a group (`array`, `bodies`, in any case) or the label of the folder it goes
        /// inside; the Bodies if absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        within: Option<String>,
    },
    /// Move bodies, towers or folders into a folder, or out of every folder (to their group).
    Move {
        /// What is moved. A folder moves with everything in it.
        items: Vec<FolderItem>,
        /// The folder they go into, by label; `None` for none: directly in their group.
        to: Option<String>,
    },
    /// Give a folder another label. What is in it stays in it.
    Rename {
        /// The label it has.
        from: String,
        /// The label it takes.
        to: String,
    },
    /// Take a folder away. Nothing in it is deleted: its bodies, towers and folders move up to
    /// the folder it was in (or to the group).
    Remove {
        /// Its label.
        name: String,
    },
    /// Copy a folder beside itself: a new folder with a copy of every folder and every body (or
    /// tower) under it, each copy half a metre east of its original (a tower: a metre) and
    /// labelled anew, added at the end of the scene's lists.
    Duplicate {
        /// The label of the folder copied.
        name: String,
    },
}

/// Which folder everything is in: the scene's folders and each body's and tower's folder, in the
/// scene's order. What an undo of a folder edit puts back (small whatever the bodies weigh).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Membership {
    /// The folders.
    pub folders: Vec<FolderSpec>,
    /// Each body's folder, in the bodies' order.
    pub bodies: Vec<Option<String>>,
    /// Each tower's folder, in the towers' order.
    pub towers: Vec<Option<String>>,
}

/// The first of `base`, `base 2`, `base 3`, … that `taken` does not hold.
fn unique(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_owned();
    }
    (2..).map(|n| format!("{base} {n}")).find(|l| !taken(l)).expect("a free label")
}

impl SceneSpec {
    /// The folder of this label.
    #[must_use]
    pub fn folder(&self, label: &str) -> Option<&FolderSpec> {
        self.folders.iter().find(|f| f.label == label)
    }

    /// The folders directly inside `parent` (`None`: directly in the group) of `group`, in the
    /// scene's order: the order the tree draws them in.
    pub fn folders_in<'a>(
        &'a self,
        group: FolderGroup,
        parent: Option<&'a str>,
    ) -> impl Iterator<Item = &'a FolderSpec> + 'a {
        self.folders.iter().filter(move |f| f.group == group && f.parent.as_deref() == parent)
    }

    /// Whether the folder `label` is `ancestor` or inside it, at any depth.
    #[must_use]
    pub fn folder_is_under(&self, label: &str, ancestor: &str) -> bool {
        let mut at = Some(label);
        // Bounded: a valid scene's chains end within `MAX_DEPTH`; an invalid one's may loop.
        for _ in 0..=self.folders.len() {
            match at {
                Some(l) if l == ancestor => return true,
                Some(l) => at = self.folder(l).and_then(|f| f.parent.as_deref()),
                None => return false,
            }
        }
        false
    }

    /// The bodies (from 0) under the folder `label` at any depth, in the bodies' order.
    #[must_use]
    pub fn bodies_under(&self, label: &str) -> Vec<usize> {
        (0..self.bodies.len())
            .filter(|&n| self.bodies[n].folder.as_deref().is_some_and(|f| self.folder_is_under(f, label)))
            .collect()
    }

    /// The towers (from 0) under the folder `label` at any depth, in the towers' order.
    #[must_use]
    pub fn towers_under(&self, label: &str) -> Vec<usize> {
        let towers = &self.array.towers;
        (0..towers.len())
            .filter(|&n| towers[n].folder.as_deref().is_some_and(|f| self.folder_is_under(f, label)))
            .collect()
    }

    /// Which folder everything is in.
    #[must_use]
    pub fn membership(&self) -> Membership {
        Membership {
            folders: self.folders.clone(),
            bodies: self.bodies.iter().map(|b| b.folder.clone()).collect(),
            towers: self.array.towers.iter().map(|t| t.folder.clone()).collect(),
        }
    }

    /// The scene with its folders and every body's and tower's folder as `membership` has them.
    ///
    /// # Errors
    /// When `membership` is of a scene with another number of bodies or towers, or the scene it
    /// makes is refused by [`Self::validate`].
    pub fn with_membership(&self, membership: &Membership) -> Result<Self, String> {
        if membership.bodies.len() != self.bodies.len() || membership.towers.len() != self.array.towers.len()
        {
            return Err(format!(
                "the folders are of a scene with {} bodies and {} towers; this one has {} and {}",
                membership.bodies.len(),
                membership.towers.len(),
                self.bodies.len(),
                self.array.towers.len()
            ));
        }
        let mut spec = self.clone();
        spec.folders.clone_from(&membership.folders);
        for (body, folder) in spec.bodies.iter_mut().zip(&membership.bodies) {
            body.folder.clone_from(folder);
        }
        for (tower, folder) in spec.array.towers.iter_mut().zip(&membership.towers) {
            tower.folder.clone_from(folder);
        }
        spec.validate().map_err(|e| e.0)?;
        Ok(spec)
    }

    /// The scene after a folder edit. Bodies and towers keep their places in their lists (a
    /// duplicate adds its copies at the ends), so every index and path stays good.
    ///
    /// # Errors
    /// Why the edit cannot be made, in words: a folder that is not there, a label taken, a body
    /// moved into a folder of the towers, a folder moved into itself, too many or too deep.
    pub fn folder_edit(&self, edit: &FolderEdit) -> Result<Self, String> {
        let mut spec = self.clone();
        let known = |spec: &Self, label: &str| -> Result<FolderSpec, String> {
            spec.folder(label).cloned().ok_or_else(|| {
                let names: Vec<&str> = spec.folders.iter().map(|f| f.label.as_str()).collect();
                if names.is_empty() {
                    format!("`{label}` is not a folder: the scene has none (`folder new <name>` makes one)")
                } else {
                    format!("`{label}` is not a folder; the folders are: {}", names.join(", "))
                }
            })
        };
        match edit {
            FolderEdit::New { name, within } => {
                let (group, parent) = match within.as_deref() {
                    None => (FolderGroup::Bodies, None),
                    Some(word) => match FolderGroup::parse(word) {
                        Some(group) => (group, None),
                        None if word.eq_ignore_ascii_case("surroundings") => {
                            return Err(
                                "the surroundings take no folders: the room, the floor and the world \
                                        are single items"
                                    .into(),
                            );
                        }
                        None => {
                            let parent = known(&spec, word)?;
                            (parent.group, Some(parent.label))
                        }
                    },
                };
                spec.folders.push(FolderSpec { label: name.trim().to_owned(), group, parent });
            }
            FolderEdit::Move { items, to } => {
                if items.is_empty() {
                    return Err("nothing to move: name a body, a tower or a folder".into());
                }
                let to = to.as_deref().map(|label| known(&spec, label)).transpose()?;
                let label = to.as_ref().map(|f| f.label.clone());
                for item in items {
                    match item {
                        FolderItem::Body(n) => {
                            let count = spec.bodies.len();
                            let body = n
                                .checked_sub(1)
                                .and_then(|n| spec.bodies.get_mut(n))
                                .ok_or_else(|| format!("there is no body {n}: the scene has {count}"))?;
                            body.folder.clone_from(&label);
                        }
                        FolderItem::Tower(n) => {
                            let count = spec.array.towers.len();
                            let tower = n
                                .checked_sub(1)
                                .and_then(|n| spec.array.towers.get_mut(n))
                                .ok_or_else(|| format!("there is no tower {n}: the scene has {count}"))?;
                            tower.folder.clone_from(&label);
                        }
                        FolderItem::Folder(moved) => {
                            known(&spec, moved)?;
                            if let Some(to) = &to
                                && spec.folder_is_under(&to.label, moved)
                            {
                                return Err(if to.label == *moved {
                                    format!("`{moved}` cannot be moved into itself")
                                } else {
                                    format!(
                                        "`{moved}` cannot be moved into `{}`, which is inside it",
                                        to.label
                                    )
                                });
                            }
                            let folder = spec.folders.iter_mut().find(|f| f.label == *moved).expect("known");
                            folder.parent.clone_from(&label);
                        }
                    }
                }
            }
            FolderEdit::Rename { from, to } => {
                known(&spec, from)?;
                let to = to.trim();
                if to != from && spec.folder(to).is_some() {
                    return Err(format!("there is a folder `{to}` already"));
                }
                let follow = |folder: &mut Option<String>| {
                    if folder.as_deref() == Some(from.as_str()) {
                        *folder = Some(to.to_owned());
                    }
                };
                for folder in &mut spec.folders {
                    if folder.label == *from {
                        to.clone_into(&mut folder.label);
                    }
                    follow(&mut folder.parent);
                }
                spec.bodies.iter_mut().for_each(|b| follow(&mut b.folder));
                spec.array.towers.iter_mut().for_each(|t| follow(&mut t.folder));
            }
            FolderEdit::Remove { name } => {
                let parent = known(&spec, name)?.parent;
                let up = |folder: &mut Option<String>| {
                    if folder.as_deref() == Some(name.as_str()) {
                        folder.clone_from(&parent);
                    }
                };
                spec.folders.retain(|f| f.label != *name);
                spec.folders.iter_mut().for_each(|f| up(&mut f.parent));
                spec.bodies.iter_mut().for_each(|b| up(&mut b.folder));
                spec.array.towers.iter_mut().for_each(|t| up(&mut t.folder));
            }
            FolderEdit::Duplicate { name } => {
                known(&spec, name)?;
                // Each folder under it (itself first: a parent is always met before its child,
                // by walking down from it) and the label of its copy.
                let mut copies: Vec<(String, String)> = Vec::new();
                let mut level = vec![name.clone()];
                while !level.is_empty() {
                    let mut next = Vec::new();
                    for original in level {
                        let folder = known(&spec, &original)?;
                        let label = unique(&folder.label, |l| spec.folder(l).is_some());
                        let parent = match copies.iter().find(|(o, _)| Some(o) == folder.parent.as_ref()) {
                            Some((_, copy)) => Some(copy.clone()),
                            None => folder.parent.clone(),
                        };
                        next.extend(
                            spec.folders_in(folder.group, Some(original.as_str())).map(|f| f.label.clone()),
                        );
                        spec.folders.push(FolderSpec { label: label.clone(), group: folder.group, parent });
                        copies.push((original, label));
                    }
                    level = next;
                }
                let copy_of = |folder: &Option<String>| {
                    copies.iter().find(|(o, _)| Some(o) == folder.as_ref()).map(|(_, c)| c.clone())
                };
                for n in 0..spec.bodies.len() {
                    if let Some(folder) = copy_of(&spec.bodies[n].folder) {
                        let body = &spec.bodies[n];
                        let copy = crate::BodySpec {
                            label: unique(&body.label, |l| spec.bodies.iter().any(|b| b.label == l)),
                            motion: body.motion.shifted([0.5, 0.0, 0.0]),
                            folder: Some(folder),
                            ..body.clone()
                        };
                        spec.bodies.push(copy);
                    }
                }
                for n in 0..spec.array.towers.len() {
                    if let Some(folder) = copy_of(&spec.array.towers[n].folder) {
                        let tower = &spec.array.towers[n];
                        let [x, y, z] = tower.position;
                        let copy = crate::TowerSpec {
                            label: unique(&tower.label, |l| spec.array.towers.iter().any(|t| t.label == l)),
                            position: [x + 1.0, y, z],
                            folder: Some(folder),
                            ..tower.clone()
                        };
                        spec.array.towers.push(copy);
                    }
                }
            }
        }
        spec.validate().map_err(|e| e.0)?;
        Ok(spec)
    }
}

/// The checks of a scene's folders ([`SceneSpec::validate`] runs them): every refusal in words,
/// naming the field.
///
/// # Errors
/// The first check that fails.
pub(crate) fn validate(spec: &SceneSpec) -> Result<(), SpecError> {
    let fail = |why: String| Err(SpecError(why));
    if spec.folders.len() > MAX_FOLDERS {
        return fail(format!("folders: {} folders is more than {MAX_FOLDERS}", spec.folders.len()));
    }
    let mut seen = std::collections::HashMap::with_capacity(spec.folders.len());
    for (n, folder) in spec.folders.iter().enumerate() {
        let at = format!("folders[{n}].label");
        if folder.label.trim().is_empty() {
            return fail(format!("{at}: must not be empty"));
        }
        crate::spec::short(&at, &folder.label, crate::spec::limits::LABEL)?;
        if RESERVED.iter().any(|word| word.eq_ignore_ascii_case(&folder.label)) {
            return fail(format!(
                "{at}: `{}` is a word the console keeps (none, array, bodies, surroundings); call the \
                 folder something else",
                folder.label
            ));
        }
        if seen.insert(folder.label.as_str(), folder).is_some() {
            return fail(format!("{at}: `{}` is used twice", folder.label));
        }
    }
    for (n, folder) in spec.folders.iter().enumerate() {
        let at = format!("folders[{n}].parent");
        // Up from the folder to its group: the chain ends, within the depth allowed, and stays
        // in one group.
        let mut chain = vec![folder.label.as_str()];
        let mut above = folder.parent.as_deref();
        while let Some(label) = above {
            let Some(parent) = seen.get(label) else {
                return fail(format!("{at}: `{label}` is not a folder of this scene"));
            };
            if parent.group != folder.group {
                return fail(format!(
                    "{at}: `{label}` is a folder of the {}, and `{}` is one of the {}: a folder is nested in \
                     its own group",
                    parent.group.name(),
                    folder.label,
                    folder.group.name()
                ));
            }
            if chain.contains(&label) {
                chain.push(label);
                return fail(format!(
                    "{at}: a cycle: {} (a folder cannot be inside itself)",
                    chain.join(" in ")
                ));
            }
            chain.push(label);
            above = parent.parent.as_deref();
        }
        if chain.len() > MAX_DEPTH {
            return fail(format!(
                "{at}: `{}` is nested {} deep; folders nest at most {MAX_DEPTH} deep",
                folder.label,
                chain.len()
            ));
        }
    }
    let member = |at: String, folder: &Option<String>, group: FolderGroup, what: &str| match folder {
        None => Ok(()),
        Some(label) => match seen.get(label.as_str()) {
            None => fail(format!("{at}: `{label}` is not a folder of this scene")),
            Some(folder) if folder.group != group => fail(format!(
                "{at}: `{label}` is a folder of the {}; a {what} goes in a folder of the {}",
                folder.group.name(),
                group.name()
            )),
            Some(_) => Ok(()),
        },
    };
    for (n, body) in spec.bodies.iter().enumerate() {
        member(format!("bodies[{n}].folder"), &body.folder, FolderGroup::Bodies, "body")?;
    }
    for (n, tower) in spec.array.towers.iter().enumerate() {
        member(format!("array.towers[{n}].folder"), &tower.folder, FolderGroup::Array, "tower")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev;

    fn new(name: &str, within: Option<&str>) -> FolderEdit {
        FolderEdit::New { name: name.into(), within: within.map(str::to_owned) }
    }

    fn moved(items: &[FolderItem], to: Option<&str>) -> FolderEdit {
        FolderEdit::Move { items: items.to_vec(), to: to.map(str::to_owned) }
    }

    #[test]
    fn folder_edits_are_scenes_and_refusals_are_words() {
        let zoo = dev::oscillator_zoo();
        let bodies = zoo.bodies.len();
        assert!(bodies >= 3);
        let spec = zoo.folder_edit(&new("rig", None)).unwrap();
        let spec = spec.folder_edit(&new("inner", Some("rig"))).unwrap();
        let spec = spec.folder_edit(&new("north", Some("array"))).unwrap();
        let spec =
            spec.folder_edit(&moved(&[FolderItem::Body(1), FolderItem::Body(2)], Some("inner"))).unwrap();
        let spec = spec.folder_edit(&moved(&[FolderItem::Tower(2)], Some("north"))).unwrap();
        assert_eq!(spec.folder("inner").unwrap().parent.as_deref(), Some("rig"));
        assert_eq!(spec.bodies_under("rig"), [0, 1], "at any depth");
        assert_eq!(spec.towers_under("north"), [1]);
        // Rename: the members and the folders inside follow.
        let renamed =
            spec.folder_edit(&FolderEdit::Rename { from: "rig".into(), to: "frame".into() }).unwrap();
        assert_eq!(renamed.folder("inner").unwrap().parent.as_deref(), Some("frame"));
        assert_eq!(renamed.bodies_under("frame"), [0, 1]);
        // Remove: nothing is deleted; what was in it moves up to its parent.
        let removed = spec.folder_edit(&FolderEdit::Remove { name: "inner".into() }).unwrap();
        assert_eq!(removed.bodies.len(), bodies, "control: removing a folder keeps its bodies");
        assert_eq!(removed.bodies[0].folder.as_deref(), Some("rig"));
        assert!(removed.folder("inner").is_none());
        let flat = removed.folder_edit(&FolderEdit::Remove { name: "rig".into() }).unwrap();
        assert_eq!(flat.bodies[0].folder, None);
        // Duplicate: the folders under it and their bodies, copied; the originals untouched.
        let twice = spec.folder_edit(&FolderEdit::Duplicate { name: "rig".into() }).unwrap();
        assert_eq!(twice.bodies.len(), bodies + 2);
        assert_eq!(twice.bodies[..bodies], spec.bodies[..]);
        assert_eq!(twice.folder("inner 2").unwrap().parent.as_deref(), Some("rig 2"));
        assert_eq!(twice.bodies_under("rig 2"), [bodies, bodies + 1]);
        // Out again.
        let out = spec.folder_edit(&moved(&[FolderItem::Body(1)], None)).unwrap();
        assert_eq!(out.bodies[0].folder, None);
        // Refusals, each in words that name what is wrong.
        let cases: [(FolderEdit, &str); 9] = [
            (new("rig", None), "used twice"),
            (new("None", None), "a word the console keeps"),
            (new(" ", None), "must not be empty"),
            (new("x", Some("surroundings")), "take no folders"),
            (new("x", Some("nowhere")), "`nowhere` is not a folder"),
            (moved(&[FolderItem::Body(1)], Some("north")), "a body goes in a folder of the Bodies"),
            (moved(&[FolderItem::Folder("rig".into())], Some("inner")), "which is inside it"),
            (moved(&[FolderItem::Folder("rig".into())], Some("north")), "nested in its own group"),
            (moved(&[FolderItem::Body(99)], Some("rig")), "there is no body 99"),
        ];
        for (edit, words) in cases {
            let why = spec.folder_edit(&edit).unwrap_err();
            assert!(why.contains(words), "{edit:?}: `{why}` should say `{words}`");
        }
        // Too deep and too many, each with its cap.
        let mut deep = zoo.clone();
        let mut parent: Option<String> = None;
        for n in 0..MAX_DEPTH {
            deep = deep.folder_edit(&new(&format!("d{n}"), parent.as_deref())).unwrap();
            parent = Some(format!("d{n}"));
        }
        let why = deep.folder_edit(&new("deeper", parent.as_deref())).unwrap_err();
        assert!(why.contains("at most 8 deep"), "{why}");
        let mut many = zoo.clone();
        many.folders = (0..MAX_FOLDERS)
            .map(|n| FolderSpec { label: format!("f{n}"), group: FolderGroup::Bodies, parent: None })
            .collect();
        assert_eq!(many.validate(), Ok(()));
        let why = many.folder_edit(&new("one more", None)).unwrap_err();
        assert!(why.contains("257 folders is more than 256"), "{why}");
        // A cycle written into a file is refused (the edits cannot make one).
        let mut cycle = spec.clone();
        cycle.folders[0].parent = Some("inner".into());
        let why = cycle.validate().unwrap_err().0;
        assert!(why.starts_with("folders[0].parent: a cycle: rig in inner in rig"), "{why}");
        // The membership puts everything back.
        assert_eq!(flat.with_membership(&spec.membership()).unwrap(), spec);
        assert!(twice.with_membership(&spec.membership()).unwrap_err().contains("bodies"));
    }
}
