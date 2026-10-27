use super::*;

fn raw(bytes: &[u8]) -> u64 {
    xxhash_rust::xxh3::xxh3_64(bytes)
}

#[test]
fn integers_are_little_endian_and_width_fixed() {
    assert_eq!(0x0102_0304u32.stable_hash_value(), raw(&[4, 3, 2, 1]));
    assert_eq!((-2i16).stable_hash_value(), raw(&[0xfe, 0xff]));
    assert_eq!(5usize.stable_hash_value(), 5u64.stable_hash_value());
    assert_eq!((-5isize).stable_hash_value(), (-5i64).stable_hash_value());
}

#[test]
fn strings_are_length_prefixed() {
    let mut expected = 2u64.to_le_bytes().to_vec();
    expected.extend_from_slice(b"hi");
    assert_eq!("hi".stable_hash_value(), raw(&expected));
    assert_eq!(
        String::from("hi").stable_hash_value(),
        "hi".stable_hash_value()
    );
    assert_ne!(
        ("ab", "c").stable_hash_value(),
        ("a", "bc").stable_hash_value()
    );
}

#[test]
fn option_and_collections_match_layout() {
    assert_eq!(None::<u8>.stable_hash_value(), raw(&[0]));
    assert_eq!(Some(7u8).stable_hash_value(), raw(&[1, 7]));
    let mut expected = 2u64.to_le_bytes().to_vec();
    expected.extend_from_slice(&[1, 2]);
    assert_eq!(vec![1u8, 2].stable_hash_value(), raw(&expected));
    assert_eq!([1u8, 2][..].stable_hash_value(), raw(&expected));
    assert_eq!((1u8, 2u8, true).stable_hash_value(), raw(&[1, 2, 1]));
}

#[test]
fn maps_hash_in_key_order_and_index_map_in_insertion_order() {
    let a: BTreeMap<u8, u8> = [(2, 20), (1, 10)].into_iter().collect();
    let b: BTreeMap<u8, u8> = [(1, 10), (2, 20)].into_iter().collect();
    assert_eq!(a.stable_hash_value(), b.stable_hash_value());
    let set: BTreeSet<u8> = [2, 1].into_iter().collect();
    assert_eq!(set.stable_hash_value(), vec![1u8, 2].stable_hash_value());

    let first: IndexMap<u8, u8> = [(2, 20), (1, 10)].into_iter().collect();
    let second: IndexMap<u8, u8> = [(1, 10), (2, 20)].into_iter().collect();
    assert_ne!(first.stable_hash_value(), second.stable_hash_value());
    assert_eq!(second.stable_hash_value(), a.stable_hash_value());
}

#[test]
fn golden_values() {
    assert_eq!(GOLDEN_INT, 42u32.stable_hash_value());
    assert_eq!(GOLDEN_STR, "strata".stable_hash_value());
    let nested: Vec<(u8, Option<String>)> = vec![(1, Some("a".into())), (2, None)];
    assert_eq!(GOLDEN_NESTED, nested.stable_hash_value());
}

const GOLDEN_INT: u64 = 2392174772787195229;
const GOLDEN_STR: u64 = 11046919839431742362;
const GOLDEN_NESTED: u64 = 6459949751945219260;
