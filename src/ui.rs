//! What World Editor's trigger editor shows: a trigger's events,
//! conditions and actions as phrases ("Unit - Create 1 Footman for
//! Player 1 (Red) at (Center of (Playable map area)) facing Default
//! building facing degrees"), each in its category, with what's inside
//! an if/then/else or a loop in its blocks.
//!
//! The words are the editor's, in its files: `UI\TriggerData.txt` (the
//! categories, the types, the presets, each function's parameter types
//! and category) and `UI\TriggerStrings.txt` (each function's title,
//! phrase and hint), with the keys they name (`WESTRING_TRIGCAT_UNIT`)
//! resolved by the caller from `UI\WorldEditStrings.txt`. Nothing about
//! the functions is built in, but which have blocks (the editor's own
//! knowledge too: [`blocks`]).
//!
//! A phrase is pieces of text and parameters: `"Create ",~Number," ",~Unit`
//! is text, the first parameter, text, the second. The `~Name`s stand for
//! the parameters in order, whatever the names.

use std::collections::HashMap;

use bni_rs::ast::{Field, ValueKind};

use crate::{Eca, FunctionKind, Param, Trigger};

/// A category of functions (`TC_UNIT`).
#[derive(Debug, Clone, PartialEq)]
pub struct Category {
    /// Its name (`Unit`).
    pub name: String,
    /// Its icon, a texture path without the extension
    /// (`ReplaceableTextures\WorldEditUI\Actions-Unit`).
    pub icon: String,
    /// Is its name shown before its functions' phrases (`Unit - ...`)?
    pub show_name: bool,
}

/// A preset value of a type (`Player00`: `Player 1 (Red)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    /// Its type (`player`).
    pub kind: String,
    /// What the editor shows.
    pub display: String,
}

/// A piece of a phrase.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Text(String),
    /// The parameter at this index.
    Param(usize),
}

/// One of the editor's functions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Function {
    /// Its category's key (`TC_UNIT`); empty when it has none.
    pub category: String,
    /// Its parameters' types.
    pub args: Vec<String>,
    /// For a call, the type it returns.
    pub returns: String,
    /// Its title, in the editor's lists (`Create Units Facing Angle`).
    pub title: String,
    /// Its phrase.
    pub phrase: Vec<Piece>,
    /// What the editor says about it; empty for nothing.
    pub hint: String,
    /// Its parameters' names in its phrase (`Number` of `~Number`).
    pub names: Vec<String>,
    /// What a new one's parameters are (`_Name_Defaults`): a preset, a
    /// call's name, a value; `_` for none.
    pub defaults: Vec<String>,
}

/// A block of an ECA: an if/then/else's conditions, then and else, a
/// loop's actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockKind {
    /// Its number, as the ECAs inside say it (`group`).
    pub group: u32,
    /// What goes in it: 1 conditions, 2 actions.
    pub kind: u32,
    /// The key of its label in `UI\WorldEditStrings.txt`.
    pub key: &'static str,
    /// Its label when the editor's strings don't have the key.
    pub english: &'static str,
}

const IF: [BlockKind; 3] = [
    BlockKind { group: 0, kind: 1, key: "WESTRING_TRIGSUBFUNC_IFCONDITIONS", english: "If - Conditions" },
    BlockKind { group: 1, kind: 2, key: "WESTRING_TRIGSUBFUNC_IFTHENACTIONS", english: "Then - Actions" },
    BlockKind { group: 2, kind: 2, key: "WESTRING_TRIGSUBFUNC_IFELSEACTIONS", english: "Else - Actions" },
];
const AND_OR: [BlockKind; 1] =
    [BlockKind { group: 0, kind: 1, key: "WESTRING_TRIGSUBFUNC_ANDORCONDITIONS", english: "Conditions" }];
const LOOP: [BlockKind; 1] =
    [BlockKind { group: 0, kind: 2, key: "WESTRING_TRIGSUBFUNC_FORLOOPACTIONS", english: "Loop - Actions" }];

