//! Height overlap, body-level reprisals and configuration of `core:engagement`; the rest of its
//! behavior is covered by pack tests.

use std::collections::BTreeMap;

use proptest::prelude::*;
use strata_kernel::{DefId, Space, Sym, Value};
use strata_model::{
    DieKind, FigureId, FigureState, FigureStatus, GameState, PlayerId, PlayerState,
};
use strata_rules::{
    EngagementMemo, LogicError, ModuleError, NativeEnv, Registrar, RuleModule, VoteQuery,
};

use super::reprisal::Reprisal;
use super::{EngagementModule, Heights, Service, overlaps};

/// The module's defaults with `name` set to `value`.
fn config(name: &str, value: Value) -> Value {
    let given = BTreeMap::from([(Sym::new(name), value)]);
    EngagementModule.config_schema().resolve(&given).unwrap()
}

fn register(cfg: &Value) -> Result<Registrar, ModuleError> {
    let mut r = Registrar::default();
    EngagementModule.register(&mut r, cfg).map(|()| r)
}

#[test]
fn same_level_figures_overlap_and_a_level_apart_do_not() {
    assert!(overlaps(1, 1, 1, 1));
    assert!(!overlaps(1, 1, 2, 1), "[1, 2) and [2, 3) only touch");
    assert!(overlaps(1, 2, 2, 1), "[1, 3) reaches [2, 3)");
    assert!(!overlaps(1, 2, 3, 5));
}

#[test]
fn the_defaults_and_ignored_heights_register() {
    let defaults = EngagementModule.config_schema().resolve(&BTreeMap::new());
    assert!(register(&defaults.unwrap()).is_ok());
    assert!(register(&config("heights", Value::Str(Sym::new("ignore")))).is_ok());
}

#[test]
fn an_unknown_height_rule_is_refused() {
    let err = register(&config("heights", Value::Str(Sym::new("sometimes"))));
    assert!(matches!(err, Err(ModuleError::Invalid(m)) if m.contains("heights")));
}

#[test]
fn a_negative_or_oversized_pool_is_refused() {
    for dice in [-1, 1025] {
        let err = register(&config("dice", Value::Int(dice)));
        assert!(matches!(err, Err(ModuleError::Invalid(m)) if m.contains("dice")));
    }
}

/// Answers every vote with its default, so engagement is plain geometry.
struct Plain;

impl NativeEnv for Plain {
    fn stat(&self, _: &GameState, _: FigureId, stat: &str) -> Result<i64, LogicError> {
        Err(LogicError(format!("no stat `{stat}`")))
    }
    fn die(&self, _: &str) -> Option<(DefId, DieKind)> {
        None
    }
    fn vote(&self, _: &GameState, query: &VoteQuery) -> Result<bool, LogicError> {
        Ok(query.default)
    }
}

fn service() -> Service {
    let reprisal = Reprisal {
        die: "combat".into(),
        dice: 1,
        wound_on: Sym::new("hit"),
    };
    Service {
        heights: Heights::Overlap,
        reprisal,
    }
}

/// Figure 0 of team 0 stands at the origin; each enemy of team 1 stands on its `(q, r)`, all on
/// level 1.
fn state(enemies: &[(i32, i32)]) -> GameState {
    let figure = |controller: u8, (q, r): (i32, i32)| FigureState {
        location: Some(Space::new(q, r, 1)),
        status: FigureStatus::OnBoard,
        controller: PlayerId(controller),
        ..FigureState::default()
    };
    let mut figures = vec![figure(0, (0, 0))];
    figures.extend(enemies.iter().map(|&at| figure(1, at)));
    let team = |team| PlayerState {
        team,
        ..PlayerState::default()
    };
    let mut state = GameState {
        players: vec![team(0), team(1)],
        figures,
        ..GameState::default()
    };
    let anchors = (0u32..).zip(&state.figures).map(|(id, f)| (f.location, id));
    let anchors: Vec<_> = anchors.collect();
    for (at, id) in anchors {
        state
            .board_changes
            .occupancy
            .insert(at.unwrap(), FigureId(id));
    }
    state
}

/// Gives figure `id` extra covered cells, as a multi-cell footprint would.
fn cover(state: &mut GameState, id: u32, cells: &[(i32, i32, u32)]) {
    for &(q, r, z) in cells {
        let at = Space::new(q, r, z);
        state.board_changes.occupancy.insert(at, FigureId(id));
    }
}

/// Whom figure 0 is engaged with standing on `at`.
fn engaged(state: &GameState, at: Space) -> Vec<FigureId> {
    use strata_rules::Engagement;
    service().engaged(state, &Plain, FigureId(0), at).unwrap()
}

