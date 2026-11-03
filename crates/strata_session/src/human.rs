//! The seat a person plays through the simulation thread's channels (design §13.1).

use crossbeam_channel::{Receiver, select};
use strata_model::{Answer, Command, Decision};

use crate::controller::Controller;
use crate::sim::Control;
use crate::view::PlayerView;

/// Blocks the simulation thread until the client sends a command or an answer. It never touches
/// the client's thread, so the client itself never blocks.
pub struct HumanController {
    commands: Receiver<Command>,
    answers: Receiver<Answer>,
    control: Receiver<Control>,
    stopped: bool,
}

impl HumanController {
    pub(crate) fn new(
        commands: Receiver<Command>,
        answers: Receiver<Answer>,
        control: Receiver<Control>,
    ) -> Self {
        Self {
            commands,
            answers,
            control,
            stopped: false,
        }
    }
}

impl Controller for HumanController {
    fn choose_command(&mut self, _: &PlayerView, _: &[Command]) -> Command {
        select! {
            recv(self.commands) -> command => {
                if let Ok(command) = command {
                    return command;
                }
            }
            recv(self.control) -> _ => {}
        }
        self.stopped = true;
        Command::EndActivation
    }

    fn answer(&mut self, _: &PlayerView, _: &Decision) -> Answer {
        select! {
            recv(self.answers) -> answer => {
                if let Ok(answer) = answer {
                    return answer;
                }
            }
            recv(self.control) -> _ => {}
        }
        self.stopped = true;
        Answer::Option(0)
    }

    fn abandoned(&self) -> bool {
        self.stopped
    }
}
