//! The effect descriptors of design §8.2 and the conditions that need modules, through the
//! `effects` fixture pack. Every effect there is plain data, so it runs natively.

use std::path::PathBuf;

use strata_kernel::{Space, Sym};
use strata_model::{Event, FigureId, PlayerId};
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/effects")];
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

const ME: FigureId = FigureId(0);
const FOE: FigureId = FigureId(1);

fn wounds_on(report: &TestReport, figure: FigureId) -> Vec<u32> {
    let wounds = report.events.iter().filter_map(|e| match e {
        Event::WoundsApplied {
            figure: f, amount, ..
        } if *f == figure => Some(*amount),
        _ => None,
    });
    wounds.collect()
}

/// The marker changes on `figure`, as `(delta, expiry)`.
fn markers_on(report: &TestReport, figure: FigureId) -> Vec<(i32, Option<u32>)> {
    let changes = report.events.iter().filter_map(|e| match e {
        Event::MarkerChanged {
            figure: f,
            delta,
            expiry,
            ..
        } if *f == figure => Some((*delta, *expiry)),
        _ => None,
    });
    changes.collect()
}

/// The moves of `figure`, as `(to, path)`.
fn moves_of(report: &TestReport, figure: FigureId) -> Vec<(Space, Vec<Space>)> {
    let moves = report.events.iter().filter_map(|e| match e {
        Event::FigureMoved {
            figure: f,
            to,
            path,
            ..
        } if *f == figure => Some((*to, path.clone())),
        _ => None,
    });
    moves.collect()
}

#[test]
fn every_effect_test_passes() {
    let reports = run();
    let failed: Vec<&str> = reports
        .iter()
        .filter(|r| r.outcome.is_err())
        .map(|r| r.name.as_str())
        .collect();
    assert!(failed.is_empty(), "{}", summary(&reports));
}

#[test]
fn wound_places_its_amount_through_the_units_slot() {
    let reports = run();
    let report = passed(&reports, "eff.wound wounds the target");
    assert_eq!(wounds_on(report, FOE), [2]);
}

#[test]
fn heal_removes_one_plain_wound() {
    let reports = run();
    let report = passed(&reports, "eff.heal takes a wound off the target");
    assert_eq!(markers_on(report, FOE), [(-1, None)]);
}

#[test]
fn place_puts_the_figure_on_the_chosen_space_without_a_path() {
    let reports = run();
    let report = passed(
        &reports,
        "eff.place puts the figure on a chosen empty space",
    );
    assert_eq!(moves_of(report, ME), [(Space::new(3, 0, 1), Vec::new())]);
}

#[test]
fn remove_takes_the_figure_out_without_destroying_it() {
    let reports = run();
    let report = passed(&reports, "eff.remove takes the target out of the game");
    let removed = |e: &&Event| matches!(e, Event::FigureRemoved(f) if *f == FOE);
    assert_eq!(report.events.iter().filter(removed).count(), 1);
    let destroyed = |e: &&Event| matches!(e, Event::FigureDestroyed(_));
    assert_eq!(report.events.iter().filter(destroyed).count(), 0);
}

#[test]
fn move_jumps_the_target_to_the_chosen_space() {
    let reports = run();
    let report = passed(&reports, "eff.move pushes the target to a chosen space");
    let to = Space::new(2, 0, 1);
    assert_eq!(moves_of(report, FOE), [(to, vec![to])]);
}

#[test]
fn grant_slot_adds_one_attack_to_the_figure() {
    let reports = run();
    let report = passed(&reports, "eff.grant_slot gives the figure another attack");
    let granted = report.events.iter().find_map(|e| match e {
        Event::SlotsChanged {
            figure,
            slot,
            remaining,
        } if *figure == ME => Some((slot.clone(), *remaining)),
        _ => None,
    });
    assert_eq!(granted, Some((Sym::new("attack"), 2)));
}

#[test]
fn add_marker_places_its_count_until_the_end_of_the_round() {
    let reports = run();
    let report = passed(&reports, "eff.add_marker marks the target for the round");
    let [(delta, Some(expiry))] = markers_on(report, FOE)[..] else {
        panic!("{:?}", report.events);
    };
    assert_eq!(delta, 2);
    // Round 0 or 1, whichever the fixture's flow is in: the marker lapses after it.
    assert!(expiry <= 1, "{expiry}");
}

#[test]
fn take_control_hands_the_target_to_the_actor() {
    let reports = run();
    let report = passed(&reports, "eff.take_control hands the target over");
    let changed = report.events.iter().find_map(|e| match e {
        Event::ControlChanged { figure, controller } if *figure == FOE => Some(*controller),
        _ => None,
    });
    assert_eq!(changed, Some(PlayerId(0)));
}

#[test]
fn cancel_wounds_in_a_window_leaves_no_wounds() {
    let reports = run();
    let report = passed(&reports, "eff.cancel_wounds as a reaction stops the wounds");
    assert!(wounds_on(report, FOE).is_empty(), "{:?}", report.events);
}

#[test]
fn cancel_wounds_as_a_replacement_drops_the_event() {
    let reports = run();
    let report = passed(
        &reports,
        "eff.cancel_wounds as a replacement cancels the wounds",
    );
    assert!(wounds_on(report, FOE).is_empty(), "{:?}", report.events);
}

#[test]
fn excess_blocks_wound_the_attacker_once_per_block_beyond_the_hits() {
    let reports = run();
    let report = passed(
        &reports,
        "eff.excess_blocks_wound_attacker wounds the attacker",
    );
    assert_eq!(wounds_on(report, ME), [1]);
    assert!(wounds_on(report, FOE).is_empty());
}

#[test]
fn attack_kind_filters_a_trigger_by_the_attack_in_progress() {
    let reports = run();
    let melee = passed(&reports, "cond.attack_kind matches a melee attack");
    assert_eq!(markers_on(melee, FOE), [(1, None)]);
    for name in [
        "cond.attack_kind does not match a ranged attack",
        "cond.attack_kind is false outside an attack",
    ] {
        let report = passed(&reports, name);
        assert!(markers_on(report, FOE).is_empty(), "{name}");
    }
}

#[test]
fn engaged_and_in_los_conditions_ask_their_modules() {
    let reports = run();
    for name in [
        "cond.engaged holds next to an enemy",
        "cond.engaged fails away from enemies",
        "cond.in_los holds when an enemy is in sight",
    ] {
        passed(&reports, name);
    }
}

#[test]
fn running_the_effects_pack_twice_gives_identical_events_and_hashes() {
    assert_eq!(run(), run());
}
