use std::path::{Path, PathBuf};

use glam::UVec2;
use strata_content::{ASSET_CONSTRUCTORS, ASSET_GLOBALS};
use strata_kernel::PackId;

use super::*;

/// What `source` declares, run the way the simulation thread runs `assets.lua`.
fn run(source: &str) -> Declarations {
    strata_lua::declare("t/assets.lua", source, &ASSET_CONSTRUCTORS, &ASSET_GLOBALS).unwrap()
}

fn add(catalog: &mut SpriteCatalog, id: &str, root: &str, source: &str) -> Result<(), AssetError> {
    catalog.add_pack(&pack(id), Path::new(root), &run(source))
}

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
    let error = add(&mut catalog, "t", "/t", source).unwrap_err();
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
    add(&mut catalog, "t", "/packs/t", ASSETS).unwrap();
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
    add(&mut catalog, "t", "/t", "").unwrap();
    assert_eq!(catalog.scale(&pack("t")), PixelScale::default());
    assert_eq!(catalog.scale(&pack("unknown")), PixelScale::default());
    assert!(one_sprite("").contains("sprite"));
    add(&mut catalog, "u", "/u", &one_sprite("")).unwrap();
    assert_eq!(catalog.sprites().count(), 1);
}

#[test]
fn bad_sprites_are_reported_with_their_line() {
    let cases = [
        (one_sprite("directions = 8"), "`directions` must be 6"),
        (one_sprite("colour = 'red'"), "unknown field `colour`"),
        (
            one_sprite("anchor = { x = 9, y = 0 }"),
            "`anchor.x` must be an integer from 0 to 8",
        ),
        (
            one_sprite("frame = { w = 0, h = 8 }"),
            "`frame.w` must be an integer from 1 to 1024",
        ),
        (
            one_sprite("frame = { w = 8, h = 4096 }"),
            "`frame.h` must be an integer from 1 to 1024",
        ),
        (
            one_sprite("animations = { idle = { frames = 65, fps = 1 } }"),
            "`idle.frames` must be an integer from 1 to 64",
        ),
        (
            one_sprite(
                "frame = { w = 1024, h = 8 }, animations = { idle = { frames = 64, fps = 1 } }",
            ),
            "wider or taller than 16384",
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
            "`pixels_per_level` must be an integer from 1 to 64",
        ),
        (
            "pixels_per_level = 4294967297".to_owned(),
            "`pixels_per_level` must be an integer from 1 to 64",
        ),
        (
            "hex_width_px = 100000".to_owned(),
            "`hex_width_px` must be an integer from 1 to 512",
        ),
        (one_sprite("directions = 4"), "t/assets.lua"),
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

fn assets(id: &str, declared: Result<Declarations, String>) -> PackAssets {
    PackAssets {
        pack: pack(id),
        root: PathBuf::from("/packs").join(id),
        declared,
    }
}

#[test]
fn packs_load_in_order_and_later_sprites_replace_earlier_ones() {
    let packs = [
        assets("base", Ok(run(&one_sprite("")))),
        assets("broken", Err("syntax error near `{`".into())),
        assets("bad", Ok(run(&one_sprite("directions = 4")))),
        assets(
            "mod",
            Ok(run(&format!("pixels_per_level = 24\n{}", one_sprite("")))),
        ),
    ];
    let (catalog, errors) = SpriteCatalog::from_packs(&packs);
    let files: Vec<_> = errors.iter().map(|e| e.file.as_str()).collect();
    assert_eq!(files, ["broken/assets.lua", "bad/assets.lua"]);
    assert!(errors[0].message.contains("syntax error"));
    let archer = catalog.sprite("sprite.test_archer").unwrap();
    assert_eq!(archer.pack, pack("mod"));
    assert_eq!(archer.sheet, PathBuf::from("/packs/mod/assets/a.png"));
    assert_eq!(catalog.scale(&pack("mod")).pixels_per_level, 24);
    assert_eq!(catalog.scale(&pack("broken")), PixelScale::default());
    let failed: Vec<_> = ["base", "broken", "bad", "mod", "absent"]
        .into_iter()
        .filter(|id| catalog.failed(&pack(id)))
        .collect();
    assert_eq!(failed, ["broken", "bad"]);
}
