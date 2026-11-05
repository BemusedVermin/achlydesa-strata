//! The `strata` command line.

use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use miette::{IntoDiagnostic, MietteHandlerOpts, Result, WrapErr};

mod lint;
mod map_check;
mod pack_test;
mod package;
mod replay;
mod validate;

/// Tools for Strata pack authors.
#[derive(Parser)]
#[command(name = "strata", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Work with packs.
    #[command(subcommand)]
    Pack(PackCommand),
    /// Work with maps.
    #[command(subcommand)]
    Map(MapCommand),
    /// Write LuaLS type stubs for editor autocomplete.
    Stubs {
        /// Directory to write the stub files into.
        #[arg(long)]
        out: PathBuf,
    },
    /// Check pack Lua for determinism problems and card-text drift.
    Lint {
        /// Pack folders, or directories whose subdirectories are packs.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Bundle the client and the public packs into a distributable folder.
    Package {
        /// Directory to write the release into.
        #[arg(long)]
        out: PathBuf,
        /// The client binary to bundle (default: `strata-client` next to this executable).
        #[arg(long)]
        client: Option<PathBuf>,
        /// Directories whose subdirectories are packs.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Report pack statistics (not yet implemented).
    Stats(Unimplemented),
    /// Run AI self-play for balance (not yet implemented).
    Sim(Unimplemented),
    /// Verify, dump or turn a saved match into a test.
    #[command(subcommand)]
    Replay(ReplayCommand),
}

#[derive(Subcommand)]
enum PackCommand {
    /// Load the pack set and print every error with its file and line.
    Validate {
        /// Directories whose subdirectories are packs.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Run the `test {}` records of the packs.
    Test {
        /// Directories whose subdirectories are packs.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Only run tests whose name contains this text.
        #[arg(long)]
        filter: Option<String>,
    },
}

#[derive(Subcommand)]
enum MapCommand {
    /// Validate a map file and print its findings.
    ///
    /// Exits 0 when the map has no errors (warnings are fine), 1 when it has errors, and 2 when
    /// it cannot be checked: the file cannot be read, is not inside a pack, the packs do not
    /// load, or the file defines no map.
    Check {
        /// The map's Lua file, inside a pack folder.
        file: PathBuf,
        /// Figures in the largest army; start zones with fewer standable spaces are errors.
        #[arg(long)]
        army_size: Option<u32>,
    },
}

#[derive(Subcommand)]
enum ReplayCommand {
    /// Replay every command and report the first state hash that differs.
    Verify {
        /// The replay file.
        file: PathBuf,
        /// Directories whose subdirectories are the packs the match used.
        #[arg(long, required = true, num_args = 1..)]
        packs: Vec<PathBuf>,
    },
    /// Print the setup, commands, answers and events as text.
    Dump {
        /// The replay file.
        file: PathBuf,
        /// Pack directories; with them, the events are included.
        #[arg(long, num_args = 1..)]
        packs: Vec<PathBuf>,
    },
    /// Write a pack test that starts after `--at` commands and expects the next one's outcome.
    ToTest {
        /// The replay file.
        file: PathBuf,
        /// Directories whose subdirectories are the packs the match used.
        #[arg(long, required = true, num_args = 1..)]
        packs: Vec<PathBuf>,
        /// How many commands have been played; the test runs the command after them (from 0).
        #[arg(long)]
        at: usize,
    },
}

/// Arguments of a subcommand that does not exist yet.
#[derive(Args)]
struct Unimplemented {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    _args: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if install_report_handler().is_err() {
        eprintln!("warning: could not configure error output");
    }
    match cli.command {
        Command::Pack(PackCommand::Validate { paths }) => validate::run(&paths),
        Command::Pack(PackCommand::Test { paths, filter }) => {
            pack_test::run(&paths, filter.as_deref())
        }
        Command::Map(MapCommand::Check { file, army_size }) => {
            let options = strata_editor::ValidationOptions { army_size };
            match map_check::run(&file, &options) {
                Ok(code) => code,
                Err(report) => {
                    eprintln!("{report:?}");
                    ExitCode::from(map_check::CANNOT_CHECK)
                }
            }
        }
        Command::Stubs { out } => finish(write_stubs(&out)),
        Command::Lint { paths } => lint::run(&paths),
        Command::Package { out, client, paths } => finish(package::run(&paths, &out, client)),
        Command::Stats(_) => unimplemented_command("stats"),
        Command::Sim(_) => unimplemented_command("sim"),
        Command::Replay(command) => finish(run_replay(command)),
    }
}

fn finish(result: Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(report) => {
            eprintln!("{report:?}");
            ExitCode::from(1)
        }
    }
}

fn unimplemented_command(name: &str) -> ExitCode {
    eprintln!("error: `strata {name}` is not implemented yet");
    ExitCode::from(1)
}

/// Piped output is plain ASCII at a fixed width, so it is stable in logs and snapshots.
fn install_report_handler() -> std::result::Result<(), miette::InstallError> {
    let tty = std::io::stderr().is_terminal();
    miette::set_hook(Box::new(move |_| {
        let opts = MietteHandlerOpts::new().terminal_links(false);
        let opts = if tty {
            opts
        } else {
            opts.unicode(false).width(100)
        };
        Box::new(opts.build())
    }))
}

fn write_stubs(out: &PathBuf) -> Result<()> {
    fs::create_dir_all(out)
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot create {}", out.display()))?;
    for file in strata_lua::stubs::generate() {
        let path = out.join(&file.name);
        fs::write(&path, file.text)
            .into_diagnostic()
            .wrap_err_with(|| format!("cannot write {}", path.display()))?;
    }
    Ok(())
}

fn run_replay(command: ReplayCommand) -> Result<()> {
    match command {
        ReplayCommand::Verify { file, packs } => replay::verify(&file, &packs),
        ReplayCommand::Dump { file, packs } => replay::dump(&file, &packs),
        ReplayCommand::ToTest { file, packs, at } => replay::to_test(&file, &packs, at),
    }
}
