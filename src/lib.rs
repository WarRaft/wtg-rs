//! Reader and writer for Warcraft III trigger files, `war3map.wtg`: the
//! map's triggers as World Editor's trigger editor shows them, with their
//! categories and variables. A trigger's events, conditions and actions
//! (its ECAs) are calls of the editor's functions, and how many
//! parameters each takes isn't in the file: it's in the editor's
//! `UI\TriggerData.txt` ([`TriggerData`]), so reading needs it, as the
//! editor does. Writing doesn't.
//!
//! ```no_run
//! let data = wtg_rs::TriggerData::parse(&std::fs::read_to_string("TriggerData.txt").unwrap());
//! let bytes = std::fs::read("war3map.wtg").unwrap();
//! let file = wtg_rs::parse(&bytes, &data).unwrap();
//! for t in &file.triggers {
//!     println!("{}: {} ECAs", t.name, t.ecas.len());
//! }
//! assert_eq!(file.to_bytes().unwrap(), bytes);
//! ```
//!
//! Reading and writing are exact. Flags the file stores as integers are
//! kept as such (a 0 or 1 in practice), so whatever a file has writes
//! back. The format is described in the README.

pub mod text;
pub mod ui;
pub mod jass;

use std::collections::HashMap;

use parser_rs::bin::{Reader, Writer};
pub use parser_rs::{BinError, BinResult};

/// `WTG!`.
pub const MAGIC: [u8; 4] = *b"WTG!";

/// What a function call is: an event, a condition, an action, or (for a
/// parameter's value only) a call that returns one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionKind {
    Event,
    Condition,
    Action,
    Call,
}

impl FunctionKind {
    /// The kind stored as `n`: 0 event, 1 condition, 2 action, 3 call.
    pub fn of(n: u32) -> Option<FunctionKind> {
        Some(match n {
            0 => FunctionKind::Event,
            1 => FunctionKind::Condition,
            2 => FunctionKind::Action,
            3 => FunctionKind::Call,
            _ => return None,
        })
    }

    /// Its section of `TriggerData.txt`.
    pub fn section(self) -> &'static str {
        match self {
            FunctionKind::Event => "TriggerEvents",
            FunctionKind::Condition => "TriggerConditions",
            FunctionKind::Action => "TriggerActions",
            FunctionKind::Call => "TriggerCalls",
        }
    }
}

/// How many parameters each of the editor's functions takes, from
/// `UI\TriggerData.txt`: a key's values but its first (the game version
/// it's in), numbers, empty ones and `nothing` are its parameter types;
/// a call's first type is what it returns. Keys starting with `_` aren't
/// functions; a key given twice keeps its first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TriggerData {
    counts: HashMap<(&'static str, String), usize>,
}

impl TriggerData {
    /// Reads `TriggerData.txt`'s text.
    pub fn parse(text: &str) -> TriggerData {
        let mut data = TriggerData::default();
        let mut section: Option<&'static str> = None;
        for line in text.lines() {
            let line = line.trim_start();
            if line.is_empty() || line.starts_with("//") || line.starts_with(';') {
                continue;
            }
            if let Some(rest) = line.strip_prefix('[') {
                let name = rest.split(']').next().unwrap_or("");
                section = [FunctionKind::Event, FunctionKind::Condition, FunctionKind::Action, FunctionKind::Call]
                    .iter()
                    .map(|k| k.section())
                    .find(|s| *s == name);
                continue;
            }
            let Some(section) = section else { continue };
            let Some((key, values)) = line.split_once('=') else { continue };
            let key = key.trim();
            if key.starts_with('_') || key.is_empty() {
                continue;
            }
            let mut n = values
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty() && !v.starts_with(|c: char| c.is_ascii_digit()) && *v != "nothing")
                .count();
            if section == "TriggerCalls" {
                n = n.saturating_sub(1);
            }
            data.counts.entry((section, key.to_string())).or_insert(n);
        }
        data
    }

    /// How many parameters `name` of `kind` takes; `None` when the data
    /// doesn't have it.
    pub fn params(&self, kind: FunctionKind, name: &str) -> Option<usize> {
        self.counts.get(&(kind.section(), name.to_string())).copied()
    }

    /// How many functions it knows.
    pub fn len(&self) -> usize {
        self.counts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }
}

/// A category of triggers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub index: i32,
    pub name: String,
    /// Version 7: a comment, not a folder of triggers.
    pub is_comment: u32,
}

