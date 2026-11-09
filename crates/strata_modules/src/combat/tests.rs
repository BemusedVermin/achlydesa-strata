//! Configuration, range, sight, cover and height of `core:combat`; whole attacks are covered by
//! the pack tests of the `combat` fixture pack.

use std::collections::BTreeMap;

use proptest::prelude::*;
use strata_kernel::{DefId, Frac, Hex, Space, Sym, Value};
use strata_model::{
    DieKind, FigureId, FigureState, FigureStatus, GameState, PlayerId, PlayerState,
};
use strata_rules::{
    FigureVolume, LogicError, LosResult, ModuleError, NativeEnv, Registrar, RuleModule, VoteQuery,
};

use super::sight::{Sight, distance, height_bonus};
use super::{CombatModule, Rules};

/// The module's defaults with `name` set to `value`.
fn config(name: &str, value: Value) -> Value {
    let given = BTreeMap::from([(Sym::new(name), value)]);
    CombatModule.config_schema().resolve(&given).unwrap()
}

fn register(cfg: &Value) -> Result<Registrar, ModuleError> {
    let mut r = Registrar::default();
    CombatModule.register(&mut r, cfg).map(|()| r)
}

fn rules(cover: bool, height_bonus: i64) -> Rules {
    Rules {
        die: "combat".into(),
        hit: Sym::new("hit"),
        block: Sym::new("block"),
        height_bonus,
        cover,
    }
}

#[test]
fn the_defaults_register() {
    let defaults = CombatModule.config_schema().resolve(&BTreeMap::new());
    assert!(register(&defaults.unwrap()).is_ok());
    assert!(register(&config("cover", Value::Bool(true))).is_ok());
}

#[test]
fn a_negative_or_oversized_height_bonus_is_refused() {
    for bonus in [-1, 1025] {
        let err = register(&config("height_bonus", Value::Int(bonus)));
        assert!(matches!(err, Err(ModuleError::Invalid(m)) if m.contains("height_bonus")));
    }
}

/// A range of 3 for everyone, votes at their defaults, and line of sight that answers `clear`
/// of 21 samples clear, visible when any is. `None` fails every line-of-sight question.
struct Env {
    clear: Option<i128>,
    /// Extra cells of figure 0's footprint, beyond the column it stands in.
    wide: Vec<Hex>,
}

impl NativeEnv for Env {
    fn stat(&self, _: &GameState, _: FigureId, stat: &str) -> Result<i64, LogicError> {
        match stat {
            "range" => Ok(3),
            other => Err(LogicError(format!("no stat `{other}`"))),
        }
    }
    fn die(&self, _: &str) -> Option<(DefId, DieKind)> {
        None
    }
    fn vote(&self, _: &GameState, query: &VoteQuery) -> Result<bool, LogicError> {
        Ok(query.default)
    }
    fn los(&self, _: &GameState, _: FigureId, _: FigureId) -> Result<LosResult, LogicError> {
        let clear = self
            .clear
            .ok_or_else(|| LogicError("no sight here".into()))?;
        Ok(LosResult {
            visible: clear > 0,
            clear_fraction: Frac::new(clear, 21),
            witnesses: Vec::new(),
        })
    }
    fn volume(&self, state: &GameState, figure: FigureId) -> Option<FigureVolume> {
        let at = state.standing(figure)?.hex();
        let mut cells = vec![at];
        if figure == FigureId(0) {
            cells.extend(&self.wide);
        }
        Some(FigureVolume {
            anchor: at,
            cells,
            target: 10,
            bottom: 0,
            top: 12,
        })
    }
}

fn env(clear: Option<i128>) -> Env {
    Env {
        clear,
        wide: Vec::new(),
    }
}

/// Figure 0 of team 0 and figure 1 of team 1, standing on these spaces.
fn state(attacker: Space, target: Space) -> GameState {
    let figure = |controller: u8, at: Space| FigureState {
        location: Some(at),
        status: FigureStatus::OnBoard,
        controller: PlayerId(controller),
        ..FigureState::default()
    };
    let team = |team| PlayerState {
        team,
        ..PlayerState::default()
    };
    GameState {
        players: vec![team(0), team(1)],
        figures: vec![figure(0, attacker), figure(1, target)],
        ..GameState::default()
    }
}

