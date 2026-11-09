use strata_kernel::DefId;
use strata_model::{Command, PlayerId, TokenChoice, UnitId};
use strata_session::PlayerView;

use super::*;
use crate::input::builder::Step;
use crate::input::tests::{figures, space, state};

fn view(legal: Vec<Command>, tokens: Option<TokenChoice>) -> PlayerView {
    let mut view = PlayerView::of(&state(), PlayerId(0));
    view.legal = legal;
    view.tokens = tokens;
    view
}

#[test]
fn a_click_on_an_activatable_figure_sends_one_activation() {
    let s = state();
    let mut input = CommandInput::of(&view(vec![Command::ActivateUnit(UnitId(0))], None));
    assert_eq!(input.highlights(&s).figures, figures(&[0, 1]));
    assert_eq!(input.click(&s, &Pick::at(&s, space(3))), None);
    assert_eq!(input.why, Some(Refusal::NotYours));
    let activate = Some(Command::ActivateUnit(UnitId(0)));
    assert_eq!(input.click(&s, &Pick::at(&s, space(0))), activate);
    assert_eq!(input.why, None);
    assert!(input.sent);
    assert_eq!(input.click(&s, &Pick::at(&s, space(0))), None);
    assert_eq!(input.highlights(&s), Highlights::default());
}

#[test]
fn while_tokens_are_placed_clicks_go_to_the_placement() {
    let s = state();
    let choice = TokenChoice {
        hand: vec![DefId(1)],
        units: vec![UnitId(0)],
    };
    let mut input = CommandInput::of(&view(Vec::new(), Some(choice)));
    assert_eq!(input.builder.step(), Step::Idle);
    assert_eq!(input.highlights(&s).figures, figures(&[0, 1]));
    assert_eq!(input.click(&s, &Pick::at(&s, space(3))), None);
    assert_eq!(input.why, Some(Refusal::NotYours));
    assert_eq!(input.click(&s, &Pick::at(&s, space(1))), None);
    assert_eq!(input.why, None);
    let placer = input.tokens.as_ref().unwrap();
    assert_eq!(placer.assigned(), [Some(UnitId(0))]);
    assert!(placer.command().is_some());
    assert!(
        !input.sent,
        "the placement goes out from the screen's button"
    );
}

#[test]
fn with_nothing_offered_a_click_explains_that_there_is_nothing_to_do() {
    let s = state();
    let mut input = CommandInput::of(&view(Vec::new(), None));
    assert_eq!(input.click(&s, &Pick::at(&s, space(0))), None);
    assert_eq!(input.why, Some(Refusal::NothingToDo));
}

#[test]
fn a_panel_press_clears_an_old_refusal() {
    let s = state();
    let mut input = CommandInput::of(&view(vec![Command::ActivateUnit(UnitId(0))], None));
    assert_eq!(input.click(&s, &Pick::at(&s, space(3))), None);
    assert_eq!(input.refusal(), Some(Refusal::NotYours));
    assert_eq!(input.press(Press::Back), None);
    assert_eq!(input.why, None, "Back replaces the refusal");
    assert_eq!(input.click(&s, &Pick::at(&s, space(3))), None);
    let activate = Some(Command::ActivateUnit(UnitId(0)));
    assert_eq!(input.press(Press::Unit(UnitId(0))), activate);
    assert_eq!(input.why, None);
    assert!(input.sent);
    assert_eq!(
        input.press(Press::Unit(UnitId(0))),
        None,
        "one command per view"
    );
}

#[test]
fn token_screen_presses_clear_or_set_the_refusal_and_place_the_hand() {
    let s = state();
    let choice = TokenChoice {
        hand: vec![DefId(1)],
        units: vec![UnitId(0)],
    };
    let mut input = CommandInput::of(&view(Vec::new(), Some(choice)));
    assert_eq!(input.press(Press::Assign(UnitId(9))), None);
    assert_eq!(input.why, Some(Refusal::NoTokenHere));
    input.press(Press::Token(0));
    assert_eq!(input.why, None, "selecting a token replaces the refusal");
    input.press(Press::Assign(UnitId(9)));
    input.press(Press::Clear);
    assert_eq!(input.why, None, "Clear replaces the refusal");
    input.press(Press::Assign(UnitId(9)));
    assert_eq!(input.press(Press::Place), None, "nothing is placed yet");
    assert_eq!(input.why, None, "Place tokens replaces the refusal");
    assert_eq!(input.press(Press::Assign(UnitId(0))), None);
    let placed = input.press(Press::Place);
    assert!(matches!(placed, Some(Command::PlaceTokens(ref put)) if put.len() == 1));
    assert!(input.sent);
    assert_eq!(input.click(&s, &Pick::at(&s, space(3))), None);
}

#[test]
fn a_refusal_is_not_shown_once_a_command_has_gone_out() {
    let mut input = CommandInput::of(&view(vec![Command::EndActivation], None));
    input.why = Some(Refusal::NotYours);
    assert_eq!(input.refusal(), Some(Refusal::NotYours));
    input.settle(Some(Command::EndActivation));
    assert_eq!(input.refusal(), None);
}

#[test]
fn end_activation_is_pressed_only_when_it_is_legal() {
    let mut input = CommandInput::of(&view(vec![Command::ActivateUnit(UnitId(0))], None));
    assert_eq!(input.press(Press::End), None);
    let mut input = CommandInput::of(&view(vec![Command::EndActivation], None));
    assert_eq!(input.press(Press::End), Some(Command::EndActivation));
}
