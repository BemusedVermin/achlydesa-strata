use strata_kernel::{Frac, Rotation, Space};
use strata_model::{FigureId, PlayerId};
use strata_scene::{DicePool, WoundOdds};
use strata_session::{MoveStop, PlayerView};

use super::*;
use crate::input::preview::{AttackModel, PoolLine};

#[test]
fn a_chance_shows_its_exact_fraction_when_short() {
    assert_eq!(chance_text(Frac::new(5, 12)), "41.7% (5/12)");
    assert_eq!(chance_text(Frac::ONE), "100.0%");
    assert_eq!(chance_text(Frac::new(1, 59_049)), "0.0%");
}

#[test]
fn a_stop_lists_its_cost_and_each_flag() {
    let stop = MoveStop {
        space: Space::new(1, 0, 1),
        rotation: Rotation::default(),
        cost: 2,
        path: Vec::new(),
        stops_here: true,
        falls: true,
        provokes_reprisal: true,
    };
    let lines = stop_lines(&stop);
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0], "Move cost 2");
    let plain = MoveStop {
        stops_here: false,
        falls: false,
        provokes_reprisal: false,
        ..stop
    };
    assert_eq!(stop_lines(&plain), ["Move cost 2"]);
}

fn odds(attack: u32, defense: u32) -> WoundOdds {
    let pool = |dice, success| DicePool {
        dice,
        success,
        faces: 6,
    };
    WoundOdds::of(pool(attack, 3), pool(defense, 2)).unwrap()
}

#[test]
fn the_odds_list_every_wound_count_and_at_least_one() {
    assert_eq!(
        odds_lines(&odds(2, 1)),
        [
            "0 wounds: 41.7% (5/12)",
            "1 wound: 41.7% (5/12)",
            "2 wounds: 16.7% (1/6)",
            "At least 1 wound: 58.3% (7/12)",
        ]
    );
    assert_eq!(odds_lines(&odds(0, 1)), ["0 wounds: 100.0%"]);
}

fn line(dice: i64, face: &str) -> PoolLine {
    PoolLine {
        dice,
        die: "combat".into(),
        face: face.into(),
    }
}

#[test]
fn an_attack_reads_as_pools_sight_cover_and_odds() {
    let view = PlayerView::of(&strata_model::GameState::default(), PlayerId(0));
    let mut attack = AttackModel {
        attacker: FigureId(0),
        target: FigureId(1),
        attack: line(2, "hit"),
        defense: line(1, "block"),
        ranged: true,
        cover: true,
        clear: Some(Frac::new(1, 3)),
        odds: Some(odds(2, 1)),
    };
    let lines = attack_lines(&view, &attack);
    assert_eq!(lines[0], "Figure 0 attacks Figure 1");
    assert_eq!(lines[1], "2 combat attack (hit counts)");
    assert_eq!(lines[2], "1 combat defense (block counts)");
    assert_eq!(lines[3], "In sight: 33.3% (1/3) of the target clear");
    assert_eq!(lines[4], "Cover: +1 defense die.");
    assert_eq!(lines.last().unwrap(), "At least 1 wound: 58.3% (7/12)");
    attack.ranged = false;
    attack.clear = None;
    attack.cover = false;
    attack.odds = None;
    let lines = attack_lines(&view, &attack);
    assert_eq!(
        lines[3..],
        [
            "Adjacent: no line of sight needed.",
            "No exact odds for these dice."
        ]
    );
}
