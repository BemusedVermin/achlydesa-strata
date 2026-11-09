use std::path::{Path, PathBuf};

use glam::UVec2;
use strata_kernel::PackId;
use strata_lua::PackSource;

use super::*;

const ASSETS: &str = r#"
pixels_per_level = 16
hex_width_px = 80

sprite {
  key = "sprite.test_soldier",
  sheet = "sprites/test_soldier.png",
  frame = { w = 48, h = 72 },
  anchor = { x = 24, y = 68 },
  directions = 6,
  animations = {
    idle   = { frames = 4, fps = 6 },
    move   = { frames = 6, fps = 10 },
  },
}
"#;

fn pack(id: &str) -> PackId {
    PackId::new(id)
}

fn one_sprite(fields: &str) -> String {
    format!(
        "sprite {{ key = 'sprite.test_archer', sheet = 'a.png', frame = {{ w = 8, h = 8 }}, \
         anchor = {{ x = 4, y = 8 }}, animations = {{ idle = {{ frames = 1, fps = 1 }} }}, {fields} }}"
    )
}

fn error_of(source: &str) -> String {
    let mut catalog = SpriteCatalog::default();
    let error = catalog
        .add_pack(&pack("t"), Path::new("/t"), source)
        .unwrap_err();
    assert_eq!(
        catalog,
        SpriteCatalog::default(),
        "nothing is added on an error"
    );
    error.to_string()
}

#[test]
fn a_pack_declares_its_scale_and_sprites() {
    let mut catalog = SpriteCatalog::default();
    catalog
        .add_pack(&pack("t"), Path::new("/packs/t"), ASSETS)
        .unwrap();
    assert_eq!(
        catalog.scale(&pack("t")),
        PixelScale {
            pixels_per_level: 16,
            hex_width_px: 80
        }
    );
    let entry = catalog.sprite("sprite.test_soldier").unwrap();
    assert_eq!(entry.pack, pack("t"));
    assert_eq!(
        entry.sheet,
        PathBuf::from("/packs/t/assets/sprites/test_soldier.png")
    );
    assert_eq!(
        (entry.def.frame, entry.def.anchor),
        (UVec2::new(48, 72), UVec2::new(24, 68))
    );
    assert_eq!(entry.def.animations.len(), 2);
    assert_eq!(
        entry.def.animations["move"],
        Animation { frames: 6, fps: 10 }
    );
    assert!(catalog.sprite("sprite.test_archer").is_none());
}

#[test]
fn a_pack_without_a_scale_or_file_gets_the_default() {
    let mut catalog = SpriteCatalog::default();
    catalog.add_pack(&pack("t"), Path::new("/t"), "").unwrap();
    assert_eq!(catalog.scale(&pack("t")), PixelScale::default());
    assert_eq!(catalog.scale(&pack("unknown")), PixelScale::default());
    assert!(one_sprite("").contains("sprite"));
    catalog
        .add_pack(&pack("u"), Path::new("/u"), &one_sprite(""))
        .unwrap();
    assert_eq!(catalog.sprites().count(), 1);
}

#[test]
fn bad_sprites_are_reported_with_their_line() {
    let cases = [
        (one_sprite("directions = 8"), "`directions` must be 6"),
        (one_sprite("colour = 'red'"), "unknown field `colour`"),
        (
            one_sprite("anchor = { x = 9, y = 0 }"),
            "outside the 8x8 frame",
        ),
        (
            one_sprite("frame = { w = 0, h = 8 }"),
            "`frame.w` must be an integer of at least 1",
        ),
        (one_sprite("sheet = '../escape.png'"), "inside `assets/`"),
        (
            one_sprite("animations = { move = { frames = 2, fps = 4 } }"),
            "needs an `idle`",
        ),
        (
            one_sprite("animations = { idle = { frames = 2 } }"),
            "`idle.fps`",
        ),
        (
            format!("{}\n{}", one_sprite(""), one_sprite("")),
            "declared twice",
        ),
        (
            "pixels_per_level = 0".to_owned(),
            "`pixels_per_level` must be a positive integer",
        ),
        (
            "terrain { key = 'terrain.grass' }".to_owned(),
            "t/assets.lua",
        ),
    ];
    for (source, expected) in cases {
        let error = error_of(&source);
        assert!(
            error.contains(expected),
            "{error} should mention {expected}"
        );
    }
    assert!(error_of(&format!("\n\n{}", one_sprite("directions = 4"))).contains("line 3"));
}

#[test]
fn packs_load_in_order_and_later_sprites_replace_earlier_ones() {
    let dir = tempfile::tempdir().unwrap();
    let write = |id: &str, source: Option<&str>| {
        let root = dir.path().join(id);
        std::fs::create_dir_all(&root).unwrap();
        if let Some(source) = source {
            std::fs::write(root.join(ASSETS_FILE), source).unwrap();
        }
        PackSource {
            id: pack(id),
            root,
            dependencies: Vec::new(),
        }
    };
    let packs = [
        write("base", Some(&one_sprite(""))),
        write("art", None),
        write("broken", Some("sprite {")),
        write(
            "mod",
            Some(&format!("pixels_per_level = 24\n{}", one_sprite(""))),
        ),
    ];
    let (catalog, errors) = SpriteCatalog::load(&packs);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].file, "broken/assets.lua");
    let archer = catalog.sprite("sprite.test_archer").unwrap();
    assert_eq!(archer.pack, pack("mod"));
    assert_eq!(archer.sheet, dir.path().join("mod/assets/a.png"));
    assert_eq!(catalog.scale(&pack("mod")).pixels_per_level, 24);
    assert_eq!(catalog.scale(&pack("broken")), PixelScale::default());
}