/// A global variable the triggers have (`udg_` and its name in the
/// script).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    /// Its type: `integer`, `unit`, `unitcode` ... (`TriggerData.txt`
    /// `[TriggerTypes]`).
    pub kind: String,
    /// World Editor removes a variable that has 0 here when it opens the
    /// map; maps have 1.
    pub unknown: u32,
    pub is_array: u32,
    /// Version 7.
    pub array_size: u32,
    pub has_initial: u32,
    pub initial: String,
}

/// A trigger, or (version 7) a comment.
#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub name: String,
    pub description: String,
    /// Version 7.
    pub is_comment: u32,
    pub enabled: u32,
    /// Custom text: its code is in `war3map.wct`.
    pub is_custom: u32,
    pub initially_off: u32,
    pub run_on_init: u32,
    /// Its category's index.
    pub category: i32,
    pub ecas: Vec<Eca>,
}

/// An empty trigger, enabled, in category 0.
impl Default for Trigger {
    fn default() -> Trigger {
        Trigger {
            name: String::new(),
            description: String::new(),
            is_comment: 0,
            enabled: 1,
            is_custom: 0,
            initially_off: 0,
            run_on_init: 0,
            category: 0,
            ecas: Vec::new(),
        }
    }
}

/// An event, a condition or an action of a trigger.
#[derive(Debug, Clone, PartialEq)]
pub struct Eca {
    /// 0 event, 1 condition, 2 action ([`FunctionKind::of`]).
    pub kind: u32,
    /// Inside another ECA (version 7): which of its blocks (an
    /// if/then/else's condition, then, else ...).
    pub group: Option<u32>,
    pub name: String,
    pub enabled: u32,
    pub params: Vec<Param>,
    /// Version 7: the ECAs inside it.
    pub children: Vec<Eca>,
}

/// A parameter's value.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// 0 preset, 1 variable, 2 function, 3 string.
    pub kind: u32,
    pub value: String,
    /// What the file stores before a call: 0 none, else there's one.
    pub has_call: u32,
    pub call: Option<Call>,
    /// Version 7, after a call: an integer no one knows (maps have 0).
    pub after_call: Option<u32>,
    /// What the file stores before an array index: 0 none, else there's
    /// one.
    pub is_array: u32,
    /// The array's index, a parameter itself.
    pub index: Option<Box<Param>>,
}

/// A call in a parameter: a function whose value the parameter is.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// 0 event, 1 condition, 2 action, 3 call ([`FunctionKind::of`]).
    pub kind: u32,
    pub name: String,
    /// 0 when it takes no parameters (none follow), else they do.
    pub has_params: u32,
    pub params: Vec<Param>,
}

/// A whole `war3map.wtg` (before 1.31).
#[derive(Debug, Clone, PartialEq)]
pub struct Triggers {
    /// 4 Reign of Chaos, 7 The Frozen Throne.
    pub version: u32,
    pub categories: Vec<Category>,
    /// After the categories: 1 Reign of Chaos, 2 The Frozen Throne.
    pub game: u32,
    pub variables: Vec<Variable>,
    pub triggers: Vec<Trigger>,
}

struct Ctx<'a> {
    data: &'a TriggerData,
    v7: bool,
}

impl Ctx<'_> {
    fn count(&self, r: &Reader, kind: u32, name: &str) -> BinResult<usize> {
        let k = FunctionKind::of(kind).ok_or_else(|| r.error(format!("{name}: unknown function kind {kind}")))?;
        self.data.params(k, name).ok_or_else(|| {
            r.error(format!(
                "`{name}` isn't in UI\\TriggerData.txt [{}]: how many parameters it takes is the editor's to say",
                k.section()
            ))
        })
    }

    fn eca(&self, r: &mut Reader, child: bool) -> BinResult<Eca> {
        let kind = r.u32()?;
        let group = if child { Some(r.u32()?) } else { None };
        let name = r.cstr()?;
        let enabled = r.u32()?;
        let n = self.count(r, kind, &name)?;
        let params = (0..n).map(|_| self.param(r)).collect::<BinResult<_>>()?;
        let children = if self.v7 { r.list(13, |r| self.eca(r, true))? } else { Vec::new() };
        Ok(Eca { kind, group, name, enabled, params, children })
    }

    fn param(&self, r: &mut Reader) -> BinResult<Param> {
        let kind = r.u32()?;
        let value = r.cstr()?;
        let has_call = r.u32()?;
        let call = if has_call != 0 {
            let kind = r.u32()?;
            let name = r.cstr()?;
            let has_params = r.u32()?;
            let params = if has_params != 0 {
                let n = self.count(r, kind, &name)?;
                (0..n).map(|_| self.param(r)).collect::<BinResult<_>>()?
            } else {
                Vec::new()
            };
            Some(Call { kind, name, has_params, params })
        } else {
            None
        };
        let after_call = if self.v7 && has_call != 0 { Some(r.u32()?) } else { None };
        let is_array = r.u32()?;
        let index = if is_array != 0 { Some(Box::new(self.param(r)?)) } else { None };
        Ok(Param { kind, value, has_call, call, after_call, is_array, index })
    }
}

