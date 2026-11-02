//! LuaLS `---@meta` stubs for the data-stage constructors (design §9.5).
//!
//! The field tables in [`classes`] sit beside the Rust definitions they mirror, and a test checks
//! that they cover exactly the kinds the data stage registers. Output is a pure function of
//! those tables, so regenerating it gives identical files.

mod classes;
mod render;

pub use classes::{Class, Field, Ty};

/// A generated stub file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StubFile {
    /// File name, such as `unit.lua`.
    pub name: String,
    /// File contents.
    pub text: String,
}

/// The stub files for every constructor, in kind order.
pub fn generate() -> Vec<StubFile> {
    classes::KINDS
        .iter()
        .map(|kind| StubFile {
            name: format!("{}.lua", kind.name),
            text: render::file(kind),
        })
        .collect()
}

/// The constructor kinds that have stubs.
pub fn kinds() -> Vec<&'static str> {
    classes::KINDS.iter().map(|kind| kind.name).collect()
}

#[cfg(test)]
mod tests;
