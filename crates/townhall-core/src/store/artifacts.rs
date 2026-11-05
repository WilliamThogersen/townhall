//! Proof artifacts of a task, append only: a row in `artifacts` and the bytes in
//! `~/.townhall/artifacts/<conversation id>/<task>/<artifact id>-<name>`. The type comes from
//! the kind and the bytes, never from the agent: a screenshot must be a real raster image,
//! every other kind must be UTF-8 text and is stored as plain text or markdown, so nothing an
//! agent sends can be served as HTML.

use std::path::PathBuf;

use rusqlite::{params, Connection};

use super::codec::{name_of, parse_name};
use super::tasks::find;
use super::Store;
use crate::files::{mime, naming};
use crate::fsutil::write_atomic;
use crate::model::{
    Artifact, ArtifactKind, NewArtifact, MAX_ARTIFACTS_PER_TASK, MAX_ARTIFACT_BYTES,
    MAX_CAPTION_CHARS,
};
use crate::{clock, ids, Error, Result};

const COLUMNS: &str =
    "id, task, conversation_id, name, kind, mime, size, author_role, caption, created_at";
const LOG_MIME: &str = "text/plain; charset=utf-8";
const NOTE_MIME: &str = "text/markdown; charset=utf-8";

fn list(conn: &Connection, task: &str) -> Result<Vec<Artifact>> {
    let sql = format!("select {COLUMNS} from artifacts where task = ?1 order by created_at, rowid");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([task], |r| {
        Ok((
            [
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(9)?,
            ],
            r.get::<_, u64>(6)?,
            r.get::<_, Option<String>>(8)?,
        ))
    })?;
    let mut out = vec![];
    for row in rows {
        let ([id, task, conversation_id, name, kind, mime, role, created_at], size, caption) = row?;
        out.push(Artifact {
            kind: parse_name("artifacts.kind", &kind)?,
            author_role: parse_name("artifacts.author_role", &role)?,
            id,
            task,
            conversation_id,
            name,
            mime,
            size,
            caption,
            created_at,
        });
    }
    Ok(out)
}

fn caption(raw: Option<&str>) -> Result<Option<String>> {
    let one_line = raw.map(|c| c.split_whitespace().collect::<Vec<_>>().join(" "));
    match one_line.filter(|c| !c.is_empty()) {
        Some(c) if c.chars().count() > MAX_CAPTION_CHARS => Err(Error::InvalidInput(format!(
            "`caption` is longer than {MAX_CAPTION_CHARS} characters"
        ))),
        other => Ok(other),
    }
}

/// The stored type and the saved name for the bytes of one kind.
fn typed(new: &NewArtifact) -> Result<(String, String)> {
    match new.kind {
        ArtifactKind::Screenshot => {
            let found = mime::sniff(&new.bytes).ok_or_else(|| {
                Error::InvalidInput(
                    "a screenshot must be a png, jpeg, gif or webp image: send the image as base64"
                        .into(),
                )
            })?;
            let name = naming::file_name(&new.name, mime::image_extensions(found));
            Ok((found.to_string(), name))
        }
        kind => {
            if std::str::from_utf8(&new.bytes).is_err() {
                return Err(Error::InvalidInput(
                    "a log, note or report must be UTF-8 text: send a screenshot as kind `screenshot`"
                        .into(),
                ));
            }
            let mime = if kind == ArtifactKind::Log {
                LOG_MIME
            } else {
                NOTE_MIME
            };
            Ok((mime.to_string(), naming::file_name(&new.name, None)))
        }
    }
}

fn insert(conn: &Connection, home: &crate::Home, task: &str, new: NewArtifact) -> Result<Artifact> {
    let task = find(conn, task)?.ok_or_else(|| Error::missing("task", task))?;
    let size = u64::try_from(new.bytes.len()).unwrap_or(u64::MAX);
    if size == 0 {
        return Err(Error::InvalidInput("`content` is empty".into()));
    }
    if size > MAX_ARTIFACT_BYTES {
        return Err(Error::TooLarge(format!(
            "an artifact is at most {} KB",
            MAX_ARTIFACT_BYTES / 1024
        )));
    }
    let held: usize = conn.query_row(
        "select count(*) from artifacts where task = ?1",
        [&task.name],
        |r| r.get(0),
    )?;
    if held >= MAX_ARTIFACTS_PER_TASK {
        return Err(Error::Conflict(format!(
            "task `{}` already has {MAX_ARTIFACTS_PER_TASK} artifacts, the most it can hold",
            task.name
        )));
    }
    let (mime, name) = typed(&new)?;
    let artifact = Artifact {
        id: ids::new_id(),
        conversation_id: task.conversation_id.clone(),
        task: task.name.clone(),
        kind: new.kind,
        author_role: new.author_role,
        caption: caption(new.caption.as_deref())?,
        mime,
        size,
        name,
        created_at: clock::now_rfc3339(),
    };
    let dir = home.artifacts_for(&artifact.conversation_id, &artifact.task);
    home.create_dir(&dir)?;
    let path = dir.join(file_of(&artifact));
    write_atomic(&path, &new.bytes, false)?;
    let recorded = conn.execute(
        "insert into artifacts (id, task, conversation_id, name, kind, mime, size, author_role, caption, created_at)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            artifact.id,
            artifact.task,
            artifact.conversation_id,
            artifact.name,
            name_of(&artifact.kind),
            artifact.mime,
            artifact.size,
            name_of(&artifact.author_role),
            artifact.caption,
            artifact.created_at
        ],
    );
    if let Err(e) = recorded {
        if let Err(cleanup) = std::fs::remove_file(&path) {
            tracing::warn!(
                target: "townhall::store", event = "store.artifact_cleanup_failed",
                path = %path.display(), error = %crate::redact::redact(&cleanup.to_string()),
                "cannot remove artifact file"
            );
        }
        return Err(e.into());
    }
    Ok(artifact)
}

fn file_of(a: &Artifact) -> String {
    format!("{}-{}", a.id, a.name)
}

impl Store {
    /// Saves one artifact on `task`. `InvalidInput` for bytes that do not fit the kind or an
    /// empty one, `TooLarge` over 5 MB, `Conflict` once the task holds 50.
    pub async fn add_artifact(&self, task: &str, new: NewArtifact) -> Result<Artifact> {
        let (home, task) = (self.home().clone(), task.to_string());
        self.call(move |conn| insert(conn, &home, &task, new)).await
    }

    /// The artifacts of `task` (ignoring case), oldest first.
    pub async fn artifacts(&self, task: &str) -> Result<Vec<Artifact>> {
        let task = task.to_string();
        self.call(move |conn| {
            let found = find(conn, &task)?.ok_or_else(|| Error::missing("task", &task))?;
            list(conn, &found.name)
        })
        .await
    }

    /// The record and the path of its bytes.
    pub async fn artifact(&self, task: &str, id: &str) -> Result<(Artifact, PathBuf)> {
        ids::validate_id("artifact", id)?;
        let found = self
            .artifacts(task)
            .await?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| Error::missing("artifact", id))?;
        let dir = self
            .home()
            .artifacts_for(&found.conversation_id, &found.task);
        let path = dir.join(file_of(&found));
        Ok((found, path))
    }
}
