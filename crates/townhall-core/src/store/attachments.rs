//! Attachment records in `attachments`; the bytes are the only files townhall keeps:
//! `~/.townhall/conversations/<conversation id>/files/<attachment id>/<name>`.

use std::path::PathBuf;

use rusqlite::{params, Connection};

use super::codec::{name_of, parse_name};
use super::conversations::get;
use super::Store;
use crate::fsutil::{create_dir_new, write_atomic};
use crate::model::{Attachment, AttachmentKind, MAX_ATTACHMENT_BYTES};
use crate::{clock, ids, Error, Home, Result};

const MAX_NAME_BYTES: usize = 200;
const MAX_MIME_LEN: usize = 255;
const DEFAULT_NAME: &str = "file";
const DEFAULT_MIME: &str = "application/octet-stream";
const COLUMNS: &str = "id, conversation_id, name, mime, size, kind, created_at";

/// The last path component, without control characters, at most 200 bytes.
pub(crate) fn safe_file_name(raw: &str) -> String {
    let last = raw.rsplit(['/', '\\']).next().unwrap_or_default();
    let clean: String = last.chars().filter(|c| !c.is_control()).collect();
    let mut cut = String::new();
    for c in clean.trim().chars() {
        if cut.len() + c.len_utf8() > MAX_NAME_BYTES {
            break;
        }
        cut.push(c);
    }
    match cut.as_str() {
        "" | "." | ".." => DEFAULT_NAME.to_string(),
        _ => cut,
    }
}

fn safe_mime(raw: &str) -> String {
    let raw = raw.trim();
    let ok = !raw.is_empty()
        && raw.len() <= MAX_MIME_LEN
        && raw.chars().all(|c| c.is_ascii_graphic() || c == ' ');
    if ok {
        raw.to_ascii_lowercase()
    } else {
        DEFAULT_MIME.to_string()
    }
}

fn folder(home: &Home, conversation_id: &str, id: &str) -> PathBuf {
    home.root()
        .join("conversations")
        .join(conversation_id)
        .join("files")
        .join(id)
}

fn list(conn: &Connection, conversation_id: &str) -> Result<Vec<Attachment>> {
    let sql = format!(
        "select {COLUMNS} from attachments where conversation_id = ?1 order by created_at, rowid"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([conversation_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, u64>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
        ))
    })?;
    let mut out = vec![];
    for row in rows {
        let (id, conversation_id, name, mime, size, kind, created_at) = row?;
        let kind: AttachmentKind = parse_name("attachments.kind", &kind)?;
        out.push(Attachment {
            id,
            conversation_id,
            name,
            mime,
            size,
            kind,
            created_at,
        });
    }
    Ok(out)
}

fn store_bytes(home: &Home, attachment: &Attachment, bytes: &[u8]) -> Result<PathBuf> {
    let dir = folder(home, &attachment.conversation_id, &attachment.id);
    if let Some(parent) = dir.parent() {
        home.create_dir(parent)?;
    }
    create_dir_new(&dir).map_err(|e| Error::io(format!("create {}", dir.display()), e))?;
    write_atomic(&dir.join(&attachment.name), bytes, false)?;
    Ok(dir)
}

fn record(conn: &Connection, a: &Attachment) -> Result<()> {
    conn.execute(
        "insert into attachments (id, conversation_id, name, mime, size, kind, created_at)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            a.id,
            a.conversation_id,
            a.name,
            a.mime,
            a.size,
            name_of(&a.kind),
            a.created_at
        ],
    )?;
    Ok(())
}

impl Store {
    /// Writes the bytes, then the record. `TooLarge` over 20 MB; the name loses any folders.
    pub async fn save_attachment(
        &self,
        conversation_id: &str,
        name: &str,
        mime: &str,
        bytes: Vec<u8>,
    ) -> Result<Attachment> {
        let name = safe_file_name(name);
        let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if size > MAX_ATTACHMENT_BYTES {
            return Err(Error::TooLarge(format!("{name} is over 20 MB")));
        }
        let mime = safe_mime(mime);
        let attachment = Attachment {
            id: ids::new_id(),
            conversation_id: conversation_id.to_string(),
            kind: AttachmentKind::of_mime(&mime),
            name,
            mime,
            size,
            created_at: clock::now_rfc3339(),
        };
        let home = self.home().clone();
        self.call(move |conn| {
            get(conn, &attachment.conversation_id)?;
            let dir = store_bytes(&home, &attachment, &bytes)?;
            if let Err(e) = record(conn, &attachment) {
                if let Err(cleanup) = std::fs::remove_dir_all(&dir) {
                    eprintln!("warn: cannot remove {}: {cleanup}", dir.display());
                }
                return Err(e);
            }
            Ok(attachment)
        })
        .await
    }

    pub async fn attachments(&self, conversation_id: &str) -> Result<Vec<Attachment>> {
        let cid = conversation_id.to_string();
        self.call(move |conn| {
            get(conn, &cid)?;
            list(conn, &cid)
        })
        .await
    }

    /// The record and the path of its bytes.
    pub async fn attachment(
        &self,
        conversation_id: &str,
        id: &str,
    ) -> Result<(Attachment, PathBuf)> {
        ids::validate_id("attachment", id)?;
        let found = self
            .attachments(conversation_id)
            .await?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| Error::missing("attachment", id))?;
        let path = folder(self.home(), conversation_id, id).join(&found.name);
        Ok((found, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_their_folders_and_controls() {
        assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_file_name("C:\\x\\shot.png"), "shot.png");
        assert_eq!(safe_file_name("a\u{0}b\n.txt"), "ab.txt");
        assert_eq!(safe_file_name(".."), "file");
        assert_eq!(safe_file_name(&"é".repeat(150)).len(), 200);
    }

    #[test]
    fn mime_falls_back() {
        assert_eq!(safe_mime("Image/PNG"), "image/png");
        assert_eq!(safe_mime(""), DEFAULT_MIME);
        assert_eq!(safe_mime("a\nb"), DEFAULT_MIME);
    }
}
