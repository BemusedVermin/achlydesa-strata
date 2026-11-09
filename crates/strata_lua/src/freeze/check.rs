//! Checks a typed definition must pass once it decodes, beyond what its fields' types enforce.

use strata_model::AbilityDef;
use strata_rules::{check_ability, check_effect};

use super::FrozenDef;

/// A fault in a decoded definition: the route to the field at fault, and what is wrong.
pub(super) struct Invalid {
    pub(super) field_path: String,
    pub(super) message: String,
}

/// Checks `def`, the record `id`. A footprint's shape must be usable by movement (design §7.3),
/// and every native descriptor of an ability, a unit's abilities, a relic's grants and a relic's
/// effect must compile (design §8.2), so a bad one fails at load rather than in play.
pub(super) fn check(id: &str, def: &FrozenDef) -> Result<(), Invalid> {
    match def {
        FrozenDef::Footprint(footprint) => footprint.check().map_err(|e| Invalid {
            field_path: e.field().to_owned(),
            message: format!("footprint `{id}`: {e}"),
        }),
        FrozenDef::Ability(ability) => abilities(id, "", std::slice::from_ref(ability)),
        FrozenDef::Unit(unit) => abilities(id, "abilities/", &unit.abilities),
        FrozenDef::Relic(relic) => {
            if let Some(grants) = &relic.grants {
                abilities(id, "grants/abilities/", &grants.abilities)?;
            }
            match &relic.effect {
                Some(effect) => check_effect(effect).map_err(|e| Invalid {
                    field_path: "effect".to_owned(),
                    message: format!("relic `{id}`: {e}"),
                }),
                None => Ok(()),
            }
        }
        _ => Ok(()),
    }
}

/// Checks each ability of a list at `prefix` (`abilities/` for a unit's), numbering entries from
/// one; an empty prefix is a single `ability` record.
fn abilities(id: &str, prefix: &str, list: &[AbilityDef]) -> Result<(), Invalid> {
    for (i, ability) in list.iter().enumerate() {
        check_ability(ability).map_err(|fault| {
            let at = if prefix.is_empty() {
                fault.path.clone()
            } else {
                format!("{prefix}{}/{}", i + 1, fault.path)
            };
            Invalid {
                field_path: at,
                message: format!("`{id}`: {}", fault.error),
            }
        })?;
    }
    Ok(())
}
