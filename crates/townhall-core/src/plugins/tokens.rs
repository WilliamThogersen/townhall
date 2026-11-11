use std::io::Read;

use super::sha256;
use crate::store::Store;
use crate::{Error, Result};

pub const TASK_SCOPES: [&str; 3] = ["tasks:read", "conversations:read", "storage"];

const CLAUDE_ONLY_SCOPES: [&str; 2] = ["tasks:create", "conversations:write"];

const API_PREFIX: &str = "api:";
const CLAUDE_API_PREFIX: &str = "claude.api:";
const TOKEN_BYTES: usize = 32;
const RANDOM: &str = "/dev/urandom";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub plugin: String,
    pub task: Option<String>,
    pub scopes: Vec<String>,
}

impl Caller {
    pub fn allows(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

pub fn hash(token: &str) -> String {
    sha256::hex(token.as_bytes())
}

fn generate() -> Result<String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    std::fs::File::open(RANDOM)
        .and_then(|mut source| source.read_exact(&mut bytes))
        .map_err(|e| Error::io(format!("read {RANDOM}"), e))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub async fn issue(store: &Store, plugin_id: &str, task: Option<&str>) -> Result<String> {
    let plugin = store.plugin(plugin_id).await?;
    let token = generate()?;
    store
        .add_plugin_token(&plugin.id, task, &hash(&token))
        .await?;
    Ok(token)
}

pub async fn revoke_plugin(store: &Store, plugin_id: &str) -> Result<()> {
    store.revoke_plugin_tokens(plugin_id).await
}

pub async fn revoke_task(store: &Store, task: &str) -> Result<()> {
    store.revoke_task_tokens(task).await
}

pub async fn authenticate(store: &Store, token: &str) -> Result<Option<Caller>> {
    let Some(row) = store.live_plugin_token(&hash(token)).await? else {
        return Ok(None);
    };
    let plugin = match store.plugin(&row.plugin_id).await {
        Ok(plugin) => plugin,
        Err(Error::Missing { .. }) => return Ok(None),
        Err(e) => return Err(e),
    };
    if !plugin.enabled {
        return Ok(None);
    }
    let approved: Vec<String> =
        serde_json::from_value(plugin.approved).map_err(|e| Error::Corrupt {
            path: "plugins.approved".into(),
            message: e.to_string(),
        })?;
    let mut scopes: Vec<String> = approved
        .iter()
        .filter_map(|item| item.strip_prefix(API_PREFIX))
        .map(str::to_string)
        .collect();
    if row.task.is_some() {
        scopes.retain(|s| TASK_SCOPES.contains(&s.as_str()));
        for scope in CLAUDE_ONLY_SCOPES {
            let asked = format!("{CLAUDE_API_PREFIX}{scope}");
            if approved.contains(&asked) {
                scopes.push(scope.to_string());
            }
        }
    }
    Ok(Some(Caller {
        plugin: plugin.id,
        task: row.task,
        scopes,
    }))
}
