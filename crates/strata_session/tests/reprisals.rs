//! Reprisals in the move action and the `provokes_reprisal` flag, through the `reprisals` fixture
//! pack, `core:movement` and `core:engagement`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use strata_kernel::{Space, Value};
use strata_model::{Event, FigureId};
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/reprisals")];
    let loaded = PackTests::load(&roots).unwrap();
    let tests = native_modules()
        .into_iter()
        .try_fold(loaded, PackTests::with_native);
    tests.unwrap().run(None)
}

/// The passing report of the test called `name`.
fn passed<'a>(reports: &'a [TestReport], name: &str) -> &'a TestReport {
    let found = reports.iter().find(|r| r.name == name);
    let found = found.unwrap_or_else(|| panic!("no test `{name}`:\n{}", summary(reports)));
    assert!(found.outcome.is_ok(), "{}", summary(reports));
    found
}

/// The custom events called `id`, by payload.
fn custom<'a>(report: &'a TestReport, id: &'a str) -> impl Iterator<Item = &'a Value> + 'a {
    report.events.iter().filter_map(move |e| match e {
        Event::Custom { id: kind, payload } if kind.as_str() == id => Some(payload),
        _ => None,
    })
}

/// The `provokes_reprisal` flag of every stop `survey` previewed, by `(q, r, o)`; `o` is -1 for
/// one-space figures.
fn flags(report: &TestReport) -> BTreeMap<(i64, i64, i64), bool> {
    let int = |p: &Value, k: &str| p.get(k).and_then(Value::as_int);
    let stop = |p: &Value| {
        let o = int(p, "o").unwrap_or(-1);
        let flagged = p.get("flagged").and_then(Value::as_bool).unwrap();
        ((int(p, "q").unwrap(), int(p, "r").unwrap(), o), flagged)
    };
    let found: BTreeMap<_, _> = custom(report, "previewed").map(stop).collect();
    assert!(!found.is_empty(), "{} previewed nothing", report.name);
    found
}

/// A stop or space as `(q, r, o)` or `(q, r, z)`.
type Triple = (i64, i64, i64);

/// The `path` of every stop `survey` previewed, by `(q, r, o)` as in [`flags`], as `(q, r, z)`.
fn paths(report: &TestReport) -> BTreeMap<Triple, Vec<Triple>> {
    let int = |p: &Value, k: &str| p.get(k).and_then(Value::as_int);
    let step = |s: &Value| {
        (
            int(s, "q").unwrap(),
            int(s, "r").unwrap(),
            int(s, "z").unwrap(),
        )
    };
    let stop = |p: &Value| {
        let o = int(p, "o").unwrap_or(-1);
        let Ok(Value::List(path)) = p.get("path") else {
            panic!("a stop without a path: {p:?}");
        };
        let path = path.iter().map(step).collect();
        ((int(p, "q").unwrap(), int(p, "r").unwrap(), o), path)
    };
    custom(report, "previewed").map(stop).collect()
}

/// The figures that rolled a reprisal, in order.
fn reprisals(report: &TestReport) -> Vec<i64> {
    let figure = |p: &Value| p.get("figure").and_then(Value::as_int).unwrap();
    custom(report, "reprisal").map(figure).collect()
}

fn rolls(report: &TestReport) -> usize {
    let rolled = report.events.iter();
    rolled
        .filter(|e| matches!(e, Event::DiceRolled { .. }))
        .count()
}

/// The paths of the moves the test committed.
fn moves(report: &TestReport) -> Vec<Vec<Space>> {
    let moved = report.events.iter().filter_map(|e| match e {
        Event::FigureMoved { path, .. } => Some(path.clone()),
        _ => None,
    });
    moved.collect()
}

