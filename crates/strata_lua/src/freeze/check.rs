//! Checks a typed definition must pass once it decodes, beyond what its fields' types enforce.

use strata_model::{Scope, UnitDef};

use super::FrozenDef;

/// A fault in a decoded definition: the route to the field at fault, and what is wrong.
pub(super) struct Invalid {
    pub(super) field_path: String,
    pub(super) message: String,
}

/// Checks `def`, the record `id`. A footprint's shape must be usable by movement (design
/// §7.3), and a unit's printed abilities must have a scope setup can attach (design §10.5).
pub(super) fn check(id: &str, def: &FrozenDef) -> Result<(), Invalid> {
    match def {
        FrozenDef::Footprint(footprint) => footprint.check().map_err(|e| Invalid {
            field_path: e.field().to_owned(),
            message: format!("footprint `{id}`: {e}"),
        }),
        FrozenDef::Unit(unit) => printed_scopes(id, unit),
        _ => Ok(()),
    }
}

/// Setup attaches a unit's printed abilities to the unit, so each must be scoped to the unit or
/// one of its figures. Army, holder and occupant scopes on a unit are not supported yet (#211).
fn printed_scopes(id: &str, unit: &UnitDef) -> Result<(), Invalid> {
    let wrong = unit.abilities.iter().enumerate();
    let mut wrong = wrong.filter(|(_, a)| !matches!(a.scope, Scope::Unit | Scope::Figure));
    let Some((at, ability)) = wrong.next() else {
        return Ok(());
    };
    let scope = match ability.scope {
        Scope::Army => "army",
        Scope::Holder => "holder",
        _ => "occupant",
    };
    Err(Invalid {
        field_path: format!("abilities/{}/scope", at + 1),
        message: format!(
            "unit `{id}`: printed ability `{}` has scope `{scope}`, but a unit's printed \
             abilities may only be scoped to `unit` or `figure` (army, holder and occupant \
             scopes on units are #211)",
            ability.name.0
        ),
    })
}
