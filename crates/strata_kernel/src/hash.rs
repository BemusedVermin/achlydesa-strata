//! Stable, platform-independent hashing for game state.
//!
//! Integers hash as little-endian bytes, `usize`/`isize` as 64-bit, and every variable-length
//! value is prefixed with its length as a `u64`, so results never depend on endianness or
//! pointer width.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::BuildHasher;

use indexmap::IndexMap;
use xxhash_rust::xxh3::Xxh3;

/// Streaming xxh3-64 hasher fed by [`StableHash`] implementations.
#[derive(Clone, Default)]
pub struct StableHasher(Xxh3);

impl StableHasher {
    /// Creates an empty hasher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds raw bytes into the hash.
    pub fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// Feeds a length or count as a 64-bit little-endian integer.
    pub fn write_len(&mut self, len: usize) {
        self.write(&(len as u64).to_le_bytes());
    }

    /// Returns the 64-bit hash of everything written so far.
    pub fn finish(&self) -> u64 {
        self.0.digest()
    }
}

/// A value that hashes identically on every platform and Rust version.
pub trait StableHash {
    /// Feeds this value into `hasher`.
    fn stable_hash(&self, hasher: &mut StableHasher);

    /// Returns the xxh3-64 hash of this value alone.
    fn stable_hash_value(&self) -> u64 {
        let mut hasher = StableHasher::new();
        self.stable_hash(&mut hasher);
        hasher.finish()
    }
}

macro_rules! impl_int {
    ($($t:ty => $as:ty),* $(,)?) => {$(
        impl StableHash for $t {
            fn stable_hash(&self, hasher: &mut StableHasher) {
                hasher.write(&(*self as $as).to_le_bytes());
            }
        }
    )*};
}

impl_int!(
    u8 => u8, u16 => u16, u32 => u32, u64 => u64, u128 => u128, usize => u64,
    i8 => i8, i16 => i16, i32 => i32, i64 => i64, i128 => i128, isize => i64,
);

impl StableHash for bool {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write(&[u8::from(*self)]);
    }
}

impl StableHash for str {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write_len(self.len());
        hasher.write(self.as_bytes());
    }
}

impl StableHash for String {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.as_str().stable_hash(hasher);
    }
}

impl<T: StableHash + ?Sized> StableHash for &T {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        (**self).stable_hash(hasher);
    }
}

impl<T: StableHash> StableHash for Option<T> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        match self {
            None => false.stable_hash(hasher),
            Some(value) => {
                true.stable_hash(hasher);
                value.stable_hash(hasher);
            }
        }
    }
}

impl<T: StableHash> StableHash for [T] {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write_len(self.len());
        for item in self {
            item.stable_hash(hasher);
        }
    }
}

impl<T: StableHash> StableHash for Vec<T> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        self.as_slice().stable_hash(hasher);
    }
}

macro_rules! impl_tuple {
    ($($name:ident),+) => {
        impl<$($name: StableHash),+> StableHash for ($($name,)+) {
            #[allow(non_snake_case)]
            fn stable_hash(&self, hasher: &mut StableHasher) {
                let ($($name,)+) = self;
                $($name.stable_hash(hasher);)+
            }
        }
    };
}

impl_tuple!(A);
impl_tuple!(A, B);
impl_tuple!(A, B, C);
impl_tuple!(A, B, C, D);

impl<K: StableHash, V: StableHash> StableHash for BTreeMap<K, V> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write_len(self.len());
        for (key, value) in self {
            key.stable_hash(hasher);
            value.stable_hash(hasher);
        }
    }
}

impl<T: StableHash> StableHash for BTreeSet<T> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write_len(self.len());
        for item in self {
            item.stable_hash(hasher);
        }
    }
}

/// Hashes in insertion order.
impl<K: StableHash, V: StableHash, S: BuildHasher> StableHash for IndexMap<K, V, S> {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        hasher.write_len(self.len());
        for (key, value) in self {
            key.stable_hash(hasher);
            value.stable_hash(hasher);
        }
    }
}

#[cfg(test)]
mod tests;
