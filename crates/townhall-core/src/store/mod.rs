//! The store: one SQLite database, `~/.townhall/townhall.db`, owned by the daemon.
//! One connection behind a mutex, every call on `spawn_blocking`; WAL, foreign keys,
//! a 5 s busy timeout. `townhall.lock` keeps a second owner out.
//!
//! Routes and units call the typed functions in this module's files and never write SQL.
//! A unit that owns tables adds them in its own `unit_*.rs` (migrations and `impl Store`
//! functions over `Store::call`). Every write publishes its `ServerEvent`s on the bus while
//! it holds the connection, so events arrive in write order.

mod artifacts;
mod attachments;
mod codec;
mod conversations;
mod handoffs;
mod import;
mod lock;
pub mod migrate;
mod plugin_tokens;
mod repos;
mod search;
mod settings;
mod tasks;
mod thread;
mod unit_2_2;
mod unit_2_3;
mod unit_2_5;
mod unit_2_6;
mod unit_2_7;
mod unit_2_8b;
mod unit_2_8c;
mod unit_2_9;
mod unit_plugins;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;

pub use import::IMPORTED_SUFFIX;
pub use lock::LOCK_FILE;
pub use migrate::Migration;
pub use plugin_tokens::TokenRow;
pub use settings::MAX_PROFILE_CHARS;
pub use unit_2_8c::{
    NewRun, Run, RunStep, StepUpdate, RUN_FAILED, RUN_FINISHED, RUN_NEEDS_YOU, RUN_RUNNING,
};
pub use unit_plugins::{Marketplace, Plugin};

use crate::events::EventBus;
use crate::{Error, Home, Result};

pub const DB_FILE: &str = "townhall.db";
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::Database(e.to_string())
    }
}

#[derive(Debug)]
struct Inner {
    conn: Mutex<Connection>,
    bus: EventBus,
    home: Home,
    _lock: lock::DbLock,
}

#[derive(Debug, Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

fn configure(conn: &Connection) -> Result<()> {
    conn.busy_timeout(BUSY_TIMEOUT)?;
    let mode: String = conn.query_row("pragma journal_mode = wal", [], |r| r.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Error::Database(format!("cannot switch to WAL, got {mode}")));
    }
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

impl Store {
    /// Locks, opens (creating it when missing), migrates, then imports an old
    /// `config.json` once. Fails at once when another townhall holds the database.
    pub fn open(home: &Home, bus: EventBus) -> Result<Self> {
        home.ensure()?;
        let lock = lock::acquire(home.root())?;
        let mut conn = Connection::open(Self::path(home))?;
        configure(&conn)?;
        migrate::apply(&mut conn, &migrate::all())?;
        if let Err(e) = import::config_json(&mut conn, home) {
            tracing::warn!(
                target: "townhall::store", event = "store.config_import_failed", error_kind = e.kind(),
                error = %crate::redact::redact(&e.to_string()), "config.json was not imported"
            );
        }
        let inner = Inner {
            conn: Mutex::new(conn),
            bus,
            home: home.clone(),
            _lock: lock,
        };
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub fn path(home: &Home) -> PathBuf {
        home.root().join(DB_FILE)
    }

    pub fn home(&self) -> &Home {
        &self.inner.home
    }

    pub fn bus(&self) -> &EventBus {
        &self.inner.bus
    }

    /// Runs `f` on the connection off the async runtime. For tables a unit owns; everything
    /// else has a typed function.
    pub async fn call<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let mut conn = inner.conn.lock().unwrap_or_else(|p| p.into_inner());
            f(&mut conn)
        })
        .await
        .map_err(|e| Error::Database(format!("store call failed: {e}")))?
    }
}
