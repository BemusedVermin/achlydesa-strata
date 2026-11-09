//! Which attached modifiers a query gathers (design §6.6), on the `grants` fixture.

use std::path::PathBuf;

use strata_kernel::{Sym, Value};
use strata_model::{
    AbilityKey, AbilityOwner, Event, FigureDefRef, FigureId, FigureState, GameState, PlayerId,
    PlayerState, RelicDef, UnitDef, UnitId, UnitState,
};

use super::*;
use crate::setup::attach_printed;

fn registry() -> ContentRegistry {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pack_test/grants");
    strata_content::load(&[root]).unwrap()
}

/// Adds a unit of `name` with `count` figures for `player`, its printed abilities attached.
fn add(state: &mut GameState, registry: &ContentRegistry, name: &str, player: u8, count: u32) {
    let (def, _) = find::<UnitDef>(registry, "t", name).unwrap();
    let unit = UnitId(state.units.len() as u32);
    let mut figures = Vec::new();
    for _ in 0..count {
        figures.push(FigureId(state.figures.len() as u32));
        state.figures.push(FigureState {
            unit,
            def: FigureDefRef {
                unit: def,
                figure: Sym::new("f"),
            },
            controller: PlayerId(player),
            ..FigureState::default()
        });
    }
    state.units.push(UnitState {
        def,
        owner: PlayerId(player),
        figures,
        ..UnitState::default()
    });
    attach_printed(state, unit, registry).unwrap();
}

/// Figures: 0 a p0 soldier, 1 and 2 one p1 slippery unit, 3 a p0 slippery unit, 4 a p1 guard.
fn state(registry: &ContentRegistry) -> GameState {
    let mut state = GameState {
        players: vec![PlayerState::default(), PlayerState::default()],
        ..GameState::default()
    };
    add(&mut state, registry, "test_soldier", 0, 1);
    add(&mut state, registry, "test_slippery", 1, 2);
    add(&mut state, registry, "test_slippery", 0, 1);
    add(&mut state, registry, "test_guard", 1, 1);
    state
}

/// How many gathered modifiers modify each query, for `subject` asked with `context`.
fn gathered(host: &Host, state: &GameState, subject: u32, context: Option<u32>) -> Vec<String> {
    let found = host.granted(state, FigureId(subject), context.map(FigureId));
    let mut queries: Vec<String> = found.iter().map(|m| m.def.query.to_string()).collect();
    queries.sort();
    queries
}

#[test]
fn a_context_side_modifier_joins_only_from_the_context_figure() {
    let registry = registry();
    let host = Host::new(&registry, "t", Vec::new(), None);
    let state = state(&registry);
    // Others asking about a slippery figure gather its veto.
    assert_eq!(gathered(&host, &state, 0, Some(1)), ["can_target"]);
    // A slippery figure asking about others does not.
    assert_eq!(gathered(&host, &state, 1, Some(0)), Vec::<String>::new());
    // Nor does a query with no context.
    assert_eq!(gathered(&host, &state, 1, None), Vec::<String>::new());
}

#[test]
fn mirror_armies_gather_only_the_context_figures_instance() {
    let registry = registry();
    let host = Host::new(&registry, "t", Vec::new(), None);
    let state = state(&registry);
    for (subject, context) in [(3, 1), (1, 3)] {
        let found = host.granted(&state, FigureId(subject), Some(FigureId(context)));
        assert_eq!(found.len(), 1, "{subject} asked about {context}");
    }
}

#[test]
fn pairs_inside_one_unit_and_on_one_figure_gather_each_modifier_once() {
    let registry = registry();
    let host = Host::new(&registry, "t", Vec::new(), None);
    let state = state(&registry);
    assert_eq!(gathered(&host, &state, 1, Some(2)), ["can_target"]);
    assert_eq!(gathered(&host, &state, 1, Some(1)), ["can_target"]);
    // The guard as both subject and context: its subject side and its context side, once each.
    let both = gathered(&host, &state, 4, Some(4));
    assert_eq!(both, ["counted", "guarded", "plain"]);
    assert_eq!(gathered(&host, &state, 4, Some(0)), ["plain"]);
    assert_eq!(gathered(&host, &state, 0, Some(4)), ["counted", "guarded"]);
}

#[test]
fn a_player_owned_grant_joins_from_the_context_side() {
    let registry = registry();
    let host = Host::new(&registry, "t", Vec::new(), None);
    let mut state = state(&registry);
    let (veil, _) = find::<RelicDef>(&registry, "t", "veil").unwrap();
    let attach = Event::AbilityAttached {
        def: AbilityKey::Relic {
            relic: veil,
            index: 0,
        },
        owner: AbilityOwner::Player(PlayerId(1)),
        params: Default::default(),
    };
    state.apply(&attach).unwrap();
    // The soldier asking about the p1 guard gathers the veil and the guard's own context side.
    let found = gathered(&host, &state, 0, Some(4));
    assert_eq!(found, ["can_target", "counted", "guarded"]);
    // The guard asking about the soldier gathers no veil.
    assert_eq!(gathered(&host, &state, 4, Some(0)), ["plain"]);
}

#[test]
fn only_a_pair_query_reads_its_number_context_as_a_figure() {
    let registry = registry();
    let state = state(&registry);
    let vote = |key: &str, context: i64| VoteQuery {
        key: Sym::new(key),
        default: false,
        subject: Value::Int(0),
        context: Value::Int(context),
    };
    let host = Host::new(&registry, "t", Vec::new(), None);
    for key in VoteQuery::PAIR_KEYS {
        assert_eq!(host.pair_context(&state, &vote(key, 1)), Some(FigureId(1)));
    }
    // A custom vote is not a pair query until declared, whatever its context holds.
    assert_eq!(host.pair_context(&state, &vote("counted", 1)), None);
    // No figure has that number.
    assert_eq!(host.pair_context(&state, &vote("can_target", 99)), None);
    let declared = [Sym::new("counted")].into_iter().collect();
    let host = host.with_pair_queries(declared);
    assert_eq!(
        host.pair_context(&state, &vote("counted", 1)),
        Some(FigureId(1))
    );
}
