//! Round-trips each definition type through its hand-written JSON fixture.

use std::fmt::Debug;

use serde::Serialize;
use serde::de::DeserializeOwned;
use strata_kernel::{SlotId, StableHash, StableHasher};
use strata_model::*;

fn hash<T: StableHash>(value: &T) -> u64 {
    let mut hasher = StableHasher::new();
    value.stable_hash(&mut hasher);
    hasher.finish()
}

fn round_trip<T>(json: &str) -> T
where
    T: Serialize + DeserializeOwned + StableHash + Clone + Debug + PartialEq,
{
    let first: T = serde_json::from_str(json).expect("fixture parses");
    let again: T = serde_json::from_str(&serde_json::to_string(&first).unwrap()).unwrap();
    assert_eq!(first, again);
    assert_eq!(hash(&first), hash(&again));
    assert_eq!(hash(&first), hash(&first.clone()));
    first
}

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("fixtures/", $name, ".json"))
    };
}

#[test]
fn stat() {
    let def: StatDef = round_trip(fixture!("stat"));
    assert_eq!((def.min, def.max, def.order), (0, 99, 5));
}

#[test]
fn die_with_faces_and_range() {
    let faces: DieDef = round_trip(fixture!("die_faces"));
    assert!(matches!(&faces.kind, DieKind::Faces(f) if f.len() == 6));
    let range: DieDef = round_trip(fixture!("die_range"));
    assert_eq!(range.kind, DieKind::Range(1, 20));
}

#[test]
fn marker() {
    let def: MarkerDef = round_trip(fixture!("marker"));
    assert_eq!(def.stacking.as_str(), "count");
}

#[test]
fn traits() {
    let def: TraitsDef = round_trip(fixture!("traits"));
    assert_eq!(def.keys.len(), 2);
}

#[test]
fn faction() {
    let def: FactionDef = round_trip(fixture!("faction"));
    assert_eq!(def.color, "#B5893A");
}

#[test]
fn terrain() {
    let def: TerrainDef = round_trip(fixture!("terrain"));
    assert_eq!(def.abilities.len(), 1);
}

#[test]
fn tile() {
    let def: TileDef = round_trip(fixture!("tile"));
    assert_eq!((def.thickness, def.shape.len()), (1, 2));
}

#[test]
fn feature() {
    let def: FeatureDef = round_trip(fixture!("feature"));
    assert!(def.blocks_los);
    assert!(!def.cells[0].segments[0].top);
    assert!(def.cells[1].segments[0].top);
}

#[test]
fn footprint() {
    let def: FootprintDef = round_trip(fixture!("footprint"));
    assert_eq!((def.move_mode, def.max_spread), (MoveMode::Snake, 1));
}

#[test]
fn map() {
    let def: MapDef = round_trip(fixture!("map"));
    assert_eq!(def.pieces.len(), 2);
    assert!(def.constraints.require_support);
}

#[test]
fn inventory() {
    let def: InventoryDef = round_trip(fixture!("inventory"));
    assert_eq!(def.tiles.len(), 2);
}

#[test]
fn hash_depends_on_content() {
    let a: StatDef = serde_json::from_str(fixture!("stat")).unwrap();
    let mut b = a.clone();
    b.max = 98;
    assert_ne!(hash(&a), hash(&b));
}

#[test]
fn unit_with_figures_and_abilities() {
    let def: UnitDef = round_trip(fixture!("unit"));
    let figure = &def.figures[0];
    assert_eq!((figure.count, figure.height), (4, 5));
    assert_eq!((figure.los.target, figure.los.hit_zone), (54, (0, 60)));
    let ability = &def.abilities[0];
    assert_eq!(ability.scope, Scope::Figure);
    assert_eq!(ability.archetype.as_ref().unwrap().family.as_str(), "aura");
    assert_eq!(ability.modifiers[0].layer, ModifierLayer::Add);
    assert!(!ability.modifiers[0].overrides);
    assert!(matches!(ability.actions[0].resolve, Logic::Script(_)));
    assert!(matches!(ability.modifiers[0].value, Logic::Native(_)));
    assert_eq!(ability.triggers[0].limit, Some(TriggerLimit::OncePerTurn));
    assert_eq!(ability.reactions[0].window.as_str(), "combat.after_wounds");
}

#[test]
fn relic() {
    let def: RelicDef = round_trip(fixture!("relic"));
    assert_eq!(def.mode, RelicMode::WhileOccupied);
    assert_eq!(def.grants.unwrap().abilities.len(), 1);
}

#[test]
fn ruleset_slots_and_defaults() {
    let def: RulesetDef = round_trip(fixture!("ruleset"));
    assert_eq!(def.slots.len(), 2);
    assert_eq!(def.slots[&SlotId::new("tokens")], None);
    assert!(!def.abilities);
    let minimal: RulesetDef = serde_json::from_str(r#"{"id":1}"#).unwrap();
    assert!(minimal.abilities);
    assert_eq!(minimal.undo, UndoPolicy::UntilRandomOrReveal);
}

#[test]
fn rule_module() {
    let def: RuleModuleDef = round_trip(fixture!("rule_module"));
    assert_eq!(def.slot, Some(SlotId::new("combat")));
    assert_eq!(def.config.len(), 1);
}

#[test]
fn scenario() {
    let def: ScenarioDef = round_trip(fixture!("scenario"));
    assert_eq!((def.players.min, def.slots.len()), (2, 2));
    assert_eq!(def.after.len(), 1);
}

#[test]
fn format() {
    let def: FormatDef = round_trip(fixture!("format"));
    assert_eq!((def.round_limit, def.clock_minutes), (Some(12), Some(45)));
}

#[test]
fn army_list() {
    let def: ArmyList = round_trip(fixture!("army"));
    assert_eq!(def.units[0].copies, 2);
}

#[test]
fn test_def() {
    let def: TestDef = round_trip(fixture!("test_def"));
    assert_eq!(def.source.line, 1);
}

#[test]
fn unit_hash_depends_on_los_profile() {
    let a: UnitDef = serde_json::from_str(fixture!("unit")).unwrap();
    let mut b = a.clone();
    b.figures[0].los.hit_zone = (0, 59);
    assert_ne!(hash(&a), hash(&b));
}
