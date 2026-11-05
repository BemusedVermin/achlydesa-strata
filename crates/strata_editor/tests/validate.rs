use std::path::PathBuf;
use std::sync::Arc;

use strata_content::{ContentRegistry, load};
use strata_editor::{
    Edit, FindingKind, MapDocument, RegistryDefs, Severity, Validation, ValidationOptions,
};
use strata_kernel::{Hex, Sym};
use strata_model::MapDef;

fn registry() -> ContentRegistry {
    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/maps"));
    load(&[root]).unwrap()
}

/// Opens the fixture map `t:name` for editing.
fn document(name: &str) -> MapDocument<RegistryDefs> {
    let registry = registry();
    let (_, map) = registry.find::<MapDef>(&format!("t:{name}")).unwrap();
    let map = map.clone();
    MapDocument::new(map, RegistryDefs::new(registry, "t"))
}

fn check(name: &str, army_size: Option<u32>) -> Arc<Validation> {
    document(name).validate(&ValidationOptions { army_size })
}

/// The kinds and cells of every finding, for compact comparisons.
fn findings(validation: &Validation) -> Vec<(Severity, FindingKind, Vec<Hex>)> {
    let f = validation.findings.iter();
    f.map(|f| (f.severity, f.kind.clone(), f.cells.clone()))
        .collect()
}

fn hexes(cells: &[(i32, i32)]) -> Vec<Hex> {
    cells.iter().map(|&(q, r)| Hex::new(q, r)).collect()
}

fn sym(s: &str) -> Sym {
    Sym::new(s)
}

#[test]
fn a_fair_map_has_no_findings() {
    let validation = check("fair", Some(2));
    assert_eq!(findings(&validation), vec![]);
}

#[test]
fn fairness_report() {
    let validation = check("fair", None);
    let fairness = validation.fairness.as_ref().unwrap();
    assert_eq!(fairness.start_zones, vec![sym("start_a"), sym("start_b")]);
    insta::assert_snapshot!(fairness.to_string());
}

#[test]
fn overlapping_pieces() {
    let validation = check("overlap", None);
    let kind = FindingKind::Overlap { piece: 1, other: 0 };
    assert_eq!(
        findings(&validation),
        vec![(Severity::Error, kind, hexes(&[(0, 0)]))]
    );
    assert_eq!(validation.fairness, None);
}

#[test]
fn unsupported_cells() {
    let validation = check("unsupported", None);
    let kind = FindingKind::Unsupported { piece: 1 };
    assert_eq!(
        findings(&validation),
        vec![(Severity::Error, kind, hexes(&[(1, 0)]))]
    );
}

#[test]
fn inventory_overruns() {
    let validation = check("inventory", None);
    let grass = FindingKind::InventoryOverrun {
        name: sym("grass_1"),
        used: 4,
        available: 3,
    };
    let hill = FindingKind::InventoryOverrun {
        name: sym("hill_2"),
        used: 1,
        available: 0,
    };
    let grass_cells = hexes(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
    assert_eq!(
        findings(&validation),
        vec![
            (Severity::Error, grass, grass_cells),
            (Severity::Error, hill, hexes(&[(4, 0)])),
        ]
    );
}

#[test]
fn an_unknown_inventory_is_an_error() {
    let mut document = document("inventory");
    let constraints = strata_model::MapConstraints {
        inventory: Some(sym("nope")),
        require_support: false,
    };
    let edit = Edit::SetMetadata(strata_editor::Metadata::Constraints(constraints));
    document.apply(edit).unwrap();
    let validation = document.validate(&ValidationOptions::default());
    let kind = FindingKind::UnknownInventory { name: sym("nope") };
    assert_eq!(findings(&validation), vec![(Severity::Error, kind, vec![])]);
}

#[test]
fn zones_on_unstandable_columns() {
    let validation = check("unstandable_zone", None);
    let kind = FindingKind::UnstandableZone { zone: sym("ford") };
    assert_eq!(
        findings(&validation),
        vec![(Severity::Warning, kind, hexes(&[(2, 0), (5, 5)]))]
    );
}

#[test]
fn start_zones_too_small_for_the_army() {
    assert_eq!(findings(&check("small_zone", Some(2))), vec![]);
    let validation = check("small_zone", Some(3));
    let kind = FindingKind::StartZoneTooSmall {
        zone: sym("start_a"),
        spaces: 2,
        army: 3,
    };
    assert_eq!(
        findings(&validation),
        vec![(Severity::Error, kind, hexes(&[(0, 0), (1, 0)]))]
    );
}

#[test]
fn relic_spots_unreachable_from_start_zones() {
    let validation = check("unreachable_relic", None);
    let spot = |name: &str| FindingKind::UnreachableRelicSpot { spot: sym(name) };
    assert_eq!(
        findings(&validation),
        vec![
            (Severity::Error, spot("island"), hexes(&[(3, 0)])),
            (Severity::Error, spot("tower"), hexes(&[(0, 1)])),
        ]
    );
}

#[test]
fn start_zones_that_cannot_reach_each_other() {
    let validation = check("apart", None);
    let apart = |from: &str, to: &str| FindingKind::StartZoneUnreachable {
        from: sym(from),
        to: sym(to),
    };
    assert_eq!(
        findings(&validation),
        vec![
            (
                Severity::Warning,
                apart("start_a", "start_b"),
                hexes(&[(2, 0)])
            ),
            (
                Severity::Warning,
                apart("start_b", "start_a"),
                hexes(&[(0, 0)])
            ),
        ]
    );
    let fairness = validation.fairness.as_ref().unwrap();
    assert_eq!(fairness.objectives[0].steps, vec![Some(0), None]);
    assert_eq!(fairness.objectives[0].spread(), None);
}

#[test]
fn a_map_without_start_zones_says_so() {
    let mut document = document("small_zone");
    for zone in ["start_a", "start_b"] {
        let edit = Edit::SetZone {
            zone: sym(zone),
            cells: None,
        };
        document.apply(edit).unwrap();
    }
    let validation = document.validate(&ValidationOptions::default());
    let kind = FindingKind::NoStartZones;
    assert_eq!(
        findings(&validation),
        vec![(Severity::Warning, kind, vec![])]
    );
}

#[test]
fn validation_of_an_unchanged_document_is_cached() {
    let mut document = document("fair");
    let options = ValidationOptions::default();
    let first = document.validate(&options);
    assert!(Arc::ptr_eq(&first, &document.validate(&options)));

    let sized = ValidationOptions { army_size: Some(9) };
    let other = document.validate(&sized);
    assert!(!Arc::ptr_eq(&first, &other));
    assert!(Arc::ptr_eq(&other, &document.validate(&sized)));

    let edit = Edit::SetRelicSpot {
        name: sym("west"),
        at: Some(Hex::new(0, 0)),
    };
    document.apply(edit).unwrap();
    let edited = document.validate(&sized);
    assert!(!Arc::ptr_eq(&other, &edited));
    assert_eq!(edited.fairness.as_ref().unwrap().objectives.len(), 3);

    document.undo().unwrap();
    let undone = document.validate(&sized);
    assert!(!Arc::ptr_eq(&edited, &undone));
    assert_eq!(*undone, *other);
}

#[test]
fn validation_is_deterministic() {
    for name in ["fair", "apart", "unreachable_relic", "inventory"] {
        assert_eq!(*check(name, Some(2)), *check(name, Some(2)), "{name}");
    }
}
