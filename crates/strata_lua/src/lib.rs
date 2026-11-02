//! The Lua host, sandbox, data-stage constructors and module adapter.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

mod convert;
mod data;
mod error;
mod freeze;
mod host;
mod runtime;
pub mod stubs;

pub use convert::{lua_to_value, value_to_lua};
pub use data::{DataStage, PackSource, Record, Source};
pub use error::{ConvertError, DataError, HostError, LoadError, RuntimeError};
pub use freeze::{Def, FrozenDef, FrozenRecord, Reference, Resolver, freeze, freeze_resolved};
pub use host::{Halt, Limits, LuaHost};
pub use runtime::RuntimeLua;
