use super::*;
use crate::text::trigger_from_text;

const DATA: &str = r#"
[TriggerTypes]
integer=0,1,1,WESTRING_TRIGTYPE_integer
unitcode=0,1,1,WESTRING_TRIGTYPE_unitcode,integer
string=0,1,1,WESTRING_TRIGTYPE_string
group=0,1,0,WESTRING_TRIGTYPE_group
boolexpr=0,0,0,WESTRING_TRIGTYPE_boolexpr
code=0,0,0,WESTRING_TRIGTYPE_code
[TriggerTypeDefaults]
integer=0
group=CreateGroup(),WESTRING_TRIGDEFAULT_GROUP
[TriggerParams]
Player00=0,player,Player(0),WESTRING_PLAYER_00
OperatorEqual=0,ComparisonOperator,"==",WESTRING_COMPARISONOPERATOR_EQUAL
OperatorAdd=0,ArithmeticOperator,"+",WESTRING_ARITHMETICOPERATOR_ADD
PlayerUnitEventDies=0,playerunitevent,EVENT_PLAYER_UNIT_DEATH,WESTRING_X
[TriggerEvents]
MapInitializationEvent=0,nothing
TriggerRegisterAnyUnitEventBJ=0,playerunitevent
[TriggerConditions]
OperatorCompareInteger=0,integer,ComparisonOperator,integer
[TriggerActions]
IfThenElseMultiple=1,nothing
ForLoopAMultiple=1,integer,integer
ForGroupMultiple=1,group
_ForGroupMultiple_ScriptName=ForGroupBJ
EnumItemsInRectBJMultiple=1,rect
_EnumItemsInRectBJMultiple_ScriptName=EnumItemsInRectBJ
CreateNUnitsAtLoc=0,integer,unitcode,player,location,real
SetVariable=0,AnyGlobal,Null
DisplayTextToForce=0,force,string
KillUnit=0,unit
RemoveItem=0,item
SetHeroLevel=0,unit,integer,onoff
_SetHeroLevel_ScriptName=SetHeroLevelBJ
CustomScriptCode=1,scriptcode
EnumItemsInRectBJ=0,rect,code
[TriggerCalls]
GetPlayersAll=0,0,force
GetEnumUnit=0,0,unit
GetEnumItem=0,0,item
GetTriggerUnit=0,0,unit
GetUnitsInRectAll=0,0,group,rect
GetUnitsInRectMatching=0,0,group,rect,boolexpr
GetPlayableMapRect=0,1,rect
OperatorInt=0,1,integer,integer,ArithmeticOperator,integer
"#;

fn ui() -> Ui {
    Ui::parse(DATA, "", &|k| k.to_string())
}

fn compiled(text: &str, visible: &[Declared]) -> String {
    let p = trigger_from_text(text, None);
    assert_eq!(p.errors, [], "{text}");
    let mut seen = p.variables.clone();
    seen.extend(visible.iter().cloned());
    compile(p.trigger.as_ref(), "file", &p.variables, &seen, &ui())
}

fn global(name: &str, kind: &str) -> Declared {
    Declared {
        variable: crate::Variable {
            name: name.into(),
            kind: kind.into(),
            unknown: 1,
            is_array: 0,
            array_size: 1,
            has_initial: 0,
            initial: String::new(),
        },
        scope: Scope::Global,
    }
}

