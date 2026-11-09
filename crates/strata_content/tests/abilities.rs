//! Mapping an `AbilityKey` back to its definition (#108).

use strata_content::{ContentRegistry, load};
use strata_kernel::DefId;
use strata_model::{AbilityKey, Logic, RelicDef, UnitDef};

const DATA: &str = r#"
stat { id = 'attack', name = 'Attack', min = 0, max = 9 }
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
local function inline(name, amount)
  return {
    name = name, text = 'Add attack.', scope = 'unit',
    modifiers = {{ query = 'attack', layer = 'add', value = amount }},
  }
end
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = { attack = 1 },
  figures = {{
    id = 's', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
  abilities = { inline('Drill', 1), inline('Brace', 2) },
}
ability { id = 'sharp', name = 'Sharp', text = 'Add attack.', scope = 'army',
  modifiers = {{ query = 'attack', layer = 'add', value = function() return 3 end }} }
relic {
  id = 'fury', name = 'Relic of Fury', mode = 'while_occupied', claim = 'ends_move_on',
  grants = { to = 'occupant_army', abilities = { inline('Fury', 4) } },
}
"#;

fn registry() -> (tempfile::TempDir, ContentRegistry) {
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("t");
    std::fs::create_dir_all(&pack).unwrap();
    let manifest = "return { id = 't', version = '1.0.0', name = 't', engine = '>=0.1, <0.2' }";
    std::fs::write(pack.join("pack.lua"), manifest).unwrap();
    std::fs::write(pack.join("data.lua"), DATA).unwrap();
    let registry = load(&[dir.path().to_owned()]).unwrap();
    (dir, registry)
}

fn unit(index: u16) -> AbilityKey {
    AbilityKey::Unit {
        unit: DefId(0),
        index,
    }
}

#[test]
fn every_kind_of_key_finds_its_definition() {
    let (_dir, registry) = registry();
    let name = |key| registry.ability(key).map(|a| a.name.0.clone());
    assert_eq!(name(unit(0)).as_deref(), Some("Drill"));
    assert_eq!(name(unit(1)).as_deref(), Some("Brace"));
    assert_eq!(name(AbilityKey::Def(DefId(0))).as_deref(), Some("Sharp"));
    let relic = AbilityKey::Relic {
        relic: DefId(0),
        index: 0,
    };
    assert_eq!(name(relic).as_deref(), Some("Fury"));
    // The unit's ability is the same entry its definition holds.
    let soldier = registry.get::<UnitDef>(DefId(0)).unwrap();
    assert_eq!(registry.ability(unit(1)), soldier.abilities.get(1));
    let fury = registry.get::<RelicDef>(DefId(0)).unwrap();
    assert_eq!(
        registry.ability(relic),
        fury.grants.as_ref().unwrap().abilities.first()
    );
}

#[test]
fn keys_name_their_ability_by_record_and_route() {
    let (_dir, registry) = registry();
    let relic = AbilityKey::Relic {
        relic: DefId(0),
        index: 0,
    };
    let names = [unit(1), AbilityKey::Def(DefId(0)), relic].map(|k| registry.ability_name(k));
    assert_eq!(
        names.map(Option::unwrap),
        [
            "t:test_soldier/abilities/2",
            "t:sharp",
            "t:fury/grants/abilities/1"
        ]
    );
}

#[test]
fn a_key_past_the_end_or_to_a_missing_record_finds_nothing() {
    let (_dir, registry) = registry();
    let missing = [
        unit(2),
        AbilityKey::Unit {
            unit: DefId(9),
            index: 0,
        },
        AbilityKey::Def(DefId(9)),
        AbilityKey::Relic {
            relic: DefId(0),
            index: 1,
        },
    ];
    for key in missing {
        assert_eq!(registry.ability(key), None, "{key:?}");
        assert_eq!(registry.ability_name(key), None, "{key:?}");
    }
}

#[test]
fn plain_data_in_an_inline_ability_freezes_as_native_logic() {
    let (_dir, registry) = registry();
    let value = |key| registry.ability(key).unwrap().modifiers[0].value.clone();
    assert!(matches!(value(unit(0)), Logic::Native(_)));
    assert!(matches!(value(AbilityKey::Def(DefId(0))), Logic::Script(_)));
}
