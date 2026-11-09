//! Height overlap and configuration of `core:engagement`; behavior is covered by pack tests.

use std::collections::BTreeMap;

use proptest::prelude::*;
use strata_kernel::{Sym, Value};
use strata_rules::{ModuleError, Registrar, RuleModule};

use super::{EngagementModule, overlaps};

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
