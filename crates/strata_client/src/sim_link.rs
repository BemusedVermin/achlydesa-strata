//! `SimLinkPlugin`: the client's end of the simulation thread's channels (design §13.1, §13.3).

use std::collections::VecDeque;
use std::sync::Mutex;

use bevy::prelude::*;
use strata_model::{Answer, Command, Event};
use strata_session::{PlayerView, SimHandle, SimMessage};

/// The running simulation. Dropping it stops the thread.
#[derive(Resource)]
pub struct Sim(pub SimHandle);

/// Event batches received from the simulation and not yet played, oldest first.
#[derive(Resource, Default)]
pub struct PendingEvents(pub VecDeque<Vec<Event>>);

/// The most events the log keeps.
pub const LOG_CAP: usize = 1000;

/// Every visible event received so far, oldest first, for the game log. Playback consumes
/// [`PendingEvents`]; this is a separate copy.
#[derive(Resource, Default)]
pub struct EventLog(pub Vec<Event>);

/// The newest view of the match from the viewer's seat.
#[derive(Resource, Default)]
pub struct LatestView(pub Option<Box<PlayerView>>);

/// How the match is going.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub enum SimStatus {
    /// The match is still being played.
    #[default]
    Running,
    /// The match reached its command limit.
    Finished,
    /// The match stopped with this message.
    Failed(String),
}

/// Asks the simulation to play a command for a human seat.
#[derive(Message)]
pub struct SendCommand(pub Command);

/// Answers the pending decision, whichever seat chooses.
#[derive(Message)]
pub struct SendAnswer(pub Answer);

/// Hands the simulation's messages to the ECS each frame and the client's commands back.
pub struct SimLinkPlugin {
    handle: Mutex<Option<SimHandle>>,
}

impl SimLinkPlugin {
    /// A plugin that talks to `handle`.
    pub fn new(handle: SimHandle) -> Self {
        Self {
            handle: Mutex::new(Some(handle)),
        }
    }
}

impl Plugin for SimLinkPlugin {
    fn build(&self, app: &mut App) {
        let handle = self.handle.lock().ok().and_then(|mut h| h.take());
        if let Some(handle) = handle {
            app.insert_resource(Sim(handle));
        }
        app.init_resource::<PendingEvents>()
            .init_resource::<EventLog>()
            .init_resource::<LatestView>()
            .init_resource::<SimStatus>()
            .add_message::<SendCommand>()
            .add_message::<SendAnswer>()
            .add_systems(PreUpdate, drain_sim)
            .add_systems(Update, (forward_commands, forward_answers));
    }
}

/// Moves everything the simulation has sent into resources without blocking.
fn drain_sim(
    sim: Option<Res<Sim>>,
    mut pending: ResMut<PendingEvents>,
    mut log: ResMut<EventLog>,
    mut view: ResMut<LatestView>,
    mut status: ResMut<SimStatus>,
) {
    let Some(sim) = sim else { return };
    while let Some(message) = sim.0.try_recv() {
        match message {
            SimMessage::Events(events) => {
                info!("received {} event(s): {events:?}", events.len());
                log.0.extend(events.iter().cloned());
                let excess = log.0.len().saturating_sub(LOG_CAP);
                log.0.drain(..excess);
                pending.0.push_back(events);
            }
            SimMessage::View(latest) => view.0 = Some(latest),
            SimMessage::Finished => {
                info!("the match finished");
                *status = SimStatus::Finished;
            }
            SimMessage::Error(error) => {
                error!("the match stopped: {error}");
                *status = SimStatus::Failed(error.to_string());
            }
        }
    }
}

/// Sends each [`SendCommand`] to the simulation.
fn forward_commands(sim: Option<Res<Sim>>, mut commands: MessageReader<SendCommand>) {
    for SendCommand(command) in commands.read() {
        if let Some(sim) = &sim {
            sim.0.send_command(command.clone());
        }
    }
}

/// Sends each [`SendAnswer`] to the simulation.
fn forward_answers(sim: Option<Res<Sim>>, mut answers: MessageReader<SendAnswer>) {
    for SendAnswer(answer) in answers.read() {
        if let Some(sim) = &sim {
            sim.0.send_answer(answer.clone());
        }
    }
}

#[cfg(test)]
mod tests;
