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
    modifiers = {{ query = 'defense', layer = 'add', when = { cond = 'adjacent' }, value = 1 }},
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
        assert_eq!(modifier.when, Some(native(&[("cond", "adjacent")])));
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
