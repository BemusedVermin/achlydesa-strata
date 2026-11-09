//! Checks a typed definition must pass once it decodes, beyond what its fields' types enforce.

use super::FrozenDef;

/// A fault in a decoded definition: the route to the field at fault, and what is wrong.
pub(super) struct Invalid {
    pub(super) field_path: &'static str,
    pub(super) message: String,
}

/// Checks `def`, the record `id`. Only footprints have checks so far: their shape must be
/// usable by movement (design §7.3).
pub(super) fn check(id: &str, def: &FrozenDef) -> Result<(), Invalid> {
    match def {
        FrozenDef::Footprint(footprint) => footprint.check().map_err(|e| Invalid {
            field_path: e.field(),
            message: format!("footprint `{id}`: {e}"),
        }),
        _ => Ok(()),
    }
}
