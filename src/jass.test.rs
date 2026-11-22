use super::*;
use crate::text::trigger_from_text;

const DATA: &str = r#"
[TriggerTypes]
integer=0,1,1,WESTRING_TRIGTYPE_integer
unitcode=0,1,1,WESTRING_TRIGTYPE_unitcode,integer
string=0,1,1,WESTRING_TRIGTYPE_string
group=0,1,0,WESTRING_TRIGTYPE_group
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
[TriggerCalls]
GetPlayersAll=0,0,force
GetEnumUnit=0,0,unit
GetEnumItem=0,0,item
GetTriggerUnit=0,0,unit
GetUnitsInRectAll=0,0,group,rect
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
        "globals\n    integer udg_Spawn_Units__Count = 0\nendglobals\n",
        "// Trigger: Spawn Units\n// Line one\n// line two\n",
        "function Trig_Spawn_Units_Conditions takes nothing returns boolean\n    if ( not ( udg_Spawn_Units__Count == 10 ) ) then\n        return false\n    endif\n    return true\nendfunction\n",
        "function Trig_Spawn_Units_Actions takes nothing returns nothing\n    local integer i = 5\n",
        "    call CreateNUnitsAtLoc(1, 'hfoo', Player(0), gg_rct_Spawn, 270.0)\n",
        "    set udg_Spawn_Units__Count = ( udg_Spawn_Units__Count + 1 )\n",
        "    set i = udg_Total\n",
        "    call SetHeroLevelBJ(GetTriggerUnit(), 2, OnOffOn)\n",
        "    call DisplayTextToForce(GetPlayersAll(), \"Say \\\"hi\\\"\")\n",
        "    call BJDebugMsg(I2S(i))\n",
        "function InitTrig_Spawn_Units takes nothing returns nothing\n    set gg_trg_Spawn_Units = CreateTrigger()\n    // Map initialization",
        "    call TriggerRegisterAnyUnitEventBJ(gg_trg_Spawn_Units, EVENT_PLAYER_UNIT_DEATH)\n",
        "    call TriggerAddCondition(gg_trg_Spawn_Units, Condition(function Trig_Spawn_Units_Conditions))\n    call DisableTrigger(gg_trg_Spawn_Units)\n    call TriggerAddAction(gg_trg_Spawn_Units, function Trig_Spawn_Units_Actions)\nendfunction\n",
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
        "    local integer n = 0\n    local group l__g1\n    local unit l__u1\n    local integer l__i2\n    local item l__e2\n",
        "    if ( ( n == 1 ) ) then\n        set bj_forLoopAIndex = 1\n        set bj_forLoopAIndexEnd = 3\n        loop\n            exitwhen bj_forLoopAIndex > bj_forLoopAIndexEnd\n            set n = 2\n            set bj_forLoopAIndex = bj_forLoopAIndex + 1\n        endloop\n    else\n",
        // ForGroup: a loop of the function, Picked unit the loop's, the
        // local seen.
        "        set l__g1 = GetUnitsInRectAll(GetPlayableMapRect())\n        loop\n            set l__u1 = FirstOfGroup(l__g1)\n            exitwhen l__u1 == null\n            call GroupRemoveUnit(l__g1, l__u1)\n            call KillUnit(l__u1)\n            set n = 3\n        endloop\n        call DestroyGroup(l__g1)\n    endif\n",
        // Enum: gathered first, then a loop.
        "function Trig_T_Gather2 takes nothing returns nothing\n    set trg__T_e2[trg__T_n2] = GetEnumItem()\n    set trg__T_n2 = trg__T_n2 + 1\nendfunction\n",
        "globals\n    item array trg__T_e2\n    integer trg__T_n2 = 0\nendglobals\n",
        "    set trg__T_n2 = 0\n    call EnumItemsInRectBJ(GetPlayableMapRect(), function Trig_T_Gather2)\n",
        "        set l__e2 = trg__T_e2[l__i2]\n        call RemoveItem(l__e2)\n",
    ] {
        assert!(j.contains(part), "{part}\n---\n{j}");
    }
}

#[test]
fn variables_alone_and_names() {
    let j = compiled("[variable]\nname=\"G\"\ntype=\"group\"\nsize=1\n\n[variable]\nname=\"A\"\ntype=\"unitcode\"\narray=1\nsize=4\n", &[]);
    assert_eq!(j, "globals\n    group udg_G\n    integer array udg_A\nendglobals\n\n");
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
    assert!(j.contains("// Disabled: gg_trg_Off is declared (null)") && !j.contains("function Trig"), "{j}");
    let j = compiled("[trigger]\nname=\"Code\"\ncustom_text=1\naction=DoNothing()\n", &[]);
    assert!(j.contains("its code is in war3map.wct") && !j.contains("function Trig"), "{j}");
}
