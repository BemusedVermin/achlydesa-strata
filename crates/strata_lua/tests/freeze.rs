use strata_kernel::{DefId, PackId, Sym, Value};
use strata_lua::{DataStage, FrozenDef, FrozenRecord, LoadError, PackSource, freeze};
use strata_model::{AbilityDef, Descriptor, Logic};

/// Runs `data.lua` of a single pack `t` and freezes the result.
fn run(data: &str) -> Result<Vec<FrozenRecord>, Vec<LoadError>> {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("data.lua"), data).unwrap();
    let pack = PackSource {
        id: PackId::new("t"),
        root: dir.path().to_path_buf(),
        dependencies: Vec::new(),
    };
    freeze(&DataStage::run(&[pack]).unwrap())
}

fn errors(data: &str) -> String {
    let errors = run(data).expect_err("expected the freeze to fail");
    let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
    lines.join("\n")
}

const UNIT: &str = r#"
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
    id = 'ironclad_line', name = 'Ironclad Line', kind = 'squad', rarity = 'common',
    faction = 'brass',
    stats = { hp = 5, defense = 3 },
    figures = {{
        id = 'f', count = 2, footprint = 'one', height = 1,
        los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
    }},
    abilities = {
        { name = 'Plain', text = 'Nothing.', scope = 'unit' },
        {
            name = 'Brace', text = 'Holds.', scope = 'figure',
            triggers = {{ event = 'figure_destroyed', effect = function() end }},
            modifiers = {{ query = 'defense', layer = 'add', value = function() end }},
        },
    },
}
rule_module { id = 'm', name = 'M', register = function() end }
"#;

fn unit(records: &[FrozenRecord]) -> &strata_model::UnitDef {
    match &records
        .iter()
        .find(|r| r.record.kind == "unit")
        .unwrap()
        .def
    {
        FrozenDef::Unit(unit) => unit,
        other => panic!("not a unit: {other:?}"),
    }
}

#[test]
fn records_become_typed_definitions_with_placeholders() {
    let records = run(UNIT).unwrap();
    let def = unit(&records);
    assert_eq!(def.name.0, "Ironclad Line");
    assert_eq!(def.figures[0].count, 2);
    assert_eq!(def.source.file, "t/data.lua");
    assert_eq!(def.source.line, 3);
    let record = records.iter().find(|r| r.record.kind == "unit").unwrap();
    let found: Vec<_> = record
        .references
        .iter()
        .map(|r| (r.field_path.as_str(), r.target.as_str(), r.key))
        .collect();
    assert_eq!(
        found,
        [
            ("id", "t:ironclad_line", false),
            ("faction", "brass", false),
            ("figures/1/footprint", "one", false),
            ("stats/defense", "defense", true),
            ("stats/hp", "hp", true),
        ]
    );
    let placeholder = |name: &str| record.references.iter().position(|r| r.target == name);
    assert_eq!(
        def.faction,
        Some(DefId(placeholder("brass").unwrap() as u32))
    );
    assert_eq!(def.stats.len(), 2);
    assert_eq!(def.stats[&DefId(placeholder("hp").unwrap() as u32)], 5);
}

#[test]
fn functions_become_script_refs_with_stable_paths() {
    let records = run(UNIT).unwrap();
    let record = records.iter().find(|r| r.record.kind == "unit").unwrap();
    let paths: Vec<_> = record.scripts.iter().map(|s| s.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "unit:ironclad_line/abilities/2/modifiers/1/value",
            "unit:ironclad_line/abilities/2/triggers/1/effect",
        ]
    );
    assert!(record.scripts.iter().all(|s| s.pack == PackId::new("t")));
    let module = records.iter().find_map(|r| match &r.def {
        FrozenDef::RuleModule(module) => Some(module),
        _ => None,
    });
    let module = module.unwrap();
    assert_eq!(module.register.path, Sym::new("rule_module:m/register"));
    assert_eq!(module.register.def, DefId(0));
}

#[test]
fn every_error_names_file_line_and_field_path() {
    let text = errors(
        r#"
stat { id = 'missing', name = 'A', min = 0 }
stat { id = 'wrong', name = 'B', min = 'low', max = 3 }
stat { id = 'float', name = 'C', min = 0, max = 1.5 }
stat { id = 'unknown', name = 'D', min = 0, max = 3, colour = 'red' }
unit {
    id = 'nested', name = 'N', kind = 'squad', rarity = 'common',
    stats = { hp = 'five' },
    figures = {{
        id = 'f', count = 1, footprint = 'one', height = 1,
        los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
    }},
}
"#,
    );
    insta::assert_snapshot!(text);
}

