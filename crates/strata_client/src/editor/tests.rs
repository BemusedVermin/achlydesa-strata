use std::path::PathBuf;

use bevy::prelude::*;
use strata_editor::{Brush, LevelMode, OpenedMap, PaletteEntry, PaletteItem, Problem};
use strata_kernel::Hex;
use strata_model::{MapDef, Text};

use bevy::camera::{CameraProjection, RenderTargetInfo};
use bevy::window::{PrimaryWindow, WindowResolution};
use strata_render::{ClientBoard, FeatureNames, IsoCameraMarker};
use strata_scene::surface_point;

use super::panel::{entry_text, problem_text, refusal, shown_level, warning};
use super::*;

fn opened(name: &str) -> OpenedMap {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/maps");
    OpenedMap::open(&root.join(format!("t/maps/{name}.lua"))).unwrap()
}

/// The editor's input handling without a window: actions in, document and feedback out.
fn app(name: &str) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(Editing::new(opened(name)))
        .init_resource::<EditorTool>()
        .init_resource::<EditorCursor>()
        .init_resource::<EditorFeedback>()
        .add_message::<EditorAction>()
        .add_systems(Update, (act, preview).chain());
    app.update();
    app
}

fn send(app: &mut App, actions: impl IntoIterator<Item = EditorAction>) {
    for action in actions {
        app.world_mut().write_message(action);
    }
    app.update();
}

fn map(app: &App) -> &MapDef {
    app.world().resource::<Editing>().document.map()
}

fn grass() -> PaletteItem {
    PaletteItem::Tile("grass_1".into())
}

#[test]
fn a_line_stroke_goes_through_the_document_and_undoes_in_one_step() {
    let mut app = app("fair");
    let before = map(&app).clone();
    send(
        &mut app,
        [
            EditorAction::Select(Some(grass())),
            EditorAction::Brush(Brush::Line),
            EditorAction::Press(Hex::new(0, 5)),
            EditorAction::Release(Hex::new(3, 5)),
        ],
    );
    assert_eq!(map(&app).pieces.len(), before.pieces.len() + 4);
    assert!(app.world().resource::<Editing>().document.can_undo());
    send(&mut app, [EditorAction::Undo]);
    assert_eq!(map(&app), &before);
    send(&mut app, [EditorAction::Redo]);
    assert_eq!(map(&app).pieces.len(), before.pieces.len() + 4);
    // The stroke is over, so the next release without a press places one hex.
    assert_eq!(app.world().resource::<EditorTool>().0.drag_start(), None);
}

#[test]
fn a_refused_placement_leaves_the_map_alone_and_says_why() {
    let mut app = app("fair");
    let before = map(&app).clone();
    send(
        &mut app,
        [
            EditorAction::Select(Some(grass())),
            EditorAction::Level(LevelMode::Locked(0)),
            EditorAction::Press(Hex::ORIGIN),
            EditorAction::Release(Hex::ORIGIN),
        ],
    );
    assert_eq!(map(&app), &before);
    assert!(!app.world().resource::<Editing>().document.can_undo());
    let message = app.world().resource::<EditorFeedback>().message.clone();
    assert_eq!(
        message.as_deref(),
        Some("Nothing placed. Overlaps terrain at (0, 0).")
    );
    send(&mut app, [EditorAction::Undo]);
    let message = app.world().resource::<EditorFeedback>().message.clone();
    assert_eq!(message.as_deref(), Some("Nothing to undo."));
}

#[test]
fn tool_settings_change_the_tool_but_not_the_map() {
    let mut app = app("fair");
    app.world_mut().resource_mut::<Editing>().set_changed();
    app.update();
    let tick = app.world().resource_ref::<Editing>().last_changed();
    send(
        &mut app,
        [
            EditorAction::Select(Some(PaletteItem::Tile("grass_2".into()))),
            EditorAction::Rotate(-1),
            EditorAction::Brush(Brush::Fill),
            EditorAction::Level(LevelMode::Locked(3)),
        ],
    );
    let tool = &app.world().resource::<EditorTool>().0;
    assert_eq!(tool.item, Some(PaletteItem::Tile("grass_2".into())));
    assert_eq!(tool.rot.steps(), 5);
    assert_eq!(tool.brush, Brush::Fill);
    assert_eq!(tool.level, LevelMode::Locked(3));
    assert_eq!(app.world().resource_ref::<Editing>().last_changed(), tick);
}

#[test]
fn the_ghost_follows_the_cursor() {
    let mut app = app("fair");
    send(&mut app, [EditorAction::Select(Some(grass()))]);
    assert_eq!(
        app.world().resource::<EditorFeedback>().preview.marks,
        vec![]
    );
    app.world_mut().resource_mut::<EditorCursor>().0 = Some(Hex::new(2, 1));
    app.update();
    let preview = &app.world().resource::<EditorFeedback>().preview;
    // The hill at (2, 1) has its top at 2, so a one-level tile's top face is at 3.
    let tops: Vec<u32> = preview.marks.iter().map(|m| m.space.z).collect();
    assert_eq!(tops, vec![3]);
}

