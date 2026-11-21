//! The text form of a trigger file: BNI, a `[wtg]` section (the
//! versions), then a section per category, variable and trigger. A
//! trigger's events, conditions and actions are lines of calls, written
//! as World Editor's functions are named in `UI\TriggerData.txt`:
//!
//! ```ini
//! [wtg]
//! version=7
//! game=2
//!
//! [category]
//! index=1
//! name="Initialization"
//!
//! [variable]
//! name="hero"
//! type="unit"
//! array=1
//! size=12
//! initial=""
//!
//! [trigger]
//! name="Level up"
//! description=""
//! category=1
//! event=TriggerRegisterAnyUnitEventBJ(EVENT_PLAYER_HERO_LEVEL)
//! condition=OperatorCompareInteger(GetHeroLevel(GetTriggerUnit()), OperatorEqual, 10)
//! action=IfThenElseMultiple()
//! >condition:0=OperatorCompareUnit(GetTriggerUnit(), OperatorEqualENE, $hero[GetConvertedPlayerId(GetOwningPlayer(GetTriggerUnit()))])
//! >action:1=DisplayTextToForce(GetPlayersAll(), "Level 10!")
//! >!action:2=DoNothing()
//! ```
//!
//! **Lines of a trigger.** The key is the kind (`event`, `condition`,
//! `action`), after a `!` when it's disabled; inside another (an
//! if/then/else, a loop) after a `>` per level, with `:` and which of its
//! blocks it's in (the parent's; for an if/then/else 0 is the condition,
//! 1 then, 2 else).
//!
//! **Parameters.** `Name(...)` a call (of a function returning a value;
//! `event:`, `condition:` or `action:` before it for one of those: an
//! `And` of conditions, an action run by another), `$name` a variable,
//! `"text"` a string ([`bni_rs::string`]: `"a"\n"b"`), a bare word a
//! preset (`PlayerALL`, `OperatorEqual`); `[index]` after any of them an
//! array's index. World Editor leaves the name of what a parameter was
//! before it became a call in it; when that isn't the call's name, it
//! follows as `~"DoNothing"`. `#n"value"` is a parameter of another kind
//! `n` (maps have `#-1""`).
//!
//! A trigger's flags are written only when they aren't the default:
//! `enabled=1`, the rest 0 (`custom_text`, `initially_off`,
//! `run_on_init`, `comment`); so are a variable's `unknown=1` and a
//! category's `comment=0`. `initial` is there when the variable has an
//! initial value, even an empty one.
//!
//! Reading the text needs no `TriggerData.txt`; given one, [`from_text`]
//! also checks every function's name and parameter count against it, as
//! the file can't be read back without.

use bni_rs::Document;
use bni_rs::ast::{Item, Section};
use bni_rs::string::quote;
use parser_rs::{ParseError, Span};

use crate::{Call, Category, Eca, FunctionKind, Param, Trigger, TriggerData, Triggers, Variable};

/// The first section.
pub const HEADER: &str = "wtg";

/// The ECA kinds' words, by kind.
const KINDS: [&str; 3] = ["event", "condition", "action"];

