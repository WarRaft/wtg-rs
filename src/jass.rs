//! A trigger of its own as JASS: what a build puts in the map's script
//! for it, laid out as World Editor's triggers are (`Trig_<name>_Conditions`,
//! `Trig_<name>_Actions`, `InitTrig_<name>`), its file's variables
//! declared.
//!
//! - A function is called by its name, or its `_Name_ScriptName`; a
//!   preset is its code (`Player(0)`); a value is written as its type
//!   wants: a string in quotes, an object's id in single quotes
//!   (`'hfoo'`), a number as it is, `scriptcode` as it is. Custom script
//!   may be lines (`\n` in its string), each indented as the action is.
//! - A trigger is named in the script as World Editor names it
//!   ([`ident`]: `gg_trg_` and its name, `_` for each byte a JASS name
//!   can't have, a `u` after a `_` at the end).
//! - Variables: a global is `udg_<name>`, a private one
//!   `udg_<trigger>__<name>` (so that no other file's clashes), a local
//!   its name; `gg_...` (the map's triggers, regions, units ...) as it is.
//! - The actions are one function: the blocks World Editor runs as
//!   functions of their own are loops of it (`ForGroupMultiple` over a
//!   copy of the group, `ForForceMultiple` over the player slots,
//!   `Enum...Multiple` over what's gathered first), and *Picked unit* and
//!   the like are the loop's. So a trigger's locals are seen in them.
//! - What's disabled isn't there; a disabled trigger says so.

use std::fmt::Write as _;

use crate::text::{Declared, Scope, jass_string};
use crate::ui::Ui;
use crate::{Call, Eca, FunctionKind, Param, Trigger};

/// What the map's script names a trigger by (`gg_trg_<ident>`,
/// `Trig_<ident>_Actions` ...), as World Editor makes it of its name: each
/// byte (UTF-8) that isn't an ASCII letter, a digit or `_` is `_` (so a
/// Cyrillic letter is two), and a `u` after a `_` at the end, as a JASS
/// name can't end with one (`DOOR CLOSE 3 1 Копировать` is
/// `DOOR_CLOSE_3_1_____________________u`). A function of the name alone:
/// what a map's references say, kept as they are.
pub fn ident(name: &str) -> String {
    let mut out: String = name.bytes().map(|b| if b.is_ascii_alphanumeric() || b == b'_' { b as char } else { '_' }).collect();
    if out.is_empty() || out.ends_with('_') {
        out.push('u');
    }
    out
}

/// The JASS of a trigger file: its variables (`own`, what the file
/// declares) and its trigger, if it has one, named `name` (its `name=`, or
/// the file's when that's empty; in the script [`ident`] of it). `visible`
/// is every variable the file sees (its own and the project's globals),
/// for their types and names.
pub fn compile(trigger: Option<&Trigger>, name: &str, own: &[Declared], visible: &[Declared], ui: &Ui) -> String {
    let id = ident(trigger.map(|t| t.name.as_str()).filter(|n| !n.is_empty()).unwrap_or(name));
    let mut c = Compiler { ui, id, own, visible, helpers: Vec::new(), temps: Vec::new(), globals: Vec::new(), n: 0, loops: Vec::new() };
    let body = trigger.map(|t| c.trigger(t));
    let mut out = String::new();
    // The file's globals, and those its loops gather into.
    let globals: Vec<String> = own
        .iter()
        .filter(|d| d.scope != Scope::Local)
        .map(|d| c.declaration(d, false))
        .chain(c.globals.iter().cloned())
        .collect();
    if !globals.is_empty() {
        out += "globals\n";
        for g in &globals {
            let _ = writeln!(out, "    {g}");
        }
        out += "endglobals\n\n";
    }
    if let Some(body) = body {
        for h in &c.helpers {
            out += h;
            out += "\n";
        }
        out += &body;
    }
    if out.is_empty() {
        out = "// Nothing: no [trigger] and no variables.\n".into();
    }
    out
}

/// Which loop a block is, for the responses inside it.
#[derive(Clone)]
struct Loop {
    /// The response it stands for (`GetEnumUnit`).
    response: &'static str,
    /// The local holding the current one.
    var: String,
}

