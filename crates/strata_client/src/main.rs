//! The Strata game client.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use clap::Parser;
use strata_session::{LaunchSpec, launch};

mod assets;
mod dev;
mod input;
mod plugins;
mod sim_link;
mod ui;

use plugins::ClientPlugins;
use sim_link::SimLinkPlugin;

/// Play a Strata match.
#[derive(Parser)]
#[command(name = "strata-client", version)]
struct Args {
    /// A pack folder, or a directory whose subdirectories are packs. Repeat for several.
    #[arg(long = "pack", required = true)]
    packs: Vec<PathBuf>,
    /// The scenario to play, possibly qualified by pack.
    #[arg(long)]
    scenario: String,
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

fn main() {
    let args = Args::parse();
    let handle = launch(LaunchSpec {
        roots: pack_roots(args.packs),
        scenario: args.scenario,
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
}