#[test]
fn stops_whose_route_leaves_an_engaged_enemy_are_flagged() {
    let reports = run();
    let flagged = flags(passed(
        &reports,
        "stops whose route leaves an engaged enemy are flagged",
    ));
    // The enemy is on (-1, 0). These stops are not next to it, so every route there leaves it;
    // (-3, 1) lies beyond it.
    for stop in [(1, 0, -1), (1, -1, -1), (-3, 1, -1)] {
        assert_eq!(flagged.get(&stop), Some(&true), "{stop:?}: {flagged:?}");
    }
    // One step to these keeps the mover next to the enemy.
    for stop in [(0, -1, -1), (-1, 1, -1)] {
        assert_eq!(flagged.get(&stop), Some(&false), "{stop:?}: {flagged:?}");
    }
}

#[test]
fn without_an_engagement_module_no_stop_is_flagged() {
    let reports = run();
    let flagged = flags(passed(
        &reports,
        "without an engagement module no stop is flagged",
    ));
    assert!(flagged.values().all(|f| !f), "{flagged:?}");
    assert!(flagged.contains_key(&(1, 0, -1)));
}

#[test]
fn a_snake_is_flagged_only_once_no_cell_stays_engaged() {
    let reports = run();
    let flagged = flags(passed(
        &reports,
        "a snake is flagged only once no cell stays engaged",
    ));
    // Onto its rear: the anchor leaves the enemy on (-1, 1), but the rear swings onto (0, 0).
    assert_eq!(flagged.get(&(1, 0, 3)), Some(&false), "{flagged:?}");
    // One step further, neither cell is next to the enemy.
    assert_eq!(flagged.get(&(2, 0, 3)), Some(&true), "{flagged:?}");
}

#[test]
fn a_preview_carries_each_stops_route() {
    let reports = run();
    let report = passed(
        &reports,
        "a preview carries the route past an enemy and flags it",
    );
    let routes = paths(report);
    assert_eq!(
        routes.get(&(1, 0, -1)),
        Some(&vec![(1, 0, 1)]),
        "{routes:?}"
    );
    // The only route to (1, 1) passes the enemy on (2, -1) at (1, 0), then leaves it.
    let detour = vec![(1, 0, 1), (1, 1, 1)];
    assert_eq!(routes.get(&(1, 1, -1)), Some(&detour), "{routes:?}");
    assert_eq!(flags(report).get(&(1, 1, -1)), Some(&true));
    // On open ground every route is a chain of single steps from the start to its stop.
    let open = passed(
        &reports,
        "stops whose route leaves an engaged enemy are flagged",
    );
    for ((q, r, _), path) in paths(open) {
        assert_eq!(path.last(), Some(&(q, r, 1)), "{path:?}");
        let mut from = Space::new(0, 0, 1);
        for &(q, r, z) in &path {
            let int = |n: i64| i32::try_from(n).unwrap();
            let to = Space::new(int(q), int(r), u32::try_from(z).unwrap());
            assert_eq!(from.hex().distance(to.hex()), 1, "{path:?}");
            from = to;
        }
    }
}

#[test]
fn a_move_that_starts_engaged_and_then_leaves_an_enemy_rolls_one_reprisal() {
    let reports = run();
    let report = passed(
        &reports,
        "a move that starts engaged and then leaves an enemy gives it one reprisal",
    );
    assert_eq!(reprisals(report), [i64::from(FigureId(1).0)]);
    assert_eq!(rolls(report), 1);
    let path = [(1, -1), (2, -1), (3, -1)].map(|(q, r)| Space::new(q, r, 1));
    assert_eq!(moves(report), [path.to_vec()]);
    // The reprisal resolves before the move applies.
    let order = report.events.iter().map(|e| match e {
        Event::Custom { id, .. } if id.as_str() == "reprisal" => 1,
        Event::FigureMoved { .. } => 2,
        _ => 0,
    });
    let order: Vec<_> = order.filter(|&n| n > 0).collect();
    assert_eq!(order, [1, 2]);
}

