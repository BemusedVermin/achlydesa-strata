use super::*;
use std::collections::BTreeSet;

#[test]
fn sym_compares_orders_and_hashes_by_content() {
    let a = Sym::new("alpha");
    let b = Sym::from(String::from("alpha"));
    assert_eq!(a, b);
    assert!(Sym::new("a") < Sym::new("b"));
    assert_eq!(a.stable_hash_value(), b.stable_hash_value());
    assert_eq!(a.stable_hash_value(), "alpha".stable_hash_value());
    let set: BTreeSet<Sym> = ["b", "a", "b"].into_iter().map(Sym::from).collect();
    assert_eq!(set.len(), 2);
}

#[test]
fn sym_serializes_as_a_plain_string() {
    assert_eq!(serde_json::to_string(&Sym::new("x")).unwrap(), "\"x\"");
    let id = PackId::new("strata_std");
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"strata_std\"");
    let back: PackId = serde_json::from_str("\"strata_std\"").unwrap();
    assert_eq!(back, id);
    assert_eq!(id.to_string(), "strata_std");
}

#[test]
fn def_id_round_trips_and_hashes_as_u32() {
    let id = DefId(7);
    let bytes = postcard::to_stdvec(&id).unwrap();
    assert_eq!(postcard::from_bytes::<DefId>(&bytes).unwrap(), id);
    assert_eq!(id.stable_hash_value(), 7u32.stable_hash_value());
}

#[test]
fn id_newtypes_hash_like_their_text() {
    assert_eq!(
        SlotId::new("combat").stable_hash_value(),
        "combat".stable_hash_value()
    );
}
