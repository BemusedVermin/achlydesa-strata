//! Errors raised while discovering and resolving packs.

use std::path::PathBuf;

use strata_kernel::PackId;

/// A pack that could not be found, read or resolved. Each variant names the pack and file.
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    /// A directory or manifest could not be read.
    #[error("{}: {source}", path.display())]
    Io {
        /// The path being read.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// `pack.lua` failed to run or returned something other than one table.
    #[error("{}: {message}", file.display())]
    Script {
        /// The manifest file.
        file: PathBuf,
        /// What went wrong.
        message: String,
    },
    /// `pack.lua` returned a table with a missing, unknown or malformed field.
    #[error("{}: invalid manifest: {message}", file.display())]
    Manifest {
        /// The manifest file.
        file: PathBuf,
        /// What went wrong.
        message: String,
    },
    /// Two packs declare the same ID.
    #[error("duplicate pack `{id}`: {} and {}", first.display(), second.display())]
    Duplicate {
        /// The shared ID.
        id: PackId,
        /// The first manifest, in path order.
        first: PathBuf,
        /// The second manifest, in path order.
        second: PathBuf,
    },
    /// A pack depends on a pack that was not found.
    #[error("{}: pack `{pack}` depends on missing pack `{dependency}`", file.display())]
    MissingDependency {
        /// The depending pack.
        pack: PackId,
        /// The manifest file.
        file: PathBuf,
        /// The missing pack.
        dependency: PackId,
    },
    /// A dependency exists but its version does not satisfy the requirement.
    #[error(
        "{}: pack `{pack}` requires `{dependency}` {required}, found {found}",
        file.display()
    )]
    VersionMismatch {
        /// The depending pack.
        pack: PackId,
        /// The manifest file.
        file: PathBuf,
        /// The dependency.
        dependency: PackId,
        /// The requirement that failed.
        required: String,
        /// The version that was found.
        found: String,
    },
    /// A pack's `engine` range excludes this engine.
    #[error("{}: pack `{pack}` needs engine {required}, this is {engine}", file.display())]
    EngineMismatch {
        /// The pack.
        pack: PackId,
        /// The manifest file.
        file: PathBuf,
        /// The range declared by the pack.
        required: String,
        /// The running engine version.
        engine: String,
    },
    /// Packs depend on each other in a loop.
    #[error("{}: dependency cycle: {}", file.display(), cycle.join(" -> "))]
    Cycle {
        /// The manifest of the first pack in the cycle.
        file: PathBuf,
        /// The pack IDs around the cycle, ending at its start.
        cycle: Vec<String>,
    },
}