#[test]
fn a_move_that_passes_an_enemy_midway_and_then_leaves_it_rolls_one_reprisal() {
    let reports = run();
    let report = passed(
        &reports,
        "a move that passes an enemy midway and then leaves it gives it one reprisal",
    );
    // Neither the start (0, 0) nor the stop (1, 1) is next to the enemy on (2, -1), so only a
    // walk of the route finds the reprisal.
    assert_eq!(reprisals(report), [i64::from(FigureId(1).0)]);
    assert_eq!(rolls(report), 1);
    let path = [(1, 0), (1, 1)].map(|(q, r)| Space::new(q, r, 1));
    assert_eq!(moves(report), [path.to_vec()]);
}

#[test]
fn a_move_that_leaves_the_same_enemy_twice_rolls_one_reprisal() {
    let reports = run();
    let report = passed(
        &reports,
        "a move that leaves the same enemy twice gives it one reprisal",
    );
    assert_eq!(reprisals(report), [i64::from(FigureId(1).0)]);
    assert_eq!(rolls(report), 1);
    // Engaged at (1, 0), clear at (1, 1), engaged again at (3, -1), clear at (4, -2).
    let path = [(1, 0), (1, 1), (2, 1), (3, 0), (3, -1), (4, -2)].map(|(q, r)| Space::new(q, r, 1));
    assert_eq!(moves(report), [path.to_vec()]);
}

#[test]
fn a_reprisal_that_destroys_the_mover_stops_the_move() {
    let reports = run();
    let report = passed(
        &reports,
        "a reprisal that destroys the mover stops the move",
    );
    assert_eq!(reprisals(report).len(), 1);
    assert!(moves(report).is_empty(), "{:?}", report.events);
}

#[test]
fn a_snake_that_keeps_contact_through_its_rear_provokes_no_reprisal() {
    let reports = run();
    let keeps = passed(
        &reports,
        "a snake that keeps contact through its rear provokes no reprisal",
    );
    assert!(reprisals(keeps).is_empty());
    assert_eq!(rolls(keeps), 0);
    assert_eq!(moves(keeps).len(), 1);
    let leaves = passed(
        &reports,
        "a snake gives one reprisal once its rear leaves too",
    );
    assert_eq!(reprisals(leaves).len(), 1);
    assert_eq!(moves(leaves).len(), 1);
}

#[test]
fn running_twice_gives_identical_reports_and_hashes() {
    let (first, second) = (run(), run());
    assert!(
        first.iter().all(|r| r.outcome.is_ok()),
        "{}",
        summary(&first)
    );
    let hashes = |reports: &[TestReport]| {
        let each = reports
            .iter()
            .map(|r| (r.name.clone(), r.outcome.clone().ok()));
        each.collect::<Vec<_>>()
    };
    assert_eq!(hashes(&first), hashes(&second));
    let events =
        |reports: &[TestReport]| reports.iter().map(|r| r.events.clone()).collect::<Vec<_>>();
    assert_eq!(events(&first), events(&second));
}

#[test]
fn an_enemy_engaged_only_through_its_rear_cell_gets_a_reprisal() {
    let reports = run();
    let report = passed(
        &reports,
        "leaving an enemy that is engaged only through its rear cell gives it a reprisal",
    );
    assert_eq!(reprisals(report), [i64::from(FigureId(1).0)]);
    assert_eq!(rolls(report), 1);
}

#[test]
fn a_turn_in_place_is_judged_like_any_step() {
    let reports = run();
    let leaves = passed(
        &reports,
        "a turn in place that takes the rear away from an enemy gives it a reprisal",
    );
    assert_eq!(reprisals(leaves), [i64::from(FigureId(1).0)]);
    assert_eq!(rolls(leaves), 1);
    // The pivot keeps the anchor where it stands.
    assert_eq!(moves(leaves), [vec![Space::new(0, 0, 1)]]);
    let keeps = passed(
        &reports,
        "a turn in place that keeps the rear next to an enemy gives it none",
    );
    assert!(reprisals(keeps).is_empty());
    assert_eq!(rolls(keeps), 0);
    assert_eq!(moves(keeps).len(), 1);
}