/// What a file from 1.31 on has after `WTG!`.
pub const MAGIC_131: u32 = 0x8000_0004;

/// One element of a 1.31 file's tree: what the trigger editor's list
/// shows, each under a parent.
#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    /// Type 4.
    Category { id: u32, name: String, is_comment: u32, has_children: u32, parent: u32 },
    /// Type 8 a trigger, 16 a comment, 32 a script; and the trigger.
    Trigger { kind: u32, trigger: Trigger, id: u32, parent: u32 },
    /// Type 64.
    Variable { id: u32, name: String, parent: u32 },
}

/// A `war3map.wtg` from 1.31 on (after `WTG!` and [`MAGIC_131`]): the
/// trigger editor's list as a tree. Laid out as one description of the
/// format has it; no map at hand is this new, so it's checked by tests
/// only.
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    /// 4 Reign of Chaos, 7 The Frozen Throne.
    pub version: u32,
    /// Four integers: 1, 0, 0, 0 (like a game version).
    pub unknown_a: [u32; 4],
    /// How many categories, triggers, comments, scripts and variables
    /// there are, each with the indexes of those deleted.
    pub counts: [(u32, Vec<u32>); 5],
    pub unknown_b: [u32; 2],
    /// 1 Reign of Chaos, 2 The Frozen Throne.
    pub game: u32,
    /// Each with its id and its parent's.
    pub variables: Vec<(Variable, u32, u32)>,
    /// How many elements the tree says it has: one more than follow.
    pub element_count: u32,
    pub unknown_c: [u32; 2],
    /// The map file's name (`map.w3x`); World Editor ignores it.
    pub map_name: String,
    pub unknown_d: [u32; 2],
    /// Version 7.
    pub unknown_e: u32,
    pub elements: Vec<Element>,
}

/// Either layout of the file.
#[derive(Debug, Clone, PartialEq)]
pub enum File {
    /// Before 1.31.
    Classic(Triggers),
    /// From 1.31.
    Tree(Box<Tree>),
}

impl File {
    pub fn to_bytes(&self) -> BinResult<Vec<u8>> {
        match self {
            File::Classic(t) => t.to_bytes(),
            File::Tree(t) => t.to_bytes(),
        }
    }
}

/// Reads a `war3map.wtg` of either layout.
pub fn parse_any(bytes: &[u8], data: &TriggerData) -> BinResult<File> {
    if bytes.get(4..8) == Some(&MAGIC_131.to_le_bytes()[..]) {
        parse_tree(bytes, data).map(|t| File::Tree(Box::new(t)))
    } else {
        parse(bytes, data).map(File::Classic)
    }
}

impl Ctx<'_> {
    /// A 1.31 trigger: the classic one with its id after the comment
    /// flag and its parent's id in place of the category.
    fn trigger_131(&self, r: &mut Reader) -> BinResult<(Trigger, u32, u32)> {
        let name = r.cstr()?;
        let description = r.cstr()?;
        let is_comment = if self.v7 { r.u32()? } else { 0 };
        let id = r.u32()?;
        let enabled = r.u32()?;
        let is_custom = r.u32()?;
        let initially_off = r.u32()?;
        let run_on_init = r.u32()?;
        let parent = r.u32()?;
        let ecas = r.list(9, |r| self.eca(r, false))?;
        let t = Trigger { name, description, is_comment, enabled, is_custom, initially_off, run_on_init, category: parent as i32, ecas };
        Ok((t, id, parent))
    }
}

