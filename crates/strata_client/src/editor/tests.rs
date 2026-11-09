use std::path::PathBuf;

use bevy::prelude::*;
use strata_editor::{Brush, LevelMode, OpenedMap, PaletteEntry, PaletteItem, Problem};
use strata_kernel::Hex;
use strata_model::{MapDef, Text};

use super::panel::{entry_text, problem_text, refusal};
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