fn ident(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && c.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ---------------------------------------------------------------- writing

fn name(s: &str) -> Result<&str, String> {
    if ident(s) { Ok(s) } else { Err(format!("the function name {s:?} isn't a word")) }
}

fn args(params: &[Param]) -> Result<String, String> {
    Ok(params.iter().map(param).collect::<Result<Vec<_>, _>>()?.join(", "))
}

fn param(p: &Param) -> Result<String, String> {
    let mut out = match (p.kind, &p.call) {
        (2, Some(c)) => {
            if p.has_call != 1 || c.has_params != 1 {
                return Err(format!("{}: a call stored as {}, {}", c.name, p.has_call, c.has_params));
            }
            let prefix = match c.kind {
                0..=2 => format!("{}:", KINDS[c.kind as usize]),
                3 => String::new(),
                k => return Err(format!("{}: a call of kind {k}", c.name)),
            };
            let mut s = format!("{prefix}{}({})", name(&c.name)?, args(&c.params)?);
            if p.value != c.name {
                s += &format!(" ~{}", quote(&p.value));
            }
            s
        }
        (_, Some(c)) => return Err(format!("{}: a call in a parameter of kind {}", c.name, p.kind)),
        (_, None) if p.has_call != 0 => return Err(format!("{:?}: a parameter says it has a call and has none", p.value)),
        (0, None) if ident(&p.value) => p.value.clone(),
        (1, None) if ident(&p.value) => format!("${}", p.value),
        (3, None) => quote(&p.value),
        (k, None) => format!("#{}{}", k as i32, quote(&p.value)),
    };
    if p.after_call.is_some_and(|n| n != 0) {
        return Err(format!("{:?}: {} after its call", p.value, p.after_call.unwrap_or(0)));
    }
    match (p.is_array, &p.index) {
        (0, None) => {}
        (1, Some(i)) => out += &format!("[{}]", param(i)?),
        (n, _) => return Err(format!("{:?}: an array stored as {n}", p.value)),
    }
    Ok(out)
}

fn eca(out: &mut String, e: &Eca, depth: usize) -> Result<(), String> {
    let kind = KINDS.get(e.kind as usize).ok_or(format!("{}: an ECA of kind {}", e.name, e.kind))?;
    let off = match e.enabled {
        0 => "!",
        1 => "",
        n => return Err(format!("{}: enabled is {n}", e.name)),
    };
    let group = e.group.map(|g| format!(":{g}")).unwrap_or_default();
    *out += &format!("{}{off}{kind}{group}={}({})\n", ">".repeat(depth), name(&e.name)?, args(&e.params)?);
    for c in &e.children {
        eca(out, c, depth + 1)?;
    }
    Ok(())
}

/// The text form of `file`. Fails for what the text can't show: values
/// no map has (a flag of 2 where 0 or 1 go, a function name that isn't a
/// word).
pub fn to_text(f: &Triggers) -> Result<String, String> {
    let v7 = f.version == 7;
    let mut out = format!("[{HEADER}]\nversion={}\ngame={}\n", f.version, f.game);
    for c in &f.categories {
        out += &format!("\n[category]\nindex={}\nname={}\n", c.index, quote(&c.name));
        if c.is_comment != 0 {
            out += &format!("comment={}\n", c.is_comment);
        }
    }
    for v in &f.variables {
        out += &format!("\n[variable]\nname={}\ntype={}\n", quote(&v.name), quote(&v.kind));
        if v.unknown != 1 {
            out += &format!("unknown={}\n", v.unknown);
        }
        if v.is_array != 0 {
            out += &format!("array={}\n", v.is_array);
        }
        if v7 {
            out += &format!("size={}\n", v.array_size);
        }
        match v.has_initial {
            0 if v.initial.is_empty() => {}
            1 => out += &format!("initial={}\n", quote(&v.initial)),
            n => return Err(format!("variable {}: an initial value stored as {n}, {:?}", v.name, v.initial)),
        }
    }
    for t in &f.triggers {
        out += &format!("\n[trigger]\nname={}\ndescription={}\ncategory={}\n", quote(&t.name), quote(&t.description), t.category);
        for (key, v, default) in [
            ("comment", t.is_comment, 0),
            ("enabled", t.enabled, 1),
            ("custom_text", t.is_custom, 0),
            ("initially_off", t.initially_off, 0),
            ("run_on_init", t.run_on_init, 0),
        ] {
            if v != default {
                out += &format!("{key}={v}\n");
            }
        }
        for e in &t.ecas {
            eca(&mut out, e, 0).map_err(|m| format!("trigger {:?}: {m}", t.name))?;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- reading

/// Is `doc` a trigger file's text form?
pub fn is_trigger_text(doc: &Document) -> bool {
    doc.sections.first().is_some_and(|s| s.name.text == HEADER)
}

/// What [`from_text`] read: the file when the text has no errors, and
/// every error, in line order.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub file: Option<Triggers>,
    pub errors: Vec<ParseError>,
}

/// A value: what follows `=` on its line, read by hand (the calls'
/// syntax isn't BNI's).
struct Value<'a> {
    chars: Vec<char>,
    at: usize,
    v7: bool,
    data: Option<&'a TriggerData>,
}

type R<T> = Result<T, String>;

impl Value<'_> {
    fn ws(&mut self) {
        while self.chars.get(self.at).is_some_and(|c| c.is_whitespace()) {
            self.at += 1;
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.ws();
        self.chars.get(self.at).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: char, what: &str) -> R<()> {
        if self.eat(c) { Ok(()) } else { Err(format!("`{c}` expected {what}{}", self.here())) }
    }

    fn here(&self) -> String {
        match self.chars.get(self.at) {
            Some(_) => format!(" at `{}`", self.chars[self.at..].iter().take(16).collect::<String>()),
            None => " at the end".into(),
        }
    }

    /// The end: nothing more, or a `//` comment.
    fn end(&mut self) -> R<()> {
        match self.peek() {
            None => Ok(()),
            Some('/') if self.chars.get(self.at + 1) == Some(&'/') => Ok(()),
            Some(_) => Err(format!("nothing more expected{}", self.here())),
        }
    }

    fn word(&mut self) -> R<String> {
        self.ws();
        let start = self.at;
        while self.chars.get(self.at).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_') {
            self.at += 1;
        }
        let w: String = self.chars[start..self.at].iter().collect();
        if ident(&w) { Ok(w) } else { Err(format!("a name expected{}", self.here_at(start))) }
    }

    fn here_at(&self, at: usize) -> String {
        Value { chars: self.chars.clone(), at, v7: self.v7, data: None }.here()
    }

    fn int(&mut self) -> R<i64> {
        self.ws();
        let start = self.at;
        if self.chars.get(self.at) == Some(&'-') {
            self.at += 1;
        }
        while self.chars.get(self.at).is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        self.chars[start..self.at].iter().collect::<String>().parse().map_err(|_| format!("a whole number expected{}", self.here_at(start)))
    }

    /// A string: quoted parts and escapes between them, as
    /// [`bni_rs::string::quote`] writes.
    fn string(&mut self) -> R<String> {
        if self.peek() != Some('"') {
            return Err(format!("a string in quotes expected{}", self.here()));
        }
        let mut out = String::new();
        loop {
            match self.chars.get(self.at) {
                Some('"') => {
                    self.at += 1;
                    loop {
                        match self.chars.get(self.at) {
                            Some('"') => break,
                            Some(c) => out.push(*c),
                            None => return Err("a string without its closing `\"`".into()),
                        }
                        self.at += 1;
                    }
                    self.at += 1;
                }
                Some('\\') => {
                    let c = match self.chars.get(self.at + 1) {
                        Some('n') => '\n',
                        Some('r') => '\r',
                        Some('t') => '\t',
                        Some('u') if self.chars.get(self.at + 2) == Some(&'{') => {
                            let close = self.chars[self.at..].iter().position(|c| *c == '}').map(|p| p + self.at);
                            let code = close.and_then(|e| {
                                u32::from_str_radix(&self.chars[self.at + 3..e].iter().collect::<String>(), 16).ok()
                            });
                            match (close, code.and_then(char::from_u32)) {
                                (Some(e), Some(c)) => {
                                    out.push(c);
                                    self.at = e + 1;
                                    continue;
                                }
                                _ => return Err("`\\u{...}` holds a character's hex code".into()),
                            }
                        }
                        _ => return Err("the escapes between quoted parts are \\n \\r \\t \\u{...}".into()),
                    };
                    out.push(c);
                    self.at += 2;
                }
                _ => return Ok(out),
            }
        }
    }

    fn count(&self, kind: FunctionKind, name: &str, n: usize) -> R<()> {
        let Some(data) = self.data else { return Ok(()) };
        match data.params(kind, name) {
            Some(m) if m == n => Ok(()),
            Some(m) => Err(format!("`{name}` takes {m} parameters (UI\\TriggerData.txt), not {n}")),
            None => Err(format!("`{name}` isn't in UI\\TriggerData.txt [{}]", kind.section())),
        }
    }

    /// `(a, b, ...)`, after a name.
    fn args(&mut self) -> R<Vec<Param>> {
        self.expect('(', "after a function's name")?;
        let mut out = Vec::new();
        if self.eat(')') {
            return Ok(out);
        }
        loop {
            out.push(self.param()?);
            if self.eat(')') {
                return Ok(out);
            }
            self.expect(',', "between parameters")?;
        }
    }

    fn param(&mut self) -> R<Param> {
        let mut p = Param { kind: 0, value: String::new(), has_call: 0, call: None, after_call: None, is_array: 0, index: None };
        match self.peek() {
            Some('"') => (p.kind, p.value) = (3, self.string()?),
            Some('$') => {
                self.at += 1;
                (p.kind, p.value) = (1, self.word()?);
            }
            Some('#') => {
                self.at += 1;
                p.kind = self.int()? as u32;
                p.value = self.string()?;
            }
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                let mut word = self.word()?;
                let mut kind = 3;
                if self.chars.get(self.at) == Some(&':') {
                    let Some(k) = KINDS.iter().position(|k| *k == word) else {
                        return Err(format!("`{word}:`: before a call go event:, condition: and action:"));
                    };
                    self.at += 1;
                    kind = k as u32;
                    word = self.word()?;
                }
                if self.peek() == Some('(') {
                    let params = self.args()?;
                    self.count(FunctionKind::of(kind).unwrap_or(FunctionKind::Call), &word, params.len())?;
                    p.kind = 2;
                    p.has_call = 1;
                    p.value = if self.eat('~') { self.string()? } else { word.clone() };
                    p.call = Some(Call { kind, name: word, has_params: 1, params });
                    p.after_call = self.v7.then_some(0);
                } else if kind != 3 {
                    return Err(format!("`{}:{word}` is a call: `(` and its parameters", KINDS[kind as usize]));
                } else {
                    p.value = word;
                }
            }
            _ => {
                return Err(format!(
                    "a parameter expected{}: Name(...) a call, $name a variable, \"text\" a string (numbers too), a preset word, #kind\"value\"",
                    self.here()
                ));
            }
        }
        if self.eat('[') {
            p.is_array = 1;
            p.index = Some(Box::new(self.param()?));
            self.expect(']', "after an array's index")?;
        }
        Ok(p)
    }
}