/// Reads a 1.31 `war3map.wtg` ([`Tree`]). Every byte must be read.
pub fn parse_tree(bytes: &[u8], data: &TriggerData) -> BinResult<Tree> {
    let mut r = Reader::new(bytes);
    if r.array::<4>()? != MAGIC || r.u32()? != MAGIC_131 {
        return Err(BinError::new(0, "not a 1.31 trigger file: it doesn't start with WTG! and 0x80000004"));
    }
    let version = r.u32()?;
    if version != 4 && version != 7 {
        return Err(BinError::new(8, format!("unknown wtg version {version} (4 and 7 are known)")));
    }
    let v7 = version == 7;
    let ctx = Ctx { data, v7 };
    let unknown_a = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
    let mut counts: [(u32, Vec<u32>); 5] = Default::default();
    for c in &mut counts {
        c.0 = r.u32()?;
        c.1 = r.list(4, |r| r.u32())?;
    }
    let unknown_b = [r.u32()?, r.u32()?];
    let game = r.u32()?;
    let variables = r.list(22, |r| {
        let name = r.cstr()?;
        let kind = r.cstr()?;
        let unknown = r.u32()?;
        let is_array = r.u32()?;
        let array_size = if v7 { r.u32()? } else { 0 };
        let has_initial = r.u32()?;
        let initial = r.cstr()?;
        Ok((Variable { name, kind, unknown, is_array, array_size, has_initial, initial }, r.u32()?, r.u32()?))
    })?;
    let element_count = r.u32()?;
    let unknown_c = [r.u32()?, r.u32()?];
    let map_name = r.cstr()?;
    let unknown_d = [r.u32()?, r.u32()?];
    let unknown_e = if v7 { r.u32()? } else { 0 };
    let mut elements = Vec::new();
    for _ in 1..element_count {
        let at = r.pos();
        elements.push(match r.u32()? {
            4 => Element::Category {
                id: r.u32()?,
                name: r.cstr()?,
                is_comment: if v7 { r.u32()? } else { 0 },
                has_children: r.u32()?,
                parent: r.u32()?,
            },
            kind @ (8 | 16 | 32) => {
                let (trigger, id, parent) = ctx.trigger_131(&mut r)?;
                Element::Trigger { kind, trigger, id, parent }
            }
            64 => Element::Variable { id: r.u32()?, name: r.cstr()?, parent: r.u32()? },
            n => return Err(BinError::new(at, format!("unknown element type {n} (4, 8, 16, 32, 64 are known)"))),
        });
    }
    r.finish()?;
    Ok(Tree {
        version,
        unknown_a,
        counts,
        unknown_b,
        game,
        variables,
        element_count,
        unknown_c,
        map_name,
        unknown_d,
        unknown_e,
        elements,
    })
}

impl Tree {
    /// The file's bytes.
    pub fn to_bytes(&self) -> BinResult<Vec<u8>> {
        let v7 = self.version == 7;
        let mut w = Writer::new();
        w.bytes(&MAGIC);
        w.u32(MAGIC_131);
        w.u32(self.version);
        for n in self.unknown_a {
            w.u32(n);
        }
        for (count, deleted) in &self.counts {
            w.u32(*count);
            w.count(deleted.len())?;
            for &d in deleted {
                w.u32(d);
            }
        }
        for n in self.unknown_b {
            w.u32(n);
        }
        w.u32(self.game);
        w.count(self.variables.len())?;
        for (v, id, parent) in &self.variables {
            w.cstr(&v.name)?;
            w.cstr(&v.kind)?;
            w.u32(v.unknown);
            w.u32(v.is_array);
            if v7 {
                w.u32(v.array_size);
            }
            w.u32(v.has_initial);
            w.cstr(&v.initial)?;
            w.u32(*id);
            w.u32(*parent);
        }
        if self.element_count as usize != self.elements.len() + 1 {
            return Err(BinError::new(w.len(), format!(
                "the tree says {} elements and has {} (it counts one more than follow)",
                self.element_count,
                self.elements.len()
            )));
        }
        w.u32(self.element_count);
        for n in self.unknown_c {
            w.u32(n);
        }
        w.cstr(&self.map_name)?;
        for n in self.unknown_d {
            w.u32(n);
        }
        if v7 {
            w.u32(self.unknown_e);
        }
        for e in &self.elements {
            match e {
                Element::Category { id, name, is_comment, has_children, parent } => {
                    w.u32(4);
                    w.u32(*id);
                    w.cstr(name)?;
                    if v7 {
                        w.u32(*is_comment);
                    }
                    w.u32(*has_children);
                    w.u32(*parent);
                }
                Element::Trigger { kind, trigger: t, id, parent } => {
                    w.u32(*kind);
                    w.cstr(&t.name)?;
                    w.cstr(&t.description)?;
                    if v7 {
                        w.u32(t.is_comment);
                    }
                    w.u32(*id);
                    for n in [t.enabled, t.is_custom, t.initially_off, t.run_on_init] {
                        w.u32(n);
                    }
                    w.u32(*parent);
                    w.count(t.ecas.len())?;
                    for e in &t.ecas {
                        write_eca(&mut w, e, v7)?;
                    }
                }
                Element::Variable { id, name, parent } => {
                    w.u32(64);
                    w.u32(*id);
                    w.cstr(name)?;
                    w.u32(*parent);
                }
            }
        }
        Ok(w.into_bytes())
    }
}

