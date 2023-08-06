use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr};
use reaper_save_rs::prelude::ReaperProject;
use std::path::PathBuf;
#[allow(unused_imports)]
use tracing::{debug, error, info, instrument, trace, warn};

/// Cli for reaper saves, for now only useful for testing
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// command to run
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// validate the file to check if parses properly
    Validate {
        /// file to validate
        #[arg(short, long)]
        file_path: PathBuf,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    let Cli { command } = Cli::parse();
    match command {
        Command::Validate { file_path } => std::fs::read_to_string(&file_path)
            .wrap_err("reading file from disk")
            .and_then(|text| ReaperProject::parse_from_str(&text).wrap_err("parsing file"))
            .wrap_err_with(|| format!("reading [{}]", file_path.display()))
            .map(|_| {
                info!(?file_path, "OK");
            }),
    }
}