#[test]
fn a_trigger_as_world_editor_lays_it_out() {
    let text = concat!(
        "[variable]\nname=\"Count\"\ntype=\"integer\"\nscope=private\nsize=1\n\n",
        "[variable]\nname=\"i\"\ntype=\"integer\"\nscope=local\nsize=1\ninitial=\"5\"\n\n",
        "[trigger]\nname=\"Spawn Units\"\ndescription=\"Line one\\nline two\"\ninitially_off=1\n",
        "event=MapInitializationEvent()\n",
        "event=TriggerRegisterAnyUnitEventBJ(PlayerUnitEventDies)\n",
        "condition=OperatorCompareInteger($Count, OperatorEqual, \"10\")\n",
        "action=CreateNUnitsAtLoc(\"1\", \"hfoo\", Player00, $gg_rct_Spawn, \"270.0\")\n",
        "action=SetVariable($Count, OperatorInt($Count, OperatorAdd, \"1\"))\n",
        "action=SetVariable($i, $Total)\n",
        "!action=KillUnit(GetTriggerUnit())\n",
        "action=SetHeroLevel(GetTriggerUnit(), \"2\", OnOffOn)\n",
        "action=DisplayTextToForce(GetPlayersAll(), \"Say \\\"hi\\\"\")\n",
        "action=CustomScriptCode(\"call BJDebugMsg(I2S(i))\")\n",
    );
    let j = compiled(text, &[global("Total", "integer")]);
    for part in [
        "integer udg_Spawn_Units__Count = 0\n\n",
        "// Trigger: Spawn Units\n// Line one\n// line two\n",
        "function Trig_Spawn_Units_Conditions returns boolean\n    if ( not ( udg_Spawn_Units__Count == 10 ) ) then\n        return false\n    endif\n    return true\nendfunction\n",
        "function Trig_Spawn_Units_Actions\n    integer i = 5\n",
        "    CreateNUnitsAtLoc(1, 'hfoo', Player(0), gg_rct_Spawn, 270.0)\n",
        "    udg_Spawn_Units__Count = ( udg_Spawn_Units__Count + 1 )\n",
        "    i = udg_Total\n",
        "    SetHeroLevelBJ(GetTriggerUnit(), 2, OnOffOn)\n",
        "    DisplayTextToForce(GetPlayersAll(), \"Say \\\"hi\\\"\")\n",
        // Custom script as it is written.
        "    call BJDebugMsg(I2S(i))\n",
        "trigger gg_trg_Spawn_Units = CreateTrigger()\n\nfunction main\n    DisableTrigger(gg_trg_Spawn_Units)\n    // Map initialization",
        "    TriggerRegisterAnyUnitEventBJ(gg_trg_Spawn_Units, EVENT_PLAYER_UNIT_DEATH)\n",
        "    TriggerAddCondition(gg_trg_Spawn_Units, Condition(function Trig_Spawn_Units_Conditions))\n    TriggerAddAction(gg_trg_Spawn_Units, function Trig_Spawn_Units_Actions)\nendfunction\n",
    ] {
        assert!(j.contains(part), "{part}\n---\n{j}");
    }
    // What's disabled isn't there.
    assert!(!j.contains("KillUnit"), "{j}");
}

#[test]
fn blocks_are_inside_the_actions() {
    let text = concat!(
        "[variable]\nname=\"n\"\ntype=\"integer\"\nscope=local\nsize=1\n\n",
        "[trigger]\nname=\"T\"\n",
        "action=IfThenElseMultiple()\n",
        ">condition:0=OperatorCompareInteger($n, OperatorEqual, \"1\")\n",
        ">action:1=ForLoopAMultiple(\"1\", \"3\")\n",
        ">>action:0=SetVariable($n, \"2\")\n",
        ">action:2=ForGroupMultiple(GetUnitsInRectAll(GetPlayableMapRect()))\n",
        ">>action:0=KillUnit(GetEnumUnit())\n",
        ">>action:0=SetVariable($n, \"3\")\n",
        "action=EnumItemsInRectBJMultiple(GetPlayableMapRect())\n",
        ">action:0=RemoveItem(GetEnumItem())\n",
    );
    let j = compiled(text, &[]);
    for part in [
        "    integer n = 0\n    group l__g1\n    unit l__u1\n    integer l__i2\n    item l__e2\n",
        "    if ( ( n == 1 ) ) then\n        bj_forLoopAIndex = 1\n        bj_forLoopAIndexEnd = 3\n        loop\n            exitwhen bj_forLoopAIndex > bj_forLoopAIndexEnd\n            n = 2\n            bj_forLoopAIndex = bj_forLoopAIndex + 1\n        endloop\n    else\n",
        // ForGroup: a loop of the function, Picked unit the loop's, the
        // local seen.
        "        l__g1 = GetUnitsInRectAll(GetPlayableMapRect())\n        loop\n            l__u1 = FirstOfGroup(l__g1)\n            exitwhen l__u1 == null\n            GroupRemoveUnit(l__g1, l__u1)\n            KillUnit(l__u1)\n            n = 3\n        endloop\n        DestroyGroup(l__g1)\n    endif\n",
        // Enum: gathered first, then a loop.
        "function Trig_T_Gather2\n    trg__T_e2[trg__T_n2] = GetEnumItem()\n    trg__T_n2 = trg__T_n2 + 1\nendfunction\n",
        "item array trg__T_e2\ninteger trg__T_n2 = 0\n\n",
        "    trg__T_n2 = 0\n    EnumItemsInRectBJ(GetPlayableMapRect(), function Trig_T_Gather2)\n",
        "        l__e2 = trg__T_e2[l__i2]\n        RemoveItem(l__e2)\n",
    ] {
        assert!(j.contains(part), "{part}\n---\n{j}");
    }
}