/// The blocks of the function `name` of `kind` (0 event, 1 condition,
/// 2 action): an if/then/else's three, `AndMultiple`'s and
/// `OrMultiple`'s conditions, a loop's actions (every other action whose
/// name ends with `Multiple`, as the editor's are: `ForLoopAMultiple`,
/// `ForGroupMultiple`, `EnumItemsInRectBJMultiple` ...). None for the
/// rest. The editor knows this itself; its files don't say.
pub fn blocks(kind: u32, name: &str) -> &'static [BlockKind] {
    match (kind, name) {
        (2, "IfThenElseMultiple") => &IF,
        (1, "AndMultiple" | "OrMultiple") => &AND_OR,
        (2, n) if n.ends_with("Multiple") => &LOOP,
        _ => &[],
    }
}

/// The editor's words for triggers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ui {
    pub categories: HashMap<String, Category>,
    /// Types' names, by type (`unit`: `Unit`).
    pub types: HashMap<String, String>,
    pub presets: HashMap<String, Preset>,
    /// Each type's base type, for a type of the editor's own
    /// (`unitcode`: `integer`).
    pub bases: HashMap<String, String>,
    functions: HashMap<(FunctionKind, String), Function>,
    /// The presets and the functions in the files' order, as the editor
    /// lists them.
    preset_order: Vec<String>,
    function_order: Vec<(FunctionKind, String)>,
    /// Labels by `WESTRING_` key, for the ones [`Ui::label`] gives.
    labels: HashMap<&'static str, String>,
}

/// Keys of labels the view shows, with their English.
const LABELS: [(&str, &str); 9] = [
    ("WESTRING_VARIABLES", "Variables"),
    ("WESTRING_EVENTS", "Events"),
    ("WESTRING_CONDITIONS", "Conditions"),
    ("WESTRING_ACTIONS", "Actions"),
    (IF[0].key, IF[0].english),
    (IF[1].key, IF[1].english),
    (IF[2].key, IF[2].english),
    (AND_OR[0].key, AND_OR[0].english),
    (LOOP[0].key, LOOP[0].english),
];

const FUNCTION_SECTIONS: [(FunctionKind, &str, &str); 4] = [
    (FunctionKind::Event, "TriggerEvents", "TriggerEventStrings"),
    (FunctionKind::Condition, "TriggerConditions", "TriggerConditionStrings"),
    (FunctionKind::Action, "TriggerActions", "TriggerActionStrings"),
    (FunctionKind::Call, "TriggerCalls", "TriggerCallStrings"),
];

