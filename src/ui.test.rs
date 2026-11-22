use super::*;
use crate::text::trigger_from_text;

const DATA: &str = r#"
[TriggerCategories]
TC_UNIT=WESTRING_TRIGCAT_UNIT,ReplaceableTextures\WorldEditUI\Actions-Unit
TC_LOGIC=WESTRING_TRIGCAT_LOGIC,ReplaceableTextures\WorldEditUI\Actions-Logical,1
TC_CONDITION=WESTRING_TRIGCAT_CONDITION,ReplaceableTextures\WorldEditUI\Actions-Logical,1
[TriggerTypes]
unit=0,1,1,WESTRING_TRIGTYPE_unit
[TriggerParams]
Player00=0,player,Player(0),WESTRING_PLAYER_00
OperatorEqual=0,ComparisonOperator,"==",WESTRING_COMPARISONOPERATOR_EQUAL
[TriggerEvents]
MapInitializationEvent=0,nothing
[TriggerConditions]
OperatorCompareInteger=0,integer,ComparisonOperator,integer
_OperatorCompareInteger_Category=TC_CONDITION
[TriggerActions]
IfThenElseMultiple=1,nothing
_IfThenElseMultiple_Category=TC_LOGIC
CreateNUnitsAtLoc=0,integer,unitcode,player,location,real
_CreateNUnitsAtLoc_Defaults=1,hfoo,Player00,GetRectCenter,RealUnitFacing
_CreateNUnitsAtLoc_Category=TC_UNIT
[TriggerCalls]
GetRectCenter=0,1,location,rect
GetPlayableMapRect=0,1,rect
GetUnitLevel=0,0,integer,unit
GetTriggerUnit=0,0,unit
"#;

const STRINGS: &str = r#"
[TriggerEventStrings]
MapInitializationEvent="Map Initialization"
MapInitializationEvent="Map initialization"
MapInitializationEventHint=
[TriggerConditionStrings]
OperatorCompareInteger="Integer Comparison"
OperatorCompareInteger=~Value," ",~Operator," ",~Value
OperatorCompareIntegerHint=
[TriggerActionStrings]
IfThenElseMultiple="If / Then / Else, Multiple Functions"
IfThenElseMultiple="If (All Conditions are True) then do (Then Actions) else do (Else Actions)"
IfThenElseMultipleHint="Add conditions and actions to this function in the main display."
CreateNUnitsAtLoc="Create Units Facing Angle"
CreateNUnitsAtLoc="Create ",~Number," ",~Unit," for ",~Player," at ",~Point," facing ",~Direction," degrees"
CreateNUnitsAtLocHint="The facing angle is expected in degrees."
[TriggerCallStrings]
GetRectCenter="Center Of Region"
GetRectCenter="Center of ",~Region
GetPlayableMapRect="Playable Map Area"
GetPlayableMapRect="Playable map area"
GetUnitLevel="Unit Level"
GetUnitLevel="Level of ",~Unit
GetTriggerUnit="Triggering Unit"
GetTriggerUnit="Triggering unit"
"#;

fn resolve(key: &str) -> String {
    match key {
        "WESTRING_TRIGCAT_UNIT" => "Unit",
        "WESTRING_TRIGCAT_LOGIC" => "If/Then/Else",
        "WESTRING_PLAYER_00" => "Player 1 (Red)",
        "WESTRING_COMPARISONOPERATOR_EQUAL" => "Equal to",
        "WESTRING_TRIGSUBFUNC_IFTHENACTIONS" => "Then - Actions",
        k => k,
    }
    .to_string()
}

/// A line's text, values in `{}` (their role's initial: p preset, v
/// variable, l value, c call, u unknown).
fn flat(spans: &[Span]) -> String {
    let mut out = String::new();
    for s in spans {
        match s {
            Span::Text(t) => out += t,
            Span::Value { role, spans } => {
                let r = match role {
                    Role::Preset => 'p',
                    Role::Variable => 'v',
                    Role::Value => 'l',
                    Role::Call => 'c',
                    Role::Unknown => 'u',
                };
                out += &format!("{r}{{{}}}", flat(spans));
            }
        }
    }
    out
}

