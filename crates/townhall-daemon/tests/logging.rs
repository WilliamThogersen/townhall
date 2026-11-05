//! `logging::init` installs a process-global subscriber and panic hook, so everything that
//! needs it runs in this one test, in order.

use serde_json::Value;
use townhall_daemon::logging::{self, LogConfig};

fn read_lines(dir: &std::path::Path) -> Vec<Value> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    files
        .iter()
        .flat_map(|f| {
            std::fs::read_to_string(f)
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .map(|l| serde_json::from_str(&l).unwrap_or_else(|e| panic!("not JSON: {l}: {e}")))
        .collect()
}

#[test]
fn init_writes_json_lines_and_a_panic_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("logs");
    let guard = logging::init(LogConfig {
        dir: dir.clone(),
        prefix: "unit".into(),
        stderr_level: tracing::Level::WARN,
    });
    // A second call is harmless.
    let again = logging::init(LogConfig {
        dir: tmp.path().join("other"),
        prefix: "other".into(),
        stderr_level: tracing::Level::WARN,
    });
    drop(again);

    tracing::info!(target: "townhall::test", event = "test.hello", task = "t1", duration_ms = 12u64, "hi there");
    let result = std::panic::catch_unwind(|| panic!("boom {}", 7));
    assert!(result.is_err());
    drop(guard); // flushes the non-blocking writer

    let files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let name = &files[0];
    assert!(
        name.starts_with("unit.") && name.ends_with(".jsonl"),
        "{name}"
    );
    let date = &name["unit.".len()..name.len() - ".jsonl".len()];
    assert_eq!(date.len(), 10, "{name}");
    assert_eq!(date.matches('-').count(), 2, "{name}");

    let lines = read_lines(&dir);
    let hello = lines
        .iter()
        .find(|l| l["event"] == "test.hello")
        .expect("hello line");
    assert_eq!(hello["level"], "INFO");
    assert_eq!(hello["target"], "townhall::test");
    assert_eq!(hello["message"], "hi there");
    assert_eq!(hello["task"], "t1");
    assert_eq!(hello["duration_ms"], 12);
    let ts = hello["timestamp"].as_str().unwrap();
    assert!(ts.ends_with('Z') && ts.contains('T'), "{ts}");

    let panic = lines
        .iter()
        .find(|l| l["event"] == "panic")
        .expect("panic line");
    assert_eq!(panic["level"], "ERROR");
    assert_eq!(panic["message"], "boom 7");
    assert!(
        panic["location"].as_str().unwrap().contains("logging.rs"),
        "{panic}"
    );
    assert!(panic["backtrace"].as_str().unwrap().len() <= 4096);
}

#[test]
fn test_capture_collects_events_on_this_thread() {
    let (capture, _guard) = logging::test_capture();
    tracing::warn!(target: "townhall::test", event = "test.captured", tool = "x", "careful");
    tracing::info!(event = "test.other", "n/a");
    let found = capture.events("test.captured");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["level"], "WARN");
    assert_eq!(found[0]["tool"], "x");
    assert_eq!(capture.lines().len(), 2);
}
