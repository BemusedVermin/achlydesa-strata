#![allow(dead_code)]

use strata_kernel::{StableHash, StableHasher};
use xxhash_rust::xxh3::xxh3_64;

fn raw(bytes: &[u8]) -> u64 {
    xxh3_64(bytes)
}

#[derive(StableHash)]
struct Named {
    a: u32,
    b: String,
}

#[derive(StableHash)]
struct Swapped {
    b: String,
    a: u32,
}

#[derive(StableHash)]
struct Tuple(u8, bool);

#[derive(StableHash)]
struct Unit;

#[derive(StableHash)]
struct Wrapper<T> {
    inner: T,
}

#[derive(StableHash)]
struct Bounded<T>
where
    T: Copy,
{
    inner: T,
}

#[derive(StableHash)]
struct Cached {
    value: u32,
    #[stable_hash(skip)]
    cache: Option<u64>,
}

#[derive(StableHash)]
enum Shape {
    Empty,
    Point(i32, i32),
    Rect { w: u16, h: u16 },
}

#[derive(StableHash)]
enum ShapeV2 {
    Empty,
    Circle(u8),
    Point(i32, i32),
    Rect { w: u16, h: u16 },
}

#[derive(StableHash)]
enum Skipping {
    A(#[stable_hash(skip)] u8, u8),
    B {
        #[stable_hash(skip)]
        junk: u8,
    },
}

fn manual_named(a: u32, b: &str) -> u64 {
    let mut hasher = StableHasher::new();
    a.stable_hash(&mut hasher);
    b.stable_hash(&mut hasher);
    hasher.finish()
}

#[test]
fn struct_matches_hand_written() {
    let value = Named {
        a: 7,
        b: "hi".into(),
    };
    assert_eq!(value.stable_hash_value(), manual_named(7, "hi"));
}

#[test]
fn struct_matches_raw_bytes() {
    let mut bytes = 7u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&2u64.to_le_bytes());
    bytes.extend_from_slice(b"hi");
    let value = Named {
        a: 7,
        b: "hi".into(),
    };
    assert_eq!(value.stable_hash_value(), raw(&bytes));
}

#[test]
fn golden_values() {
    assert_eq!(Unit.stable_hash_value(), raw(&[]));
    assert_eq!(Tuple(3, true).stable_hash_value(), raw(&[3, 1]));
    assert_eq!(Shape::Empty.stable_hash_value(), raw(&0u32.to_le_bytes()));
    let mut point = 1u32.to_le_bytes().to_vec();
    point.extend_from_slice(&(-1i32).to_le_bytes());
    point.extend_from_slice(&2i32.to_le_bytes());
    assert_eq!(Shape::Point(-1, 2).stable_hash_value(), raw(&point));
    let mut rect = 2u32.to_le_bytes().to_vec();
    rect.extend_from_slice(&[3, 0, 4, 0]);
    assert_eq!(Shape::Rect { w: 3, h: 4 }.stable_hash_value(), raw(&rect));
}

#[test]
fn reordering_fields_changes_hash() {
    let named = Named {
        a: 7,
        b: "hi".into(),
    };
    let swapped = Swapped {
        b: "hi".into(),
        a: 7,
    };
    assert_ne!(named.stable_hash_value(), swapped.stable_hash_value());
}

#[test]
fn adding_a_variant_changes_hash() {
    assert_ne!(
        Shape::Point(1, 2).stable_hash_value(),
        ShapeV2::Point(1, 2).stable_hash_value()
    );
    assert_ne!(
        Shape::Rect { w: 1, h: 2 }.stable_hash_value(),
        ShapeV2::Rect { w: 1, h: 2 }.stable_hash_value()
    );
}

#[test]
fn generics_hash_like_their_contents() {
    assert_eq!(
        Wrapper { inner: 9u8 }.stable_hash_value(),
        9u8.stable_hash_value()
    );
    assert_eq!(
        Wrapper {
            inner: "x".to_string()
        }
        .stable_hash_value(),
        "x".stable_hash_value()
    );
    assert_eq!(
        Bounded { inner: 5u16 }.stable_hash_value(),
        5u16.stable_hash_value()
    );
}

#[test]
fn skip_ignores_the_field() {
    let a = Cached {
        value: 1,
        cache: None,
    };
    let b = Cached {
        value: 1,
        cache: Some(99),
    };
    assert_eq!(a.stable_hash_value(), b.stable_hash_value());
    assert_eq!(a.stable_hash_value(), 1u32.stable_hash_value());
    assert_ne!(
        a.stable_hash_value(),
        Cached {
            value: 2,
            cache: None
        }
        .stable_hash_value()
    );
}

#[test]
fn skip_in_enum_variants() {
    let mut a = 0u32.to_le_bytes().to_vec();
    a.push(5);
    assert_eq!(Skipping::A(1, 5).stable_hash_value(), raw(&a));
    assert_eq!(
        Skipping::B { junk: 8 }.stable_hash_value(),
        raw(&1u32.to_le_bytes())
    );
}

#[test]
fn hashing_twice_is_identical() {
    let value = Shape::Rect { w: 3, h: 4 };
    assert_eq!(value.stable_hash_value(), value.stable_hash_value());
}