/// The enemies owed a reprisal when figure 0's body moves from `before` to `after`.
fn owed(state: &GameState, before: &[(i32, i32)], after: &[(i32, i32)]) -> Vec<FigureId> {
    let on = |cells: &[(i32, i32)]| -> Vec<Space> {
        cells.iter().map(|&(q, r)| Space::new(q, r, 1)).collect()
    };
    let service = service();
    let memo = EngagementMemo::new(&service, state, &Plain);
    memo.reprisals(FigureId(0), &on(before), &on(after))
        .unwrap()
}

#[test]
fn a_snake_whose_rear_steps_next_to_the_enemy_owes_nothing() {
    // The anchor leaves the enemy at (-1, 0); the rear follows onto (0, 0), next to it.
    let state = state(&[(-1, 0)]);
    assert!(owed(&state, &[(0, 0), (1, 0)], &[(1, -1), (0, 0)]).is_empty());
    // The anchor's own step, asked alone, would owe one.
    assert_eq!(owed(&state, &[(0, 0)], &[(1, -1)]), [FigureId(1)]);
    // Once no cell is next to the enemy, the body has left it.
    assert_eq!(
        owed(&state, &[(1, -1), (0, 0)], &[(2, -1), (1, -1)]),
        [FigureId(1)]
    );
}

#[test]
fn a_rigid_shape_that_keeps_the_enemy_through_another_cell_owes_nothing() {
    // The enemy at (1, -1) touches both cells; sliding west the rear stays next to it.
    let state = state(&[(1, -1)]);
    assert!(owed(&state, &[(0, 0), (1, 0)], &[(-1, 0), (0, 0)]).is_empty());
    assert_eq!(
        owed(&state, &[(0, 0), (1, 0)], &[(-1, 1), (0, 1)]),
        [FigureId(1)]
    );
}

#[test]
fn each_enemy_is_judged_on_its_own() {
    // Enemy 1 at (-1, 0) is kept by the new rear on (0, 0); enemy 2 at (2, -1) is left.
    let state = state(&[(-1, 0), (2, -1)]);
    let before = [(0, 0), (1, 0)];
    assert_eq!(owed(&state, &before, &[(-1, 1), (0, 0)]), [FigureId(2)]);
    // Moving towards enemy 2 keeps it, and the old anchor's cell keeps enemy 1.
    assert!(owed(&state, &before, &[(1, -1), (0, 0)]).is_empty());
}

#[test]
fn an_enemys_rear_cell_engages() {
    // The enemy's anchor at (3, 0) is out of reach; its rear on (2, 0) and (1, 0) is not.
    let mut state = state(&[(3, 0)]);
    let at = Space::new(0, 0, 1);
    assert!(engaged(&state, at).is_empty());
    cover(&mut state, 1, &[(2, 0, 1), (1, 0, 1)]);
    assert_eq!(engaged(&state, at), [FigureId(1)]);
    // Two of its cells are next to (1, -1): it is still listed once.
    assert_eq!(engaged(&state, Space::new(1, -1, 1)), [FigureId(1)]);
}

#[test]
fn heights_are_compared_cell_by_cell() {
    // The enemy's anchor is far away; its rear cell on (1, 0) stands two levels up.
    let mut state = state(&[(3, 0)]);
    cover(&mut state, 1, &[(1, 0, 3)]);
    assert!(
        engaged(&state, Space::new(0, 0, 1)).is_empty(),
        "a level apart"
    );
    assert_eq!(engaged(&state, Space::new(0, 0, 3)), [FigureId(1)]);
    // A figure's own cells never engage it.
    let mut state = self::state(&[]);
    cover(&mut state, 0, &[(1, 0, 1)]);
    assert!(engaged(&state, Space::new(2, 0, 1)).is_empty());
}

#[test]
fn leaving_a_long_enemy_is_judged_on_every_cell() {
    // The enemy's rear on (1, 0) touches the mover's old cell; moving west leaves it.
    let mut state = state(&[(3, 0)]);
    cover(&mut state, 1, &[(2, 0, 1), (1, 0, 1)]);
    assert_eq!(owed(&state, &[(0, 0)], &[(-1, 0)]), [FigureId(1)]);
    assert!(
        owed(&state, &[(0, 0)], &[(0, 1)]).is_empty(),
        "(0, 1) touches (1, 0)"
    );
    assert_eq!(
        owed(&state, &[(0, 0)], &[(-1, 0)]),
        owed(&state, &[(0, 0)], &[(-1, 0)])
    );
}

proptest! {
    #[test]
    fn overlap_is_symmetric_and_shifts_with_both(
        a in 0u32..20, ah in 1u32..6, b in 0u32..20, bh in 1u32..6, lift in 0u32..10,
    ) {
        prop_assert_eq!(overlaps(a, ah, b, bh), overlaps(b, bh, a, ah));
        prop_assert_eq!(overlaps(a, ah, b, bh), overlaps(a + lift, ah, b + lift, bh));
        prop_assert!(overlaps(a, ah, a, bh), "figures on one level always overlap");
    }
}
