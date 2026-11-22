use super::*;
use crate::tests::{DATA, sample};

#[test]
fn text_reads_back_to_the_same_file() {
    let data = TriggerData::parse(DATA);
    let f = sample();
    let text = to_text(&f).unwrap();
    let back = from_text(&text, Some(&data));
    assert_eq!(back.errors, [], "{text}");
    assert_eq!(back.file.unwrap(), f, "{text}");
    for part in [
        "[wtg]\nversion=7\ngame=2\n\n[category]\nindex=0\nname=\"Init\"\n\n[category]\nindex=1\nname=\"--\"\ncomment=1\n",
        "[variable]\nname=\"hero\"\ntype=\"unit\"\narray=1\nsize=12\n\n",
        "[variable]\nname=\"n\"\ntype=\"integer\"\nsize=1\ninitial=\"5\"\n",
        "name=\"Say \\\"hi\\\"\"\ndescription=\"line one\\nline two, with a comma\"\ncategory=0\ninitially_off=1\n",
        "event=MapInitializationEvent()\nevent=TriggerRegisterTimerEventPeriodic(\"2.00\")\naction=IfThenElseMultiple()\n",
        ">condition:0=OperatorCompareInteger(GetUnitLevel(GetTriggerUnit()) ~\"\", OperatorEqualENE, #-1\"\")\n",
        ">action:1=SetVariable($hero[\"1\"], \"x\")\n>!action:2=DoNothing()\n",
    ] {
        assert!(text.contains(part), "{part:?} in\n{text}");
    }
    assert!(is_trigger_text(&bni_rs::parse(&text)));
    // Reign of Chaos: no comments, sizes or nesting.
    let mut roc = sample();
    roc.version = 4;
    roc.categories.iter_mut().for_each(|c| c.is_comment = 0);
    roc.variables.iter_mut().for_each(|v| v.array_size = 0);
    fn flat(p: &mut Param) {
        p.after_call = None;
        p.call.iter_mut().flat_map(|c| c.params.iter_mut()).for_each(flat);
    }
    let t = &mut roc.triggers[0];
    t.ecas[2].children.clear();
    t.ecas.iter_mut().flat_map(|e| e.params.iter_mut()).for_each(flat);
    let text = to_text(&roc).unwrap();
    assert_eq!(from_text(&text, None).file.unwrap(), roc);
}

#[test]
fn errors_in_the_text() {
    let data = TriggerData::parse(DATA);
    let lines = |t: &str| -> Vec<(usize, String)> {
        from_text(t, Some(&data)).errors.into_iter().map(|e| (e.line, e.message)).collect()
    };
    let e = lines(concat!(
        "[wtg]\n",
        "version=7\n",
        "[trigger]\n",
        "event=MapInitializationEvent(\"1\")\n",
        "condition=Nope()\n",
        ">>action:1=DoNothing()\n",
        "action=DoNothing() x\n",
        ">action=DoNothing()\n",
        "action=SetVariable($a, foo:Bar())\n",
        "action=SetVariable(\"a\n",
        "what=1\n",
        "[thing]\n",
    ));
    let at: Vec<usize> = e.iter().map(|x| x.0).collect();
    assert_eq!(at, [4, 5, 6, 7, 8, 9, 10, 11, 12], "{e:#?}");
    assert!(e[0].1.contains("takes 0 parameters"), "{e:?}");
    assert!(lines("[wtg]\n[trigger]\nevent=TriggerRegisterTimerEventPeriodic(2)\n")[0].1.contains("a preset word"));
    assert!(e[1].1.contains("isn't in UI\\TriggerData.txt [TriggerConditions]"), "{e:?}");
    assert!(lines("[wtg]\nversion=4\n[category]\ncomment=1\n")[0].1.contains("version 7"));
    // Without the data, names and counts aren't checked.
    assert!(from_text("[wtg]\n[trigger]\ncondition=Nope(\"1\", x)\n", None).file.is_some());
}

