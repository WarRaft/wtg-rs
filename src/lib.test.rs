use super::*;

/// Every map's triggers read and write back, with the game's
/// `TriggerData.txt` (`WTG_DATA`): `WTG_CORPUS` is a folder of
/// `<map>/war3map.wtg`.
#[test]
#[ignore]
fn corpus() {
    let data = TriggerData::parse(&std::fs::read_to_string(std::env::var("WTG_DATA").expect("WTG_DATA")).unwrap());
    let dir = std::env::var("WTG_CORPUS").expect("WTG_CORPUS");
    let (mut n, mut triggers, mut ecas, mut bad) = (0, 0, 0, Vec::new());
    fn count(e: &[Eca]) -> usize {
        e.iter().map(|e| 1 + count(&e.children)).sum()
    }
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path().join("war3map.wtg");
        let Ok(bytes) = std::fs::read(&path) else { continue };
        n += 1;
        match parse(&bytes, &data) {
            Ok(f) if f.to_bytes().unwrap() == bytes => {
                match text::to_text(&f) {
                    Ok(t) => {
                        let p = text::from_text(&t, Some(&data));
                        match p.file {
                            Some(back) if back.to_bytes().unwrap() == bytes => {}
                            Some(_) => bad.push(format!("{}: differs through text", path.display())),
                            None => bad.push(format!("{}: text: {:?}", path.display(), &p.errors[..p.errors.len().min(3)])),
                        }
                    }
                    Err(e) => bad.push(format!("{}: no text: {e}", path.display())),
                }
                triggers += f.triggers.len();
                ecas += f.triggers.iter().map(|t| count(&t.ecas)).sum::<usize>();
            }
            Ok(_) => bad.push(format!("{}: differs", path.display())),
            Err(e) => bad.push(format!("{}: {e}", path.display())),
        }
    }
    println!("{n} files, {triggers} triggers, {ecas} ECAs; TriggerData has {} functions", data.len());
    assert!(bad.is_empty(), "{} bad:\n{}", bad.len(), bad.join("\n"));
}


pub(crate) const DATA: &str = "[TriggerEvents]\nMapInitializationEvent=0,nothing\nTriggerRegisterTimerEventPeriodic=0,real\n\
[TriggerConditions]\nOperatorCompareInteger=0,integer,ComparisonOperator,integer\n\
[TriggerActions]\nIfThenElseMultiple=0,nothing\nDoNothing=0,nothing\nSetVariable=0,AnyGlobal,Null\n_SetVariable_Defaults=x\n\
[TriggerCalls]\nGetUnitLevel=0,0,integer,unit\nGetTriggerUnit=0,0,unit\n";

pub(crate) fn param(kind: u32, value: &str) -> Param {
    Param { kind, value: value.into(), has_call: 0, call: None, after_call: None, is_array: 0, index: None }
}

pub(crate) fn call(kind: u32, name: &str, params: Vec<Param>, v7: bool) -> Param {
    Param {
        kind: 2,
        value: name.into(),
        has_call: 1,
        call: Some(Call { kind, name: name.into(), has_params: 1, params }),
        after_call: v7.then_some(0),
        is_array: 0,
        index: None,
    }
}

pub(crate) fn sample() -> Triggers {
    let eca = |kind, group, name: &str, params, children| Eca { kind, group, name: name.into(), enabled: 1, params, children };
    let mut level = call(3, "GetUnitLevel", vec![call(3, "GetTriggerUnit", vec![], true)], true);
    level.value = String::new();
    let mut hero = param(1, "hero");
    hero.is_array = 1;
    hero.index = Some(Box::new(param(3, "1")));
    let mut off = eca(2, Some(2), "DoNothing", vec![], vec![]);
    off.enabled = 0;
    Triggers {
        version: 7,
        categories: vec![Category { index: 0, name: "Init".into(), is_comment: 0 }, Category { index: 1, name: "--".into(), is_comment: 1 }],
        game: 2,
        variables: vec![
            Variable { name: "hero".into(), kind: "unit".into(), unknown: 1, is_array: 1, array_size: 12, has_initial: 0, initial: String::new() },
            Variable { name: "n".into(), kind: "integer".into(), unknown: 1, is_array: 0, array_size: 1, has_initial: 1, initial: "5".into() },
        ],
        triggers: vec![Trigger {
            name: "Say \"hi\"".into(),
            description: "line one\nline two, with a comma".into(),
            is_comment: 0,
            enabled: 1,
            is_custom: 0,
            initially_off: 1,
            run_on_init: 0,
            category: 0,
            ecas: vec![
                eca(0, None, "MapInitializationEvent", vec![], vec![]),
                eca(0, None, "TriggerRegisterTimerEventPeriodic", vec![param(3, "2.00")], vec![]),
                eca(2, None, "IfThenElseMultiple", vec![], vec![
                    eca(1, Some(0), "OperatorCompareInteger", vec![level, param(0, "OperatorEqualENE"), param(u32::MAX, "")], vec![]),
                    eca(2, Some(1), "SetVariable", vec![hero, param(3, "x")], vec![]),
                    off,
                ]),
            ],
        }],
    }
}

#[test]
fn reads_and_writes_back() {
    let data = TriggerData::parse(DATA);
    assert_eq!(data.len(), 8);
    assert_eq!(data.params(FunctionKind::Condition, "OperatorCompareInteger"), Some(3));
    assert_eq!(data.params(FunctionKind::Call, "GetTriggerUnit"), Some(0));
    assert_eq!(data.params(FunctionKind::Action, "_SetVariable_Defaults"), None);
    let f = sample();
    let bytes = f.to_bytes().unwrap();
    assert_eq!(parse(&bytes, &data).unwrap(), f);
    assert_eq!(parse_any(&bytes, &data).unwrap(), File::Classic(f));
    // A function the data doesn't have: the file can't be read.
    let err = parse(&bytes, &TriggerData::parse("[TriggerEvents]\n")).unwrap_err().to_string();
    assert!(err.contains("`MapInitializationEvent` isn't in UI\\TriggerData.txt [TriggerEvents]"), "{err}");
    assert!(parse(b"WTG!\x05\0\0\0", &data).unwrap_err().to_string().contains("unknown wtg version 5"));
}

#[test]
fn the_131_tree_reads_and_writes_back() {
    let data = TriggerData::parse(DATA);
    let t = sample().triggers.remove(0);
    let tree = Tree {
        version: 7,
        unknown_a: [1, 0, 0, 0],
        counts: [(1, vec![]), (1, vec![3]), (0, vec![]), (0, vec![]), (1, vec![])],
        unknown_b: [0, 0],
        game: 2,
        variables: vec![(sample().variables.remove(0), 0x0C00_0000, 0)],
        element_count: 4,
        unknown_c: [0, 0],
        map_name: "map.w3x".into(),
        unknown_d: [0, 0],
        unknown_e: 0,
        elements: vec![
            Element::Category { id: 0x0200_0000, name: "Init".into(), is_comment: 0, has_children: 1, parent: 0 },
            Element::Trigger { kind: 8, trigger: Trigger { category: 0x0200_0000, ..t }, id: 0x0300_0000, parent: 0x0200_0000 },
            Element::Variable { id: 0x0C00_0000, name: "hero".into(), parent: 0x0200_0000 },
        ],
    };
    let bytes = tree.to_bytes().unwrap();
    assert_eq!(parse_tree(&bytes, &data).unwrap(), tree);
    assert!(parse(&bytes, &data).unwrap_err().to_string().contains("parse_tree"));
    assert_eq!(parse_any(&bytes, &data).unwrap(), File::Tree(Box::new(tree)));
}
