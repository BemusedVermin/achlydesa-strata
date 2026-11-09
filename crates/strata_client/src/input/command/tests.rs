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
