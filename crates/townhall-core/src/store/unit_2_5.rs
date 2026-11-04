//! Unit 2.5 agent states: the last state the agent watcher acted on, per repo and agent, so
//! a restarted daemon neither repeats thread entries nor publishes unchanged states again.
//! Detection never reads it: live state comes from tmux and the transcript. Migrations 0500
//! to 0599, files `migrations/05xx_<name>.sql`.

use rusqlite::params;

use super::codec::{from_json, to_json};
use super::migrate::Migration;
use super::Store;
use crate::clock;
use crate::model::AgentState;
use crate::Result;

pub const MIGRATIONS: &[Migration] = &[Migration {
    id: 501,
    name: "0501_agent_states",
    sql: include_str!("migrations/0501_agent_states.sql"),
}];

/// A remembered state: `(repo, agent, state)`.
pub type SeenState = (String, String, AgentState);

impl Store {
    /// Every remembered agent state, by repo and agent name.
    pub async fn seen_agent_states(&self) -> Result<Vec<SeenState>> {
        self.call(|conn| {
            let mut stmt =
                conn.prepare("select repo, agent, state from agent_states order by repo, agent")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?)))?
                .collect::<rusqlite::Result<Vec<(String, String, String)>>>()?;
            rows.into_iter()
                .map(|(repo, agent, state)| {
                    Ok((repo, agent, from_json("agent_states.state", &state)?))
                })
                .collect()
        })
        .await
    }

    /// Remembers `state` as the one the watcher acted on for `agent` in `repo`.
    pub async fn see_agent_state(&self, repo: &str, agent: &str, state: &AgentState) -> Result<()> {
        let (repo, agent, state) = (repo.to_string(), agent.to_string(), to_json(state)?);
        self.call(move |conn| {
            conn.execute(
                "insert into agent_states (repo, agent, state, updated_at) values (?1, ?2, ?3, ?4)
                 on conflict (repo, agent) do update set state = excluded.state,
                 updated_at = excluded.updated_at",
                params![repo, agent, state, clock::now_rfc3339()],
            )?;
            Ok(())
        })
        .await
    }

    /// Forgets `agent` in `repo`, once it no longer exists.
    pub async fn forget_agent_state(&self, repo: &str, agent: &str) -> Result<()> {
        let (repo, agent) = (repo.to_string(), agent.to_string());
        self.call(move |conn| {
            conn.execute(
                "delete from agent_states where repo = ?1 and agent = ?2",
                params![repo, agent],
            )?;
            Ok(())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::events::EventBus;
    use crate::model::AgentState;
    use crate::store::Store;
    use crate::Home;

    #[tokio::test]
    async fn remembers_updates_and_forgets_agent_states() {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path().join("h"));
        let store = Store::open(&home, EventBus::new()).unwrap();
        let waiting = AgentState::Waiting {
            question: "ok?".into(),
        };
        store.see_agent_state("shop", "a", &waiting).await.unwrap();
        store
            .see_agent_state("Shop", "b", &AgentState::Ended)
            .await
            .unwrap();
        let failed = AgentState::Error { exit_code: Some(3) };
        store.see_agent_state("SHOP", "A", &failed).await.unwrap();
        let seen = store.seen_agent_states().await.unwrap();
        assert_eq!(
            seen,
            vec![
                ("shop".into(), "a".into(), failed),
                ("Shop".into(), "b".into(), AgentState::Ended),
            ]
        );
        store.forget_agent_state("shop", "B").await.unwrap();
        drop(store);
        let reopened = Store::open(&home, EventBus::new()).unwrap();
        assert_eq!(reopened.seen_agent_states().await.unwrap().len(), 1);
    }
}
