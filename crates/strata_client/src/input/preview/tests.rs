use strata_kernel::{DefId, Frac, Hex, Point, Point3, Rotation, Seg3, Sym, Value};
use strata_model::PlayerId;
use strata_rules::{FigureVolume, LosResult};
use strata_session::{AttackPreview, MoveStop, PlayerView, PoolPreview};

use super::*;
use crate::input::Press;
use crate::input::tests::{space, state};

const WALK: DefId = DefId(5);
const ATTACK: DefId = DefId(6);

fn walk(q: i32) -> Command {
    let at = [q, 0, 1].map(|n| Value::Int(i64::from(n)));
    Command::Action {
        figure: FigureId(0),
        action: WALK,
        args: Value::List(at.to_vec()),
    }
}

fn attack(target: u32) -> Command {
    Command::Action {
        figure: FigureId(0),
        action: ATTACK,
        args: Value::Int(i64::from(target)),
    }
}

fn stop(q: i32, cost: i64, stops_here: bool, provokes_reprisal: bool) -> MoveStop {
    MoveStop {
        space: space(q),
        rotation: Rotation::default(),
        cost,
        path: (1..=q).map(space).collect(),
        stops_here,
        falls: false,
        provokes_reprisal,
    }
}

fn pool(dice: i64, face: &str, counting: u32) -> PoolPreview {
    PoolPreview {
        dice,
        die: "combat".into(),
        face: Sym::new(face),
        odds: Some((counting, 6)),
    }
}

fn ray() -> Seg3 {
    Seg3::new(
        Point3::new(Point::new(0, 0), 22),
        Point3::new(Point::new(45, 0), 18),
    )
}

/// Figure 0 may walk to spaces 2 and 4, or attack figure 2 on space 3 from range with 3 dice
/// against 2.
fn view() -> PlayerView {
    let mut view = PlayerView::of(&state(), PlayerId(0));
    view.legal = vec![walk(2), walk(4), attack(2)];
    let stops = vec![stop(2, 2, false, true), stop(4, 3, true, false)];
    view.previews.moves.insert(FigureId(0), stops);
    view.previews.attacks.push(AttackPreview {
        attacker: FigureId(0),
        target: FigureId(2),
        action: ATTACK,
        attack: pool(3, "hit", 3),
        defense: pool(2, "block", 2),
        ranged: true,
        cover: false,
        sight: Some(LosResult {
            visible: true,
            clear_fraction: Frac::new(2, 3),
            witnesses: vec![ray()],
        }),
        volume: Some(FigureVolume {
            anchor: Hex::new(3, 0),
            cells: vec![Hex::new(3, 0)],
            target: 22,
            bottom: 12,
            top: 24,
        }),
    });
    view
}

/// Input with figure 0 selected and `action` chosen.
fn choosing(view: &PlayerView, action: DefId) -> CommandInput {
    let mut input = CommandInput::of(view);
    assert_eq!(
        input.click(&view.state, &Pick::at(&view.state, space(0))),
        None
    );
    assert_eq!(input.press(Press::Action(action)), None);
    assert_eq!(input.builder.step(), Step::Target(FigureId(0), action));
    input
}

#[test]
fn a_move_labels_every_stop_and_flags_the_ones_that_need_it() {
    let view = view();
    let model = PreviewModel::of(&view, &choosing(&view, WALK), None);
    assert_eq!(model.costs, [(space(2), 2), (space(4), 3)]);
    let flags: Vec<(bool, bool)> = model
        .marks
        .stops
        .iter()
        .map(|s| (s.provokes_reprisal, s.stops_here))
        .collect();
    assert_eq!(flags, [(true, false), (false, true)]);
    assert_eq!(
        (model.marks.route.clone(), model.stop.clone()),
        (None, None)
    );
    assert_eq!(model.attack, None);
}

#[test]
fn hovering_a_stop_shows_its_route() {
    let view = view();
    let model = PreviewModel::of(&view, &choosing(&view, WALK), Some(space(4)));
    let path = vec![space(1), space(2), space(3), space(4)];
    assert_eq!(model.marks.route, Some((space(0), path)));
    assert_eq!(model.stop.map(|s| s.cost), Some(3));
}

#[test]
fn hovering_an_attack_target_shows_its_volume_rays_and_odds() {
    let view = view();
    let input = choosing(&view, ATTACK);
    assert_eq!(
        PreviewModel::of(&view, &input, None),
        PreviewModel::default()
    );
    let model = PreviewModel::of(&view, &input, Some(space(3)));
    let volume = model.marks.volume.as_ref().unwrap();
    assert_eq!(
        (volume.cells.as_slice(), volume.bottom, volume.top),
        (&[Hex::new(3, 0)][..], 12, 24)
    );
    assert_eq!(model.marks.witnesses, [ray()]);
    let attack = model.attack.unwrap();
    assert_eq!((attack.attack.dice, attack.defense.dice), (3, 2));
    assert_eq!(attack.clear, Some(Frac::new(2, 3)));
    // Three hits need all three dice (1/8) and no block on either defense die (4/9).
    let odds = attack.odds.unwrap();
    assert_eq!(odds.chance(3), Frac::new(1, 18));
    let (weights, total) = odds.weights();
    assert_eq!(weights.iter().sum::<i128>(), total);
}

#[test]
fn nothing_is_previewed_before_the_target_step() {
    let view = view();
    let input = CommandInput::of(&view);
    assert_eq!(
        PreviewModel::of(&view, &input, Some(space(2))),
        PreviewModel::default()
    );
}

#[test]
fn an_attack_preview_for_another_action_is_not_shown() {
    let mut view = view();
    view.previews.attacks[0].action = DefId(9);
    let model = PreviewModel::of(&view, &choosing(&view, ATTACK), Some(space(3)));
    assert_eq!(model, PreviewModel::default());
}

#[test]
fn a_die_without_faces_to_count_has_no_odds() {
    let mut view = view();
    view.previews.attacks[0].defense.odds = None;
    let model = PreviewModel::of(&view, &choosing(&view, ATTACK), Some(space(3)));
    assert_eq!(model.attack.map(|a| a.odds), Some(None));
}

#[test]
fn the_board_marks_follow_the_hovered_space() {
    let mut app = crate::input::tests::input_app();
    app.world_mut()
        .resource_mut::<crate::sim_link::LatestView>()
        .0 = Some(Box::new(view()));
    app.update();
    let mut input = app.world().resource::<CommandInput>().clone();
    input.click(&view().state, &Pick::at(&view().state, space(0)));
    input.press(Press::Action(WALK));
    *app.world_mut().resource_mut::<CommandInput>() = input;
    app.world_mut()
        .resource_mut::<strata_render::HoveredSpace>()
        .0 = Some(space(2));
    app.update();
    let marks = app.world().resource::<PreviewMarks>();
    assert_eq!(marks.stops.len(), 2);
    assert_eq!(marks.route, Some((space(0), vec![space(1), space(2)])));
    let model = app.world().resource::<PreviewModel>();
    assert_eq!(model.stop.as_ref().map(|s| s.cost), Some(2));
}
