**Task T-048b** · M4 Playable client · Est. core lines: ~100 / 500 (mostly moved code)

## Goal

Give presentation math a Bevy-free home before terrain and sprites land, and stop `strata_client` from becoming a monolith.

## Design references

- `docs/design/strata-engine.md` §3 Architecture
- `docs/design/strata-engine.md` §13 Bevy client

## Scope

- New crate `strata_scene`: depends on `strata_kernel`, `strata_model`, `strata_content` and `glam` (same version as Bevy's). No Bevy. It is presentation code, not a core crate: floats are allowed, and it does not carry `#![deny(clippy::float_arithmetic)]`.
- Move `crates/strata_client/src/scene/` (projection, yaw, ray picking from T-048) and its tests into `strata_scene` unchanged apart from paths.
- New crate `strata_render`: depends on `strata_scene` and Bevy. Move `CameraPlugin`, `InputPlugin` and the plugin skeletons from T-047 (`BoardPlugin`, `FigurePlugin`, `OcclusionPlugin`, `PlaybackPlugin`, `PreviewPlugin`) into it.
- `strata_client` keeps the app shell, `SimLinkPlugin`, UI panels and editor UI, and adds the `strata_render` plugins.
- `deny.toml`: allow `bevy` under both `strata_client` and `strata_render` in `wrappers`.
- Update `docs/design/strata-engine.md` §3 (13 crates, layers, crate table) and §13.3 (which crate owns each plugin). Update the layering line and the Bevy rule in `CLAUDE.md`; this task may edit `CLAUDE.md`.

## Acceptance criteria

- [ ] The client behaves exactly as before.
- [ ] `cargo tree -p strata_scene` contains no `bevy` crate.
- [ ] The picking and projection tests run without Bevy.
- [ ] `cargo deny check` passes with the new `wrappers`.

## Tests

- The moved tests, unchanged.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Moves are real moves (`git mv`), so the line budget sees renames rather than new code
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._