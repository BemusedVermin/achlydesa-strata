//! Identifiers shared by every layer (design §4.4).

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{StableHash, StableHasher};

/// An immutable string compared, ordered and hashed by content.
///
/// There is no global interner, so nothing depends on interning order.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Sym(Arc<str>);

impl Sym {
    /// Creates a symbol holding `text`.
    pub fn new(text: &str) -> Self {
        Self(Arc::from(text))
    }

    /// Returns the symbol's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Sym {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl fmt::Display for Sym {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Sym {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl From<String> for Sym {
    fn from(text: String) -> Self {
        Self(Arc::from(text))
    }
}

impl AsRef<str> for Sym {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for Sym {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl StableHash for Sym {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.as_str().stable_hash(hasher);
    }
}

/// A dense handle for a definition, assigned in sorted ID order when content is frozen.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    StableHash,
)]
pub struct DefId(pub u32);

macro_rules! sym_id {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(
            Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
            Serialize, Deserialize, StableHash,
        )]
        pub struct $name(pub Sym);

        impl $name {
            /// Creates an ID from `text`.
            pub fn new(text: &str) -> Self {
                Self(Sym::new(text))
            }

            /// Returns the ID's text.
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl From<&str> for $name {
            fn from(text: &str) -> Self {
                Self::new(text)
            }
        }
    )*};
}

sym_id!(
    /// The name of a content pack.
    PackId,
    /// The name of a rule module.
    ModuleId,
    /// The name of a rules slot.
    SlotId,
    /// The name of a trigger or replacement window.
    WindowId,
);

#[cfg(test)]
mod tests;
