//! One `list-panes -a` call describes every session's pane: alive or dead, how it exited,
//! when the session started and when it last printed. The answer is kept for a moment, so
//! one poll of many agents asks tmux once.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::{Error, Result};

const FORMAT: &str = "#{pane_dead}|#{pane_dead_status}|#{pane_dead_signal}|#{session_created}|\
                      #{window_activity}|#{pane_id}|#{session_name}";
/// How long a snapshot answers lookups before tmux is asked again.
const FRESH_FOR: Duration = Duration::from_secs(1);

/// How a dead pane's process ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    /// Killed by a signal, named the way tmux names it (`term`, `kill`).
    Signal(String),
    /// tmux kept the pane but did not say why it died.
    Unknown,
}

/// The first pane of a session, which runs the agent's command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    /// The full session name.
    pub session: String,
    /// tmux's pane id (`%12`), unique while the tmux server runs.
    pub id: String,
    /// Set once the process exited; `remain-on-exit` keeps the pane until it is killed.
    pub exit: Option<Exit>,
    /// When the session was created, in unix seconds.
    pub created: u64,
    /// When the pane last printed, in unix seconds.
    pub activity: u64,
}

impl Pane {
    pub fn dead(&self) -> bool {
        self.exit.is_some()
    }
}

fn exit_of(dead: &str, status: &str, signal: &str) -> Option<Exit> {
    if dead != "1" {
        return None;
    }
    if let Ok(code) = status.parse::<i32>() {
        return Some(Exit::Code(code));
    }
    if !signal.is_empty() {
        return Some(Exit::Signal(signal.to_string()));
    }
    Some(Exit::Unknown)
}

fn line(text: &str) -> Option<Pane> {
    let fields: Vec<&str> = text.splitn(7, '|').collect();
    let [dead, status, signal, created, activity, id, session] = fields.as_slice() else {
        return None;
    };
    if session.is_empty() {
        return None;
    }
    Some(Pane {
        session: session.to_string(),
        id: id.to_string(),
        exit: exit_of(dead, status, signal),
        created: created.parse().ok()?,
        activity: activity.parse().ok()?,
    })
}

/// One pane per session: the first one tmux lists. Lines it cannot read are skipped.
pub(super) fn parse(out: &str) -> Vec<Pane> {
    out.lines().filter_map(line).fold(vec![], |acc, pane| {
        if acc.iter().any(|p: &Pane| p.session == pane.session) {
            acc
        } else {
            acc.into_iter().chain([pane]).collect()
        }
    })
}

type Snapshot = (Instant, Arc<Vec<Pane>>);

static CACHE: Mutex<Option<Snapshot>> = Mutex::new(None);

fn no_server(e: &Error) -> bool {
    matches!(e, Error::Command { stderr, .. }
        if stderr.contains("no server running")
            || stderr.contains("error connecting to")
            || stderr.contains("server exited unexpectedly"))
}

fn store(panes: Arc<Vec<Pane>>) {
    let mut cache = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    *cache = Some((Instant::now(), panes));
}

/// Asks tmux now and keeps the answer for the next lookups. No server means no panes.
pub(super) async fn fetch() -> Result<Arc<Vec<Pane>>> {
    let panes = match super::run_tmux(&["list-panes", "-a", "-F", FORMAT]).await {
        Ok(out) => Arc::new(parse(&out)),
        Err(e) if no_server(&e) => Arc::new(vec![]),
        Err(e) => return Err(e),
    };
    store(Arc::clone(&panes));
    Ok(panes)
}

/// The kept answer while it is fresh, else a new one.
pub(super) async fn cached() -> Result<Arc<Vec<Pane>>> {
    let kept = {
        let cache = CACHE.lock().unwrap_or_else(|p| p.into_inner());
        cache
            .as_ref()
            .filter(|(at, _)| at.elapsed() < FRESH_FOR)
            .map(|(_, panes)| Arc::clone(panes))
    };
    match kept {
        Some(panes) => Ok(panes),
        None => fetch().await,
    }
}

/// Drops the kept answer: a session was created or killed.
pub(super) fn invalidate() {
    let mut cache = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    *cache = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_that_exited_while_listing_counts_as_no_server() {
        let gone = |stderr: &str| Error::Command {
            command: "tmux list-panes".into(),
            code: Some(1),
            stderr: stderr.into(),
        };
        assert!(no_server(&gone("server exited unexpectedly")));
        assert!(no_server(&gone("no server running on /tmp/x")));
        assert!(!no_server(&gone("unknown option")));
    }

    #[test]
    fn reads_live_exited_and_signalled_panes() {
        let out = "0|||100|105|%1|townhall-k-a\n\
                   1|3||100|101|%2|townhall-k-b\n\
                   1|0||100|101|%3|townhall-k-c\n\
                   1||term|100|101|%4|townhall-k-d\n\
                   1|||100|101|%5|townhall-k-e";
        let panes = parse(out);
        let exits: Vec<Option<Exit>> = panes.iter().map(|p| p.exit.clone()).collect();
        assert_eq!(
            exits,
            vec![
                None,
                Some(Exit::Code(3)),
                Some(Exit::Code(0)),
                Some(Exit::Signal("term".into())),
                Some(Exit::Unknown),
            ]
        );
        assert_eq!(
            panes[0],
            Pane {
                session: "townhall-k-a".into(),
                id: "%1".into(),
                exit: None,
                created: 100,
                activity: 105,
            }
        );
        assert!(!panes[0].dead() && panes[1].dead());
    }

    #[test]
    fn keeps_the_first_pane_of_a_session_and_skips_bad_lines() {
        let out = "0|||1|2|%1|s\n1|1||1|2|%2|s\ngarbage\n0|||x|2|%3|t\n0|||1|2|%4|a|b";
        let panes = parse(out);
        assert_eq!(panes.len(), 2);
        assert_eq!((panes[0].id.as_str(), panes[0].dead()), ("%1", false));
        assert_eq!(panes[1].session, "a|b");
    }
}
