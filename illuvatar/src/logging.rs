use std::io::Write;
use std::path::Path;
use std::{fs::OpenOptions, io::stdout};

use slog::{o, Drain, Level, Logger};
use slog_async::{self};
use slog_scope::{self, GlobalLoggerGuard};
use slog_term;

pub fn init_logger<P: AsRef<Path>>(
    log_path: Option<P>,
    verbosity: u8,
) -> Result<GlobalLoggerGuard, std::io::Error> {
    let log_file: Box<dyn Write + Send> = match log_path {
        Some(p) => Box::new(
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(p)?,
        ),
        None => Box::new(stdout()),
    };
    let log_decorator = slog_term::PlainDecorator::new(log_file);

    let log_level = match verbosity {
        0 => Level::Info,
        1 => Level::Debug,
        _ => Level::Trace,
    };

    let drain = slog_term::CompactFormat::new(log_decorator)
        .use_local_timestamp() // TODO this does not seem to work?
        .build()
        .fuse();

    let drain = slog_async::Async::new(drain)
        .thread_name("illulogger".to_string())
        .overflow_strategy(slog_async::OverflowStrategy::DropAndReport)
        .build();

    let drain = drain.filter_level(log_level);

    let guard = slog_scope::set_global_logger(Logger::root(drain.fuse(), o!()));

    // register slog logger as `log` logger
    slog_stdlog::init().expect("Failed to initialize logging");

    Ok(guard)
}
