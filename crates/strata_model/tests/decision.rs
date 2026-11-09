//! `Decision::check`: decisions no answer can satisfy are refused (issue #181).

use strata_kernel::{Space, Sym};
use strata_model::{Decision, DecisionKind, FigureId, Unanswerable};

fn check(kind: DecisionKind) -> Result<(), Unanswerable> {
    Decision {
        prompt: Sym::new("pick"),
        kind,
    }
    .check()
}

#[test]
fn empty_lists_are_unanswerable_and_named() {
    let cases = [
        (
            DecisionKind::ChooseFigure(vec![]),
            "the decision offers no figures",
        ),
        (
            DecisionKind::ChooseSpace(vec![]),
            "the decision offers no spaces",
        ),
        (
            DecisionKind::ChooseOption(vec![]),
            "the decision offers no options",
        ),
        (
            DecisionKind::Order(vec![]),
            "the decision offers no items to order",
        ),
    ];
    for (kind, message) in cases {
        let err = check(kind).unwrap_err();
        assert_eq!(err.to_string(), message);
    }
}

#[test]
fn an_inverted_range_is_unanswerable() {
    let err = check(DecisionKind::Number { min: 3, max: 1 }).unwrap_err();
    assert_eq!(err, Unanswerable::InvertedRange { min: 3, max: 1 });
    assert_eq!(err.to_string(), "number min 3 is greater than max 1");
}

#[test]
fn answerable_decisions_pass() {
    let kinds = [
        DecisionKind::ChooseFigure(vec![FigureId(0)]),
        DecisionKind::ChooseSpace(vec![Space::new(0, 0, 0)]),
        DecisionKind::ChooseOption(vec![Sym::new("a")]),
        DecisionKind::Order(vec![Sym::new("a")]),
        DecisionKind::YesNo,
        DecisionKind::Number { min: 2, max: 2 },
        DecisionKind::Number { min: -5, max: 5 },
    ];
    for kind in kinds {
        assert_eq!(check(kind), Ok(()));
    }
}