#[test]
fn panel_text() {
    let entry = PaletteEntry {
        item: grass(),
        full_id: "t:grass_1".into(),
        label: Text("Grass".into()),
        cells: 1,
        thickness: 1,
    };
    assert_eq!(entry_text(&entry, None), "Grass (1 hex × 1 lv)");
    assert_eq!(
        entry_text(&entry, Some(-2)),
        "Grass (1 hex × 1 lv) · -2 left"
    );
    let tree = PaletteEntry {
        item: PaletteItem::Feature("tree".into()),
        label: Text::default(),
        full_id: "t:tree".into(),
        cells: 1,
        thickness: 0,
    };
    assert_eq!(entry_text(&tree, Some(1)), "t:tree (1 hex) · 1 left");
    let cells = vec![Hex::new(1, -2), Hex::new(0, 0)];
    assert_eq!(
        problem_text(&Problem::Overlap(cells)),
        "Overlaps terrain at (1, -2) (0, 0)."
    );
    assert_eq!(refusal(&strata_editor::Preview::default()), None);
}

/// The tick at which [`Editing`] last changed.
fn editing_changed(app: &App) -> bevy::ecs::change_detection::Tick {
    app.world().resource_ref::<Editing>().last_changed()
}

#[test]
fn undo_and_redo_with_nothing_to_do_leave_the_map_unchanged() {
    let mut app = app("fair");
    app.update();
    let tick = editing_changed(&app);
    send(&mut app, [EditorAction::Undo, EditorAction::Redo]);
    assert_eq!(editing_changed(&app), tick);
    let message = app.world().resource::<EditorFeedback>().message.clone();
    assert_eq!(message.as_deref(), Some("Nothing to redo."));
    // A refused placement does not mark the map changed either.
    send(
        &mut app,
        [
            EditorAction::Select(Some(grass())),
            EditorAction::Level(LevelMode::Locked(0)),
            EditorAction::Press(Hex::ORIGIN),
            EditorAction::Release(Hex::ORIGIN),
        ],
    );
    assert_eq!(editing_changed(&app), tick);
    // A placement and its undo do.
    send(
        &mut app,
        [
            EditorAction::Level(LevelMode::Snap),
            EditorAction::Press(Hex::new(9, 9)),
            EditorAction::Release(Hex::new(9, 9)),
        ],
    );
    let placed = editing_changed(&app);
    assert_ne!(placed, tick);
    send(&mut app, [EditorAction::Undo]);
    assert_ne!(editing_changed(&app), placed);
}

#[test]
fn placing_past_the_inventory_goes_ahead_and_says_validation_will_fail() {
    let mut app = app("inventory");
    let before = map(&app).pieces.len();
    send(
        &mut app,
        [
            EditorAction::Select(Some(grass())),
            EditorAction::Press(Hex::new(0, 5)),
            EditorAction::Release(Hex::new(0, 5)),
        ],
    );
    assert_eq!(map(&app).pieces.len(), before + 1);
    let message = app.world().resource::<EditorFeedback>().message.clone();
    assert_eq!(
        message.as_deref(),
        Some(
            "Placed. Over the inventory: -2 left after this. The map will fail validation \
             (`strata map check`) until the inventory is fixed."
        )
    );
}

#[test]
fn feature_names_change_only_with_the_board_shown() {
    let mut app = app("fair");
    app.init_resource::<FeatureNames>()
        .add_systems(Update, show_board.after(act));
    app.update();
    assert!(app.world().contains_resource::<ClientBoard>());
    assert!(app.world().resource::<FeatureNames>().0.is_empty());
    let tree = || PaletteItem::Feature("tree".into());
    // Overlap the grass at (0, 0) so the map stops compiling, then add a tree: the board on
    // screen is the last one that compiled, and its names must stay with it.
    let overlap = strata_editor::Edit::PlacePiece(strata_model::PlacedPiece {
        piece: "grass_1".into(),
        at: Hex::ORIGIN,
        rot: strata_kernel::Rotation::default(),
        level: 0,
    });
    {
        let mut editing = app.world_mut().resource_mut::<Editing>();
        editing.document.apply(overlap).unwrap();
    }
    send(
        &mut app,
        [
            EditorAction::Select(Some(tree())),
            EditorAction::Press(Hex::new(1, 0)),
            EditorAction::Release(Hex::new(1, 0)),
        ],
    );
    assert_eq!(map(&app).features.len(), 1);
    assert!(app.world().resource::<Editing>().document.board().is_err());
    assert!(app.world().resource::<FeatureNames>().0.is_empty());
    // Removing the overlapping piece makes the map compile again, and the names catch up.
    let mut editing = app.world_mut().resource_mut::<Editing>();
    let index = editing.document.map().pieces.len() - 1;
    let remove = strata_editor::Edit::RemovePiece { index };
    editing.document.apply(remove).unwrap();
    app.update();
    assert!(app.world().resource::<Editing>().document.board().is_ok());
    assert_eq!(
        app.world().resource::<FeatureNames>().0,
        vec!["tree".into()]
    );
}