impl Ui {
    /// Reads `TriggerData.txt` and `TriggerStrings.txt`; `resolve` turns a
    /// `WESTRING_` key into its text (and returns anything else as it
    /// is). A key given twice keeps its first, as the editor does.
    pub fn parse(trigger_data: &str, trigger_strings: &str, resolve: &dyn Fn(&str) -> String) -> Ui {
        let mut ui = Ui::default();
        for (key, english) in LABELS {
            let text = resolve(key);
            ui.labels.insert(key, if text == key || text.is_empty() { english.to_string() } else { text });
        }
        let field = |fields: &[Field], i: usize| fields.get(i).map(|f| f.text().trim().to_string()).unwrap_or_default();
        let data = bni_rs::parse(trigger_data);
        for section in &data.sections {
            let name = section.name.text.as_str();
            for item in &section.items {
                let Some(key) = item.key.as_ref().map(|k| k.text.trim()) else { continue };
                let f = item.fields.as_slice();
                match name {
                    "TriggerCategories" => {
                        ui.categories.entry(key.to_string()).or_insert_with(|| Category {
                            name: resolve(&field(f, 0)),
                            icon: field(f, 1),
                            show_name: field(f, 2) != "1",
                        });
                    }
                    "TriggerTypes" => {
                        ui.types.entry(key.to_string()).or_insert_with(|| resolve(&field(f, 3)));
                        let base = field(f, 4);
                        if !base.is_empty() {
                            ui.bases.entry(key.to_string()).or_insert(base);
                        }
                    }
                    "TriggerParams" => {
                        if !ui.presets.contains_key(key) {
                            ui.preset_order.push(key.to_string());
                            ui.presets.insert(key.to_string(), Preset { kind: field(f, 1), display: resolve(&field(f, 3)) });
                        }
                    }
                    _ => {
                        let Some(&(kind, _, _)) = FUNCTION_SECTIONS.iter().find(|s| s.1 == name) else { continue };
                        if let Some(function) = key.strip_prefix('_').and_then(|k| k.strip_suffix("_Defaults")) {
                            let entry = ui.functions.entry((kind, function.to_string())).or_default();
                            if entry.defaults.is_empty() {
                                entry.defaults = f.iter().map(|v| v.text().trim().to_string()).collect();
                            }
                            continue;
                        }
                        if let Some(function) = key.strip_prefix('_').and_then(|k| k.strip_suffix("_Category")) {
                            let entry = ui.functions.entry((kind, function.to_string())).or_default();
                            if entry.category.is_empty() {
                                entry.category = field(f, 0);
                            }
                            continue;
                        }
                        if key.starts_with('_') {
                            continue;
                        }
                        if !ui.function_order.iter().any(|(k, n)| *k == kind && n == key) {
                            ui.function_order.push((kind, key.to_string()));
                        }
                        let entry = ui.functions.entry((kind, key.to_string())).or_default();
                        if !entry.args.is_empty() || !entry.returns.is_empty() {
                            continue;
                        }
                        // As `TriggerData::parse` counts them: numbers (the
                        // version, a call's flag), empty ones and
                        // `nothing` aren't parameters.
                        let mut types = f
                            .iter()
                            .map(|v| v.text().trim().to_string())
                            .filter(|v| !v.is_empty() && !v.starts_with(|c: char| c.is_ascii_digit()) && v != "nothing");
                        if kind == FunctionKind::Call {
                            entry.returns = types.next().unwrap_or_default();
                        }
                        entry.args = types.collect();
                    }
                }
            }
        }
        let strings = bni_rs::parse(trigger_strings);
        for section in &strings.sections {
            let Some(&(kind, _, _)) = FUNCTION_SECTIONS.iter().find(|s| s.2 == section.name.text) else { continue };
            // How many lines each function has had: its title, then its
            // phrase.
            let mut seen: HashMap<String, usize> = HashMap::new();
            for item in &section.items {
                let Some(key) = item.key.as_ref().map(|k| k.text.trim()) else { continue };
                if let Some(function) = key.strip_suffix("Hint")
                    && seen.contains_key(function)
                {
                    if let Some(entry) = ui.functions.get_mut(&(kind, function.to_string())) {
                        entry.hint = item.value_text();
                    }
                    continue;
                }
                let n = seen.entry(key.to_string()).or_insert(0);
                *n += 1;
                let entry = ui.functions.entry((kind, key.to_string())).or_default();
                match *n {
                    1 => entry.title = item.value_text(),
                    2 => {
                        entry.phrase = phrase(&item.fields);
                        entry.names = item
                            .fields
                            .iter()
                            .filter_map(|f| f.text().trim().strip_prefix('~').map(str::to_string))
                            .collect();
                    }
                    _ => {}
                }
            }
        }
        ui
    }

    /// The function `name` of `kind`, when the editor's files have it.
    pub fn function(&self, kind: FunctionKind, name: &str) -> Option<&Function> {
        self.functions.get(&(kind, name.to_string()))
    }

    /// The label of a `WESTRING_` key of [`blocks`] or of the view's
    /// headings (`WESTRING_VARIABLES`, `WESTRING_EVENTS`,
    /// `WESTRING_CONDITIONS`, `WESTRING_ACTIONS`).
    pub fn label(&self, key: &str) -> String {
        match self.labels.get(key) {
            Some(l) => l.clone(),
            None => LABELS.iter().find(|(k, _)| *k == key).map_or(key, |(_, e)| *e).to_string(),
        }
    }