/// What an ECA line's key says: its depth, whether it's enabled, its kind
/// and block.
fn eca_key(key: &str) -> Option<(usize, u32, u32, Option<u32>)> {
    let depth = key.len() - key.trim_start_matches('>').len();
    let rest = &key[depth..];
    let (enabled, rest) = match rest.strip_prefix('!') {
        Some(r) => (0, r),
        None => (1, rest),
    };
    let (kind, group) = match rest.split_once(':') {
        Some((k, g)) => (k, Some(g.parse().ok()?)),
        None => (rest, None),
    };
    let kind = KINDS.iter().position(|k| *k == kind)? as u32;
    Some((depth, enabled, kind, group))
}

struct Reader<'a> {
    lines: Vec<&'a str>,
    errors: Vec<ParseError>,
    v7: bool,
    data: Option<&'a TriggerData>,
}

impl<'a> Reader<'a> {
    fn err(&mut self, item: &Item, message: impl Into<String>) {
        self.errors.push(error(item_span(item), message));
    }

    fn value(&self, item: &Item) -> Value<'a> {
        let line = self.lines.get(item.line.wrapping_sub(1)).copied().unwrap_or("");
        let raw = line.split_once('=').map_or("", |(_, v)| v);
        Value { chars: raw.chars().collect(), at: 0, v7: self.v7, data: self.data }
    }

    /// Reads the item's value with `read`, which must read all of it.
    fn read<T>(&mut self, item: &Item, read: impl FnOnce(&mut Value) -> R<T>) -> Option<T> {
        let mut v = self.value(item);
        let key = item.key.as_ref().map_or(String::new(), |k| k.text.trim().to_string());
        match read(&mut v).and_then(|t| v.end().map(|_| t)) {
            Ok(t) => Some(t),
            Err(m) => {
                self.err(item, format!("`{key}`: {m}"));
                None
            }
        }
    }

    fn int<T: TryFrom<i64>>(&mut self, item: &Item) -> Option<T> {
        let n = self.read(item, |v| v.int())?;
        let t = T::try_from(n).ok();
        if t.is_none() {
            self.err(item, format!("{n} is out of range"));
        }
        t
    }

    fn string(&mut self, item: &Item) -> Option<String> {
        self.read(item, |v| v.string())
    }

    /// The section's items with a key, each with it; a duplicate scalar
    /// key is an error.
    fn items(&mut self, section: &'a Section, scalars: &[&str]) -> Vec<(String, &'a Item)> {
        let mut out = Vec::new();
        let mut seen = Vec::new();
        for item in &section.items {
            let Some(key) = item.key.as_ref().map(|k| k.text.trim().to_string()) else {
                self.err(item, "a field without a name");
                continue;
            };
            if scalars.contains(&key.as_str()) {
                if seen.contains(&key) {
                    self.err(item, format!("`{key}` is set twice"));
                    continue;
                }
                seen.push(key.clone());
            }
            out.push((key, item));
        }
        out
    }

    fn unknown(&mut self, item: &Item, key: &str, section: &str, known: &[&str]) {
        self.err(item, format!("no field `{key}` in [{section}]: it has {}", known.join(", ")));
    }

    fn v7_only(&mut self, item: &Item, key: &str) -> bool {
        if !self.v7 {
            self.err(item, format!("`{key}` is in version 7 (The Frozen Throne)"));
        }
        self.v7
    }
}