/// The editor with its real input systems, a window and a camera looking straight down on `at`,
/// so the cursor in the middle of the window is over that hex.
fn input_app(name: &str, at: Hex) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(Editing::new(opened(name)))
        .init_resource::<EditorTool>()
        .init_resource::<EditorCursor>()
        .init_resource::<EditorFeedback>()
        .init_resource::<FeatureNames>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<EditorAction>()
        .add_systems(
            Update,
            (aim, (mouse, keys), act, show_board, preview).chain(),
        );
    let mut window = Window {
        resolution: WindowResolution::new(800, 600),
        ..default()
    };
    window.set_cursor_position(Some(Vec2::new(400.0, 300.0)));
    app.world_mut().spawn((window, PrimaryWindow));
    let mut camera = Camera::default();
    camera.computed.target_info = Some(RenderTargetInfo {
        physical_size: UVec2::new(800, 600),
        scale_factor: 1.0,
    });
    let mut projection = OrthographicProjection::default_3d();
    projection.update(800.0, 600.0);
    camera.computed.clip_from_view = projection.get_clip_from_view();
    let below = surface_point(Orientation::Pointy, at, 0);
    let eye = Transform::from_translation(below + Vec3::Y * 20.0).looking_at(below, Vec3::Z);
    app.world_mut()
        .spawn((camera, GlobalTransform::from(eye), IsoCameraMarker));
    app.update();
    app
}

/// Runs one frame with `change` applied to the input `T`, then clears its just-pressed and
/// just-released state as Bevy's input plugin would.
fn frame<T: Copy + Eq + std::hash::Hash + Send + Sync + 'static>(
    app: &mut App,
    change: impl FnOnce(&mut ButtonInput<T>),
) {
    change(&mut app.world_mut().resource_mut::<ButtonInput<T>>());
    app.update();
    app.world_mut().resource_mut::<ButtonInput<T>>().clear();
}

#[test]
fn a_click_on_the_board_places_and_ctrl_z_undoes_it() {
    let at = Hex::new(6, 3);
    let mut app = input_app("fair", at);
    assert_eq!(app.world().resource::<EditorCursor>().0, Some(at));
    send(&mut app, [EditorAction::Select(Some(grass()))]);
    let before = map(&app).clone();
    // Down in one frame and up in the next.
    frame(&mut app, |b: &mut ButtonInput<MouseButton>| {
        b.press(MouseButton::Left)
    });
    assert_eq!(
        app.world().resource::<EditorTool>().0.drag_start(),
        Some(at)
    );
    frame(&mut app, |b: &mut ButtonInput<MouseButton>| {
        b.release(MouseButton::Left)
    });
    assert_eq!(map(&app).pieces.len(), before.pieces.len() + 1);
    assert_eq!(map(&app).pieces.last().map(|p| p.at), Some(at));
    // Ctrl+Z.
    frame(&mut app, |k: &mut ButtonInput<KeyCode>| {
        k.press(KeyCode::ControlLeft);
        k.press(KeyCode::KeyZ);
    });
    assert_eq!(map(&app), &before);
    // Ctrl+Y redoes.
    frame(&mut app, |k: &mut ButtonInput<KeyCode>| {
        k.release(KeyCode::KeyZ);
        k.press(KeyCode::KeyY);
    });
    assert_eq!(map(&app).pieces.len(), before.pieces.len() + 1);
}

#[test]
fn a_click_pressed_and_released_in_one_frame_still_places() {
    let at = Hex::new(6, 3);
    let mut app = input_app("fair", at);
    send(&mut app, [EditorAction::Select(Some(grass()))]);
    let before = map(&app).pieces.len();
    frame(&mut app, |b: &mut ButtonInput<MouseButton>| {
        b.press(MouseButton::Left);
        b.release(MouseButton::Left);
    });
    assert_eq!(map(&app).pieces.len(), before + 1);
    assert_eq!(app.world().resource::<EditorTool>().0.drag_start(), None);
}

#[test]
fn a_release_without_a_press_places_nothing() {
    let mut app = input_app("fair", Hex::new(6, 3));
    send(&mut app, [EditorAction::Select(Some(grass()))]);
    let before = map(&app).clone();
    frame(&mut app, |b: &mut ButtonInput<MouseButton>| {
        b.press(MouseButton::Left);
        b.clear();
        b.release(MouseButton::Left);
    });
    assert_eq!(map(&app), &before);
}

#[test]
fn the_lock_level_control_starts_where_the_ghost_snapped() {
    assert_eq!(shown_level(LevelMode::Snap, Some(2)), 2);
    assert_eq!(shown_level(LevelMode::Snap, None), 0);
    assert_eq!(shown_level(LevelMode::Locked(5), Some(2)), 5);
}

#[test]
fn warnings_and_new_problems_have_text() {
    assert_eq!(warning(&strata_editor::Preview::default()), None);
    assert_eq!(
        problem_text(&Problem::NotEnclosed),
        "Not an enclosed area: fill on open ground fills holes inside the terrain only."
    );
    assert_eq!(
        problem_text(&Problem::TooLarge),
        format!(
            "The region is larger than {} hexes.",
            strata_editor::FILL_LIMIT
        )
    );
}
