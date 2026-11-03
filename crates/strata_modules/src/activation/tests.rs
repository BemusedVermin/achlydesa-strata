use super::*;

fn list(items: &[&str]) -> Value {
    Value::List(items.iter().map(|s| Value::Str(Sym::new(s))).collect())
}

fn config(steps: Value, budgets: &[(&str, i64)], all: bool) -> Value {
    let budgets = budgets
        .iter()
        .map(|(k, v)| (Sym::new(k), Value::Int(*v)))
        .collect();
    Value::Map(
        [
            (Sym::new("steps"), steps),
            (Sym::new("budgets"), Value::Map(budgets)),
            (Sym::new("all_figures_per_step"), Value::Bool(all)),
        ]
        .into(),
    )
}

#[test]
fn the_default_steps_are_move_then_attack_with_all_figures_per_step() {
    let defaults = Activation
        .config_schema()
        .resolve(&BTreeMap::new())
        .unwrap();
    let rules = parse(&defaults).unwrap();
    assert_eq!(rules.steps, [Sym::new("move"), Sym::new("attack")]);
    assert_eq!(rules.ordering, OrderingPolicy::AllFiguresPerStep);
    assert!(rules.budgets.is_empty());
}

#[test]
fn per_figure_ordering_and_budgets_are_read() {
    let cfg = config(list(&["move", "attack"]), &[("move", 2)], false);
    let rules = parse(&cfg).unwrap();
    assert_eq!(rules.ordering, OrderingPolicy::PerFigure);
    assert_eq!(rules.budgets[&Sym::new("move")], 2);
}

#[test]
fn bad_configuration_is_refused() {
    assert!(parse(&config(list(&[]), &[], true)).is_err());
    assert!(parse(&config(list(&["move"]), &[("move", 300)], true)).is_err());
    assert!(parse(&config(Value::List(vec![Value::Int(1)]), &[], true)).is_err());
}
