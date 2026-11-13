use rusqlite::{params, Connection};

use super::codec::{from_json, name_of, parse_name, to_json};
use super::tasks::find;
use super::Store;
use crate::model::{PrStatus, ServerEvent, TrackedPr};
use crate::{clock, Error, Result};

const COLUMNS: &str = "repo, number, url, branch, base, owner, pusher, head, status, detail, \
                       reruns, woke_for, seen_pending, updated_at";

type Row = (TrackedPr, String, String);

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    let pr = TrackedPr {
        repo: r.get(0)?,
        number: r.get(1)?,
        url: r.get(2)?,
        branch: r.get(3)?,
        base: r.get(4)?,
        owner: r.get(5)?,
        pusher: r.get(6)?,
        head: r.get(7)?,
        status: PrStatus::Pending,
        detail: r.get(9)?,
        reruns: vec![],
        woke_for: r.get(11)?,
        seen_pending: r.get(12)?,
        updated_at: r.get(13)?,
    };
    Ok((pr, r.get(8)?, r.get(10)?))
}

fn build((pr, status, reruns): Row) -> Result<TrackedPr> {
    Ok(TrackedPr {
        status: parse_name("tracked_prs.status", &status)?,
        reruns: from_json("tracked_prs.reruns", &reruns)?,
        ..pr
    })
}

pub(super) fn all(conn: &Connection) -> Result<Vec<TrackedPr>> {
    let mut stmt = conn.prepare(&format!(
        "select {COLUMNS} from tracked_prs order by updated_at, rowid"
    ))?;
    let rows = stmt
        .query_map([], row)?
        .collect::<rusqlite::Result<Vec<Row>>>()?;
    rows.into_iter().map(build).collect()
}

fn get(conn: &Connection, repo: &str, number: u64) -> Result<Option<TrackedPr>> {
    Ok(all(conn)?
        .into_iter()
        .find(|p| p.repo == repo && p.number == number))
}

fn write(conn: &Connection, p: &TrackedPr) -> Result<()> {
    conn.execute(
        "insert into tracked_prs (repo, number, url, branch, base, owner, pusher, head, status,
         detail, reruns, woke_for, seen_pending, updated_at)
         values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         on conflict (repo, number) do update set url = ?3, branch = ?4, base = ?5, owner = ?6,
         pusher = ?7, head = ?8, status = ?9, detail = ?10, reruns = ?11, woke_for = ?12,
         seen_pending = ?13, updated_at = ?14",
        params![
            p.repo,
            p.number,
            p.url,
            p.branch,
            p.base,
            p.owner,
            p.pusher,
            p.head,
            name_of(&p.status),
            p.detail,
            to_json(&p.reruns)?,
            p.woke_for,
            p.seen_pending,
            p.updated_at
        ],
    )?;
    Ok(())
}

fn publish(conn: &Connection, store: &Store, names: &[Option<&str>]) -> Result<()> {
    let mut told: Vec<String> = vec![];
    for name in names.iter().flatten().filter(|n| !n.is_empty()) {
        if told.iter().any(|t| t.eq_ignore_ascii_case(name)) {
            continue;
        }
        told.push((*name).to_string());
        if let Some(task) = find(conn, name)? {
            store.bus().publish(ServerEvent::TaskUpdated { task });
        }
    }
    Ok(())
}

impl Store {
    pub async fn track_pr(&self, pr: TrackedPr) -> Result<TrackedPr> {
        let store = self.clone();
        self.call(move |conn| {
            let tx = conn.transaction()?;
            let before = get(&tx, &pr.repo, pr.number)?;
            let next = match &before {
                Some(old) => TrackedPr {
                    url: pr.url.clone(),
                    owner: match old.owner.is_empty() {
                        true => pr.owner.clone(),
                        false => old.owner.clone(),
                    },
                    status: PrStatus::Pending,
                    head: None,
                    detail: None,
                    updated_at: clock::now_rfc3339(),
                    ..old.clone()
                },
                None => TrackedPr {
                    updated_at: clock::now_rfc3339(),
                    ..pr
                },
            };
            write(&tx, &next)?;
            tx.commit()?;
            let old_pusher = before.as_ref().and_then(|b| b.pusher.as_deref());
            publish(
                conn,
                &store,
                &[Some(&next.owner), next.pusher.as_deref(), old_pusher],
            )?;
            Ok(next)
        })
        .await
    }

    pub async fn open_prs(&self) -> Result<Vec<TrackedPr>> {
        self.call(|conn| {
            Ok(all(conn)?
                .into_iter()
                .filter(|p| !matches!(p.status, PrStatus::Merged | PrStatus::Closed))
                .collect())
        })
        .await
    }