#[test]
fn the_editors_data() {
    let ui = Ui::parse(DATA, STRINGS, &resolve);
    let f = ui.function(FunctionKind::Action, "CreateNUnitsAtLoc").unwrap();
    assert_eq!(f.category, "TC_UNIT");
    assert_eq!(f.args, ["integer", "unitcode", "player", "location", "real"]);
    assert_eq!(f.title, "Create Units Facing Angle");
    assert_eq!(f.hint, "The facing angle is expected in degrees.");
    assert_eq!(f.phrase[..3], [Piece::Text("Create ".into()), Piece::Param(0), Piece::Text(" ".into())]);
    let call = ui.function(FunctionKind::Call, "GetRectCenter").unwrap();
    assert_eq!((call.returns.as_str(), call.args.as_slice()), ("location", &["rect".to_string()][..]));
    assert_eq!(ui.categories["TC_UNIT"], Category { name: "Unit".into(), icon: "ReplaceableTextures\\WorldEditUI\\Actions-Unit".into(), show_name: true });
    assert!(!ui.categories["TC_LOGIC"].show_name);
    assert_eq!(ui.presets["Player00"], Preset { kind: "player".into(), display: "Player 1 (Red)".into() });
    assert_eq!(ui.types["unit"], "WESTRING_TRIGTYPE_unit");
    // Labels: the editor's, English for a key it doesn't have.
    assert_eq!(ui.label("WESTRING_TRIGSUBFUNC_IFTHENACTIONS"), "Then - Actions");
    assert_eq!(ui.label("WESTRING_TRIGSUBFUNC_IFELSEACTIONS"), "Else - Actions");
    assert_eq!(ui.label("WESTRING_EVENTS"), "Events");
}

#[test]
fn a_trigger_as_the_editor_shows_it() {
    let ui = Ui::parse(DATA, STRINGS, &resolve);
    let text = concat!(
        "[trigger]\n",
        "name=\"Spawn\"\n",
        "event=MapInitializationEvent()\n",
        "action=IfThenElseMultiple()\n",
        ">condition:0=OperatorCompareInteger(GetUnitLevel(GetTriggerUnit()), OperatorEqual, \"10\")\n",
        ">!action:1=CreateNUnitsAtLoc(\"1\", \"hfoo\", Player00, GetRectCenter(GetPlayableMapRect()), RealUnitFacing)\n",
        "action=Mystery($x[\"1\"])\n",
    );
    let p = trigger_from_text(text, None);
    assert_eq!(p.errors, []);
    assert_eq!(p.lines, [3, 4, 5, 6, 7]);
    let view = ui.render(p.trigger.as_ref().unwrap(), &p.lines);
    assert_eq!(view.events.len(), 1);
    assert_eq!(flat(&view.events[0].text), "Map initialization");
    assert!(view.conditions.is_empty());
    let [ite, mystery] = &view.actions[..] else { panic!("{view:?}") };
    // A category whose name isn't shown; blocks, each with its own.
    assert_eq!(flat(&ite.text), "If (All Conditions are True) then do (Then Actions) else do (Else Actions)");
    assert_eq!((ite.line, ite.icon.as_str()), (4, "ReplaceableTextures\\WorldEditUI\\Actions-Logical"));
    let labels: Vec<_> = ite.blocks.iter().map(|b| (b.label.as_str(), b.kind, b.nodes.len())).collect();
    assert_eq!(labels, [("If - Conditions", 1, 1), ("Then - Actions", 2, 1), ("Else - Actions", 2, 0)]);
    let cond = &ite.blocks[0].nodes[0];
    assert_eq!(flat(&cond.text), "c{(Level of c{(Triggering unit)})} p{Equal to} l{10}");
    let create = &ite.blocks[1].nodes[0];
    assert!(!create.enabled);
    assert_eq!(create.line, 6);
    assert_eq!(create.hint, "The facing angle is expected in degrees.");
    assert_eq!(
        flat(&create.text),
        "Unit - Create l{1} l{hfoo} for p{Player 1 (Red)} at c{(Center of c{(Playable map area)})} facing p{RealUnitFacing} degrees"
    );
    // Unknown: its name and parameters, as a call.
    assert!(!mystery.known);
    assert_eq!(flat(&mystery.text), "u{Mystery(}v{x[l{1}]}u{)}");
}

#[test]
fn which_functions_have_blocks() {
    assert_eq!(blocks(2, "IfThenElseMultiple").len(), 3);
    assert_eq!(blocks(1, "AndMultiple")[0].kind, 1);
    assert_eq!(blocks(2, "ForGroupMultiple")[0].kind, 2);
    assert_eq!(blocks(2, "IfThenElse"), []);
    assert_eq!(blocks(1, "IfThenElseMultiple"), []);
}
