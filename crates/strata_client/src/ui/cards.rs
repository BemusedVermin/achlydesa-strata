//! Unit cards: queried stats, card text, figures alive and wounds.

use std::cmp::Ordering;

use bevy_egui::egui;
use strata_session::{AbilityLine, PlayerView, StatLine, UnitCard};

/// How a stat's current value compares with its printed one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Above the printed value.
    Buffed,
    /// Below the printed value.
    Debuffed,
    /// Equal to the printed value.
    Plain,
}

/// One stat as the card shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatModel {
    /// Short label.
    pub label: String,
    /// The queried value, with every modifier applied.
    pub value: i64,
    /// Whether the modifiers raised or lowered it.
    pub tone: Tone,
}

impl StatModel {
    fn of(line: &StatLine) -> Self {
        let tone = match line.value.cmp(&line.base) {
            Ordering::Greater => Tone::Buffed,
            Ordering::Less => Tone::Debuffed,
            Ordering::Equal => Tone::Plain,
        };
        Self {
            label: line.label.clone(),
            value: line.value,
            tone,
        }
    }
}

/// Everything one unit card displays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardModel {
    /// Display name.
    pub name: String,
    /// The unit's kind.
    pub kind: String,
    /// Whether the viewer owns the unit.
    pub mine: bool,
    /// Stats in display order.
    pub stats: Vec<StatModel>,
    /// Abilities with their card text.
    pub abilities: Vec<AbilityLine>,
    /// Figures still in play.
    pub alive: usize,
    /// Figures the unit started with.
    pub total: usize,
    /// Wounds on all of its figures.
    pub wounds: u32,
}

impl CardModel {
    fn of(view: &PlayerView, card: &UnitCard) -> Self {
        let unit = view.state.units.get(card.unit.index());
        Self {
            name: card.name.clone(),
            kind: card.kind.clone(),
            mine: unit.is_some_and(|u| u.owner == view.viewer),
            stats: card.stats.iter().map(StatModel::of).collect(),
            abilities: card.abilities.clone(),
            alive: card.figures.iter().filter(|f| f.alive).count(),
            total: card.figures.len(),
            wounds: card.figures.iter().map(|f| f.wounds).sum(),
        }
    }
}

/// A card model for every unit in `view`.
pub fn card_models(view: &PlayerView) -> Vec<CardModel> {
    view.cards.iter().map(|c| CardModel::of(view, c)).collect()
}

fn color(tone: Tone) -> egui::Color32 {
    match tone {
        Tone::Buffed => egui::Color32::LIGHT_GREEN,
        Tone::Debuffed => egui::Color32::LIGHT_RED,
        Tone::Plain => egui::Color32::GRAY,
    }
}

/// Draws the cards, the viewer's own first.
pub fn show(ui: &mut egui::Ui, cards: &[CardModel]) {
    let mut sorted: Vec<&CardModel> = cards.iter().collect();
    sorted.sort_by_key(|c| !c.mine);
    egui::ScrollArea::vertical().show(ui, |ui| {
        for card in sorted {
            ui.group(|ui| {
                ui.heading(format!("{} ({})", card.name, card.kind));
                let (alive, total) = (card.alive, card.total);
                ui.label(format!("Figures {alive}/{total} · Wounds {}", card.wounds));
                ui.horizontal(|ui| {
                    for stat in &card.stats {
                        let text = format!("{} {}", stat.label, stat.value);
                        ui.colored_label(color(stat.tone), text);
                    }
                });
                for ability in &card.abilities {
                    ui.label(egui::RichText::new(&ability.name).strong());
                    ui.label(&ability.text);
                }
            });
        }
    });
}
