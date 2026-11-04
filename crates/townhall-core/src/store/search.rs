//! `Store::search` (unit 3.3), read only. Conversations come newest activity first and stop at
//! the limit; each one's messages are read newest first until one matches. Matching happens in
//! Rust (`search::Needle`), not with `like`, so case folding covers every letter and `%` or `_`
//! in the search text are plain characters.

use rusqlite::{Connection, Statement};

use super::Store;
use crate::search::{Needle, SearchHit, SearchMatch, SearchQuery};
use crate::Result;

struct Candidate {
    id: String,
    title: String,
    updated_at: String,
}

fn candidate(r: &rusqlite::Row<'_>) -> rusqlite::Result<Candidate> {
    Ok(Candidate {
        id: r.get(0)?,
        title: r.get(1)?,
        updated_at: r.get(2)?,
    })
}

/// The newest of the conversation's messages that holds the needle: its id and snippet.
fn newest_match(
    messages: &mut Statement<'_>,
    conversation_id: &str,
    needle: &Needle,
) -> Result<Option<(u64, String)>> {
    let rows = messages.query_map([conversation_id], |r| {
        Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, text) = row?;
        if let Some(snippet) = needle.snippet(&text) {
            return Ok(Some((id, snippet)));
        }
    }
    Ok(None)
}

fn hit_for(
    messages: &mut Statement<'_>,
    found: Candidate,
    needle: &Needle,
) -> Result<Option<SearchHit>> {
    let hit = |matched, entry_id, snippet| SearchHit {
        conversation_id: found.id.clone(),
        title: found.title.clone(),
        matched,
        entry_id,
        snippet,
        updated_at: found.updated_at.clone(),
    };
    if needle.is_in(&found.title) {
        return Ok(Some(hit(SearchMatch::Title, None, None)));
    }
    let message = newest_match(messages, &found.id, needle)?;
    Ok(message.map(|(id, snippet)| hit(SearchMatch::Message, Some(id), Some(snippet))))
}

fn search_in(conn: &Connection, query: &SearchQuery) -> Result<Vec<SearchHit>> {
    let needle = Needle::new(query.text());
    let mut conversations = conn.prepare(
        "select id, title, updated_at from conversations
         order by updated_at desc, created_at desc",
    )?;
    let mut messages =
        conn.prepare("select id, text from messages where conversation_id = ?1 order by id desc")?;
    let hits = conversations
        .query_map([], candidate)?
        .filter_map(|row| match row {
            Ok(found) => hit_for(&mut messages, found, &needle).transpose(),
            Err(e) => Some(Err(e.into())),
        })
        .take(query.limit())
        .collect();
    hits
}

impl Store {
    /// One hit per conversation whose title or one of whose `you` and `townhall` entries holds
    /// the text, ignoring case and runs of whitespace. A title match wins; else the newest
    /// matching entry with a one-line snippet. Newest activity first, at most `query.limit()`.
    pub async fn search(&self, query: SearchQuery) -> Result<Vec<SearchHit>> {
        self.call(move |conn| search_in(conn, &query)).await
    }
}
