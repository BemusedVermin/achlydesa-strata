//! The `strata` command line.

use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result, WrapErr};

/// Tools for Strata pack authors.
#[derive(Parser)]
#[command(name = "strata", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Write LuaLS type stubs for editor autocomplete.
    Stubs {
        /// Directory to write the stub files into.
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Stubs { out } => write_stubs(&out),
    }
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
