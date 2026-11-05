//! Structured JSONL logging: `<townhall home>/logs/<prefix>.YYYY-MM-DD.jsonl`.
//! The path, line schema, redaction rules and event catalog are in `docs/logging.md`.
//!
//! Every line must carry a stable dotted `event` field, and free-form text (errors,
//! client strings) goes through `townhall_core::redact::redact`. Never log prompt or
//! message bodies, only their lengths.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};
use std::time::SystemTime;

use serde_json::Value;
use tracing::{Level, Subscriber};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Env var holding the file filter (`EnvFilter` syntax).
pub const FILTER_ENV: &str = "TOWNHALL_LOG";
const DEFAULT_FILTER: &str = "info,townhall=debug,townhall::git=info,townhall::tmux=info";
const SUFFIX: &str = "jsonl";
const KEEP_FILES: usize = 14;
/// The folder is pruned to this many bytes.
pub const MAX_LOG_BYTES: u64 = 200 * 1024 * 1024;
const BACKTRACE_CAP: usize = 4096;

pub struct LogConfig {
    pub dir: PathBuf,
    /// File name prefix: `daemon` or `app`. Two processes never share a prefix.
    pub prefix: String,
    /// Lowest level the human-readable stderr layer shows.
    pub stderr_level: Level,
}

/// Keeps the background writer alive; drop it to flush. Hold it for the whole process.
#[must_use = "dropping the guard stops and flushes the log writer"]
pub struct Guard {
    _worker: Option<WorkerGuard>,
}

/// Sets up the file layer, the stderr layer and the panic hook, and prunes old files.
/// Safe to call again: when a subscriber is already installed this changes nothing.
pub fn init(config: LogConfig) -> Guard {
    if let Err(e) = prune(&config.dir, MAX_LOG_BYTES) {
        eprintln!(
            "townhall: cannot prune logs in {}: {e}",
            config.dir.display()
        );
    }
    let (file, worker) = match file_writer(&config) {
        Ok((writer, worker)) => (Some(writer), Some(worker)),
        Err(e) => {
            eprintln!("townhall: file logging disabled: {e}");
            (None, None)
        }
    };
    let filter = std::env::var(FILTER_ENV)
        .ok()
        .and_then(|v| EnvFilter::try_new(v).ok())
        .unwrap_or_else(|| EnvFilter::new(DEFAULT_FILTER));
    let file_layer = file.map(|writer| json_layer(writer).with_filter(filter));
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(io::stderr)
        .with_filter(LevelFilter::from_level(config.stderr_level));
    let installed = tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .try_init()
        .is_ok();
    if installed {
        install_panic_hook();
        Guard { _worker: worker }
    } else {
        // Someone else owns the global subscriber: our writer would never get a line.
        Guard { _worker: None }
    }
}

fn file_writer(config: &LogConfig) -> Result<(NonBlocking, WorkerGuard), String> {
    std::fs::create_dir_all(&config.dir).map_err(|e| format!("{}: {e}", config.dir.display()))?;
    let appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix(&config.prefix)
        .filename_suffix(SUFFIX)
        .max_log_files(KEEP_FILES)
        .build(&config.dir)
        .map_err(|e| e.to_string())?;
    Ok(tracing_appender::non_blocking(appender))
}

/// The JSON line format shared by the files and `test_capture`.
fn json_layer<S, W>(writer: W) -> impl Layer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'w> MakeWriter<'w> + 'static,
{
    tracing_subscriber::fmt::layer()
        .json()
        .flatten_event(true)
        .with_current_span(true)
        .with_span_list(false)
        .with_ansi(false)
        .with_writer(writer)
}

fn install_panic_hook() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
                (*s).to_owned()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "non-string panic payload".to_owned()
            };
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_default();
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();
            tracing::error!(
                target: "townhall::panic",
                event = "panic",
                location = %location,
                backtrace = %cut(&backtrace, BACKTRACE_CAP),
                "{}",
                townhall_core::redact::redact(&message)
            );
            previous(info);
        }));
    });
}