    /// Does it know any function?
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// The functions of `kind` in the files' order, those of
    /// `TriggerData.txt` only (not a name `TriggerStrings.txt` alone has).
    pub fn functions(&self, kind: FunctionKind) -> Vec<(&str, &Function)> {
        self.function_order
            .iter()
            .filter(|(k, _)| *k == kind)
            .filter_map(|(k, n)| self.functions.get(&(*k, n.clone())).map(|f| (n.as_str(), f)))
            .collect()
    }

    /// The calls that return `ty`, in order.
    pub fn calls_returning(&self, ty: &str) -> Vec<(&str, &Function)> {
        self.functions(FunctionKind::Call).into_iter().filter(|(_, f)| f.returns == ty).collect()
    }

    /// The presets of `ty`, in order: key and preset.
    pub fn presets_of(&self, ty: &str) -> Vec<(&str, &Preset)> {
        self.preset_order.iter().filter_map(|k| self.presets.get(k).filter(|p| p.kind == ty).map(|p| (k.as_str(), p))).collect()
    }

    /// `ty`'s base type, or `ty` itself.
    pub fn base<'a>(&'a self, ty: &'a str) -> &'a str {
        self.bases.get(ty).map_or(ty, String::as_str)
    }

    /// Is a value of `ty` typed in (a number, a string, an id: its base is
    /// `integer`, `real` or `string`)?
    pub fn takes_value(&self, ty: &str) -> bool {
        matches!(self.base(ty), "integer" | "real" | "string")
    }

    /// A new parameter of `ty` from a function's default `d` (see
    /// [`Function::defaults`]), as the text writes a parameter: the preset
    /// it names, the call it names with its own defaults, the value it is;
    /// for none (`_`), an empty value, or the type's first preset when it
    /// takes no value.
    pub fn default_param(&self, ty: &str, d: &str) -> String {
        self.default_at(ty, d, 0)
    }

    fn default_at(&self, ty: &str, d: &str, depth: usize) -> String {
        if d.is_empty() || d == "_" {
            // None: an empty value, or a type's first preset when it takes
            // no value (a player, an operator).
            return match self.presets_of(ty).first() {
                Some((key, _)) if !self.takes_value(ty) => key.to_string(),
                _ => "\"\"".into(),
            };
        }
        if self.presets.contains_key(d) {
            return d.to_string();
        }
        if depth < 6
            && let Some(f) = self.function(FunctionKind::Call, d)
        {
            return format!("{d}({})", self.default_args(f, depth + 1));
        }
        crate::text::jass_string(d)
    }

    fn default_args(&self, f: &Function, depth: usize) -> String {
        let args: Vec<String> = f
            .args
            .iter()
            .enumerate()
            .map(|(i, ty)| self.default_at(ty, f.defaults.get(i).map_or("_", String::as_str), depth))
            .collect();
        args.join(", ")
    }

    /// A new line's value: the function `name` of `kind` with its default
    /// parameters (`CreateNUnitsAtLoc("1", "hfoo", Player00, ...)`).
    pub fn new_call(&self, kind: FunctionKind, name: &str) -> String {
        match self.function(kind, name) {
            Some(f) => format!("{name}({})", self.default_args(f, 0)),
            None => format!("{name}()"),
        }
    }
}

/// A phrase's pieces: a field that's one unquoted `~Name` is a parameter,
/// the next one; anything else is text.
fn phrase(fields: &[Field]) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut next = 0;
    for f in fields {
        let param = matches!(f.parts.as_slice(), [p] if !matches!(p.kind, ValueKind::Quoted { .. }) && p.text.trim_start().starts_with('~'));
        if param {
            out.push(Piece::Param(next));
            next += 1;
        } else {
            let text = f.text();
            match out.last_mut() {
                Some(Piece::Text(t)) => *t += &text,
                _ => out.push(Piece::Text(text)),
            }
        }
    }
    out
}

