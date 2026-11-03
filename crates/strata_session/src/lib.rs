//! Match setup, simulation thread, controllers, saves and replays.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

pub mod controller;
pub mod pack_test;
pub mod replay;
pub mod session;
pub mod setup;
pub mod view;

pub use controller::Controller;
pub use replay::{Replay, ReplayError, ReplayRecord, SNAPSHOT_EVERY, Snapshot};
pub use session::{Session, SessionError};
pub use setup::{MatchBuild, SetupError, build_match, pack_identities};
pub use view::{PlayerView, REDACTED, visible_events};

#[cfg(test)]
mod tests;