fn sight(rules: &Rules, env: &Env, to: (i32, i32)) -> Result<Option<Sight>, String> {
    let s = state(Space::new(0, 0, 1), Space::new(to.0, to.1, 1));
    let found = Sight::of(rules, &s, env, FigureId(0), FigureId(1), None);
    found.map_err(|h| format!("{h:?}"))
}

#[test]
fn an_adjacent_target_needs_no_line_of_sight() {
    let found = sight(&rules(true, 0), &env(None), (1, 0));
    let melee = Sight {
        ranged: false,
        cover: false,
    };
    assert_eq!(found, Ok(Some(melee)));
}

#[test]
fn a_target_beyond_adjacency_needs_line_of_sight() {
    assert!(sight(&rules(false, 0), &env(None), (2, 0)).is_err());
    assert_eq!(sight(&rules(false, 0), &env(Some(0)), (2, 0)), Ok(None));
    let found = sight(&rules(false, 0), &env(Some(21)), (3, 0));
    assert_eq!(found.unwrap().map(|s| s.ranged), Some(true));
}

#[test]
fn a_target_beyond_range_is_not_a_target() {
    assert_eq!(sight(&rules(false, 0), &env(Some(21)), (4, 0)), Ok(None));
    assert_eq!(sight(&rules(false, 0), &env(Some(21)), (0, -4)), Ok(None));
}

#[test]
fn range_counts_from_the_nearest_cell_of_a_wide_footprint() {
    let wide = Env {
        clear: Some(21),
        wide: vec![Hex::new(1, 0)],
    };
    let found = sight(&rules(false, 0), &wide, (4, 0));
    assert_eq!(found.unwrap().map(|s| s.ranged), Some(true));
}

#[test]
fn cover_takes_at_least_half_of_the_samples_blocked() {
    let cover = |clear| {
        let found = sight(&rules(true, 0), &env(Some(clear)), (2, 0));
        found.unwrap().map(|s| s.cover)
    };
    assert_eq!(cover(10), Some(true), "11 of 21 blocked");
    assert_eq!(cover(11), Some(false), "10 of 21 blocked");
    assert_eq!(cover(1), Some(true));
    assert_eq!(cover(21), Some(false));
    let off = sight(&rules(false, 0), &env(Some(1)), (2, 0));
    assert_eq!(off.unwrap().map(|s| s.cover), Some(false));
}

#[test]
fn a_figure_cannot_attack_itself() {
    let s = state(Space::new(0, 0, 1), Space::new(1, 0, 1));
    let found = Sight::of(
        &rules(false, 0),
        &s,
        &env(None),
        FigureId(0),
        FigureId(0),
        None,
    );
    assert_eq!(found.ok(), Some(None));
}

proptest! {
    #[test]
    fn distance_is_symmetric(q in -6i32..6, r in -6i32..6, z in 1u32..4) {
        let s = state(Space::new(0, 0, 1), Space::new(q, r, z));
        let e = env(None);
        let there = distance(&s, &e, FigureId(0), FigureId(1));
        prop_assert_eq!(there, distance(&s, &e, FigureId(1), FigureId(0)));
        prop_assert_eq!(there, Some(Hex::new(0, 0).distance(Hex::new(q, r))));
    }

    #[test]
    fn only_the_higher_figure_gets_the_height_bonus(a in 1u32..6, t in 1u32..6, bonus in 0i64..3) {
        let rules = rules(false, bonus);
        let s = state(Space::new(0, 0, a), Space::new(1, 0, t));
        let (attack, defense) = height_bonus(&rules, &s, FigureId(0), FigureId(1));
        let swapped = state(Space::new(0, 0, t), Space::new(1, 0, a));
        prop_assert_eq!((defense, attack), height_bonus(&rules, &swapped, FigureId(0), FigureId(1)));
        prop_assert_eq!(attack, if a > t { bonus } else { 0 });
        prop_assert_eq!(defense, if t > a { bonus } else { 0 });
    }
}
