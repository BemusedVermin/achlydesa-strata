use super::*;

fn sample() -> Value {
    let mut map = BTreeMap::new();
    map.insert(Sym::new("zeta"), Value::Int(-3));
    map.insert(
        Sym::new("alpha"),
        Value::List(vec![Value::Nil, true.into()]),
    );
    map.insert(Sym::new("mid"), Value::Id(DefId(9)));
    map.insert(Sym::new("name"), "hi".into());
    Value::Map(map)
}

#[test]
fn round_trips_through_postcard() {
    let value = sample();
    let bytes = postcard::to_allocvec(&value).unwrap();
    assert_eq!(postcard::from_bytes::<Value>(&bytes).unwrap(), value);
}

#[test]
fn round_trips_through_json() {
    let value = sample();
    let text = serde_json::to_string(&value).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), value);
}

#[test]
fn map_iterates_in_key_order() {
    let value = sample();
    let keys: Vec<&str> = value.as_map().unwrap().keys().map(Sym::as_str).collect();
    assert_eq!(keys, ["alpha", "mid", "name", "zeta"]);
    let text = serde_json::to_string(&value).unwrap();
    assert!(text.find("alpha").unwrap() < text.find("zeta").unwrap());
}

#[test]
fn accessors_return_typed_errors() {
    let v = sample();
    assert_eq!(v.get("zeta").unwrap().as_int(), Ok(-3));
    assert_eq!(v.get("name").unwrap().as_str(), Ok("hi"));
    assert_eq!(v.get("mid").unwrap().as_id(), Ok(DefId(9)));
    assert_eq!(v.get("alpha").unwrap().at(1).unwrap().as_bool(), Ok(true));
    assert!(v.get("alpha").unwrap().at(0).unwrap().is_nil());
    assert_eq!(
        v.get("zeta").unwrap().as_str(),
        Err(ValueError::Type {
            expected: "str",
            found: "int"
        })
    );
    assert_eq!(v.get("nope"), Err(ValueError::MissingKey("nope".into())));
    assert_eq!(v.get("alpha").unwrap().at(5), Err(ValueError::Index(5)));
    assert_eq!(
        Value::Nil.get("a"),
        Err(ValueError::Type {
            expected: "map",
            found: "nil"
        })
    );
}

#[test]
fn hash_is_stable_and_distinguishes_variants() {
    assert_eq!(sample().stable_hash_value(), sample().stable_hash_value());
    assert_ne!(
        Value::Int(0).stable_hash_value(),
        Value::Bool(false).stable_hash_value()
    );
    assert_ne!(
        Value::Nil.stable_hash_value(),
        Value::List(vec![]).stable_hash_value()
    );
}

#[test]
fn golden_hashes() {
    assert_eq!(Value::Nil.stable_hash_value(), 0x48b2_c926_16fc_193d);
    assert_eq!(sample().stable_hash_value(), 0xcc41_64e0_81d0_3baf);
    assert_eq!(
        Sym::new("strata").stable_hash_value(),
        0x994e_8b10_9b8b_539a
    );
}
