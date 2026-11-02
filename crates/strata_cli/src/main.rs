//! The `strata` command line.

use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use miette::{IntoDiagnostic, MietteHandlerOpts, Result, WrapErr};

mod lint;
mod package;
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
    /// Verify, dump or convert replays (not yet implemented).
    Replay(Unimplemented),
}

#[derive(Subcommand)]
enum PackCommand {
    /// Load the pack set and print every error with its file and line.
    Validate {
        /// Directories whose subdirectories are packs.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Run a pack's Lua tests (not yet implemented).
    Test(Unimplemented),
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
        Command::Pack(PackCommand::Test(_)) => unimplemented_command("pack test"),
        Command::Stubs { out } => finish(write_stubs(&out)),
        Command::Lint { paths } => lint::run(&paths),
        Command::Package { out, client, paths } => finish(package::run(&paths, &out, client)),
        Command::Stats(_) => unimplemented_command("stats"),
        Command::Sim(_) => unimplemented_command("sim"),
        Command::Replay(_) => unimplemented_command("replay"),
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
