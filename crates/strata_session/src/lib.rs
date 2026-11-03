//! Match setup, simulation thread, controllers, saves and replays.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

pub mod controller;
pub mod human;
pub mod pack_test;
pub mod random;
pub mod replay;
pub mod replay_text;
pub mod session;
pub mod setup;
pub mod sim;
pub mod view;

pub use controller::Controller;
pub use human::HumanController;
pub use random::RandomController;
pub use replay::{Replay, ReplayError, ReplayRecord, SNAPSHOT_EVERY, Snapshot};
pub use replay_text::dump;
pub use session::{Session, SessionError};
pub use setup::{MatchBuild, SetupError, build_match, pack_identities};
pub use sim::{Control, SimError, SimHandle, SimLink, SimMessage};
pub use view::{PlayerView, REDACTED, visible_events};

#[cfg(test)]
mod replay_tools_tests;
#[cfg(test)]
mod sim_tests;
#[cfg(test)]
mod tests;
