use std::collections::BTreeMap;

use strata_kernel::{DefId, Space, Sym, Value};
use strata_model::{Command, FigureId, PlayerId, UnitId};

use super::*;
use crate::input::pick::{Pick, Refusal};
use crate::input::tests::{figures, space, state};

fn act(figure: u32, action: u32, args: Value) -> Command {
    Command::Action {
        figure: FigureId(figure),
        action: DefId(action),
        args,
    }
}

fn coords(q: i64, r: i64, z: i64) -> Value {
    Value::List(vec![Value::Int(q), Value::Int(r), Value::Int(z)])
}

/// Walk takes no target, shoot targets figure 2, step targets space (2, 0, 1).
fn activation_legal() -> Vec<Command> {
    vec![
        act(0, 0, Value::Nil),
        act(0, 1, Value::Int(2)),
        act(0, 2, coords(2, 0, 1)),
        act(1, 0, Value::Nil),
        Command::EndActivation,
    ]
}

#[test]
fn nothing_legal_means_every_click_is_refused() {
    let mut builder = CommandBuilder::new(PlayerId(0), Vec::new());
    assert_eq!(builder.step(), Step::Idle);
    let refused = builder.click(&state(), &Pick::at(&state(), space(0)));
    assert_eq!(refused, Err(Refusal::NothingToDo));
}

#[test]
fn clicking_a_figure_activates_its_unit_when_that_is_legal() {
    let s = state();
    let mut builder = CommandBuilder::new(PlayerId(0), vec![Command::ActivateUnit(UnitId(0))]);
    assert_eq!(builder.step(), Step::Unit);
    assert_eq!(builder.highlights(&s).figures, figures(&[0, 1]));
    let click = |b: &mut CommandBuilder, q| b.click(&s, &Pick::at(&s, space(q)));
    assert_eq!(click(&mut builder, 3), Err(Refusal::NotYours));
    assert_eq!(click(&mut builder, 2), Err(Refusal::NoFigure));
    let activate = Some(Command::ActivateUnit(UnitId(0)));
    assert_eq!(click(&mut builder, 1), Ok(activate));
}

#[test]
fn a_unit_that_may_not_activate_says_so() {
    let s = state();
    let mut builder = CommandBuilder::new(PlayerId(1), vec![Command::ActivateUnit(UnitId(0))]);
    let pick = Pick::at(&s, space(3));
    assert_eq!(builder.click(&s, &pick), Err(Refusal::CannotActivate));
}

#[test]
fn figure_then_action_then_target_builds_a_legal_command() {
    let s = state();
    let mut builder = CommandBuilder::new(PlayerId(0), activation_legal());
    assert_eq!(builder.step(), Step::Figure);
    assert_eq!(builder.actors(), [FigureId(0), FigureId(1)]);
    assert_eq!(builder.highlights(&s).figures, figures(&[0, 1]));
    assert!(builder.can_end());

    let click = |b: &mut CommandBuilder, q| b.click(&s, &Pick::at(&s, space(q)));
    assert_eq!(click(&mut builder, 3), Err(Refusal::NotYours));
    assert_eq!(click(&mut builder, 0), Ok(None));
    assert_eq!(builder.step(), Step::Action(FigureId(0)));
    assert_eq!(builder.actions(), [DefId(0), DefId(1), DefId(2)]);

    assert_eq!(builder.choose_action(DefId(1)), None);
    assert_eq!(builder.step(), Step::Target(FigureId(0), DefId(1)));
    assert_eq!(builder.highlights(&s).figures, figures(&[2]));
    assert_eq!(click(&mut builder, 2), Err(Refusal::NotATarget));
    assert_eq!(click(&mut builder, 3), Ok(Some(act(0, 1, Value::Int(2)))));
    assert_eq!(builder.step(), Step::Figure);
}

#[test]
fn a_space_target_is_highlighted_and_taken_from_the_board() {
    let s = state();
    let mut builder = CommandBuilder::new(PlayerId(0), activation_legal());
    builder.click(&s, &Pick::figure(FigureId(0))).unwrap();
    builder.choose_action(DefId(2));
    let shown = builder.highlights(&s);
    assert_eq!(shown.spaces, [Space::new(2, 0, 1)].into());
    assert!(shown.figures.is_empty());
    let pick = Pick::at(&s, Space::new(2, 0, 1));
    assert_eq!(
        builder.click(&s, &pick),
        Ok(Some(act(0, 2, coords(2, 0, 1))))
    );
}

