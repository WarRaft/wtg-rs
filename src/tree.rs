//! A 1.31 file ([`Tree`]) as a classic one ([`Triggers`]) and what its
//! tree has beyond it ([`TreeExtra`]): the ids, the parents, the order of
//! the elements, the header's numbers. Everything made for the classic
//! layout (the text's sections, an editor's operations, a build) works on
//! the `Triggers`; [`join`] makes the tree again, so a file read and joined
//! unchanged is the same bytes.
//!
//! In the `Triggers`: a category's `index` is its id, a trigger's
//! `category` its parent's id (a category's, or the map's at the top), a
//! comment trigger `is_comment`; the variables are the file's list, in its
//! order. A variable element and its definition are one (the tree lists
//! them in the same order, by the same id and name: [`split`] says when a
//! file doesn't).

use crate::{Category, Element, Trigger, Triggers, Tree, Variable};

/// What a 1.31 file has beyond a classic one's.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreeExtra {
    /// [`Tree::unknown_a`].
    pub unknown_a: [u32; 4],
    /// [`Tree::counts`], as read.
    pub counts: [(u32, Vec<u32>); 5],
    /// How many categories, triggers, comments, scripts and variables
    /// there were when read: a count changes by as many as were added or
    /// deleted since.
    pub numbers: [u32; 5],
    pub unknown_b: [u32; 2],
    /// [`Tree::element_count`] past one more than the elements (0 for
    /// World Editor's).
    pub element_count_past: i64,
    /// The map's element: [`Tree::unknown_c`], its name, `unknown_d`,
    /// `unknown_e`.
    pub map: MapElement,
    /// Each category's (in the `Triggers`' order): open in the editor, its
    /// parent's id; `None` for a new one (closed, at the top).
    pub categories: Vec<Option<(u32, u32)>>,
    /// Each variable's: its id, its parent's; `None` for a new one.
    pub variables: Vec<Option<(u32, u32)>>,
    /// Each trigger's: its id, its element's type (8 a trigger, 16 a
    /// comment, 32 a script); `None` for a new one.
    pub triggers: Vec<Option<(u32, u32)>>,
    /// The elements in their order: which list, and the place in it.
    pub order: Vec<(Kind, usize)>,
}

/// The tree's first element, the map (type 1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MapElement {
    /// Its type (1) and id.
    pub unknown_c: [u32; 2],
    pub name: String,
    /// (Version 7) is a comment, open; else open, its parent.
    pub unknown_d: [u32; 2],
    /// (Version 7) its parent.
    pub unknown_e: u32,
}

impl MapElement {
    /// Its id: what's at the top has it as its parent.
    pub fn id(&self) -> u32 {
        self.unknown_c[1]
    }
}

/// An element's list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Category,
    Trigger,
    Variable,
}

/// The type of a trigger's element as World Editor writes it: a comment
/// 16, custom text 32, else 8.
pub fn trigger_kind(t: &Trigger) -> u32 {
    if t.is_comment != 0 {
        16
    } else if t.is_custom != 0 {
        32
    } else {
        8
    }
}

/// `tree` as a classic file and what it has beyond it; an error when its
/// variables' elements aren't its variables, one for one in order.
pub fn split(tree: &Tree) -> Result<(Triggers, TreeExtra), String> {
    let mut file = Triggers { version: tree.version, categories: Vec::new(), game: tree.game, variables: Vec::new(), triggers: Vec::new() };
    let mut x = TreeExtra {
        unknown_a: tree.unknown_a,
        counts: tree.counts.clone(),
        unknown_b: tree.unknown_b,
        element_count_past: tree.element_count as i64 - (tree.elements.len() as i64 + 1),
        map: MapElement { unknown_c: tree.unknown_c, name: tree.map_name.clone(), unknown_d: tree.unknown_d, unknown_e: tree.unknown_e },
        ..Default::default()
    };
    for (v, id, parent) in &tree.variables {
        file.variables.push(v.clone());
        x.variables.push(Some((*id, *parent)));
    }
    let mut seen_variables = 0;
    for e in &tree.elements {
        match e {
            Element::Category { id, name, is_comment, has_children, parent } => {
                x.order.push((Kind::Category, file.categories.len()));
                file.categories.push(Category { index: *id as i32, name: name.clone(), is_comment: *is_comment });
                x.categories.push(Some((*has_children, *parent)));
            }
            Element::Trigger { kind, trigger, id, parent } => {
                x.order.push((Kind::Trigger, file.triggers.len()));
                let mut t = trigger.clone();
                t.category = *parent as i32;
                file.triggers.push(t);
                x.triggers.push(Some((*id, *kind)));
            }
            Element::Variable { id, name, parent } => {
                let k = seen_variables;
                match tree.variables.get(k) {
                    Some((v, vid, vparent)) if v.name == *name && vid == id && vparent == parent => {}
                    _ => return Err(format!("the tree's variable {name:?} (id {id}) isn't its list's {}th: the text can't show it", k + 1)),
                }
                x.order.push((Kind::Variable, k));
                seen_variables += 1;
            }
        }
    }
    if seen_variables != tree.variables.len() {
        return Err(format!("the tree has {seen_variables} of its {} variables: the text can't show it", tree.variables.len()));
    }
    x.numbers = numbers(&file);
    Ok((file, x))
}