#[test]
fn variables_alone_and_names() {
    let j = compiled("[variable]\nname=\"G\"\ntype=\"group\"\nsize=1\n\n[variable]\nname=\"A\"\ntype=\"unitcode\"\narray=1\nsize=4\n", &[]);
    assert_eq!(j, "group udg_G\ninteger array udg_A\n\n");
    assert_eq!(compile(None, "x", &[], &[], &ui()), "// Nothing: no [trigger] and no variables.\n");
    assert_eq!(ident("Spawn: 1 (b)"), "Spawn__1__b_u");
    // A trigger without a name is the file's.
    assert!(compiled("[trigger]\naction=CustomScriptCode(\"x\")\n", &[]).contains("function Trig_file_Actions"));
}

#[test]
fn custom_script_of_lines() {
    let j = compiled("[trigger]\naction=CustomScriptCode(\"local integer n = 1\\ncall BJDebugMsg(I2S(n))  \")\n", &[]);
    assert!(j.contains("\n    local integer n = 1\n    call BJDebugMsg(I2S(n))\n"), "{j}");
}

#[test]
fn trigger_names_as_world_editors() {
    // A byte each, a `u` after a `_` at the end: as a map's war3map.j has
    // them (CruiserCommandRU, 523 triggers).
    assert_eq!(ident("DOOR CLOSE 3 1 Копировать"), format!("DOOR_CLOSE_3_1{}u", "_".repeat(21)));
    assert_eq!(ident("DOOR CLOSE 3 1 Копировать 2"), format!("DOOR_CLOSE_3_1{}2", "_".repeat(22)));
    assert_eq!(ident("Melee Initialization"), "Melee_Initialization");
    assert_eq!(ident(""), "u");
    let j = compiled("[trigger]\nname=\"Шляпа\"\naction=DoNothing()\n", &[]);
    assert!(j.contains(&format!("function Trig_{}u_Actions", "_".repeat(10))), "{j}");
}

#[test]
fn comments_disabled_and_custom_text_as_world_editor_builds_them() {
    let j = compiled("[trigger]\nname=\"--- Init ---\"\ncomment=1\n", &[]);
    assert!(j.contains("// A comment, not a trigger") && !j.contains("function Trig"), "{j}");
    let j = compiled("[trigger]\nname=\"Off\"\nenabled=0\naction=DoNothing()\n", &[]);
    assert!(j.contains("// Disabled: gg_trg_Off is declared (null)") && j.contains("\ntrigger gg_trg_Off = null\n") && !j.contains("\nfunction"), "{j}");
    let j = compiled("[trigger]\nname=\"Code\"\ncustom_text=1\naction=DoNothing()\n", &[]);
    assert!(j.contains("its code is in war3map.wct") && !j.contains("function Trig"), "{j}");
}

