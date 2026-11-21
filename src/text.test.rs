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
        "name=\"Say \"\\u{22}\"hi\"\\u{22}\ndescription=\"line one\"\\n\"line two, with a comma\"\ncategory=0\ninitially_off=1\n",
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
