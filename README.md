# wtg-rs

Reader and writer for Warcraft III trigger files, `war3map.wtg`: the
map's triggers as World Editor's trigger editor shows them (GUI
triggers), with their categories and variables. A library, a small CLI
and a text form; the [wc3-rs](../wc3-rs) editor opens the file as that
text.

A trigger's events, conditions and actions (ECAs) are calls of the
editor's functions, and **how many parameters each takes isn't in the
file**: it's in the editor's `UI\TriggerData.txt`, so reading needs it,
as World Editor does (`TriggerData::parse`). Writing doesn't. A map made
with a modded editor (UMSWE, YDWE, JAPI's) needs that editor's file: one
map at hand uses `CreateTimer`, which only UMSWE's has.

Reading and writing are exact, and so is the text form, on every map of
the game and the corpus: 207 files, 15 787 triggers, 218 486 ECAs.

## Format

Little-endian; strings are NUL-terminated UTF-8.

| type | field |
|---|---|
| char[4] | `WTG!` |
| u32 | version: 4 Reign of Chaos, 7 The Frozen Throne (`0x80000004` from 1.31: below) |
| u32 + categories | index (i32), name, (7) is a comment (u32) |
| u32 | 1 Reign of Chaos, 2 The Frozen Throne |
| u32 + variables | name, type (`unit`, `integer` ...), unknown (u32: 1; World Editor drops a variable with 0), is an array (u32), (7) array size (u32), has an initial value (u32), initial value |
| u32 + triggers | name, description, (7) is a comment (u32), enabled, custom text (its code in `war3map.wct`), initially off, run on map initialization (u32 each), category index (i32), u32 + ECAs |

An **ECA**: kind (u32: 0 event, 1 condition, 2 action), (inside another)
which block of it (u32), name, enabled (u32), its parameters (as many as
`TriggerData.txt` says), and (7) u32 + the ECAs inside it (an
if/then/else's condition 0, then 1, else 2; a loop's actions 0).

A **parameter**: kind (u32: 0 preset, 1 variable, 2 function, 3 string;
numbers are strings), value, has a call (u32), then the call: kind
(u32: 0–2 as an ECA's, 3 a function returning a value), name, has
parameters (u32: always 1), its parameters; (7, after a call) a u32,
0; is an array (u32), then the index, a parameter.

**What maps have.** A function parameter's value is often what the
parameter was before (`DoNothing` under a `ReturnAction` call, `""`
under a condition): kept. Some parameters have kind `-1` and value `""`
(85). A Frozen Throne trigger's "is a comment" is 0 or 1 for World
Editor's own, and in maps edited by other tools any number (several
hundred triggers, up to 32 754): kept as read.

**From 1.31** the file is the trigger editor's list as a tree (`Tree`,
`parse_tree`; `parse_any` reads either layout): after `0x80000004`, the
version, four u32 (1, 0, 0, 0), for categories, triggers, comments,
scripts and variables each a count and a list of the indexes deleted,
two u32, the game (1 or 2), the variables (as above, each followed by
its id and its parent's), the number of elements (one more than follow),
two u32, the map's file name, two u32, (7) one more u32; then each
element: u32 type (4 category: id, name, (7) is a comment, has children,
parent; 8, 16, 32 a trigger, comment or script: a trigger as above with
its id after the comment flag and its parent in place of the category;
64 a variable: id, name, parent). No map at hand is that new: the layout
follows one description of it and is checked by tests only. It has no
text form yet.

## Text

`text::to_text` and `text::from_text`. BNI: `[wtg]` (the versions), then
a section per category, variable and trigger; a trigger's ECAs are lines
of calls. A value is the rest of its line as it is (BNI's quotes and
commas mean nothing in it: `bni_rs::parse_with`), and
`text::semantic_tokens` highlights it by what it is: functions,
variables, strings, presets, numbers:

```ini
[wtg]
version=7
game=2

[category]
index=1
name="Hints"

[variable]
name="DraeneiUsed"
type="boolean"
size=1
initial="false"

[trigger]
name="Hint Draenei"
description=""
category=1
event=MapInitializationEvent()
action=IfThenElseMultiple()
>condition:0=OperatorCompareBoolean($DraeneiUsed, OperatorEqualENE, "false")
>action:1=QueuedTriggerAddBJ($gg_trg_Hint_Draenei_Q, CheckingIgnoringChecking)
>action:1=TriggerSleepAction("30.00")
>!action:2=DoNothing()
```

- The key: `event`, `condition` or `action`, after `!` when disabled;
  inside another after a `>` per level, with `:` and its block.
- Parameters: `Name(...)` a call (`event:`, `condition:`, `action:`
  before it for those), `$name` a variable, `"text"` a string (as JASS
  writes one: `\"`, `\\`, `\n`, `\r`, `\t`; names and descriptions
  too), a bare word a preset, `[index]` an array's index,
  `~"value"` the value under a call when it isn't the call's name,
  `#n"value"` a parameter of kind `n`.
- Flags only when they aren't the default (`enabled=1`; `comment`,
  `custom_text`, `initially_off`, `run_on_init` 0), a variable's
  `unknown` only when it isn't 1.

Reading the text needs no `TriggerData.txt`; given one, every function's
name and parameter count is checked against it.

### A trigger of its own

`text::trigger_from_text` and `text::trigger_to_text`: one trigger as a
file (`.trg` in the [wc3-rs](../wc3-rs) editor), a `[trigger]` section
as above without `category` (its folder is its category), always of
version 7. What it reads has the trigger as far as it reads (a line with
an error is left out, and what's inside it), and the line of each ECA,
in the order a walk of the tree meets them, for an editor to map what
it shows back to the text.

## What the editor shows

`ui::Ui::parse` reads World Editor's `UI\TriggerData.txt` (categories
with their names and icons, types, presets, each function's parameter
types and category) and `UI\TriggerStrings.txt` (each function's title,
phrase and hint), the `WESTRING_` keys they name resolved by the caller
from `UI\WorldEditStrings.txt`. `Ui::render` shows a trigger as the
trigger editor does: its events, conditions and actions as phrases,
each after its category's name unless the category hides it
(`Unit - Create 1 hfoo for Player 1 (Red) at (Center of (Playable map
area)) facing Default building facing degrees`), calls in parentheses,
presets by their names, and what's inside a line in its blocks. A
phrase's `~Name`s are the parameters in order. Which functions have
blocks isn't in the files (the editor knows it): `ui::blocks` says,
an if/then/else's three, `AndMultiple`'s and `OrMultiple`'s conditions,
and a loop's actions for every other action named `...Multiple`.

## CLI

```sh
wtg-rs TriggerData.txt war3map.wtg                       # read and write back: a check
wtg-rs --text TriggerData.txt war3map.wtg                # the text form
wtg-rs --from-text TriggerData.txt map.ini war3map.wtg
```

## Tests

```sh
cargo test
WTG_DATA=<TriggerData.txt> WTG_CORPUS=<folder of map folders> cargo test --release corpus -- --ignored
```
