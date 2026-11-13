use rusqlite::params;

use super::migrate::Migration;
use super::Store;
use crate::model::{
    NewPermissionDecision, PermissionDecision, PermissionSuggestion, PermissionVerdict,
};
use crate::redact::redact;
use crate::{clock, Error, Result};

pub const MIGRATIONS: &[Migration] = &[Migration {
    id: 1100,
    name: "1100_permission_decisions",
    sql: include_str!("migrations/1100_permission_decisions.sql"),
}];

pub const SUGGEST_AFTER: u32 = 3;

impl Store {
    pub async fn record_permission_decision(&self, d: NewPermissionDecision) -> Result<()> {
        let command = redact(&d.command).to_string();
        let reason = redact(&d.reason).to_string();
        self.call(move |conn| {
            conn.execute(
                "insert into permission_decisions
                   (task, agent_type, command, pattern, verdict, reason, created_at)
                 values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    d.task,
                    d.agent_type,
                    command,
                    d.pattern,
                    d.verdict.as_str(),
                    reason,
                    clock::now_rfc3339()
                ],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn permission_decisions(&self, limit: u32) -> Result<Vec<PermissionDecision>> {
        self.call(move |conn| {
            let mut stmt = conn.prepare(
                "select id, task, agent_type, command, pattern, verdict, reason, created_at
                 from permission_decisions order by id desc limit ?1",
            )?;
            let rows = stmt.query_map([limit], |r| {
                Ok((
                    PermissionDecision {
                        id: r.get(0)?,
                        task: r.get(1)?,
                        agent_type: r.get(2)?,
                        command: r.get(3)?,
                        pattern: r.get(4)?,
                        verdict: PermissionVerdict::Allow,
                        reason: r.get(6)?,
                        created_at: r.get(7)?,
                    },
                    r.get::<_, String>(5)?,
                ))
            })?;
            let mut out = Vec::new();
            for row in rows {
                let (mut decision, verdict) = row?;
                decision.verdict = PermissionVerdict::parse(&verdict).ok_or_else(|| {
                    Error::InvalidInput(format!("unknown permission verdict `{verdict}`"))
                })?;
                out.push(decision);
            }
            Ok(out)
        })
        .await
    }

    pub async fn permission_suggestions(&self) -> Result<Vec<PermissionSuggestion>> {
        let found = self
            .call(|conn| {
                let mut stmt = conn.prepare(
                    "select agent_type, pattern, count(*) from permission_decisions
                     where verdict = 'allow' and pattern <> ''
                     group by agent_type collate nocase, pattern
                     having count(*) >= ?1 order by count(*) desc, agent_type, pattern",
                )?;
                let rows = stmt.query_map([SUGGEST_AFTER], |r| {
                    Ok(PermissionSuggestion {
                        agent_type: r.get(0)?,
                        tool: r.get(1)?,
                        approvals: r.get(2)?,
                    })
                })?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(Into::into)
            })
            .await?;
        let mut open = Vec::new();
        for s in found {
            let tools = match self.agent_type(&s.agent_type).await? {
                Some(t) => t.def.tools,
                None => continue,
            };
            if !tools.iter().any(|t| t.trim() == s.tool) {
                open.push(s);
            }
        }
        Ok(open)
    }
}