struct Compiler<'a> {
    ui: &'a Ui,
    id: String,
    own: &'a [Declared],
    visible: &'a [Declared],
    /// Functions the trigger needs before its own (an enum's gathering).
    helpers: Vec<String>,
    /// Locals the actions need for their loops: type and name.
    temps: Vec<String>,
    /// Globals the trigger needs (what an enum gathers).
    globals: Vec<String>,
    /// The loops made so far, to number their names.
    n: usize,
    /// The loops the statement being made is inside, innermost last.
    loops: Vec<Loop>,
}

impl Compiler<'_> {
    fn trig(&self) -> String {
        format!("gg_trg_{}", self.id)
    }

    /// A variable by the name the text has: its name in the script.
    fn var_name(&self, name: &str) -> String {
        if name.starts_with("gg_") {
            return name.to_string();
        }
        match self.own.iter().find(|d| d.variable.name == name).map(|d| d.scope) {
            Some(Scope::Local) => name.to_string(),
            Some(Scope::Private) => format!("udg_{}__{name}", self.id),
            _ => format!("udg_{name}"),
        }
    }

    /// The type of the variable `name`, as the editor names types.
    fn var_type(&self, name: &str) -> String {
        self.visible.iter().find(|d| d.variable.name == name).map_or(String::new(), |d| d.variable.kind.clone())
    }

    /// A type as JASS has it: its base for one of the editor's own.
    fn jass_type(&self, ty: &str) -> String {
        match self.ui.base(ty) {
            "" => "handle".into(),
            "integervar" => "integer".into(),
            b => b.to_string(),
        }
    }

    /// A variable's declaration: `integer udg_n = 0`, `unit array udg_u`;
    /// `local` before a local's.
    fn declaration(&self, d: &Declared, local: bool) -> String {
        let v = &d.variable;
        let ty = self.jass_type(&v.kind);
        let name = self.var_name(&v.name);
        let prefix = if local { "local " } else { "" };
        if v.is_array != 0 {
            return format!("{prefix}{ty} array {name}");
        }
        let init = if v.has_initial != 0 && !v.initial.is_empty() {
            Some(self.value(&v.initial, &v.kind))
        } else {
            // A global's default is set where the map starts; a call can't
            // be in `globals`.
            self.ui.type_defaults.get(&v.kind).filter(|d| local || !d.contains('(')).cloned()
        };
        match init {
            Some(i) => format!("{prefix}{ty} {name} = {i}"),
            None => format!("{prefix}{ty} {name}"),
        }
    }

    /// A value typed in, as its type `ty` wants it.
    fn value(&self, v: &str, ty: &str) -> String {
        let base = self.ui.base(ty);
        match () {
            _ if ty == "scriptcode" => v.to_string(),
            _ if base == "string" => jass_string(v),
            _ if base == "integer" && ty != "integer" && v.chars().count() == 4 && v.parse::<i64>().is_err() => format!("'{v}'"),
            _ if v.is_empty() && matches!(base, "integer" | "real") => "0".into(),
            _ if v.is_empty() => "null".into(),
            _ => v.to_string(),
        }
    }

    /// A parameter of type `ty` as an expression.
    fn expr(&self, p: &Param, ty: &str) -> String {
        let mut out = match (p.kind, &p.call) {
            (0, _) => self.ui.presets.get(&p.value).map_or(p.value.clone(), |pr| pr.code.clone()),
            (1, _) => self.var_name(&p.value),
            (2, Some(c)) => self.call(c),
            (3, _) => self.value(&p.value, ty),
            _ => p.value.clone(),
        };
        if let Some(i) = &p.index {
            out = format!("{out}[{}]", self.expr(i, "integer"));
        }
        out
    }

    fn function(&self, kind: u32, name: &str) -> Option<&crate::ui::Function> {
        self.ui.function(FunctionKind::of(kind).unwrap_or(FunctionKind::Call), name)
    }

    /// A function's name in the script.
    fn script_name(&self, kind: u32, name: &str) -> String {
        self.function(kind, name).map(|f| f.script.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| name.to_string())
    }

    /// The arguments of a call of `name` (of `kind`), each by its type.
    fn args(&self, kind: u32, name: &str, params: &[Param]) -> Vec<String> {
        let f = self.function(kind, name);
        params.iter().enumerate().map(|(i, p)| self.expr(p, f.and_then(|f| f.args.get(i)).map_or("", String::as_str))).collect()
    }

    /// A call as an expression: an operator's in parentheses, a
    /// condition's as one, a response inside a loop as the loop's.
    fn call(&self, c: &Call) -> String {
        if c.kind == 1 {
            return self.condition(&c.name, &c.params, &[]);
        }
        if let Some(l) = self.loops.iter().rev().find(|l| l.response == c.name) {
            return l.var.clone();
        }
        let a = self.args(c.kind, &c.name, &c.params);
        match (c.name.as_str(), a.as_slice()) {
            ("OperatorInt" | "OperatorReal", [x, op, y]) => format!("( {x} {op} {y} )"),
            ("OperatorString", [x, y]) => format!("( {x} + {y} )"),
            _ => format!("{}({})", self.script_name(c.kind, &c.name), a.join(", ")),
        }
    }

    /// A condition as a boolean expression: a comparison, `AndMultiple`
    /// and `OrMultiple` of the conditions inside (`children`), a call.
    fn condition(&self, name: &str, params: &[Param], children: &[Eca]) -> String {
        let a = self.args(1, name, params);
        if name.starts_with("OperatorCompare")
            && let [x, op, y] = a.as_slice()
        {
            return format!("( {x} {op} {y} )");
        }
        match name {
            "AndMultiple" => self.conditions(children, " and ", "true"),
            "OrMultiple" => self.conditions(children, " or ", "false"),
            _ => format!("{}({})", self.script_name(1, name), a.join(", ")),
        }
    }

    /// Conditions joined with `with`; `empty` for none.
    fn conditions(&self, ecas: &[Eca], with: &str, empty: &str) -> String {
        let parts: Vec<String> =
            ecas.iter().filter(|e| e.enabled != 0 && e.kind == 1).map(|e| self.condition(&e.name, &e.params, &e.children)).collect();
        if parts.is_empty() { empty.to_string() } else { format!("( {} )", parts.join(with)) }
    }

    /// A new number for a loop's names.
    fn next(&mut self) -> usize {
        self.n += 1;
        self.n
    }

    fn temp(&mut self, ty: &str, name: &str) {
        self.temps.push(format!("local {ty} {name}"));
    }

    /// Statements of actions, at `indent`.
    fn statements(&mut self, ecas: &[Eca], indent: usize, out: &mut String) {
        for e in ecas.iter().filter(|e| e.enabled != 0 && e.kind == 2) {
            self.statement(e, indent, out);
        }
    }

    fn statement(&mut self, e: &Eca, indent: usize, out: &mut String) {
        let pad = "    ".repeat(indent);
        let block = |e: &Eca, g: u32| -> Vec<Eca> { e.children.iter().filter(|c| c.group.unwrap_or(0) == g).cloned().collect() };
        let a = self.args(2, &e.name, &e.params);
        match (e.name.as_str(), a.as_slice()) {
            ("SetVariable", [var, _]) => {
                let ty = e.params.first().map_or(String::new(), |p| self.var_type(&p.value));
                let value = self.expr(&e.params[1], &ty);
                let _ = writeln!(out, "{pad}set {var} = {value}");
            }
            ("IfThenElseMultiple", _) => {
                let cond = self.conditions(&block(e, 0), " and ", "true");
                let _ = writeln!(out, "{pad}if {cond} then");
                self.statements(&block(e, 1), indent + 1, out);
                let otherwise = block(e, 2);
                if otherwise.iter().any(|c| c.enabled != 0) {
                    let _ = writeln!(out, "{pad}else");
                    self.statements(&otherwise, indent + 1, out);
                }
                let _ = writeln!(out, "{pad}endif");
            }
            ("IfThenElse", _) => {
                let cond = e.params.first().map_or("true".into(), |p| self.expr(p, "boolexpr"));
                let _ = writeln!(out, "{pad}if ( {cond} ) then");
                self.inline_action(e.params.get(1), indent + 1, out);
                let _ = writeln!(out, "{pad}else");
                self.inline_action(e.params.get(2), indent + 1, out);
                let _ = writeln!(out, "{pad}endif");
            }
            ("ForLoopAMultiple" | "ForLoopBMultiple" | "ForLoopA" | "ForLoopB", [from, to, ..]) => {
                let i = if e.name.starts_with("ForLoopA") { "bj_forLoopAIndex" } else { "bj_forLoopBIndex" };
                let _ = writeln!(out, "{pad}set {i} = {from}\n{pad}set {i}End = {to}\n{pad}loop\n{pad}    exitwhen {i} > {i}End");
                match e.name.ends_with("Multiple") {
                    true => self.statements(&block(e, 0), indent + 1, out),
                    false => self.inline_action(e.params.get(2), indent + 1, out),
                }
                let _ = writeln!(out, "{pad}    set {i} = {i} + 1\n{pad}endloop");
            }
            ("ForLoopVarMultiple" | "ForLoopVar", [var, from, to, ..]) => {
                let _ = writeln!(out, "{pad}set {var} = {from}\n{pad}loop\n{pad}    exitwhen {var} > {to}");
                match e.name.ends_with("Multiple") {
                    true => self.statements(&block(e, 0), indent + 1, out),
                    false => self.inline_action(e.params.get(3), indent + 1, out),
                }
                let _ = writeln!(out, "{pad}    set {var} = {var} + 1\n{pad}endloop");
            }
            ("ForGroupMultiple", [group]) => {
                let n = self.next();
                let (g, u) = (format!("l__g{n}"), format!("l__u{n}"));
                self.temp("group", &g);
                self.temp("unit", &u);
                // A group a call makes is the copy; one that's kept is
                // copied, so the loop can empty it.
                if e.params[0].kind == 2 {
                    let _ = writeln!(out, "{pad}set {g} = {group}");
                } else {
                    let _ = writeln!(out, "{pad}set {g} = CreateGroup()\n{pad}call GroupAddGroup({group}, {g})");
                }
                let _ = writeln!(out, "{pad}loop\n{pad}    set {u} = FirstOfGroup({g})\n{pad}    exitwhen {u} == null\n{pad}    call GroupRemoveUnit({g}, {u})");
                self.loops.push(Loop { response: "GetEnumUnit", var: u });
                self.statements(&block(e, 0), indent + 1, out);
                self.loops.pop();
                let _ = writeln!(out, "{pad}endloop\n{pad}call DestroyGroup({g})");
            }
            ("ForForceMultiple", [force]) => {
                let n = self.next();
                let (f, i, p) = (format!("l__f{n}"), format!("l__i{n}"), format!("l__p{n}"));
                self.temp("force", &f);
                self.temp("integer", &i);
                self.temp("player", &p);
                let _ = writeln!(out, "{pad}set {f} = {force}\n{pad}set {i} = 0\n{pad}loop\n{pad}    exitwhen {i} >= bj_MAX_PLAYER_SLOTS");
                let _ = writeln!(out, "{pad}    set {p} = Player({i})\n{pad}    if IsPlayerInForce({p}, {f}) then");
                self.loops.push(Loop { response: "GetEnumPlayer", var: p });
                self.statements(&block(e, 0), indent + 2, out);
                self.loops.pop();
                let _ = writeln!(out, "{pad}    endif\n{pad}    set {i} = {i} + 1\n{pad}endloop");
            }
            ("EnumItemsInRectBJMultiple" | "EnumDestructablesInRectAllMultiple" | "EnumDestructablesInCircleBJMultiple", _) => {
                let n = self.next();
                let (ty, response) =
                    if e.name.starts_with("EnumItems") { ("item", "GetEnumItem") } else { ("destructable", "GetEnumDestructable") };
                let (all, count) = (format!("trg__{}_e{n}", self.id), format!("trg__{}_n{n}", self.id));
                let gather = format!("Trig_{}_Gather{n}", self.id);
                self.globals.push(format!("{ty} array {all}"));
                self.globals.push(format!("integer {count} = 0"));
                self.helpers.push(format!(
                    "function {gather} takes nothing returns nothing\n    set {all}[{count}] = {response}()\n    set {count} = {count} + 1\nendfunction\n"
                ));
                let (i, x) = (format!("l__i{n}"), format!("l__e{n}"));
                self.temp("integer", &i);
                self.temp(ty, &x);
                let _ = writeln!(out, "{pad}set {count} = 0");
                let _ = writeln!(out, "{pad}call {}({}{}function {gather})", self.script_name(2, &e.name), a.join(", "), if a.is_empty() { "" } else { ", " });
                let _ = writeln!(out, "{pad}set {i} = 0\n{pad}loop\n{pad}    exitwhen {i} >= {count}\n{pad}    set {x} = {all}[{i}]");
                self.loops.push(Loop { response, var: x });
                self.statements(&block(e, 0), indent + 1, out);
                self.loops.pop();
                let _ = writeln!(out, "{pad}    set {i} = {i} + 1\n{pad}endloop");
            }
            ("CommentString", _) => {
                let text = e.params.first().map_or("", |p| p.value.as_str());
                let _ = writeln!(out, "{pad}// {text}");
            }
            ("CustomScriptCode", _) => {
                // Lines of their own (`\n` in the text), each indented.
                let code = e.params.first().map_or("", |p| p.value.as_str());
                for line in code.lines() {
                    let _ = writeln!(out, "{pad}{}", line.trim_end());
                }
            }
            ("ReturnAction" | "SkipRemainingActions", _) => {
                let _ = writeln!(out, "{pad}return");
            }
            _ => {
                let _ = writeln!(out, "{pad}call {}({})", self.script_name(2, &e.name), a.join(", "));
            }
        }
    }

    /// An action given as a parameter (`IfThenElse`'s, `ForLoopA`'s).
    fn inline_action(&mut self, p: Option<&Param>, indent: usize, out: &mut String) {
        let pad = "    ".repeat(indent);
        match p.and_then(|p| p.call.as_ref()) {
            Some(c) => {
                let e = Eca { kind: 2, group: None, name: c.name.clone(), enabled: 1, params: c.params.clone(), children: Vec::new() };
                self.statement(&e, indent, out);
            }
            None => {
                let _ = writeln!(out, "{pad}call DoNothing()");
            }
        }
    }

    fn trigger(&mut self, t: &Trigger) -> String {
        let id = self.id.clone();
        let trig = self.trig();
        let mut out = String::new();
        let rule = "//===========================================================================";
        let _ = writeln!(out, "{rule}\n// Trigger: {}", if t.name.is_empty() { &id } else { &t.name });
        for line in t.description.lines() {
            let _ = writeln!(out, "// {line}");
        }
        if t.enabled == 0 {
            out += "//\n// Disabled: not in the map's script.\n";
        }
        let _ = writeln!(out, "{rule}");

        // Conditions.
        let conditions: Vec<&Eca> = t.ecas.iter().filter(|e| e.kind == 1 && e.enabled != 0).collect();
        if !conditions.is_empty() {
            let _ = writeln!(out, "function Trig_{id}_Conditions takes nothing returns boolean");
            for e in conditions {
                let _ = writeln!(out, "    if ( not {} ) then\n        return false\n    endif", self.condition(&e.name, &e.params, &e.children));
            }
            out += "    return true\nendfunction\n\n";
        }

        // Actions: the statements first, for the locals their loops need.
        let mut body = String::new();
        self.statements(&t.ecas, 1, &mut body);
        let _ = writeln!(out, "function Trig_{id}_Actions takes nothing returns nothing");
        let locals: Vec<String> =
            self.own.iter().filter(|d| d.scope == Scope::Local).map(|d| self.declaration(d, true)).chain(self.temps.iter().cloned()).collect();
        for l in &locals {
            let _ = writeln!(out, "    {l}");
        }
        out += &body;
        out += "endfunction\n\n";

        // Its registration.
        let _ = writeln!(out, "{rule}\nfunction InitTrig_{id} takes nothing returns nothing\n    set {trig} = CreateTrigger()");
        for e in t.ecas.iter().filter(|e| e.kind == 0 && e.enabled != 0) {
            if e.name == "MapInitializationEvent" {
                out += "    // Map initialization: run when the map starts (RunInitializationTriggers).\n";
                continue;
            }
            let a = self.args(0, &e.name, &e.params);
            let args = std::iter::once(trig.clone()).chain(a).collect::<Vec<_>>().join(", ");
            let _ = writeln!(out, "    call {}({args})", self.script_name(0, &e.name));
        }
        if t.ecas.iter().any(|e| e.kind == 1 && e.enabled != 0) {
            let _ = writeln!(out, "    call TriggerAddCondition({trig}, Condition(function Trig_{id}_Conditions))");
        }
        if t.initially_off != 0 {
            let _ = writeln!(out, "    call DisableTrigger({trig})");
        }
        let _ = writeln!(out, "    call TriggerAddAction({trig}, function Trig_{id}_Actions)\nendfunction");
        out
    }
}

#[cfg(test)]
#[path = "jass.test.rs"]
mod tests;
