use std::path::PathBuf;
use std::sync::Arc;

use strata_content::{ContentRegistry, load};
use strata_editor::{
    Edit, FindingKind, MapDocument, Objective, RegistryDefs, Severity, Validation,
    ValidationOptions,
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
    MapDocument::new(map, RegistryDefs::new(registry, "t".to_owned()))
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
    let spreads: Vec<_> = fairness.objectives.iter().map(Objective::spread).collect();
    assert_eq!(spreads, vec![Some(2), Some(0)]);
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
    let tree = FindingKind::InventoryOverrun {
        name: sym("tree"),
        used: 2,
        available: 1,
    };
    // `grass_1` and `t:grass_1` count as one tile, and `water_1` matches the box's `t:water_1`,
    // so neither spelling gets an overrun of its own.
    let grass_cells = hexes(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
    assert_eq!(
        findings(&validation),
        vec![
            (Severity::Error, grass, grass_cells),
            (Severity::Error, hill, hexes(&[(4, 0)])),
            (Severity::Error, tree, hexes(&[(1, 0), (2, 0)])),
        ]
    );
}

fn set_require_support(document: &mut MapDocument<RegistryDefs>, require_support: bool) {
    let constraints = strata_model::MapConstraints {
        inventory: None,
        require_support,
    };
    let edit = Edit::SetMetadata(strata_editor::Metadata::Constraints(constraints));
    document.apply(edit).unwrap();
}

fn is_unsupported(document: &mut MapDocument<RegistryDefs>) -> bool {
    let validation = document.validate(&ValidationOptions::default());
    let mut kinds = validation.findings.iter().map(|f| &f.kind);
    kinds.any(|kind| matches!(kind, FindingKind::Unsupported { .. }))
}

#[test]
fn toggling_require_support_recompiles_the_board() {
    let mut document = document("unsupported");
    assert!(is_unsupported(&mut document));
    assert!(document.board().is_err());

    set_require_support(&mut document, false);
    assert!(!is_unsupported(&mut document));
    assert!(document.board().is_ok());

    document.undo().unwrap();
    assert!(is_unsupported(&mut document));
    document.redo().unwrap();
    assert!(!is_unsupported(&mut document));

    set_require_support(&mut document, true);
    assert!(is_unsupported(&mut document));
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
    // `island` is across water; `tower` is a climb of two levels, one more than the reference
    // figure manages.
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
fn the_reference_figure_drops_where_it_cannot_climb() {
    // From the top of the tower the figure steps down two levels to `start_b`, but cannot climb
    // back, as `core:movement` decides with a climb limit of one.
    let mut document = document("unreachable_relic");
    let edit = Edit::SetZone {
        zone: sym("start_a"),
        cells: Some(vec![Hex::new(0, 1)]),
    };
    document.apply(edit).unwrap();
    let validation = document.validate(&ValidationOptions::default());
    let fairness = validation.fairness.as_ref().unwrap();
    assert_eq!(
        fairness.between,
        vec![vec![Some(0), Some(1)], vec![None, Some(0)]]
    );
    let tower = &fairness.objectives[1];
    assert_eq!(tower.steps, vec![Some(0), None]);
    let unreachable = FindingKind::StartZoneUnreachable {
        from: sym("start_b"),
        to: sym("start_a"),
    };
    let kinds: Vec<_> = validation.findings.iter().map(|f| f.kind.clone()).collect();
    let island = FindingKind::UnreachableRelicSpot {
        spot: sym("island"),
    };
    assert_eq!(kinds, vec![unreachable, island]);
}

#[test]
fn the_reference_figure_drops_any_height_and_walks_on() {
    // From the top of the cliff the figure drops four levels, more than `core:movement`'s
    // default fall limit of two, and still walks on to the relic spot. A fall limit would end
    // its move at the foot of the cliff.
    let validation = check("cliff", None);
    assert_eq!(findings(&validation), vec![]);
    let fairness = validation.fairness.as_ref().unwrap();
    let foot = &fairness.objectives[0];
    assert_eq!(
        (foot.at, foot.steps.clone()),
        (Hex::new(2, 0), vec![Some(2)])
    );
}

#[test]
fn a_start_zone_with_nowhere_to_stand_is_only_unstandable() {
    // `start_b` is moved onto water, off the board, and emptied in turn. Each time it is
    // unstandable, and no zone-to-zone reachability is reported on top of that.
    for cells in [hexes(&[(2, 0)]), hexes(&[(9, 9)]), vec![]] {
        let mut document = document("fair");
        let edit = Edit::SetZone {
            zone: sym("start_b"),
            cells: Some(cells.clone()),
        };
        document.apply(edit).unwrap();
        let validation = document.validate(&ValidationOptions::default());
        let kind = FindingKind::UnstandableZone {
            zone: sym("start_b"),
        };
        assert_eq!(
            findings(&validation),
            vec![(Severity::Warning, kind, cells.clone())],
            "{cells:?}"
        );
        let fairness = validation.fairness.as_ref().unwrap();
        assert_eq!(
            fairness.between,
            vec![vec![Some(0), None], vec![None, None]]
        );
    }
}

#[test]
fn a_relic_spot_one_start_zone_reaches_is_not_unreachable() {
    let validation = check("apart", None);
    let fairness = validation.fairness.as_ref().unwrap();
    assert_eq!(fairness.objectives[0].steps, vec![Some(0), None]);
    assert_eq!(fairness.objectives[0].spread(), None);
    let mut kinds = validation.findings.iter().map(|f| &f.kind);
    assert!(!kinds.any(|k| matches!(k, FindingKind::UnreachableRelicSpot { .. })));
}

#[test]
fn start_zones_that_cannot_reach_each_other() {
    let validation = check("apart", None);
    let apart = |from: &str, to: &str| FindingKind::StartZoneUnreachable {
        from: sym(from),
        to: sym(to),
    };
    let warning = Severity::Warning;
    assert_eq!(
        findings(&validation),
        vec![
            (warning, apart("start_a", "start_b"), hexes(&[(2, 0)])),
            (warning, apart("start_b", "start_a"), hexes(&[(0, 0)])),
        ]
    );
    let fairness = validation.fairness.as_ref().unwrap();
    assert_eq!(
        fairness.between,
        vec![vec![Some(0), None], vec![None, Some(0)]]
    );
}

#[test]
fn moving_a_relic_spot_changes_the_report_through_undo_and_redo() {
    let mut document = document("fair");
    let options = ValidationOptions::default();
    let before = document.validate(&options);
    let edit = Edit::SetRelicSpot {
        name: sym("east"),
        at: Some(Hex::new(1, 0)),
    };
    document.apply(edit).unwrap();
    let after = document.validate(&options);
    let east = &after.fairness.as_ref().unwrap().objectives[0];
    assert_eq!(
        (east.at, east.steps.clone()),
        (Hex::new(1, 0), vec![Some(1), Some(4)])
    );
    document.undo().unwrap();
    assert_eq!(*document.validate(&options), *before);
    document.redo().unwrap();
    assert_eq!(*document.validate(&options), *after);
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
    let fairness = validation.fairness.as_ref().unwrap();
    assert_eq!(
        fairness.to_string(),
        "fairness: no start zones to compare\n"
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

    let edit = Edit::SetZone {
        zone: sym("start_a"),
        cells: Some(vec![Hex::new(0, 0)]),
    };
    document.apply(edit).unwrap();
    let edited = document.validate(&sized);
    assert!(!Arc::ptr_eq(&other, &edited));
    assert_eq!(edited.findings[0].cells, vec![Hex::new(0, 0)]);
    assert_ne!(*edited, *other);

    document.undo().unwrap();
    let undone = document.validate(&sized);
    assert!(!Arc::ptr_eq(&edited, &undone));
    assert_eq!(*undone, *other);

    document.redo().unwrap();
    let redone = document.validate(&sized);
    assert!(!Arc::ptr_eq(&undone, &redone));
    assert_eq!(*redone, *edited);
    assert!(Arc::ptr_eq(&redone, &document.validate(&sized)));

    // Removing the grass under (0, 0) changes the board, so the zone there becomes unstandable.
    document.apply(Edit::RemovePiece { index: 0 }).unwrap();
    let removed = document.validate(&sized);
    assert!(!Arc::ptr_eq(&redone, &removed));
    let kind = FindingKind::UnstandableZone {
        zone: sym("start_a"),
    };
    let found = removed.findings.iter().find(|f| f.kind == kind).unwrap();
    assert_eq!(found.cells, vec![Hex::new(0, 0)]);
    assert!(Arc::ptr_eq(&removed, &document.validate(&sized)));

    document.undo().unwrap();
    assert_eq!(*document.validate(&sized), *edited);
}

#[test]
fn validation_is_deterministic() {
    let names = ["fair", "unstandable_zone", "small_zone", "inventory"];
    for name in names.into_iter().chain(["unreachable_relic", "apart"]) {
        assert_eq!(*check(name, Some(2)), *check(name, Some(2)), "{name}");
    }
}
