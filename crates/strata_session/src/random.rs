//! A controller that picks uniformly among what is legal (design §15.1).

use strata_kernel::Pcg32;
use strata_model::{Answer, Command, Decision, DecisionKind};

use crate::controller::Controller;
use crate::view::PlayerView;

/// Plays at random with its own seeded PCG32 stream, so the same seed gives the same game.
#[derive(Clone, Debug)]
pub struct RandomController {
    rng: Pcg32,
}

impl RandomController {
    /// Seeds the controller's private stream.
    pub fn new(seed: u64, stream: u64) -> Self {
        Self {
            rng: Pcg32::new(seed, stream),
        }
    }

    /// A uniform index below `len`, or zero for an empty list.
    fn index(&mut self, len: usize) -> usize {
        match u32::try_from(len) {
            Ok(0) => 0,
            Ok(n) => self.rng.bounded(n) as usize,
            Err(_) => self.rng.next_u32() as usize,
        }
    }

    fn number(&mut self, min: i32, max: i32) -> i32 {
        let span = i64::from(max) - i64::from(min) + 1;
        if span <= 0 {
            return min;
        }
        let offset = match u32::try_from(span) {
            Ok(n) => self.rng.bounded(n),
            Err(_) => self.rng.next_u32(),
        };
        (i64::from(min) + i64::from(offset)) as i32
    }
}

impl Controller for RandomController {
    /// Picks one of `legal`; with nothing legal it offers `EndActivation`, which the rules will
    /// refuse if it does not apply.
    fn choose_command(&mut self, _: &PlayerView, legal: &[Command]) -> Command {
        match legal.get(self.index(legal.len())) {
            Some(command) => command.clone(),
            None => Command::EndActivation,
        }
    }

    fn answer(&mut self, _: &PlayerView, decision: &Decision) -> Answer {
        match &decision.kind {
            DecisionKind::ChooseFigure(figures) => match figures.get(self.index(figures.len())) {
                Some(figure) => Answer::Figure(*figure),
                None => Answer::Option(0),
            },
            DecisionKind::ChooseSpace(spaces) => match spaces.get(self.index(spaces.len())) {
                Some(space) => Answer::Space(*space),
                None => Answer::Option(0),
            },
            DecisionKind::ChooseOption(options) => Answer::Option(self.index(options.len()) as u32),
            DecisionKind::YesNo => Answer::YesNo(self.index(2) == 1),
            DecisionKind::Number { min, max } => Answer::Number(self.number(*min, *max)),
            DecisionKind::Order(items) => {
                let mut order: Vec<u32> = (0..items.len() as u32).collect();
                for i in (1..order.len()).rev() {
                    let j = self.index(i + 1);
                    order.swap(i, j);
                }
                Answer::Order(order)
            }
        }
    }
}