#[test]
fn freezing_twice_gives_identical_records_and_paths() {
    let (first, second) = (run(UNIT).unwrap(), run(UNIT).unwrap());
    assert_eq!(first, second);
}

const NATIVE: &str = r#"
local function drill(id)
  return {
    id = id, name = 'Drill', text = 'Add 1 near a friend.', scope = 'unit',
    modifiers = {{ query = 'defense', layer = 'add', when = { cond = 'engaged' }, value = 1 }},
  }
end
ability(drill('drill'))
relic {
  id = 'fury', name = 'Fury', mode = 'on_claim', claim = 'ends_move_on',
  grants = { to = 'occupant_army', abilities = { drill() } },
  effect = { native = 'boom' },
}
"#;

#[test]
fn plain_data_in_a_logic_field_freezes_as_a_native_descriptor() {
    let records = run(NATIVE).unwrap();
    let native = |entries: &[(&str, &str)]| {
        let entries = entries
            .iter()
            .map(|(k, v)| (Sym::new(k), Value::Str(Sym::new(v))));
        Logic::Native(Descriptor(Value::Map(entries.collect())))
    };
    let check = |ability: &AbilityDef| {
        let modifier = &ability.modifiers[0];
        assert_eq!(modifier.when, Some(native(&[("cond", "engaged")])));
        assert_eq!(modifier.value, Logic::Native(Descriptor(Value::Int(1))));
        assert_eq!(modifier.query, Sym::new("defense"));
    };
    for record in &records {
        match &record.def {
            FrozenDef::Ability(ability) => check(ability),
            FrozenDef::Relic(relic) => {
                check(&relic.grants.as_ref().unwrap().abilities[0]);
                assert_eq!(relic.effect, Some(native(&[("native", "boom")])));
                let effect = relic.effect.as_ref().unwrap();
                assert_eq!(strata_rules::native_name(effect), Some("boom"));
            }
            other => panic!("unexpected record {other:?}"),
        }
        assert!(record.scripts.is_empty());
    }
}

#[test]
fn a_record_with_two_faults_reports_the_same_one_in_every_fresh_state() {
    let data = "stat { id = 'two', name = 'B', min = 'low', max = 'high', zz = 1 }";
    let first = errors(data);
    for _ in 0..16 {
        assert_eq!(errors(data), first);
    }
}

const LISTS: &str = r#"
scenario {
  id = 'duel', name = 'Duel', map = 'flat', players = { min = 2, max = 2 }, slots = {},
  army = { { max_points = 100 } },
  victory = { { cond = 'eliminate_all' }, function() end },
}
format {
  id = 'open', name = 'Open', ruleset = 'core', selection = 'build',
  scenarios = { pool = { 'duel' }, pick = 'alternate_veto' },
  army = { { max_copies = 2 } },
}
"#;

#[test]
fn plain_data_in_scenario_and_format_lists_freezes_as_native_logic() {
    let records = run(LISTS).unwrap();
    let native = |key: &str, value: Value| {
        Logic::Native(Descriptor(Value::Map([(Sym::new(key), value)].into())))
    };
    for record in &records {
        match &record.def {
            FrozenDef::Scenario(scenario) => {
                assert_eq!(scenario.army, [native("max_points", Value::Int(100))]);
                let cond = Value::Str(Sym::new("eliminate_all"));
                assert_eq!(scenario.victory[0], native("cond", cond));
                assert!(matches!(scenario.victory[1], Logic::Script(_)));
            }
            FrozenDef::Format(format) => {
                assert_eq!(format.army, [native("max_copies", Value::Int(2))]);
            }
            other => panic!("unexpected record {other:?}"),
        }
    }
}

const BAD_FOOTPRINTS: &str = r#"
footprint { id = 'empty', name = 'E', cells = {}, move = 'rigid', max_spread = 1 }
footprint { id = 'off_anchor', name = 'O', cells = {{ q = 1, r = 0 }, { q = 2, r = 0 }}, move = 'rigid', max_spread = 1 }
footprint { id = 'split', name = 'S', cells = {{ q = 0, r = 0 }, { q = 2, r = 0 }}, move = 'rigid', max_spread = 1 }
footprint { id = 'long_snake', name = 'L', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }, { q = 2, r = 0 }}, move = 'snake', max_spread = 1 }
footprint { id = 'long_rigid', name = 'R', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }, { q = 2, r = 0 }}, move = 'rigid', max_spread = 1 }
footprint { id = 'two_snake', name = 'T', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }}, move = 'snake', max_spread = 1 }
"#;

