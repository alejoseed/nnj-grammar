use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::Context;
use slog::{o, Drain, Logger};
use time::OffsetDateTime;

/// Build the root logger. Records always go to the terminal; when `log_dir`
/// is given they are also appended to `<log_dir>/YYYY-MM-DD.log`, switching
/// to a new file when the date changes.
pub fn root_logger(log_dir: Option<PathBuf>) -> anyhow::Result<Logger> {
    let term_decorator = slog_term::TermDecorator::new().stdout().build();
    let term_drain = slog_term::FullFormat::new(term_decorator).build().fuse();

    let drain: Box<dyn Drain<Ok = (), Err = slog::Never> + Send> = match log_dir {
        Some(dir) => {
            let writer = DateRotatingWriter::new(dir.clone())
                .with_context(|| format!("failed to open log directory {}", dir.display()))?;
            let file_decorator = slog_term::PlainSyncDecorator::new(writer);
            let file_drain = slog_term::FullFormat::new(file_decorator).build().fuse();
            Box::new(slog::Duplicate::new(term_drain, file_drain).fuse())
        }
        None => Box::new(term_drain),
    };

    let drain = slog_async::Async::new(drain).build().fuse();
    Ok(Logger::root(drain, o!()))
}

/// A logger that drops every record, for callers that don't want logging
/// (tests, library embedders).
pub fn discard_logger() -> Logger {
    Logger::root(slog::Discard, o!())
}

/// Appends to `<dir>/YYYY-MM-DD.log`, reopening when the date changes so the
/// log rolls over to a new file at midnight.
struct DateRotatingWriter {
    dir: PathBuf,
    date: String,
    file: File,
}

impl DateRotatingWriter {
    fn new(dir: PathBuf) -> io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let date = today();
        let file = open_dated(&dir, &date)?;
        Ok(Self { dir, date, file })
    }
}

impl Write for DateRotatingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let date = today();
        if date != self.date {
            self.file = open_dated(&self.dir, &date)?;
            self.date = date;
        }
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn open_dated(dir: &Path, date: &str) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{date}.log")))
}

fn today() -> String {
    // Local date when the offset is known; UTC otherwise (e.g. containers
    // without tzdata, or when the offset can't be determined safely).
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    )
}
