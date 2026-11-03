use strata_kernel::DefId;
use strata_model::{
    FigureId, GameState, PlayerId, PlayerState, Token, UnitId, UnitState, Visibility,
};
use strata_rules::ActivationGate;

use super::*;

fn token(kind: u32, visibility: Visibility) -> Token {
    Token {
        kind: DefId(kind),
        visibility,
    }
}

/// Two players. Unit 0 (player 0) holds a face-up 1; unit 1 (player 0) a face-down 2;
/// unit 2 (player 1) a face-up 2 and unit 3 (player 1) nothing.
fn state(turn: u32) -> GameState {
    let unit = |owner: u8, tokens: Vec<Token>| UnitState {
        owner: PlayerId(owner),
        figures: vec![FigureId(0)],
        tokens,
        ..Default::default()
    };
    let mut state = GameState {
        players: vec![PlayerState::default(); 2],
        units: vec![
            unit(0, vec![token(1, Visibility::Public)]),
            unit(0, vec![token(2, Visibility::Hidden)]),
            unit(1, vec![token(2, Visibility::Public)]),
            unit(1, Vec::new()),
        ],
        ..Default::default()
    };
    state.flow.turn = turn;
    state
}

fn may(state: &GameState, player: u8, unit: u32) -> bool {
    Gate.may_activate(state, PlayerId(player), UnitId(unit))
}

#[test]
fn turns_alternate_between_two_players_so_every_second_turn_is_the_next_token() {
    let state = state(1);
    let tokens: Vec<u32> = (1..=6).map(|turn| nth_turn(&state, turn)).collect();
    assert_eq!(tokens, [1, 1, 2, 2, 3, 3]);
}

#[test]
fn only_the_unit_with_the_revealed_token_of_this_turn_may_act() {
    // Turn 1 is player 0's first turn: token 1, face up on unit 0.
    let first = state(1);
    assert!(may(&first, 0, 0));
    assert!(
        !may(&first, 0, 1),
        "the face-down 2 is not this turn's token"
    );
    assert!(
        !may(&first, 1, 2),
        "a player cannot activate on the opponent's turn token"
    );
    assert!(!may(&first, 1, 3));
}

#[test]
fn a_face_down_token_does_not_open_the_gate_until_it_is_revealed() {
    // Turn 3 is player 0's second turn: token 2. Unit 1 holds it, but face down.
    let mut third = state(3);
    assert!(!may(&third, 0, 1));
    third.units[1].tokens[0].visibility = Visibility::Public;
    assert!(may(&third, 0, 1));
    assert!(!may(&third, 0, 0), "token 1 is spent");
}

#[test]
fn a_player_cannot_activate_a_unit_that_is_not_theirs() {
    let second = state(2);
    assert!(!may(&second, 1, 2), "token 1 is needed on turn 2");
    let third = state(3);
    assert!(!may(&third, 0, 2), "unit 2 is player 1's");
    assert!(!may(&third, 0, 99), "an unknown unit is refused");
}