    pub async fn update_pr<F>(&self, repo: &str, number: u64, change: F) -> Result<TrackedPr>
    where
        F: FnOnce(TrackedPr) -> Result<TrackedPr> + Send + 'static,
    {
        let (repo, store) = (repo.to_string(), self.clone());
        self.call(move |conn| {
            let tx = conn.transaction()?;
            let current = get(&tx, &repo, number)?
                .ok_or_else(|| Error::missing("pull request", format!("{repo}#{number}")))?;
            let old_pusher = current.pusher.clone();
            let next = TrackedPr {
                repo: current.repo.clone(),
                number,
                updated_at: clock::now_rfc3339(),
                ..change(current)?
            };
            write(&tx, &next)?;
            tx.commit()?;
            publish(
                conn,
                &store,
                &[
                    Some(&next.owner),
                    next.pusher.as_deref(),
                    old_pusher.as_deref(),
                ],
            )?;
            Ok(next)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::events::EventBus;
    use crate::model::{NewTask, PrStatus, ServerEvent, TrackedPr};
    use crate::store::Store;
    use crate::Home;

    fn new_task(name: &str) -> NewTask {
        NewTask {
            name: name.into(),
            title: format!("do {name}"),
            prompt: "p".into(),
            repo: "shop".into(),
            base_branch: "main".into(),
            after: vec![],
            attachments: vec![],
            agent: "lander".into(),
            pr_base: None,
            done: Default::default(),
        }
    }

    fn tracked(owner: &str) -> TrackedPr {
        TrackedPr {
            repo: "shop".into(),
            number: 7,
            url: Some("https://github.com/o/r/pull/7".into()),
            branch: format!("townhall/{owner}"),
            base: "main".into(),
            owner: owner.into(),
            pusher: None,
            head: None,
            status: PrStatus::Pending,
            detail: None,
            reruns: vec![],
            woke_for: None,
            seen_pending: false,
            updated_at: String::new(),
        }
    }

    async fn env() -> (tempfile::TempDir, EventBus, Store) {
        let dir = tempfile::tempdir().unwrap();
        let bus = EventBus::new();
        let store = Store::open(&Home::at(dir.path().join("h")), bus.clone()).unwrap();
        let c = store.create_conversation().await.unwrap();
        for name in ["alpha", "beta"] {
            store.create_task(&c.id, new_task(name)).await.unwrap();
        }
        (dir, bus, store)
    }

    #[tokio::test]
    async fn a_tracked_pr_is_listed_on_its_owner_and_its_pusher() {
        let (_dir, _bus, store) = env().await;
        store.track_pr(tracked("alpha")).await.unwrap();
        store
            .update_pr("shop", 7, |p| {
                Ok(TrackedPr {
                    pusher: Some("Beta".into()),
                    ..p
                })
            })
            .await
            .unwrap();
        for name in ["alpha", "beta"] {
            let task = store.find_task(name).await.unwrap().unwrap();
            assert_eq!(task.prs.len(), 1, "{name}");
            assert_eq!(task.prs[0].number, 7);
            assert!(!task.prs[0].updated_at.is_empty());
        }
    }

    #[tokio::test]
    async fn tracking_again_resets_the_row_and_keeps_its_owner() {
        let (_dir, _bus, store) = env().await;
        store.track_pr(tracked("alpha")).await.unwrap();
        store
            .update_pr("shop", 7, |p| {
                Ok(TrackedPr {
                    head: Some("abc".into()),
                    status: PrStatus::Red,
                    detail: Some("build".into()),
                    woke_for: Some("abc".into()),
                    ..p
                })
            })
            .await
            .unwrap();
        let again = store
            .track_pr(TrackedPr {
                url: Some("https://x/7".into()),
                ..tracked("beta")
            })
            .await
            .unwrap();
        assert_eq!(again.owner, "alpha");
        assert_eq!(again.url.as_deref(), Some("https://x/7"));
        assert_eq!(again.status, PrStatus::Pending);
        assert_eq!((again.head, again.detail), (None, None));
        assert_eq!(again.woke_for.as_deref(), Some("abc"));
    }

    #[tokio::test]
    async fn update_pr_publishes_a_task_update_for_the_owner_and_the_pusher() {
        let (_dir, bus, store) = env().await;
        store.track_pr(tracked("alpha")).await.unwrap();
        let mut events = bus.subscribe();
        store
            .update_pr("shop", 7, |p| {
                Ok(TrackedPr {
                    pusher: Some("beta".into()),
                    ..p
                })
            })
            .await
            .unwrap();
        let mut seen = vec![];
        while let Ok(ServerEvent::TaskUpdated { task }) = events.try_recv() {
            assert_eq!(task.prs.len(), 1);
            seen.push(task.name);
        }
        seen.sort();
        assert_eq!(seen, vec!["alpha", "beta"]);
    }

    #[tokio::test]
    async fn a_merged_or_closed_row_leaves_the_open_list() {
        let (_dir, _bus, store) = env().await;
        store.track_pr(tracked("alpha")).await.unwrap();
        assert_eq!(store.open_prs().await.unwrap().len(), 1);
        store
            .update_pr("shop", 7, |p| {
                Ok(TrackedPr {
                    status: PrStatus::Merged,
                    ..p
                })
            })
            .await
            .unwrap();
        assert!(store.open_prs().await.unwrap().is_empty());
        let task = store.find_task("alpha").await.unwrap().unwrap();
        assert_eq!(task.prs[0].status, PrStatus::Merged);
    }
}
