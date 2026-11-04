//! Short memories that keep a poll of many agents cheap: a transcript's turn until the file
//! changes, and a pane's screen until the pane prints again. Both only save work; what they
//! hold is always what reading again would give.

use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use super::transcript::Turn;

/// Entries kept per memory; past it the memory starts over.
const MAX_ENTRIES: usize = 512;

/// The transcript file as it was read: its size and modified time.
pub(super) type FileStamp = (u64, SystemTime);

type Turns = HashMap<PathBuf, (FileStamp, Option<Turn>)>;

static TURNS: LazyLock<Mutex<Turns>> = LazyLock::new(Mutex::default);

/// A screen and the pane activity it was captured at, plus the second it was captured in.
#[derive(Clone)]
struct Screen {
    activity: u64,
    captured: u64,
    text: String,
}

static SCREENS: LazyLock<Mutex<HashMap<String, Screen>>> = LazyLock::new(Mutex::default);

fn keep<K: Eq + Hash, V>(map: &Mutex<HashMap<K, V>>, key: K, value: V) {
    let mut map = map.lock().unwrap_or_else(|p| p.into_inner());
    if map.len() >= MAX_ENTRIES && !map.contains_key(&key) {
        map.clear();
    }
    map.insert(key, value);
}

/// The turn read last time from `path`, when the file has not changed since.
pub(super) fn turn(path: &Path, stamp: FileStamp) -> Option<Option<Turn>> {
    let map = TURNS.lock().unwrap_or_else(|p| p.into_inner());
    map.get(path)
        .filter(|(kept, _)| *kept == stamp)
        .map(|(_, turn)| turn.clone())
}

pub(super) fn keep_turn(path: &Path, stamp: FileStamp, turn: Option<Turn>) {
    keep(&TURNS, path.to_path_buf(), (stamp, turn));
}

/// The screen captured last time from pane `id`, when the pane has not printed since.
/// tmux counts activity in whole seconds, so a capture in the same second as the last
/// output is never trusted: more output may have followed within that second.
pub(super) fn screen(id: &str, activity: u64) -> Option<String> {
    let map = SCREENS.lock().unwrap_or_else(|p| p.into_inner());
    map.get(id)
        .filter(|s| s.activity == activity && s.captured > activity)
        .map(|s| s.text.clone())
}

pub(super) fn keep_screen(id: &str, activity: u64, captured: u64, text: &str) {
    let screen = Screen {
        activity,
        captured,
        text: text.to_string(),
    };
    keep(&SCREENS, id.to_string(), screen);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_is_reused_only_while_the_pane_is_quiet() {
        keep_screen("%memo-1", 100, 101, "hello");
        assert_eq!(screen("%memo-1", 100).as_deref(), Some("hello"));
        assert!(screen("%memo-1", 102).is_none());
        keep_screen("%memo-2", 100, 100, "same second");
        assert!(screen("%memo-2", 100).is_none());
        assert!(screen("%memo-none", 1).is_none());
    }

    #[test]
    fn a_turn_is_reused_only_for_the_same_file_stamp() {
        let path = Path::new("/memo/test.jsonl");
        let stamp = (10, SystemTime::UNIX_EPOCH);
        keep_turn(path, stamp, Some(Turn::Busy));
        assert_eq!(turn(path, stamp), Some(Some(Turn::Busy)));
        assert_eq!(turn(path, (11, SystemTime::UNIX_EPOCH)), None);
    }

    #[test]
    fn a_full_memory_starts_over() {
        let map: Mutex<HashMap<usize, usize>> = Mutex::default();
        (0..MAX_ENTRIES).for_each(|i| keep(&map, i, i));
        keep(&map, 0, 7);
        assert_eq!(map.lock().unwrap().len(), MAX_ENTRIES);
        keep(&map, MAX_ENTRIES, 1);
        assert_eq!(map.lock().unwrap().len(), 1);
    }
}