// ---------------------------------------------------------------- the view

/// What a value shown in a phrase is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A preset (`Player 1 (Red)`).
    Preset,
    Variable,
    /// A value typed in: a number, a string, an object's id.
    Value,
    /// A call's phrase, in parentheses.
    Call,
    /// A function the editor's files don't have, or a parameter of a kind
    /// they don't know.
    Unknown,
}

/// A piece of a line of the view.
#[derive(Debug, Clone, PartialEq)]
pub enum Span {
    Text(String),
    /// A parameter: what it is, where (its path: the parameter's number
    /// from 0, then a call's parameter's after a `.`, and `i` for an
    /// array's index: `2.0.i`; empty for what isn't one), and how it
    /// shows, which for a call is its own phrase.
    Value { role: Role, path: String, spans: Vec<Span> },
}

/// A line of the view: an event, condition or action.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Its line in the text (from 1).
    pub line: usize,
    /// 0 event, 1 condition, 2 action.
    pub kind: u32,
    pub enabled: bool,
    /// Its function's name (`CreateNUnitsAtLoc`).
    pub name: String,
    /// Its category's key (`TC_UNIT`); empty when unknown.
    pub category: String,
    /// Its category's icon (see [`Category::icon`]); empty when unknown.
    pub icon: String,
    /// Do the editor's files have its function?
    pub known: bool,
    /// What it shows: the category's name, then the phrase.
    pub text: Vec<Span>,
    pub hint: String,
    /// Its blocks, each with what's inside.
    pub blocks: Vec<Block>,
}

/// A block of a line, with what's inside it.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub label: String,
    pub group: u32,
    /// What goes in it: 1 conditions, 2 actions.
    pub kind: u32,
    pub nodes: Vec<Node>,
}

/// A trigger as the editor shows it: its events, conditions and actions.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub events: Vec<Node>,
    pub conditions: Vec<Node>,
    pub actions: Vec<Node>,
}

impl Ui {
    /// The view of `t`, whose ECAs are at `lines` (see
    /// `text::ParsedTrigger::lines`; a line missing is 0).
    pub fn render(&self, t: &Trigger, lines: &[usize]) -> View {
        let mut at = 0;
        let mut view = View { events: Vec::new(), conditions: Vec::new(), actions: Vec::new() };
        for e in &t.ecas {
            let node = self.node(e, lines, &mut at);
            match e.kind {
                0 => view.events.push(node),
                1 => view.conditions.push(node),
                _ => view.actions.push(node),
            }
        }
        view
    }

    fn node(&self, e: &Eca, lines: &[usize], at: &mut usize) -> Node {
        let line = lines.get(*at).copied().unwrap_or(0);
        *at += 1;
        let kind = FunctionKind::of(e.kind).unwrap_or(FunctionKind::Action);
        let f = self.function(kind, &e.name);
        let category = f.and_then(|f| self.categories.get(&f.category));
        let mut text = Vec::new();
        if let Some(c) = category.filter(|c| c.show_name && !c.name.is_empty()) {
            text.push(Span::Text(format!("{} - ", c.name)));
        }
        text.extend(self.phrase(f, &e.name, &e.params, ""));
        let mut blocks: Vec<Block> = blocks(e.kind, &e.name)
            .iter()
            .map(|b| Block { label: self.label(b.key), group: b.group, kind: b.kind, nodes: Vec::new() })
            .collect();
        for c in &e.children {
            let node = self.node(c, lines, at);
            let group = c.group.unwrap_or(0);
            match blocks.iter_mut().find(|b| b.group == group) {
                Some(b) => b.nodes.push(node),
                // A block the editor wouldn't have: shown as it is.
                None => blocks.push(Block { label: format!("Block {group}"), group, kind: c.kind, nodes: vec![node] }),
            }
        }
        Node {
            line,
            kind: e.kind,
            enabled: e.enabled != 0,
            name: e.name.clone(),
            category: f.map(|f| f.category.clone()).unwrap_or_default(),
            icon: category.map(|c| c.icon.clone()).unwrap_or_default(),
            known: f.is_some(),
            text,
            hint: f.map(|f| f.hint.clone()).unwrap_or_default(),
            blocks,
        }
    }

