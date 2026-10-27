//! Procedural macros shared by the kernel and model crates.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

mod stable_hash;

use proc_macro::TokenStream;

/// Derives `strata_kernel::StableHash`.
///
/// Struct fields hash in declaration order. Enums hash the variant index as a `u32`, then the
/// variant's fields. Reordering fields or variants, or inserting a variant before another,
/// changes the hash. Mark a field `#[stable_hash(skip)]` to leave it out, for caches.
/// Type parameters receive a `StableHash` bound.
#[proc_macro_derive(StableHash, attributes(stable_hash))]
pub fn derive_stable_hash(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    stable_hash::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
