//! Errors raised by the Lua host and the `Value` bridge.

/// A Lua value that cannot cross into or out of Rust (design §9.4).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConvertError {
    /// A float (or NaN) crossed the bridge; scripts must use integers.
    #[error("floats cannot cross the Lua bridge; use integers (`//` instead of `/`)")]
    Float,
    /// A function appeared where plain data was expected.
    #[error("functions cannot be converted to a value")]
    Function,
    /// A Lua type or `Value` variant with no counterpart on the other side.
    #[error("{0} cannot be converted")]
    Unsupported(&'static str),
    /// A string was not valid UTF-8.
    #[error("string is not valid UTF-8")]
    NotUtf8,
    /// A table mixed sequence and string keys, had gaps, or used another key type.
    #[error("table must be either a sequence or a string-keyed map")]
    BadTable,
    /// A table nested too deeply or contained a cycle.
    #[error("table nesting is too deep or cyclic")]
    TooDeep,
}

/// A failed call into the Lua host.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// Lua or `mlua` failed outside a protected call.
    #[error("lua error: {0}")]
    Lua(#[from] mlua::Error),
    /// The script raised an error.
    #[error("script error: {0}")]
    Script(String),
    /// The call used up its instruction budget.
    #[error("instruction budget exceeded")]
    Budget,
    /// The script raised the `Halt` sentinel.
    #[error("script halted")]
    Halt,
    /// A value could not be converted.
    #[error(transparent)]
    Convert(#[from] ConvertError),
}

/// A failure while running the data stages.
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    /// A stage file could not be read.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file.
        path: std::path::PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A pack has no `data.lua`.
    #[error("pack `{pack}` has no data.lua at {}", path.display())]
    MissingEntry {
        /// The pack.
        pack: strata_kernel::PackId,
        /// Where `data.lua` was expected.
        path: std::path::PathBuf,
    },
    /// A script raised an error; the message names the file and line.
    #[error("pack `{pack}` failed in stage `{stage}`: {message}")]
    Script {
        /// The pack.
        pack: strata_kernel::PackId,
        /// The stage: `data`, `data_updates` or `data_final`.
        stage: &'static str,
        /// The error text.
        message: String,
    },
    /// The host failed for a reason other than a script error.
    #[error(transparent)]
    Host(#[from] HostError),
}

impl From<mlua::Error> for DataError {
    fn from(error: mlua::Error) -> Self {
        Self::Host(error.into())
    }
}