/// Reads a `war3map.wtg` from before 1.31, with the editor's function
/// data. Every byte must be read. ([`parse_any`] reads either layout.)
pub fn parse(bytes: &[u8], data: &TriggerData) -> BinResult<Triggers> {
    let mut r = Reader::new(bytes);
    if r.array::<4>()? != MAGIC {
        return Err(BinError::new(0, "not a trigger file: it doesn't start with WTG!"));
    }
    let version = r.u32()?;
    if version == MAGIC_131 {
        return Err(BinError::new(4, "a trigger file of 1.31 or later: parse_tree or parse_any reads it"));
    }
    if version != 4 && version != 7 {
        return Err(BinError::new(4, format!("unknown wtg version {version} (4 and 7 are known)")));
    }
    let v7 = version == 7;
    let ctx = Ctx { data, v7 };
    let categories = r.list(5, |r| {
        Ok(Category { index: r.i32()?, name: r.cstr()?, is_comment: if v7 { r.u32()? } else { 0 } })
    })?;
    let game = r.u32()?;
    let variables = r.list(14, |r| {
        let name = r.cstr()?;
        let kind = r.cstr()?;
        let unknown = r.u32()?;
        let is_array = r.u32()?;
        let array_size = if v7 { r.u32()? } else { 0 };
        let has_initial = r.u32()?;
        Ok(Variable { name, kind, unknown, is_array, array_size, has_initial, initial: r.cstr()? })
    })?;
    let triggers = r.list(26, |r| {
        let name = r.cstr()?;
        let description = r.cstr()?;
        let is_comment = if v7 { r.u32()? } else { 0 };
        let enabled = r.u32()?;
        let is_custom = r.u32()?;
        let initially_off = r.u32()?;
        let run_on_init = r.u32()?;
        let category = r.i32()?;
        let ecas = r.list(9, |r| ctx.eca(r, false))?;
        Ok(Trigger { name, description, is_comment, enabled, is_custom, initially_off, run_on_init, category, ecas })
    })?;
    r.finish()?;
    Ok(Triggers { version, categories, game, variables, triggers })
}

impl Triggers {
    /// The file's bytes.
    pub fn to_bytes(&self) -> BinResult<Vec<u8>> {
        let v7 = self.version == 7;
        let mut w = Writer::new();
        w.bytes(&MAGIC);
        w.u32(self.version);
        w.count(self.categories.len())?;
        for c in &self.categories {
            w.i32(c.index);
            w.cstr(&c.name)?;
            if v7 {
                w.u32(c.is_comment);
            }
        }
        w.u32(self.game);
        w.count(self.variables.len())?;
        for v in &self.variables {
            w.cstr(&v.name)?;
            w.cstr(&v.kind)?;
            w.u32(v.unknown);
            w.u32(v.is_array);
            if v7 {
                w.u32(v.array_size);
            }
            w.u32(v.has_initial);
            w.cstr(&v.initial)?;
        }
        w.count(self.triggers.len())?;
        for t in &self.triggers {
            w.cstr(&t.name)?;
            w.cstr(&t.description)?;
            if v7 {
                w.u32(t.is_comment);
            }
            for n in [t.enabled, t.is_custom, t.initially_off, t.run_on_init] {
                w.u32(n);
            }
            w.i32(t.category);
            w.count(t.ecas.len())?;
            for e in &t.ecas {
                write_eca(&mut w, e, v7)?;
            }
        }
        Ok(w.into_bytes())
    }
}

fn write_eca(w: &mut Writer, e: &Eca, v7: bool) -> BinResult<()> {
    w.u32(e.kind);
    if let Some(g) = e.group {
        w.u32(g);
    }
    w.cstr(&e.name)?;
    w.u32(e.enabled);
    for p in &e.params {
        write_param(w, p)?;
    }
    if v7 {
        w.count(e.children.len())?;
        for c in &e.children {
            write_eca(w, c, v7)?;
        }
    }
    Ok(())
}

fn write_param(w: &mut Writer, p: &Param) -> BinResult<()> {
    w.u32(p.kind);
    w.cstr(&p.value)?;
    w.u32(p.has_call);
    if let Some(c) = &p.call {
        w.u32(c.kind);
        w.cstr(&c.name)?;
        w.u32(c.has_params);
        for q in &c.params {
            write_param(w, q)?;
        }
    }
    if let Some(n) = p.after_call {
        w.u32(n);
    }
    w.u32(p.is_array);
    if let Some(i) = &p.index {
        write_param(w, i)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