#[test]
fn strings_are_jass_and_values_are_tokens() {
    let data = TriggerData::parse("[TriggerActions]\nCustomScriptCode=1,scriptcode\n");
    let code = "call DestroyEffect(AddSpecialEffect( \"Abilities\\\\Spells\\\\Human\\\\x.mdl\", 0, 0))";
    let text = format!("[wtg]\n[trigger]\nname=\"T\"\n>action:1=CustomScriptCode({})\n", jass_string(code));
    assert!(text.contains(r#"CustomScriptCode("call DestroyEffect(AddSpecialEffect( \"Abilities\\\\Spells"#), "{text}");
    let p = from_text(&text.replace(">action:1", "action"), Some(&data));
    assert_eq!(p.errors, []);
    let t = &p.file.unwrap().triggers[0];
    assert_eq!(t.ecas[0].params[0].value, code);

    let toks: Vec<(usize, usize, usize, T)> = semantic_tokens("[wtg]\n[trigger]\n>!action:1=F($v[\"1\"], P, G() ~\"x\") // c\n")
        .into_iter()
        .map(|t| (t.line, t.start_col, t.end_col, t.ty))
        .collect();
    assert_eq!(&toks[..2], [(1, 2, 5, T::Namespace), (2, 2, 9, T::Namespace)]);
    assert_eq!(&toks[2..], [
        (3, 1, 3, T::Operator), (3, 3, 9, T::Keyword), (3, 9, 10, T::Operator), (3, 10, 11, T::Number),
        (3, 12, 13, T::Function), (3, 14, 16, T::Variable), (3, 17, 20, T::String),
        (3, 23, 24, T::EnumMember), (3, 26, 27, T::Function), (3, 30, 31, T::Operator), (3, 31, 34, T::String),
        (3, 36, 40, T::Comment),
    ]);
    // A string's escapes.
    assert!(from_text("[wtg]\n[trigger]\nname=\"a\\q\"\n", None).errors[0].message.contains("escapes"));
}

#[test]
fn categories_by_index() {
    let doc = parse_bni("[wtg]\n[category]\nindex=38\nname=\"A \\\"b\\\"\"\n[category]\nname=\"no index\"\n");
    assert_eq!(categories(&doc), [(38, "A \"b\"".to_string())]);
    assert_eq!(read_string(" \"x\\ny\" "), Some("x\ny".into()));
    assert_eq!(read_string("\"x\" y"), None);
}

#[test]
fn a_trigger_file_of_its_own() {
    let data = TriggerData::parse(DATA);
    let mut t = sample().triggers.remove(0);
    t.category = 0;
    let text = trigger_to_text(&t).unwrap();
    assert!(text.starts_with("[trigger]\nname=\"Say \\\"hi\\\"\"\ndescription="), "{text}");
    assert!(!text.contains("category"), "{text}");
    let p = trigger_from_text(&text, Some(&data));
    assert_eq!(p.errors, [], "{text}");
    assert_eq!(p.trigger, Some(t));
    // Every ECA's line, a parent before what's inside it.
    let ecas: Vec<usize> = text.lines().enumerate().filter(|(_, l)| l.contains('(')).map(|(i, _)| i + 1).collect();
    assert_eq!(p.lines, ecas);

    let lines = |t: &str| -> Vec<(usize, String)> {
        trigger_from_text(t, Some(&data)).errors.into_iter().map(|e| (e.line, e.message)).collect()
    };
    let e = lines("[trigger]\ncategory=1\n[trigger]\n[wtg]\n");
    assert_eq!(e.iter().map(|x| x.0).collect::<Vec<_>>(), [2, 3, 4], "{e:?}");
    assert!(e[0].1.contains("its folder is its category"), "{e:?}");
    assert!(e[1].1.contains("a second [trigger]"), "{e:?}");
    assert!(e[2].1.contains("no section [wtg]"), "{e:?}");
    assert_eq!(lines(""), []);

    // A line that doesn't read is left out, and what's inside it; the rest
    // is read.
    let p = trigger_from_text("[trigger]\naction=Nope()\n>action:0=DoNothing()\naction=DoNothing()\n", Some(&data));
    assert_eq!(p.errors.len(), 1, "{:?}", p.errors);
    assert_eq!(p.trigger.unwrap().ecas.len(), 1);
    assert_eq!(p.lines, [4]);
}

#[test]
fn variables_in_a_trigger_file() {
    let f = sample();
    let text = variables_to_text(&f.variables).unwrap();
    assert!(text.starts_with("[variable]\nname=\"hero\"\ntype=\"unit\"\narray=1\nsize=12\n\n[variable]"), "{text}");
    let p = trigger_from_text(&text, None);
    assert_eq!(p.errors, []);
    assert_eq!((p.trigger, p.variables), (None, f.variables.clone()));
    // With a trigger, in any order.
    let both = format!("{text}\n[trigger]\nname=\"T\"\naction=DoNothing()\n");
    let p = trigger_from_text(&both, None);
    assert_eq!(p.errors, []);
    assert_eq!((p.trigger.unwrap().name.as_str(), p.variables.len()), ("T", 2));
}

#[test]
fn triggers_as_files() {
    assert_eq!(file_name("a/b: c?"), "a_b_ c_");
    assert_eq!(file_name("Init. "), "Init");
    assert_eq!(file_name("con"), "con_");
    assert_eq!(file_name("COM1.x"), "COM1.x_");
    assert_eq!(file_name(""), "_");

    let mut f = sample();
    f.categories.push(Category { index: 5, name: "init".into(), is_comment: 0 });
    let mut t = f.triggers[0].clone();
    t.category = 0;
    f.triggers.push(t.clone());
    t.category = 99;
    t.name = "_variables".into();
    f.triggers.push(t);
    let made = to_files(&f).unwrap();
    assert_eq!(made.folders, ["000_Init", "001_--", "002_init"]);
    let files = made.files;
    let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(paths, [
        "_variables.trg",
        "000_Init/000_Say _hi_.trg",
        "000_Init/001_Say _hi_.trg",
        "000__variables.trg",
    ], "{files:#?}");
    // Wider numbers when there are more than a thousand.
    assert_eq!((number(7, 12), number(7, 1000), number(7, 1001)), ("007".into(), "007".into(), "0007".into()));
    // Every file reads back to what it came from.
    let p = trigger_from_text(&files[1].1, None);
    assert_eq!(p.errors, []);
    let mut back = p.trigger.unwrap();
    back.category = f.triggers[0].category;
    assert_eq!(back, f.triggers[0]);
    assert_eq!(trigger_from_text(&files[0].1, None).variables, f.variables);
}

#[test]
fn one_param_or_call() {
    let p = read_param("GetRectCenter(GetPlayableMapRect())").unwrap();
    assert_eq!(param_text(&p).unwrap(), "GetRectCenter(GetPlayableMapRect())");
    assert_eq!(param_text(&read_param(" $x[\"1\"] ").unwrap()).unwrap(), "$x[\"1\"]");
    assert!(read_param("a b").is_err());
    let (name, params) = read_call("F(P, \"2\") // c").unwrap();
    assert_eq!(call_text(&name, &params).unwrap(), "F(P, \"2\")");
}
