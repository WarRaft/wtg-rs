use super::*;
use crate::tests::sample;

/// A 1.31 tree: a category, a trigger in it, a variable in it, a comment
/// at the top.
fn tree() -> Tree {
    let t = sample().triggers.remove(0);
    let mut comment = t.clone();
    comment.name = "Read me".into();
    comment.is_comment = 1;
    comment.ecas.clear();
    Tree {
        version: 7,
        unknown_a: [1, 0, 0, 0],
        counts: [(1, vec![]), (1, vec![3]), (1, vec![]), (0, vec![]), (1, vec![])],
        unknown_b: [0, 0],
        game: 2,
        variables: vec![(sample().variables.remove(0), 0x0C00_0000, 0x0200_0000)],
        element_count: 5,
        unknown_c: [1, 0],
        map_name: "map.w3x".into(),
        unknown_d: [0, 1],
        unknown_e: 0,
        elements: vec![
            Element::Category { id: 0x0200_0000, name: "Init".into(), is_comment: 0, has_children: 1, parent: 0 },
            Element::Trigger { kind: 8, trigger: Trigger { category: 0x0200_0000, ..t }, id: 0x0300_0000, parent: 0x0200_0000 },
            Element::Variable { id: 0x0C00_0000, name: "hero".into(), parent: 0x0200_0000 },
            Element::Trigger { kind: 16, trigger: Trigger { category: 0, ..comment }, id: 0x0400_0000, parent: 0 },
        ],
    }
}

#[test]
fn split_and_joined_is_the_tree() {
    let t = tree();
    let (file, x) = split(&t).unwrap();
    // A classic file: a category by its id, a trigger by its parent's.
    assert_eq!(file.categories[0].index, 0x0200_0000);
    assert_eq!((file.triggers[0].category, file.triggers[1].category), (0x0200_0000, 0));
    assert_eq!(x.order, [(Kind::Category, 0), (Kind::Trigger, 0), (Kind::Variable, 0), (Kind::Trigger, 1)]);
    assert_eq!(join(&file, &x), t);
    assert_eq!(join(&file, &x).to_bytes().unwrap(), t.to_bytes().unwrap());
}

#[test]
fn what_changed_joins_too() {
    let (mut file, mut x) = split(&tree()).unwrap();
    // The comment gone, a new trigger in the category in its place (as the
    // text reads it: no id).
    let mut new = file.triggers[0].clone();
    new.name = "New".into();
    file.triggers[1] = new;
    x.triggers[1] = None;
    let joined = join(&file, &x);
    let ids: Vec<(u32, u32, u32)> = joined
        .elements
        .iter()
        .filter_map(|e| if let Element::Trigger { kind, id, parent, .. } = e { Some((*kind, *id, *parent)) } else { None })
        .collect();
    // The kept one as it was; the new one an id after the last.
    assert_eq!(ids, [(8, 0x0300_0000, 0x0200_0000), (8, 0x0300_0001, 0x0200_0000)]);
    // One more trigger, one comment less: so are their counts.
    assert_eq!(joined.counts.iter().map(|c| c.0).collect::<Vec<_>>(), [1, 2, 0, 0, 1]);
    assert_eq!(joined.element_count, 5);
}

#[test]
fn a_variable_not_its_lists_isnt_shown() {
    let mut t = tree();
    t.variables[0].2 = 0;
    assert!(split(&t).unwrap_err().contains("hero"));
}

#[test]
fn the_text_of_a_tree_and_back() {
    let t = tree();
    let text = crate::text::to_text_tree(&t).unwrap();
    // The sections in the tree's order, the classic form's keys and the tree's.
    assert!(text.starts_with("[wtg]\nversion=7\ngame=2\ntree=1\nmap=\"map.w3x\"\nmap_element=1,0,0,1,0\ndeleted_trigger=3\n\n[category]\nindex=33554432\nname=\"Init\"\nopen=1\n\n[trigger]\n"), "{text}");
    assert!(text.contains("category=33554432\nid=50331648\n"), "{text}");
    assert!(text.contains("[variable]\nname=\"hero\"") && text.contains("id=201326592\nparent=33554432\n"), "{text}");
    assert!(text.contains("category=0\nid=67108864\ncomment=1\n"), "{text}");
    let p = crate::text::from_text(&text, None);
    assert_eq!(p.errors, []);
    let back = p.to_file().unwrap();
    assert_eq!(back, crate::File::Tree(Box::new(t.clone())));
    assert_eq!(back.to_bytes().unwrap(), t.to_bytes().unwrap());
    // A trigger added in the text (no id): an id of its own when saved.
    let more = format!("{text}\n[trigger]\nname=\"New\"\ndescription=\"\"\ncategory=33554432\n");
    let p = crate::text::from_text(&more, None);
    assert_eq!(p.errors, []);
    let crate::File::Tree(joined) = p.to_file().unwrap() else { panic!() };
    assert!(matches!(joined.elements.last(), Some(Element::Trigger { kind: 8, id: 0x0300_0001, parent: 0x0200_0000, .. })), "{:?}", joined.elements.last());
}

#[test]
fn a_trigger_removed_from_both() {
    let (mut file, mut x) = split(&tree()).unwrap();
    file.triggers.remove(0);
    x.remove_trigger(0);
    let joined = join(&file, &x);
    // The comment kept, its id as it was; the order without the trigger.
    assert_eq!(joined.elements.len(), 3);
    assert!(matches!(joined.elements[2], Element::Trigger { kind: 16, id: 0x0400_0000, .. }), "{:?}", joined.elements);
    assert_eq!(joined.counts[1].0, 0);
}