    /// The phrase of `f` with `params`, the parameters at `base` (their
    /// paths: see [`Span::Value`]); for a function the files don't have,
    /// its name and parameters as a call.
    fn phrase(&self, f: Option<&Function>, name: &str, params: &[Param], base: &str) -> Vec<Span> {
        let at = |i: usize| if base.is_empty() { i.to_string() } else { format!("{base}.{i}") };
        let fixed = |t: &str| Span::Value { role: Role::Unknown, path: String::new(), spans: vec![Span::Text(t.into())] };
        let Some(f) = f.filter(|f| !f.phrase.is_empty() || !f.title.is_empty()) else {
            let mut out = vec![fixed(&format!("{name}("))];
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    out.push(Span::Text(", ".into()));
                }
                out.push(self.param(p, &at(i), ""));
            }
            out.push(fixed(")"));
            return out;
        };
        let pieces = if f.phrase.is_empty() { vec![Piece::Text(f.title.clone())] } else { f.phrase.clone() };
        let mut out = Vec::new();
        for piece in pieces {
            match piece {
                Piece::Text(t) => out.push(Span::Text(t)),
                Piece::Param(i) => match params.get(i) {
                    Some(p) => out.push(self.param(p, &at(i), f.names.get(i).map_or("", String::as_str))),
                    None => out.push(fixed("?")),
                },
            }
        }
        // Parameters the phrase doesn't show (a modded editor's data out
        // of step with its strings) aren't lost.
        let shown = f.phrase.iter().filter(|p| matches!(p, Piece::Param(_))).count();
        for (i, p) in params.iter().enumerate().skip(shown) {
            out.push(Span::Text(", ".into()));
            out.push(self.param(p, &at(i), ""));
        }
        out
    }

    /// How a parameter at `path` shows; `name` is its name in the phrase
    /// (`Value`), shown for an empty value, as the editor does.
    fn param(&self, p: &Param, path: &str, name: &str) -> Span {
        let (role, mut spans) = match (p.kind, &p.call) {
            (2, Some(c)) => {
                let fk = FunctionKind::of(c.kind).unwrap_or(FunctionKind::Call);
                let f = self.function(fk, &c.name);
                let phrase = self.phrase(f, &c.name, &c.params, path);
                // A phrase in parentheses, as the editor shows a call; one
                // the files don't have shows as a call already.
                let spans = match f {
                    Some(_) => [vec![Span::Text("(".into())], phrase, vec![Span::Text(")".into())]].concat(),
                    None => phrase,
                };
                (Role::Call, spans)
            }
            (0, _) => {
                let display = self.presets.get(&p.value).map_or(p.value.clone(), |pr| pr.display.clone());
                (Role::Preset, vec![Span::Text(display)])
            }
            (1, _) => (Role::Variable, vec![Span::Text(p.value.clone())]),
            (3, _) if p.value.is_empty() => {
                let shown = if name.is_empty() { "Value" } else { name };
                (Role::Unknown, vec![Span::Text(shown.into())])
            }
            (3, _) => (Role::Value, vec![Span::Text(p.value.clone())]),
            _ => (Role::Unknown, vec![Span::Text(p.value.clone())]),
        };
        if let Some(i) = &p.index {
            spans.push(Span::Text("[".into()));
            spans.push(self.param(i, &format!("{path}.i"), "Index"));
            spans.push(Span::Text("]".into()));
        }
        Span::Value { role, path: path.to_string(), spans }
    }
}

#[cfg(test)]
#[path = "ui.test.rs"]
mod tests;
