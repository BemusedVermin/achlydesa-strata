//! The dynamic value shared by scripts, events and parameters (design §5.1).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::StableHash;
use crate::id::{DefId, Sym};

/// A dynamically typed value. There is deliberately no floating-point variant.
#[derive(
    Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, StableHash,
)]
pub enum Value {
    /// The absence of a value.
    #[default]
    Nil,
    /// A boolean.
    Bool(bool),
    /// A signed 64-bit integer.
    Int(i64),
    /// A string.
    Str(Sym),
    /// A definition handle.
    Id(DefId),
    /// An ordered list.
    List(Vec<Value>),
    /// A map iterated in key order.
    Map(BTreeMap<Sym, Value>),
}

/// A failed [`Value`] accessor.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ValueError {
    /// The value has a different type than requested.
    #[error("expected {expected}, found {found}")]
    Type {
        /// The requested type name.
        expected: &'static str,
        /// The actual type name.
        found: &'static str,
    },
    /// A map has no entry for the key.
    #[error("missing key `{0}`")]
    MissingKey(String),
    /// A list has no element at the index.
    #[error("index {0} out of range")]
    Index(usize),
}

macro_rules! accessor {
    ($(#[$doc:meta])* $name:ident, $variant:ident, $ret:ty, $label:literal, |$v:ident| $conv:expr) => {
        $(#[$doc])*
        pub fn $name(&self) -> Result<$ret, ValueError> {
            match self {
                Self::$variant($v) => Ok($conv),
                other => Err(other.mismatch($label)),
            }
        }
    };
}

impl Value {
    /// Returns the name of this value's type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Nil => "nil",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Str(_) => "str",
            Self::Id(_) => "id",
            Self::List(_) => "list",
            Self::Map(_) => "map",
        }
    }

    fn mismatch(&self, expected: &'static str) -> ValueError {
        ValueError::Type {
            expected,
            found: self.type_name(),
        }
    }

    /// Returns whether this value is `Nil`.
    pub fn is_nil(&self) -> bool {
        matches!(self, Self::Nil)
    }

    accessor!(
        /// Returns the boolean, or a type error.
        as_bool, Bool, bool, "bool", |v| *v
    );
    accessor!(
        /// Returns the integer, or a type error.
        as_int, Int, i64, "int", |v| *v
    );
    accessor!(
        /// Returns the string, or a type error.
        as_str, Str, &str, "str", |v| v.as_str()
    );
    accessor!(
        /// Returns the definition handle, or a type error.
        as_id, Id, DefId, "id", |v| *v
    );
    accessor!(
        /// Returns the list elements, or a type error.
        as_list, List, &[Value], "list", |v| v.as_slice()
    );
    accessor!(
        /// Returns the map, or a type error.
        as_map, Map, &BTreeMap<Sym, Value>, "map", |v| v
    );

    /// Returns the map entry for `key`, or an error if this is not a map or the key is absent.
    pub fn get(&self, key: &str) -> Result<&Value, ValueError> {
        self.as_map()?
            .get(key)
            .ok_or_else(|| ValueError::MissingKey(key.to_owned()))
    }

    /// Returns the list element at `index`, or an error if this is not a list or it is too short.
    pub fn at(&self, index: usize) -> Result<&Value, ValueError> {
        self.as_list()?.get(index).ok_or(ValueError::Index(index))
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Str(Sym::new(v))
    }
}

impl From<DefId> for Value {
    fn from(v: DefId) -> Self {
        Self::Id(v)
    }
}

#[cfg(test)]
mod tests;
