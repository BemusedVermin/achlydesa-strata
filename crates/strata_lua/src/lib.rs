//! The Lua host, sandbox, data-stage constructors and module adapter.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

mod convert;
mod error;
mod host;

pub use convert::{lua_to_value, value_to_lua};
pub use error::{ConvertError, HostError};
pub use host::{Halt, Limits, LuaHost};
