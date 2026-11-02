//! Module configuration schemas.

use std::collections::BTreeMap;

use strata_kernel::{Sym, Value};
use strata_model::ConfigField;

/// The value types a config field may declare, plus `any`.
const KINDS: [&str; 7] = ["int", "bool", "str", "id", "list", "map", "any"];

/// A config value that does not fit its schema.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// The ruleset sets an option the schema does not declare.
    #[error("unknown option `{0}`")]
    UnknownOption(String),
    /// An option has no value and no default.
    #[error("option `{0}` is required")]
    MissingOption(String),
    /// An option has the wrong type.
    #[error("option `{option}` expects {expected}, found {found}")]
    Type {
        /// The option name.
        option: String,
        /// The declared type.
        expected: String,
        /// The type of the supplied value.
        found: &'static str,
    },
    /// The schema declares a type that does not exist.
    #[error("option `{option}` declares unknown type `{kind}`")]
    UnknownType {
        /// The option name.
        option: String,
        /// The declared type.
        kind: String,
    },
}

/// The options a module accepts, with their types and defaults.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigSchema(BTreeMap<Sym, ConfigField>);

impl ConfigSchema {
    /// Creates a schema from its fields.
    pub fn new(fields: BTreeMap<Sym, ConfigField>) -> Self {
        Self(fields)
    }

    /// Checks `given` against the schema and returns the full config map with defaults filled in.
    pub fn resolve(&self, given: &BTreeMap<Sym, Value>) -> Result<Value, ConfigError> {
        if let Some(key) = given.keys().find(|k| !self.0.contains_key(*k)) {
            return Err(ConfigError::UnknownOption(key.to_string()));
        }
        let mut out = BTreeMap::new();
        for (name, field) in &self.0 {
            let option = || name.to_string();
            let kind = field.kind.as_str();
            if !KINDS.contains(&kind) {
                return Err(ConfigError::UnknownType {
                    option: option(),
                    kind: kind.to_owned(),
                });
            }
            let value = match given.get(name) {
                Some(v) => v,
                None if field.default.is_nil() => return Err(ConfigError::MissingOption(option())),
                None => &field.default,
            };
            if kind != "any" && value.type_name() != kind {
                return Err(ConfigError::Type {
                    option: option(),
                    expected: kind.to_owned(),
                    found: value.type_name(),
                });
            }
            out.insert(name.clone(), value.clone());
        }
        Ok(Value::Map(out))
    }
}