const BAD_DESCRIPTORS: &str = r#"
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
ability {
  id = 'sulk', name = 'Sulk', text = 'Sulks.', scope = 'figure',
  modifiers = {{ query = 'defense', layer = 'add', value = 1, when = { cond = 'sulking' } }},
}
unit {
  id = 'grump', name = 'Grump', kind = 'squad', rarity = 'common', stats = {},
  figures = {{
    id = 'f', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
  abilities = {
    { name = 'Fine', text = 'Fine.', scope = 'figure', reactions = {{
      window = 'w', when = { cond = 'attack_kind', is = 'ranged' }, effect = { eff = 'cancel_wounds' },
    }} },
    { name = 'Typo', text = 'Typo.', scope = 'figure',
      triggers = {{ event = 'attack', effect = { eff = 'wound', amonut = 2 } }} },
  },
}
relic {
  id = 'dud', name = 'Dud', mode = 'on_claim', claim = 'ends_move_on',
  effect = { sel = 'enemies' },
}
"#;

#[test]
fn unknown_or_malformed_descriptors_fail_at_load_with_their_route() {
    let errors = run(BAD_DESCRIPTORS).expect_err("bad descriptors must not freeze");
    let routes: Vec<(&str, &str)> = errors
        .iter()
        .map(|e| (e.field_path.as_str(), e.message.as_str()))
        .collect();
    assert_eq!(
        routes,
        [
            (
                "modifiers/1/when",
                "`t:sulk`: unknown descriptor `cond.sulking`"
            ),
            (
                "effect",
                "relic `t:dud`: an effect must be an `eff` descriptor, `{ native = \"<name>\" }` or a function",
            ),
            (
                "abilities/2/triggers/1/effect",
                "`t:grump`: `eff.wound` has no field `amonut`",
            ),
        ],
        "{errors:#?}"
    );
}

#[test]
fn every_descriptor_of_the_vocabulary_freezes() {
    let records = run(r#"
ability {
  id = 'all', name = 'All', text = 'All.', scope = 'figure',
  modifiers = {{ query = 'attack', layer = 'add', value = { val = 'count', of = { sel = 'target' } },
                 select = { sel = 'allies', engaged = true, in_los = 'self' },
                 when = { cond = 'all', of = { { cond = 'engaged' }, { cond = 'in_los', to = 'target' },
                                               { cond = 'attack_kind', is = 'melee' } } } }},
  actions = {{ id = 'mend', slots = { 'attack' }, targets = { sel = 'allies', within = 1 },
               resolve = { eff = 'heal', amount = 2 } }},
  triggers = {
    { event = 'a', effect = { eff = 'wound' } },
    { event = 'b', effect = { eff = 'place', within = 2 } },
    { event = 'c', effect = { eff = 'remove' } },
    { event = 'd', effect = { eff = 'move', up_to = 1 } },
    { event = 'e', effect = { eff = 'grant_slot', slot = 'attack' } },
    { event = 'f', effect = { eff = 'add_marker', marker = 'stunned', rounds = 1 } },
    { event = 'g', effect = { eff = 'take_control' } },
  },
  replacements = {{ event = 'wounds_applied', replace = { eff = 'cancel_wounds' } }},
  reactions = {{ window = 'w', effect = { eff = 'excess_blocks_wound_attacker' } }},
}
"#);
    let records = records.unwrap();
    let FrozenDef::Ability(ability) = &records[0].def else {
        panic!("not an ability")
    };
    assert!(strata_rules::check_ability(ability).is_ok());
    assert_eq!(ability.triggers.len(), 7);
}

#[test]
fn unusable_footprint_shapes_are_refused_by_name() {
    let errors = run(BAD_FOOTPRINTS).expect_err("bad footprints must not freeze");
    let find = |id: &str| {
        let named = format!("footprint `t:{id}`");
        let found = errors.iter().find(|e| e.message.contains(&named));
        found.unwrap_or_else(|| panic!("no error for `{id}` in {errors:#?}"))
    };
    assert_eq!(errors.len(), 4, "{errors:#?}");
    let empty = find("empty");
    assert_eq!(empty.field_path, "cells");
    assert!(empty.message.contains("at least one cell"), "{empty}");
    assert!(find("off_anchor").message.contains("anchor cell"));
    assert!(find("split").message.contains("connected"));
    let snake = find("long_snake");
    assert_eq!(snake.field_path, "move");
    assert!(snake.message.contains("use `move = \"rigid\"`"), "{snake}");
    assert!(
        snake
            .to_string()
            .starts_with("t/data.lua:5: move: footprint `t:long_snake`")
    );
}