/// Reads the text form back; with `data`, checks every function against
/// it too.
pub fn from_text(text: &str, data: Option<&TriggerData>) -> Parsed {
    from_bni(&bni_rs::parse(text), text, data)
}

/// [`from_text`] of a document already parsed from `text`.
pub fn from_bni(doc: &Document, text: &str, data: Option<&TriggerData>) -> Parsed {
    let mut r = Reader { lines: text.lines().collect(), errors: doc.errors.clone(), v7: true, data };
    let mut file = Triggers { version: 7, categories: Vec::new(), game: 2, variables: Vec::new(), triggers: Vec::new() };
    for item in &doc.preamble {
        r.err(item, format!("a field outside a section: the file starts with [{HEADER}]"));
    }
    let Some((head, rest)) = doc.sections.split_first().filter(|(h, _)| h.name.text == HEADER) else {
        let span = doc.sections.first().map_or(Span::new(0, 0, 0, 1), |s| s.name.span);
        r.errors.push(error(span, format!("the file starts with [{HEADER}]")));
        return finish(r, file);
    };
    for (key, item) in r.items(head, &["version", "game"]) {
        match key.as_str() {
            "version" => match r.int::<u32>(item) {
                Some(v @ (4 | 7)) => file.version = v,
                Some(v) => r.err(item, format!("version {v}: 4 (Reign of Chaos) or 7 (The Frozen Throne)")),
                None => {}
            },
            "game" => file.game = r.int(item).unwrap_or(file.game),
            _ => r.unknown(item, &key, HEADER, &["version", "game"]),
        }
    }
    r.v7 = file.version == 7;
    for section in rest {
        match section.name.text.as_str() {
            "category" => {
                const KEYS: [&str; 3] = ["index", "name", "comment"];
                let mut c = Category { index: 0, name: String::new(), is_comment: 0 };
                for (key, item) in r.items(section, &KEYS) {
                    match key.as_str() {
                        "index" => c.index = r.int(item).unwrap_or(0),
                        "name" => c.name = r.string(item).unwrap_or_default(),
                        "comment" if r.v7_only(item, &key) => c.is_comment = r.int(item).unwrap_or(0),
                        "comment" => {}
                        _ => r.unknown(item, &key, "category", &KEYS),
                    }
                }
                file.categories.push(c);
            }
            "variable" => {
                const KEYS: [&str; 6] = ["name", "type", "unknown", "array", "size", "initial"];
                let mut v = Variable {
                    name: String::new(),
                    kind: String::new(),
                    unknown: 1,
                    is_array: 0,
                    array_size: 0,
                    has_initial: 0,
                    initial: String::new(),
                };
                for (key, item) in r.items(section, &KEYS) {
                    match key.as_str() {
                        "name" => v.name = r.string(item).unwrap_or_default(),
                        "type" => v.kind = r.string(item).unwrap_or_default(),
                        "unknown" => v.unknown = r.int(item).unwrap_or(1),
                        "array" => v.is_array = r.int(item).unwrap_or(0),
                        "size" if r.v7_only(item, &key) => v.array_size = r.int(item).unwrap_or(0),
                        "size" => {}
                        "initial" => {
                            v.has_initial = 1;
                            v.initial = r.string(item).unwrap_or_default();
                        }
                        _ => r.unknown(item, &key, "variable", &KEYS),
                    }
                }
                file.variables.push(v);
            }
            "trigger" => {
                const KEYS: [&str; 8] =
                    ["name", "description", "category", "comment", "enabled", "custom_text", "initially_off", "run_on_init"];
                let mut t = Trigger {
                    name: String::new(),
                    description: String::new(),
                    is_comment: 0,
                    enabled: 1,
                    is_custom: 0,
                    initially_off: 0,
                    run_on_init: 0,
                    category: 0,
                    ecas: Vec::new(),
                };
                // The ECAs being filled: the last one at each depth.
                let mut path: Vec<usize> = Vec::new();
                for (key, item) in r.items(section, &KEYS) {
                    match key.as_str() {
                        "name" => t.name = r.string(item).unwrap_or_default(),
                        "description" => t.description = r.string(item).unwrap_or_default(),
                        "category" => t.category = r.int(item).unwrap_or(0),
                        "comment" if r.v7_only(item, &key) => t.is_comment = r.int(item).unwrap_or(0),
                        "comment" => {}
                        "enabled" => t.enabled = r.int(item).unwrap_or(1),
                        "custom_text" => t.is_custom = r.int(item).unwrap_or(0),
                        "initially_off" => t.initially_off = r.int(item).unwrap_or(0),
                        "run_on_init" => t.run_on_init = r.int(item).unwrap_or(0),
                        _ => {
                            let Some((depth, enabled, kind, group)) = eca_key(&key) else {
                                r.err(item, format!(
                                    "no field `{key}` in [trigger]: it has {}, and event, condition, action lines (`!` before one disabled, `>` and `:block` inside another)",
                                    KEYS.join(", ")
                                ));
                                continue;
                            };
                            if depth > path.len() {
                                r.err(item, format!("{} `>` here: the line before is at {} levels", depth, path.len()));
                                continue;
                            }
                            if (depth > 0) != group.is_some() {
                                r.err(item, if depth > 0 {
                                    "a line inside another says its block: `:` and a number after its kind"
                                } else {
                                    "a line at the top has no block (`:`)"
                                });
                                continue;
                            }
                            if depth > 0 && !r.v7 {
                                r.err(item, "lines inside others are in version 7 (The Frozen Throne)");
                                continue;
                            }
                            let read = r.read(item, |v| {
                                let name = v.word()?;
                                let params = v.args()?;
                                v.count(FunctionKind::of(kind).unwrap_or(FunctionKind::Action), &name, params.len())?;
                                Ok((name, params))
                            });
                            let Some((name, params)) = read else { continue };
                            path.truncate(depth);
                            let e = Eca { kind, group, name, enabled, params, children: Vec::new() };
                            let mut list = &mut t.ecas;
                            for &i in &path {
                                list = &mut list[i].children;
                            }
                            list.push(e);
                            path.push(list.len() - 1);
                        }
                    }
                }
                file.triggers.push(t);
            }
            other => r.errors.push(error(
                section.name.span,
                format!("no section [{other}]: after [{HEADER}] come [category], [variable] and [trigger]"),
            )),
        }
    }
    finish(r, file)
}

fn finish(mut r: Reader, file: Triggers) -> Parsed {
    r.errors.sort_by_key(|e| (e.line, e.col));
    let file = r.errors.is_empty().then_some(file);
    Parsed { file, errors: r.errors }
}

fn error(span: Span, message: impl Into<String>) -> ParseError {
    let end_col = if span.end_line == span.start_line { span.end_col.max(span.start_col + 1) } else { span.start_col + 1 };
    ParseError { message: message.into(), line: span.start_line, col: span.start_col, end_col }
}

fn item_span(item: &Item) -> Span {
    let start = item.key.as_ref().map_or(item.eq, |k| k.span);
    let end = item.fields.last().map_or(item.eq, |f| f.span);
    Span::new(start.start_line, start.start_col, end.end_line, end.end_col)
}

#[cfg(test)]
#[path = "text.test.rs"]
mod tests;
