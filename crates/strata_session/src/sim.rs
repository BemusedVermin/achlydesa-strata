//! The simulation thread and the channels the client talks to it through (design §13.1).

use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TryRecvError, unbounded};
use strata_model::{Answer, Command, Event, PlayerId};

use crate::cards::{CardSource, NoCards};
use crate::human::HumanController;
use crate::session::{Session, SessionError};
use crate::view::{PlayerView, visible_events};

/// A control message for the simulation thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Stop after the current step and let the thread exit.
    Shutdown,
}

/// What the simulation thread sends the client.
#[derive(Clone, Debug)]
pub enum SimMessage {
    /// Events that committed, already filtered for the client's viewer.
    Events(Vec<Event>),
    /// The viewer's projection of the state after a step, including any pending decision.
    View(Box<PlayerView>),
    /// The match reached its command limit.
    Finished,
    /// The match stopped because of an error.
    Error(SessionError),
}

/// The simulation thread did not end cleanly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the simulation thread panicked")]
pub struct SimError;

/// The simulation thread's end of the channels, handed to the closure that runs on it.
pub struct SimLink {
    commands: Receiver<Command>,
    answers: Receiver<Answer>,
    control: Receiver<Control>,
    output: Sender<SimMessage>,
}

impl SimLink {
    /// A controller for a seat played from the client. Several seats share the same inbound
    /// channels, which suits hot-seat play.
    pub fn human(&self) -> HumanController {
        let (commands, answers) = (self.commands.clone(), self.answers.clone());
        HumanController::new(commands, answers, self.control.clone())
    }

    /// Steps `session` until `commands` commands have committed, sending `viewer`'s events and
    /// views out as it goes. Returns early on a shutdown request, an error or a closed client.
    pub fn run(&self, session: &mut Session<'_>, viewer: PlayerId, commands: u32) {
        self.run_with(session, viewer, commands, &NoCards);
    }

    /// As [`run`](Self::run), with every view carrying the cards `cards` writes.
    pub fn run_with(
        &self,
        session: &mut Session<'_>,
        viewer: PlayerId,
        commands: u32,
        cards: &dyn CardSource,
    ) {
        if !self.send_view(session, viewer, cards) {
            return;
        }
        while session.state().command_index < commands {
            if self.shutdown_requested() {
                return;
            }
            match session.step() {
                Ok(events) => {
                    let visible = visible_events(session.state(), viewer, &events);
                    let sent = events.is_empty() || self.send(SimMessage::Events(visible));
                    if !(sent && self.send_view(session, viewer, cards)) {
                        return;
                    }
                }
                Err(SessionError::Stopped) => return,
                Err(error) => {
                    self.send(SimMessage::Error(error));
                    return;
                }
            }
        }
        self.send(SimMessage::Finished);
    }

    /// Reports that the match could not run.
    pub(crate) fn fail(&self, error: SessionError) {
        self.send(SimMessage::Error(error));
    }

    fn send(&self, message: SimMessage) -> bool {
        self.output.send(message).is_ok()
    }

    fn send_view(&self, session: &Session<'_>, viewer: PlayerId, cards: &dyn CardSource) -> bool {
        let mut view = PlayerView::of(session.state(), viewer);
        view.cards = cards.cards(session.state());
        view.names = cards.names();
        self.send(SimMessage::View(Box::new(view)))
    }

    fn shutdown_requested(&self) -> bool {
        !matches!(self.control.try_recv(), Err(TryRecvError::Empty))
    }
}

/// The client's handle on the simulation thread. Dropping it shuts the thread down and joins it.
pub struct SimHandle {
    commands: Sender<Command>,
    answers: Sender<Answer>,
    control: Sender<Control>,
    output: Receiver<SimMessage>,
    thread: Option<JoinHandle<()>>,
}

impl SimHandle {
    /// Starts the simulation thread and runs `body` on it. `body` builds its own runtime Lua
    /// state, so nothing non-`Send` crosses threads, then plays through the [`SimLink`].
    pub fn spawn<F>(body: F) -> Self
    where
        F: FnOnce(SimLink) + Send + 'static,
    {
        let (commands, command_rx) = unbounded();
        let (answers, answer_rx) = unbounded();
        let (control, control_rx) = unbounded();
        let (output_tx, output) = unbounded();
        let link = SimLink {
            commands: command_rx,
            answers: answer_rx,
            control: control_rx,
            output: output_tx,
        };
        let thread = std::thread::Builder::new()
            .name("strata-sim".into())
            .spawn(move || body(link))
            .expect("the operating system refused to start a thread");
        Self {
            commands,
            answers,
            control,
            output,
            thread: Some(thread),
        }
    }

    /// Queues a command for a human seat.
    pub fn send_command(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// Queues the answer to a pending decision.
    pub fn send_answer(&self, answer: Answer) {
        let _ = self.answers.send(answer);
    }

    /// The next message if one is ready; never blocks.
    pub fn try_recv(&self) -> Option<SimMessage> {
        self.output.try_recv().ok()
    }

    /// Waits up to `timeout` for the next message.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<SimMessage, RecvTimeoutError> {
        self.output.recv_timeout(timeout)
    }

    /// Asks the thread to stop and waits for it to exit.
    pub fn shutdown(mut self) -> Result<(), SimError> {
        self.stop()
    }

    fn stop(&mut self) -> Result<(), SimError> {
        let _ = self.control.send(Control::Shutdown);
        match self.thread.take() {
            Some(thread) => thread.join().map_err(|_| SimError),
            None => Ok(()),
        }
    }
}

impl Drop for SimHandle {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
