//! Who plays a seat (design §15.1).

use strata_model::{Answer, Command, Decision};

use crate::view::PlayerView;

/// Produces one seat's commands and answers from that player's view. The session never knows
/// whether it is talking to a person, an AI or a script.
pub trait Controller: Send {
    /// Picks the next command from `legal`, the commands the rules accept right now.
    fn choose_command(&mut self, view: &PlayerView, legal: &[Command]) -> Command;
    /// Answers a question the rules asked this player mid-resolution.
    fn answer(&mut self, view: &PlayerView, decision: &Decision) -> Answer;
}
