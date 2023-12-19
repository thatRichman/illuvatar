pub(crate) mod accumulator;
pub(crate) mod bcl;
pub(crate) mod logging;

use std::sync::OnceLock;
use std::{path::PathBuf, process};

use clap::{arg, command, value_parser, Parser};
use slog::{slog_error, slog_info, slog_o};
use slog_scope;

use samplesheet::{reader, SampleSheet};
use seqdir::{SeqDir, SequencingDirectory};

use thiserror::Error;

static SAMPLESHEET: OnceLock<SampleSheet> = OnceLock::new();

#[derive(Debug, Error)]
pub enum IlluvatarError {
    #[error(transparent)]
    SampleSheetError(#[from] samplesheet::SampleSheetError),
    #[error(transparent)]
    SeqDirError(#[from] seqdir::SeqDirError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("")]
    Noop,
}

fn illuvatar(args: Illuvatar) -> Result<(), IlluvatarError> {
    let path = args.input;
    let seq_dir = slog_scope::scope(
        &slog_scope::logger().new(slog_o!("scope" => "SeqDir")),
        || SeqDir::from_path(path),
    )?;

    slog_scope::scope(
        &slog_scope::logger().new(slog_o!("scope" => "SampleSheet")),
        || -> Result<(), IlluvatarError> {
            let samplesheet = seq_dir.samplesheet()?;
            SAMPLESHEET
                .set(reader::read_samplesheet(samplesheet)?)
                .expect("Unable to initialize SampleSheet");
            Ok(())
        },
    )?;
    slog_info!(
        slog_scope::logger(),
        "Initialized samplesheet version {:?}",
        SAMPLESHEET.get().unwrap().version()
    );

    Ok(())
}

fn main() {
    let args = Illuvatar::parse();
    let _log_guard = logging::init_logger(args.logfile.as_ref(), args.verbose).map_err(|e| {
        eprintln!("Failed to initialize logger: {e}");
        process::exit(1)
    });

    slog_scope::scope(
        &slog_scope::logger().new(slog_o!("scope" => "main")),
        || match illuvatar(args) {
            Ok(()) => {}
            Err(e) => {
                slog_error!(slog_scope::logger(), "{}", e);
            }
        },
    )
}

#[derive(Parser, Debug)]
#[clap(author = "Spencer Richman", version = "0.0.1", about, long_about = None)]
#[command(arg_required_else_help(true))]
struct Illuvatar {
    /// Sequencing output directory
    #[arg(short, long, value_name = "SEQUENCING DIR")]
    input: PathBuf,

    /// Log file name
    #[arg(short, long, global = true, default_value = None)]
    logfile: Option<PathBuf>,

    /// Verbosity of logging
    #[arg(short, long, global = true, value_parser = value_parser!(u8).range(0..=2), default_value_t = 0)]
    verbose: u8,
}
