//! Saves and replays: the same file (design §12.6, §15.4).

use serde::{Deserialize, Serialize};
use strata_content::ContentRegistry;
use strata_model::{Answer, Command, Event, GameState, MatchSetup, PlayerId};
use strata_rules::{CommandRecord, Engine, Outcome};

use crate::setup::pack_identities;

/// Commands between snapshots.
pub const SNAPSHOT_EVERY: u32 = 20;

/// The saved form of a replay. Bump it whenever that form changes without an engine release,
/// such as a new field in `GameState`, so older files fail with [`ReplayError::Version`].
/// 1: abilities name their definition with an `AbilityKey`.
/// 2: match setup attaches each unit's printed abilities, so the first state differs.
pub const REPLAY_FORMAT: u32 = 2;

/// The version a replay records: the engine's, with [`REPLAY_FORMAT`] as semver build metadata.
pub fn engine_version() -> String {
    format!("{}+replay.{REPLAY_FORMAT}", env!("CARGO_PKG_VERSION"))
}

/// One committed command with the hash the state had afterwards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayRecord {
    /// Who submitted it.
    pub player: PlayerId,
    /// What was submitted.
    pub command: Command,
    /// The answers it finished with.
    pub answers: Vec<Answer>,
    /// The state hash after the command.
    pub state_hash: u64,
}

/// The state after `command_index` commands, kept for fast seeking.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// How many commands had been committed.
    pub command_index: u32,
    /// The state at that point.
    pub state: GameState,
}

/// Everything needed to play a match again. The setup carries the pack identities.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replay {
    /// The engine version that recorded it.
    pub engine_version: String,
    /// The inputs fixed before the first command.
    pub setup: MatchSetup,
    /// The command log, oldest first.
    pub commands: Vec<ReplayRecord>,
    /// Snapshots, every [`SNAPSHOT_EVERY`] commands.
    pub snapshots: Vec<Snapshot>,
}

/// A replay could not be saved, loaded or reproduced.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReplayError {
    /// The bytes are not a replay.
    #[error("not a replay: {0}")]
    Format(String),
    /// It was recorded by another engine version.
    #[error("recorded by engine {recorded}, this is {current}")]
    Version {
        /// The recording version.
        recorded: String,
        /// This engine's version.
        current: String,
    },
    /// The loaded packs are not the ones it was recorded with.
    #[error("the loaded packs differ from the recorded ones")]
    Packs,
    /// The engine does not start from this replay's setup.
    #[error("the engine does not start from the replay's setup")]
    WrongStart,
    /// A command did not commit as recorded.
    #[error("command {index} did not commit as recorded")]
    Diverged {
        /// Its position in the log.
        index: usize,
    },
    /// A state hash differs from the recorded one.
    #[error("state hash after command {index} is {got:#x}, recorded {expected:#x}")]
    HashMismatch {
        /// Its position in the log.
        index: usize,
        /// The recorded hash.
        expected: u64,
        /// The replayed hash.
        got: u64,
    },
    /// A snapshot is not the state its command produced.
    #[error("snapshot at command {index} does not match the replayed state")]
    SnapshotMismatch {
        /// How many commands had been committed.
        index: u32,
    },
}

impl Replay {
    /// A replay of `log` recorded by this engine version.
    pub fn new(setup: MatchSetup, log: &[CommandRecord], snapshots: Vec<Snapshot>) -> Self {
        let record = |r: &CommandRecord| ReplayRecord {
            player: r.player,
            command: r.command.clone(),
            answers: r.answers.clone(),
            state_hash: r.state_hash,
        };
        Self {
            engine_version: engine_version(),
            setup,
            commands: log.iter().map(record).collect(),
            snapshots,
        }
    }

    /// The compact machine form.
    pub fn save(&self) -> Result<Vec<u8>, ReplayError> {
        postcard::to_allocvec(self).map_err(|e| ReplayError::Format(e.to_string()))
    }

    /// Reads a replay saved by [`Replay::save`]. The version, saved first, is checked before
    /// the rest, whose form may differ between versions.
    pub fn load(bytes: &[u8]) -> Result<Self, ReplayError> {
        let format = |e: postcard::Error| ReplayError::Format(e.to_string());
        let (recorded, _) = postcard::take_from_bytes::<String>(bytes).map_err(format)?;
        check_version(&recorded)?;
        postcard::from_bytes(bytes).map_err(format)
    }

    /// Checks the engine version and that `registry` holds exactly the recorded packs.
    pub fn verify(&self, registry: &ContentRegistry) -> Result<(), ReplayError> {
        check_version(&self.engine_version)?;
        if pack_identities(registry) != self.setup.packs {
            return Err(ReplayError::Packs);
        }
        Ok(())
    }

    /// Plays the log through `engine`, which must stand at the start of this setup, and checks
    /// every state hash, including each snapshot's.
    pub fn play(&self, engine: &mut Engine<'_>) -> Result<(), ReplayError> {
        self.play_events(engine).map(drop)
    }

    /// Like [`Replay::play`], and returns the events each command committed.
    pub fn play_events(&self, engine: &mut Engine<'_>) -> Result<Vec<Vec<Event>>, ReplayError> {
        self.check_start(engine)?;
        let mut all = Vec::new();
        for (index, record) in self.commands.iter().enumerate() {
            all.push(record.apply(engine, index)?);
            let done = index as u32 + 1;
            let got = record.state_hash;
            let mut taken = self.snapshots.iter().filter(|s| s.command_index == done);
            if taken.any(|s| s.state.hash() != got) {
                return Err(ReplayError::SnapshotMismatch { index: done });
            }
        }
        Ok(all)
    }

    /// Checks that `engine` stands at the start of this setup.
    pub fn check_start(&self, engine: &Engine<'_>) -> Result<(), ReplayError> {
        let start = engine.state();
        if start.command_index != 0 || *start.setup != self.setup {
            return Err(ReplayError::WrongStart);
        }
        Ok(())
    }
}

/// Checks that a replay recorded as `recorded` was saved by this engine and replay format.
fn check_version(recorded: &str) -> Result<(), ReplayError> {
    let current = engine_version();
    if recorded != current {
        return Err(ReplayError::Version {
            recorded: recorded.into(),
            current,
        });
    }
    Ok(())
}

impl ReplayRecord {
    /// Submits this command and its answers to `engine`, which has committed `index` commands,
    /// and checks the state hash. Returns the events it committed.
    pub fn apply(&self, engine: &mut Engine<'_>, index: usize) -> Result<Vec<Event>, ReplayError> {
        let mut outcome = engine.submit(self.player, self.command.clone());
        for answer in &self.answers {
            outcome = engine.answer(answer.clone());
        }
        let Outcome::Committed(events) = outcome else {
            return Err(ReplayError::Diverged { index });
        };
        let got = engine.state().hash();
        let expected = self.state_hash;
        if got != expected {
            return Err(ReplayError::HashMismatch {
                index,
                expected,
                got,
            });
        }
        Ok(events)
    }
}
