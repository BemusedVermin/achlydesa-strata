//! What the previews show for the command being built (design §13.6).
//!
//! The simulation thread sends [`Previews`](strata_session::Previews) with every view. While the
//! viewer picks a target, [`PreviewModel::of`] picks out the ones for the selected figure and
//! action: a move's stops, with their costs and flags, and the route to the hovered one; an
//! attack's pools, line of sight and exact wound odds for the hovered target. The board draws
//! [`PreviewMarks`]; the UI draws the labels and the odds.

use bevy::prelude::*;
use strata_kernel::{Frac, Space};
use strata_model::{Command, FigureId};
use strata_render::{PreviewMarks, StopMark, VolumeMark};
use strata_scene::{DicePool, WoundOdds};
use strata_session::{AttackPreview, MoveStop, PlayerView, PoolPreview};

use super::CommandInput;
use super::builder::{Step, Target};
use super::pick::Pick;

/// One side of an attack as the preview shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolLine {
    /// How many dice.
    pub dice: i64,
    /// The die's name.
    pub die: String,
    /// The face that counts.
    pub face: String,
}

impl PoolLine {
    fn of(pool: &PoolPreview) -> Self {
        Self {
            dice: pool.dice,
            die: pool.die.clone(),
            face: pool.face.as_str().to_owned(),
        }
    }
}

/// The attack on the hovered target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttackModel {
    /// The attacker.
    pub attacker: FigureId,
    /// The target.
    pub target: FigureId,
    /// The attacker's pool.
    pub attack: PoolLine,
    /// The target's pool.
    pub defense: PoolLine,
    /// Whether line of sight was needed.
    pub ranged: bool,
    /// Whether the target has cover.
    pub cover: bool,
    /// The share of the target's samples in clear sight, for a ranged attack.
    pub clear: Option<Frac>,
    /// The exact wound distribution; `None` when a die has no faces to count or the pools are
    /// too large for exact odds.
    pub odds: Option<WoundOdds>,
}

/// A pool for the convolution, when its die has faces to count.
fn dice(pool: &PoolPreview) -> Option<DicePool> {
    let (success, faces) = pool.odds?;
    let dice = u32::try_from(pool.dice).ok()?;
    Some(DicePool {
        dice,
        success,
        faces,
    })
}

impl AttackModel {
    /// The model of `preview`, with its wound odds convolved from both pools.
    pub fn of(preview: &AttackPreview) -> Self {
        let odds = dice(&preview.attack)
            .zip(dice(&preview.defense))
            .and_then(|(a, d)| WoundOdds::of(a, d));
        Self {
            attacker: preview.attacker,
            target: preview.target,
            attack: PoolLine::of(&preview.attack),
            defense: PoolLine::of(&preview.defense),
            ranged: preview.ranged,
            cover: preview.cover,
            clear: preview.sight.as_ref().map(|s| s.clear_fraction),
            odds,
        }
    }
}

/// Everything the previews show now.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct PreviewModel {
    /// What the board draws.
    pub marks: PreviewMarks,
    /// The cost to label on each stop of the move being chosen.
    pub costs: Vec<(Space, i64)>,
    /// The hovered stop of the move being chosen.
    pub stop: Option<MoveStop>,
    /// The attack on the hovered target.
    pub attack: Option<AttackModel>,
}

impl PreviewModel {
    /// The previews for the target step of `input`, with the cursor over `hovered`; nothing at
    /// any other step or once the command has gone out.
    pub fn of(view: &PlayerView, input: &CommandInput, hovered: Option<Space>) -> Self {
        let Step::Target(figure, action) = input.builder.step() else {
            return Self::default();
        };
        if input.sent || input.tokens.is_some() {
            return Self::default();
        }
        let state = &view.state;
        let targets = input.builder.options().into_iter().filter_map(|c| match c {
            Command::Action { args, .. } => Some(Target::of(args, state)),
            _ => None,
        });
        let mut model = Self::default();
        let hovered_figure = hovered.and_then(|s| Pick::at(state, s).figure);
        for target in targets.collect::<Vec<_>>() {
            match target {
                Target::Space(space) => {
                    let Some(stop) = view.previews.stop(figure, action, space) else {
                        continue;
                    };
                    model.marks.stops.push(StopMark {
                        space,
                        stops_here: stop.stops_here,
                        falls: stop.falls,
                        provokes_reprisal: stop.provokes_reprisal,
                    });
                    model.costs.push((space, stop.cost));
                    if hovered == Some(space) {
                        let from = state.standing(figure);
                        model.marks.route = from.map(|f| (f, stop.path.clone()));
                        model.stop = Some(stop.clone());
                    }
                }
                Target::Figure(target) if hovered_figure == Some(target) => {
                    let preview = view.previews.attack(figure, target);
                    let Some(preview) = preview.filter(|p| p.action == action) else {
                        continue;
                    };
                    model.marks.volume = preview.volume.as_ref().map(|v| VolumeMark {
                        cells: v.cells.clone(),
                        bottom: v.bottom,
                        top: v.top,
                    });
                    let witnesses = preview.sight.as_ref().map(|s| s.witnesses.clone());
                    model.marks.witnesses = witnesses.unwrap_or_default();
                    model.attack = Some(AttackModel::of(preview));
                }
                Target::Figure(_) | Target::Other(_) => {}
            }
        }
        model
    }
}

#[cfg(test)]
mod tests;