#[test]
fn world_editors_own_actions() {
    let data = format!("{DATA}AddTriggerEvent=0,trigger,event\nWaitForCondition=0,boolexpr,real\n[TriggerEvents]\nTriggerRegisterUnitEvent=0,unit,unitevent\n[TriggerParams]\nUnitEventDeath=0,unitevent,EVENT_UNIT_DEATH,WESTRING_X\n");
    let ui = Ui::parse(&data, "", &|k| k.to_string());
    let p = trigger_from_text(concat!(
        "[trigger]\nname=\"T\"\n",
        "action=AddTriggerEvent($gg_trg_X, event:TriggerRegisterUnitEvent(GetTriggerUnit(), UnitEventDeath) ~\"\")\n",
        "action=WaitForCondition(condition:OperatorCompareInteger($n, OperatorEqual, \"1\") ~\"\", \"0.40\")\n",
        "action=ForGroupMultiple(GetUnitsInRectAll(GetPlayableMapRect()))\n",
        ">action:0=CustomScriptCode(\"local unit u=GetEnumUnit()\")\n",
        "action=ForGroupMultiple(GetUnitsInRectAll(GetPlayableMapRect()))\n",
        ">action:0=CustomScriptCode(\"local unit u = GetEnumUnit()\")\n",
        ">action:0=CustomScriptCode(\"local unit v\")\n",
        ">action:0=CustomScriptCode(\"local unit v\")\n",
    ), None);
    assert_eq!(p.errors, [], "parse");
    let n = global("n", "integer");
    let j = compile(p.trigger.as_ref(), "file", &[], &[n], &ui);
    for part in [
        "    TriggerRegisterUnitEvent(gg_trg_X, GetTriggerUnit(), EVENT_UNIT_DEATH)\n",
        "    loop\n        exitwhen ( udg_n == 1 )\n        TriggerSleepAction(RMaxBJ(bj_WAIT_FOR_COND_MIN_INTERVAL, 0.40))\n    endloop\n",
        "        local unit u=GetEnumUnit()\n",
        "        set u = GetEnumUnit()\n",
    ] {
        assert!(j.contains(part), "{part}\n---\n{j}");
    }
    assert_eq!(j.matches("local unit v").count(), 1, "{j}");
}

#[test]
fn variables_as_world_editor_initializes_them() {
    let p = trigger_from_text(concat!(
        "[variable]\nname=\"n\"\ntype=\"integer\"\nsize=1\n\n",
        "[variable]\nname=\"g\"\ntype=\"group\"\nsize=1\n\n",
        "[variable]\nname=\"u\"\ntype=\"unit\"\nsize=1\n\n",
        "[variable]\nname=\"s\"\ntype=\"string\"\nsize=1\ninitial=\"hi\"\n\n",
        "[variable]\nname=\"a\"\ntype=\"integer\"\narray=1\nsize=3\ninitial=\"7\"\n\n",
        "[variable]\nname=\"e\"\ntype=\"string\"\nsize=1\n\n",
        "[variable]\nname=\"t\"\ntype=\"trigger\"\narray=1\nsize=1\n\n",
        "[variable]\nname=\"p\"\ntype=\"player\"\nsize=1\ninitial=\"Player00\"\n",
    ), None);
    assert_eq!(p.errors, []);
    let data = format!("{DATA}[TriggerTypes]\nunit=0,1,1,WESTRING_TRIGTYPE_unit\ntrigger=0,1,1,WESTRING_TRIGTYPE_trigger\n");
    let ui = Ui::parse(&data, "", &|k| k.to_string());
    type Seen<'a> = (String, String, Option<u32>, Option<&'a str>, Option<String>);
    let got: Vec<Seen> = p.variables.iter().map(|d| map_variable(d, &ui)).map(|m| (m.ty, m.name, m.array, m.empty, m.value)).collect();
    let s = |x: &str| x.to_string();
    assert_eq!(
        got,
        [
            (s("integer"), s("udg_n"), None, Some("0"), Some(s("0"))),
            (s("group"), s("udg_g"), None, Some("null"), Some(s("CreateGroup()"))),
            (s("unit"), s("udg_u"), None, Some("null"), None),
            (s("string"), s("udg_s"), None, Some("\"\""), Some(s("\"hi\""))),
            (s("integer"), s("udg_a"), Some(3), None, Some(s("7"))),
            (s("string"), s("udg_e"), None, Some("\"\""), Some(s("\"\""))),
            // The game's editor makes no trigger.
            (s("trigger"), s("udg_t"), Some(1), None, None),
            (s("player"), s("udg_p"), None, Some("null"), Some(s("Player(0)"))),
        ]
    );
    // An editor whose data makes one (CruiserCommand's).
    let ui = Ui::parse(&format!("{data}[TriggerTypeDefaults]\ntrigger=CreateTrigger()\n"), "", &|k| k.to_string());
    let t = p.variables.iter().find(|d| d.variable.name == "t").unwrap();
    assert_eq!(map_variable(t, &ui).value.as_deref(), Some("CreateTrigger()"));
}

