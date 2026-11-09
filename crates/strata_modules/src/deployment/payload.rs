//! The `place_unit` payload: the deployment slot's command contract (design §11.2).
//!
//! Any module that fills the `deployment` slot accepts this payload, so the UI, the AI and the
//! pack-test `act.deploy` step can build placements without knowing which module runs them:
//!
//! ```lua
//! { unit = <unit id>, at = { { q, r, z, o }, ... } }
//! ```
//!
//! `at` lists one spot per figure, in the order the unit lists its figures. A spot is the
//! figure's anchor space `(q, r, z)` and its rotation `o`, in 60-degree clockwise steps from
//! `0` to `5` (design §7.3). `o` may be left out, which means `0`, the default facing.

use strata_kernel::{Rotation, Space, Sym, Value, ValueError};
use strata_model::UnitId;

/// The custom command a player sends to place a unit.
pub const PLACE_UNIT: &str = "place_unit";

/// Where one figure goes: its anchor space and the way it faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Spot {
    /// The space the footprint's anchor cell stands on.
    pub anchor: Space,
    /// The footprint's rotation about the anchor.
    pub rotation: Rotation,
}

/// A decoded `place_unit` payload: one unit and a spot for each of its figures.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaceUnit {
    /// The unit to place.
    pub unit: UnitId,
    /// One spot per figure, in the order the unit lists its figures.
    pub at: Vec<Spot>,
}

impl PlaceUnit {
    /// Reads a payload, with or without the rotations. The error names what is malformed.
    pub fn from_value(payload: &Value) -> Result<Self, String> {
        let bad = |e: ValueError| format!("place_unit: {e}");
        let unit = payload.get("unit").and_then(Value::as_int).map_err(bad)?;
        let unit = u32::try_from(unit).map_err(|_| "place_unit: no such unit".to_string())?;
        let spots = payload.get("at").and_then(Value::as_list).map_err(bad)?;
        let at = spots.iter().map(spot).collect::<Result<_, _>>()?;
        Ok(Self {
            unit: UnitId(unit),
            at,
        })
    }

    /// The payload, with every spot's rotation written out.
    pub fn to_value(&self) -> Value {
        let spot = |s: &Spot| {
            let Space { q, r, z } = s.anchor;
            let o = s.rotation.steps();
            let parts = [i64::from(q), i64::from(r), i64::from(z), i64::from(o)];
            Value::List(parts.into_iter().map(Value::Int).collect())
        };
        let entries = [
            (Sym::new("unit"), Value::Int(i64::from(self.unit.0))),
            (
                Sym::new("at"),
                Value::List(self.at.iter().map(spot).collect()),
            ),
        ];
        Value::Map(entries.into())
    }
}

/// Reads one spot, `{ q, r, z }` or `{ q, r, z, o }`.
fn spot(value: &Value) -> Result<Spot, String> {
    let shape = || "place_unit: each spot is { q, r, z } or { q, r, z, o }".to_string();
    let parts = value.as_list().map_err(|_| shape())?;
    let ints = parts
        .iter()
        .map(Value::as_int)
        .collect::<Result<Vec<_>, _>>();
    let ints = ints.map_err(|_| shape())?;
    let (q, r, z, o) = match ints[..] {
        [q, r, z] => (q, r, z, 0),
        [q, r, z, o] => (q, r, z, o),
        _ => return Err(shape()),
    };
    let coord = |n: i64| i32::try_from(n).map_err(|_| format!("place_unit: {n} is off the board"));
    let z = u32::try_from(z).map_err(|_| "place_unit: z is negative".to_string())?;
    let o = u8::try_from(o).ok().and_then(Rotation::new);
    let o = o.ok_or_else(|| "place_unit: a rotation is from 0 to 5".to_string())?;
    Ok(Spot {
        anchor: Space::new(coord(q)?, coord(r)?, z),
        rotation: o,
    })
}