/// `text` cut to at most `max` bytes on a char boundary.
fn cut(text: &str, max: usize) -> &str {
    let mut end = max.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Deletes the oldest `*.jsonl` files in `dir` until the rest fit in `max_bytes`. The newest
/// file is always kept (it is the one being written). Returns how many files were deleted.
pub fn prune(dir: &Path, max_bytes: u64) -> io::Result<usize> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut files: Vec<(SystemTime, PathBuf, u64)> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == SUFFIX))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| {
                (
                    meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    e.path(),
                    meta.len(),
                )
            })
        })
        .collect();
    files.sort();
    let mut total: u64 = files.iter().map(|f| f.2).sum();
    let mut deleted = 0;
    for (_, path, len) in files.iter().take(files.len().saturating_sub(1)) {
        if total <= max_bytes {
            break;
        }
        std::fs::remove_file(path)?;
        total -= len;
        deleted += 1;
    }
    Ok(deleted)
}

/// Lines captured in memory by [`test_capture`].
#[derive(Clone, Default)]
pub struct Capture {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl Capture {
    /// Every captured line, parsed.
    pub fn lines(&self) -> Vec<Value> {
        let buf = self.buf.lock().unwrap_or_else(|e| e.into_inner());
        String::from_utf8_lossy(&buf)
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    /// The captured lines whose `event` is `event`.
    pub fn events(&self, event: &str) -> Vec<Value> {
        self.lines()
            .into_iter()
            .filter(|l| l["event"] == event)
            .collect()
    }
}

impl io::Write for Capture {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = Capture;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// For tests: installs the JSON layer as this thread's subscriber (all levels) and returns
/// what it captures. Events from other threads are not seen, so use a current-thread
/// runtime when the code under test spawns tasks. Keep the guard alive for the test.
pub fn test_capture() -> (Capture, tracing::subscriber::DefaultGuard) {
    let capture = Capture::default();
    let subscriber = tracing_subscriber::registry().with(json_layer(capture.clone()));
    let guard = tracing::subscriber::set_default(subscriber);
    (capture, guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn file(dir: &Path, name: &str, bytes: usize, age_secs: u64) {
        let path = dir.join(name);
        std::fs::write(&path, vec![b'x'; bytes]).unwrap();
        let modified = SystemTime::now() - Duration::from_secs(age_secs);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn prune_deletes_the_oldest_until_under_the_cap() {
        let tmp = tempfile::tempdir().unwrap();
        file(tmp.path(), "a.2024-01-01.jsonl", 100, 3000);
        file(tmp.path(), "a.2024-01-02.jsonl", 100, 2000);
        file(tmp.path(), "a.2024-01-03.jsonl", 100, 1000);
        assert_eq!(prune(tmp.path(), 250).unwrap(), 1);
        assert_eq!(
            names(tmp.path()),
            ["a.2024-01-02.jsonl", "a.2024-01-03.jsonl"]
        );
        assert_eq!(prune(tmp.path(), 250).unwrap(), 0, "already under the cap");
    }

    #[test]
    fn prune_orders_by_age_across_prefixes_and_ignores_other_files() {
        let tmp = tempfile::tempdir().unwrap();
        file(tmp.path(), "daemon.2024-01-02.jsonl", 100, 3000);
        file(tmp.path(), "app.2024-01-01.jsonl", 100, 1000);
        file(tmp.path(), "notes.txt", 10_000, 9000);
        assert_eq!(prune(tmp.path(), 100).unwrap(), 1);
        assert_eq!(names(tmp.path()), ["app.2024-01-01.jsonl", "notes.txt"]);
    }

    #[test]
    fn prune_keeps_the_newest_file_even_when_it_alone_is_over_the_cap() {
        let tmp = tempfile::tempdir().unwrap();
        file(tmp.path(), "a.2024-01-01.jsonl", 500, 100);
        assert_eq!(prune(tmp.path(), 10).unwrap(), 0);
        assert_eq!(names(tmp.path()).len(), 1);
    }

    #[test]
    fn prune_of_a_missing_folder_is_fine() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(prune(&tmp.path().join("nope"), 1).unwrap(), 0);
    }

    #[test]
    fn cut_respects_char_boundaries() {
        assert_eq!(cut("aéb", 2), "a");
        assert_eq!(cut("abc", 10), "abc");
    }
}