#[test]
fn triggers_run_at_initialization() {
    let t = |text: &str| trigger_from_text(text, None).trigger.unwrap();
    let all = [
        t("[trigger]\nname=\"Init A\"\nevent=MapInitializationEvent()\n"),
        t("[trigger]\nname=\"Off\"\nenabled=0\nevent=MapInitializationEvent()\n"),
        t("[trigger]\nname=\"Event off\"\n!event=MapInitializationEvent()\n"),
        t("[trigger]\nname=\"Initially off\"\ninitially_off=1\nevent=MapInitializationEvent()\n"),
        t("[trigger]\nname=\"Code\"\ncustom_text=1\nrun_on_init=1\n"),
        t("[trigger]\nname=\"Other\"\nevent=TriggerRegisterAnyUnitEventBJ(PlayerUnitEventDies)\n"),
    ];
    assert_eq!(initialization_triggers(&all), ["gg_trg_Init_A", "gg_trg_Code"]);
}

#[test]
fn custom_text_is_made_by_its_code() {
    let out = compiled("[trigger]\nname=\"Item Pickup\"\ncustom_text=1\n", &[]);
    assert!(out.contains("trigger gg_trg_Item_Pickup = null\n\nfunction main\n    InitTrig_Item_Pickup()\nendfunction"), "{out}");
    // Off: declared only.
    let out = compiled("[trigger]\nname=\"Off\"\ncustom_text=1\nenabled=0\n", &[]);
    assert!(out.contains("trigger gg_trg_Off = null") && !out.contains("InitTrig_Off"), "{out}");
}

#[test]
fn a_condition_given_as_a_filter_is_a_function() {
    let out = compiled(
        "[variable]\nname=\"g\"\ntype=\"group\"\nsize=1\n\n[trigger]\nname=\"Pick\"\naction=SetVariable($g, GetUnitsInRectMatching(GetPlayableMapRect(), condition:OperatorCompareInteger(\"1\", OperatorEqual, \"2\")))\n",
        &[],
    );
    assert!(out.contains("function Trig_Pick_Filter1 returns boolean\n    return ( 1 == 2 )\nendfunction"), "{out}");
    assert!(out.contains("udg_g = GetUnitsInRectMatching(GetPlayableMapRect(), Condition(function Trig_Pick_Filter1))"), "{out}");
}

#[test]
fn an_action_given_as_code_is_a_function() {
    let out = compiled("[trigger]\nname=\"Sweep\"\naction=EnumItemsInRectBJ(GetPlayableMapRect(), action:RemoveItem(GetEnumItem()))\n", &[]);
    assert!(out.contains("function Trig_Sweep_Action1\n    RemoveItem(GetEnumItem())\nendfunction"), "{out}");
    assert!(out.contains("EnumItemsInRectBJ(GetPlayableMapRect(), function Trig_Sweep_Action1)"), "{out}");
}

#[test]
fn an_action_given_as_code_is_made_as_actions_are() {
    let out = compiled("[variable]\nname=\"n\"\ntype=\"integer\"\nsize=1\n\n[trigger]\nname=\"Count\"\naction=EnumItemsInRectBJ(GetPlayableMapRect(), action:SetVariable($n, OperatorInt($n, OperatorAdd, \"1\")))\n", &[]);
    assert!(out.contains("function Trig_Count_Action1\n    udg_n = ( udg_n + 1 )\nendfunction"), "{out}");
}
