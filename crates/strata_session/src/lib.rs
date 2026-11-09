//! Match setup, simulation thread, controllers, saves and replays.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

pub mod army;
pub mod cards;
pub mod controller;
pub mod human;
pub mod inspect;
pub mod launch;
pub mod names;
pub mod pack_test;
pub mod random;
pub mod replay;
pub mod replay_text;
pub mod session;
pub mod setup;
pub mod sim;
pub mod view;

pub use army::{army_units, validate_army};
pub use cards::{AbilityLine, CardSource, FigureLine, NoCards, StatLine, UnitCard};
pub use controller::Controller;
pub use human::HumanController;
pub use inspect::{Change, Clamp, InspectStep, Inspection, TraceMismatch};
pub use launch::{LaunchSpec, launch};
pub use names::Names;
pub use random::RandomController;
pub use replay::{
    REPLAY_FORMAT, Replay, ReplayError, ReplayRecord, SNAPSHOT_EVERY, Snapshot, engine_version,
};
pub use replay_text::dump;
pub use session::{Session, SessionError};
pub use setup::{MatchBuild, SetupError, build_match, check_victory, pack_identities};
pub use sim::{Control, SimError, SimHandle, SimLink, SimMessage};
pub use view::{BoardInfo, PlayerView, REDACTED, visible_events};

#[cfg(test)]
mod replay_tools_tests;
#[cfg(test)]
mod sim_tests;
#[cfg(test)]
mod tests;
