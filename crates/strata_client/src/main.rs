//! The Strata game client, and with `--edit` the map editor.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use clap::Parser;
use strata_editor::OpenedMap;
use strata_render::RenderPlugins;
use strata_session::{LaunchSpec, launch};

mod dev;
mod editor;
mod input;
mod plugins;
mod sim_link;
mod ui;

use editor::EditorPlugin;
use plugins::ClientPlugins;
use sim_link::SimLinkPlugin;

/// Play a Strata match, or edit a map.
#[derive(Parser)]
#[command(name = "strata-client", version)]
struct Args {
    /// A pack folder, or a directory whose subdirectories are packs. Repeat for several.
    #[arg(long = "pack", required_unless_present = "edit")]
    packs: Vec<PathBuf>,
    /// The scenario to play, possibly qualified by pack.
    #[arg(long, required_unless_present = "edit")]
    scenario: Option<String>,
    /// Edit the map defined in this file instead of playing. The packs next to the map's pack
    /// are loaded, as `strata map check` does.
    #[arg(long, conflicts_with_all = ["packs", "scenario"])]
    edit: Option<PathBuf>,
    /// The ruleset; needed only when the packs define more than one.
    #[arg(long)]
    ruleset: Option<String>,
    /// The match seed.
    #[arg(long, default_value_t = 0)]
    seed: u64,
    /// Stop the match after this many commands.
    #[arg(long, default_value_t = 100_000)]
    commands: u32,
}

/// Pack roots hold packs as subdirectories, so a path that is itself a pack stands for its parent.
fn pack_roots(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for path in paths {
        let parent = path.parent().filter(|_| path.join("pack.lua").is_file());
        let root = parent.map_or(path.clone(), |p| p.to_owned());
        let root = if root.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            root
        };
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    roots
}

fn main() -> std::process::ExitCode {
    let args = Args::parse();
    if let Some(file) = args.edit {
        return edit(&file);
    }
    let handle = launch(LaunchSpec {
        roots: pack_roots(args.packs),
        scenario: args.scenario.unwrap_or_default(),
        ruleset: args.ruleset,
        seed: args.seed,
        armies: Vec::new(),
        commands: args.commands,
    });
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_plugins(SimLinkPlugin::new(handle))
        .add_plugins(ClientPlugins)
        .run();
    std::process::ExitCode::SUCCESS
}

/// Runs the map editor on `file`, or says why the map cannot be opened.
fn edit(file: &std::path::Path) -> std::process::ExitCode {
    let opened = match OpenedMap::open(file) {
        Ok(opened) => opened,
        Err(error) => {
            let mut message = error.to_string();
            let mut source = std::error::Error::source(&error);
            while let Some(cause) = source {
                message.push_str(&format!(": {cause}"));
                source = cause.source();
            }
            eprintln!("strata-client: {message}");
            return std::process::ExitCode::from(2);
        }
    };
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_plugins((RenderPlugins, EditorPlugin::new(opened)))
        .run();
    std::process::ExitCode::SUCCESS
}