#[test]
fn an_untargeted_action_is_ready_at_once_and_back_undoes_a_step() {
    let s = state();
    let mut builder = CommandBuilder::new(PlayerId(0), activation_legal());
    assert_eq!(builder.choose_action(DefId(0)), None, "no figure yet");
    builder.click(&s, &Pick::figure(FigureId(0))).unwrap();
    builder.choose_action(DefId(1));
    builder.back();
    assert_eq!(builder.step(), Step::Action(FigureId(0)));
    builder.back();
    assert_eq!(builder.step(), Step::Figure);
    builder.click(&s, &Pick::figure(FigureId(1))).unwrap();
    assert_eq!(builder.actions(), [DefId(0)]);
    assert_eq!(
        builder.choose_action(DefId(1)),
        None,
        "not legal for figure 1"
    );
    assert_eq!(builder.choose_action(DefId(0)), Some(act(1, 0, Value::Nil)));
}

#[test]
fn a_figure_that_cannot_act_says_so_and_another_actor_can_be_reselected() {
    let s = state();
    let legal = vec![act(0, 1, Value::Int(2))];
    let mut builder = CommandBuilder::new(PlayerId(0), legal);
    assert_eq!(
        builder.click(&s, &Pick::figure(FigureId(1))),
        Err(Refusal::CannotAct)
    );
    builder.click(&s, &Pick::figure(FigureId(0))).unwrap();
    builder.choose_action(DefId(1));
    assert_eq!(builder.choose_option(0), Some(act(0, 1, Value::Int(2))));
}

#[test]
fn targets_read_figures_and_spaces_and_leave_the_rest_to_a_list() {
    let s = state();
    let map: BTreeMap<Sym, Value> = [("q", 1), ("r", 2), ("z", 3)]
        .map(|(k, v)| (Sym::new(k), Value::Int(v)))
        .into();
    assert_eq!(Target::of(&Value::Int(3), &s), Target::Figure(FigureId(3)));
    assert_eq!(Target::of(&Value::Int(7), &s), Target::Other(Value::Int(7)));
    assert_eq!(
        Target::of(&coords(1, 2, 3), &s),
        Target::Space(Space::new(1, 2, 3))
    );
    assert_eq!(
        Target::of(&Value::Map(map), &s),
        Target::Space(Space::new(1, 2, 3))
    );
    let below = coords(1, 2, -1);
    assert_eq!(Target::of(&below, &s), Target::Other(below.clone()));
}

#[test]
fn a_unit_can_be_chosen_from_a_list_but_only_if_it_is_legal() {
    let builder = CommandBuilder::new(PlayerId(0), vec![Command::ActivateUnit(UnitId(0))]);
    assert_eq!(builder.units(), [UnitId(0)]);
    let activate = Some(Command::ActivateUnit(UnitId(0)));
    assert_eq!(builder.choose_unit(UnitId(0)), activate);
    assert_eq!(builder.choose_unit(UnitId(1)), None);
}

#[test]
fn with_only_the_end_left_the_builder_says_so_and_explains_clicks() {
    let s = state();
    let mut builder = CommandBuilder::new(PlayerId(0), vec![Command::EndActivation]);
    assert_eq!(builder.step(), Step::End);
    assert!(builder.can_end());
    assert!(builder.highlights(&s).figures.is_empty());
    let click = |b: &mut CommandBuilder, q| b.click(&s, &Pick::at(&s, space(q)));
    assert_eq!(click(&mut builder, 0), Err(Refusal::CannotAct));
    assert_eq!(click(&mut builder, 3), Err(Refusal::NotYours));
}

#[test]
fn every_command_the_builder_returns_is_one_of_the_legal_commands() {
    let s = state();
    let legal = activation_legal();
    let mut built = Vec::new();
    for figure in 0..2 {
        for action in 0..3 {
            for q in 0..4 {
                let mut builder = CommandBuilder::new(PlayerId(0), legal.clone());
                let _ = builder.click(&s, &Pick::figure(FigureId(figure)));
                built.extend(builder.choose_action(DefId(action)));
                let pick = Pick::at(&s, space(q));
                built.extend(builder.click(&s, &pick).ok().flatten());
                built.extend(builder.choose_option(0));
            }
        }
    }
    assert!(!built.is_empty());
    assert!(built.iter().all(|c| legal.contains(c)), "{built:?}");
}