/// How many categories, triggers, comments, scripts and variables `f` has,
/// as a 1.31 file counts them.
fn numbers(f: &Triggers) -> [u32; 5] {
    let of = |k: u32| f.triggers.iter().filter(|t| trigger_kind(t) == k).count() as u32;
    [f.categories.len() as u32, of(8), of(16), of(32), f.variables.len() as u32]
}

/// The tree of `file` and `x` (what [`split`] gave, or what the text
/// read): what's new (no id in `x`) gets the next id of its kind, and goes
/// at the end of the order; what's gone is gone. A trigger's parent is its
/// `category`.
pub fn join(file: &Triggers, x: &TreeExtra) -> Tree {
    let next = |ids: &mut dyn Iterator<Item = u32>| ids.max().map_or(0, |m| m + 1);
    let mut var_next = next(&mut x.variables.iter().flatten().map(|v| v.0));
    // A new trigger's id: the next of its type's (16 a comment ...), as
    // World Editor's ids are of their type; the next of all for the first.
    let mut taken: Vec<(u32, u32)> = x.triggers.iter().flatten().copied().collect();
    let top = x.map.id();
    let category = |k: usize| x.categories.get(k).copied().flatten().unwrap_or((0, top));
    let mut variable_ids = Vec::new();
    let variables: Vec<(Variable, u32, u32)> = file
        .variables
        .iter()
        .enumerate()
        .map(|(k, v)| {
            let (id, parent) = x.variables.get(k).copied().flatten().unwrap_or_else(|| {
                var_next += 1;
                (var_next - 1, top)
            });
            variable_ids.push((id, parent));
            (v.clone(), id, parent)
        })
        .collect();
    let mut trigger = |k: usize| -> Element {
        let t = &file.triggers[k];
        let (id, kind) = x.triggers.get(k).copied().flatten().unwrap_or_else(|| {
            let kind = trigger_kind(t);
            let of_kind = taken.iter().filter(|(_, k)| *k == kind).map(|(id, _)| *id).max();
            let id = of_kind.or_else(|| taken.iter().map(|(id, _)| *id).max()).map_or(0, |m| m + 1);
            taken.push((id, kind));
            (id, kind)
        });
        Element::Trigger { kind, trigger: t.clone(), id, parent: t.category as u32 }
    };
    // The order read, of what's still there; then what's new.
    let mut elements = Vec::new();
    let (mut used_c, mut used_t, mut used_v) = (vec![false; file.categories.len()], vec![false; file.triggers.len()], vec![false; file.variables.len()]);
    let mut push = |kind: Kind, k: usize, elements: &mut Vec<Element>| match kind {
        Kind::Category => {
            let c = &file.categories[k];
            let (open, parent) = category(k);
            elements.push(Element::Category { id: c.index as u32, name: c.name.clone(), is_comment: c.is_comment, has_children: open, parent });
        }
        Kind::Trigger => elements.push(trigger(k)),
        Kind::Variable => {
            let (id, parent) = variable_ids[k];
            elements.push(Element::Variable { id, name: file.variables[k].name.clone(), parent });
        }
    };
    for &(kind, k) in &x.order {
        let used = match kind {
            Kind::Category => &mut used_c,
            Kind::Trigger => &mut used_t,
            Kind::Variable => &mut used_v,
        };
        if k < used.len() && !used[k] {
            used[k] = true;
            push(kind, k, &mut elements);
        }
    }
    for (kind, used) in [(Kind::Category, used_c), (Kind::Variable, used_v), (Kind::Trigger, used_t)] {
        for (k, u) in used.into_iter().enumerate() {
            if !u {
                push(kind, k, &mut elements);
            }
        }
    }
    // The counts: as read, by as many as were added or deleted since.
    let now = numbers(file);
    let mut counts = x.counts.clone();
    for (k, c) in counts.iter_mut().enumerate() {
        c.0 = (c.0 as i64 + now[k] as i64 - x.numbers[k] as i64).max(0) as u32;
    }
    Tree {
        version: file.version,
        unknown_a: x.unknown_a,
        counts,
        unknown_b: x.unknown_b,
        game: file.game,
        variables,
        element_count: (elements.len() as i64 + 1 + x.element_count_past).max(0) as u32,
        unknown_c: x.map.unknown_c,
        map_name: x.map.name.clone(),
        unknown_d: x.map.unknown_d,
        unknown_e: x.map.unknown_e,
        elements,
    }
}

#[cfg(test)]
#[path = "tree.test.rs"]
mod tests;
