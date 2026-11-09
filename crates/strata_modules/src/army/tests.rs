//! The native army rules on hand-built armies; whole lists are covered by pack tests.

use strata_kernel::{DefId, Sym, Value};
use strata_model::Descriptor;
use strata_rules::{ArmyRules, ArmyUnit, NameKind};

use super::Rules;

fn rule(fields: &[(&str, Value)]) -> Descriptor {
    let map = fields.iter().map(|(k, v)| (Sym::new(k), v.clone()));
    Descriptor(Value::Map(map.collect()))
}

fn soldier(points: Option<i64>) -> ArmyUnit {
    ArmyUnit {
        unit: Sym::new("p:test_soldier"),
        def: DefId(0),
        copies: 2,
        points,
        rarity: Sym::new("common"),
        faction: None,
        faction_def: None,
        footprint: 1,
    }
}

fn nothing(_: NameKind, _: &str) -> Option<DefId> {
    None
}

fn max_points(limit: i64) -> Descriptor {
    rule(&[
        ("army_rule", Value::Str(Sym::new("max_points"))),
        ("points", Value::Int(limit)),
    ])
}

#[test]
fn max_points_prices_each_copy_at_its_printed_points() {
    let found = Rules.check(&max_points(50), &[soldier(Some(30))], &nothing);
    assert_eq!(
        found.unwrap(),
        ["the army costs 60 points, over the limit of 50"]
    );
    let found = Rules.check(&max_points(60), &[soldier(Some(30))], &nothing);
    assert_eq!(found.unwrap(), Vec::<String>::new());
}

#[test]
fn max_points_without_a_points_stat_is_an_error() {
    let found = Rules.check(&max_points(50), &[soldier(None)], &nothing);
    assert_eq!(
        found.unwrap_err().0,
        "army_rule.max_points: the packs define no `points` stat"
    );
}

#[test]
fn bans_take_one_name_or_a_list_and_compare_definitions() {
    let found_as = |id| {
        move |kind: NameKind, name: &str| {
            (kind == NameKind::Unit && name == "test_soldier").then_some(DefId(id))
        }
    };
    let one = rule(&[
        ("army_rule", Value::Str(Sym::new("banned"))),
        ("units", Value::Str(Sym::new("test_soldier"))),
    ]);
    let found = Rules.check(&one, &[soldier(Some(30))], &found_as(0));
    assert_eq!(found.unwrap(), ["`p:test_soldier` is banned"]);
    // The same bare name, resolved to another pack's unit, bans nothing here.
    let list = rule(&[
        ("army_rule", Value::Str(Sym::new("banned"))),
        (
            "units",
            Value::List(vec![Value::Str(Sym::new("test_soldier"))]),
        ),
    ]);
    let found = Rules.check(&list, &[soldier(Some(30))], &found_as(7));
    assert_eq!(found.unwrap(), Vec::<String>::new());
}
