use proptest::prelude::*;
use strata_kernel::{Rotation, Space, Sym, Value};
use strata_model::UnitId;

use super::{PlaceUnit, Spot};

fn payload(spots: Vec<Vec<i64>>) -> Value {
    let at = spots
        .into_iter()
        .map(|s| Value::List(s.into_iter().map(Value::Int).collect()))
        .collect();
    let entries = [
        (Sym::new("unit"), Value::Int(3)),
        (Sym::new("at"), Value::List(at)),
    ];
    Value::Map(entries.into())
}

#[test]
fn a_missing_rotation_means_the_default_facing() {
    let put = PlaceUnit::from_value(&payload(vec![vec![1, -2, 3]])).unwrap();
    let spot = Spot {
        anchor: Space::new(1, -2, 3),
        rotation: Rotation::default(),
    };
    assert_eq!(
        put,
        PlaceUnit {
            unit: UnitId(3),
            at: vec![spot]
        }
    );
}

#[test]
fn a_rotation_is_read_from_the_fourth_place() {
    let put = PlaceUnit::from_value(&payload(vec![vec![0, 0, 1, 4]])).unwrap();
    assert_eq!(put.at[0].rotation, Rotation::new(4).unwrap());
}

#[test]
fn a_rotation_outside_zero_to_five_is_refused() {
    for o in [-1, 6] {
        let err = PlaceUnit::from_value(&payload(vec![vec![0, 0, 1, o]])).unwrap_err();
        assert!(err.contains("rotation"), "{err}");
    }
}

#[test]
fn a_spot_of_the_wrong_shape_is_refused() {
    for spot in [vec![0, 0], vec![0, 0, 1, 0, 0]] {
        let err = PlaceUnit::from_value(&payload(vec![spot])).unwrap_err();
        assert!(err.contains("each spot"), "{err}");
    }
    let err = PlaceUnit::from_value(&payload(vec![vec![0, 0, -1]])).unwrap_err();
    assert!(err.contains("negative"), "{err}");
}

proptest! {
    #[test]
    fn a_payload_reads_back_what_it_wrote(
        unit in 0u32..1000,
        spots in prop::collection::vec((-50i32..50, -50i32..50, 0u32..20, 0u8..6), 0..4),
    ) {
        let at = spots.into_iter().map(|(q, r, z, o)| Spot {
            anchor: Space::new(q, r, z),
            rotation: Rotation::new(o).unwrap(),
        });
        let put = PlaceUnit { unit: UnitId(unit), at: at.collect() };
        prop_assert_eq!(PlaceUnit::from_value(&put.to_value()), Ok(put));
    }
}
