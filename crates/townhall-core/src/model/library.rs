//! Saved agent types (2.12): how a task's agent runs. The TOML is the text people and the
//! conversation write; the parsed definition is 2.8a's `AgentDef`.

use serde::Serialize;

use crate::definitions::{AdapterKind, AgentDef, PermissionMode};

/// The type `create_task` picks when it is given none and a type of this name exists.
pub const DEFAULT_AGENT_TYPE: &str = "builder";

/// One saved agent type: the parsed definition, the TOML as written and when it was saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentType {
    #[serde(flatten)]
    pub def: AgentDef,
    pub source: String,
    pub updated_at: String,
}

/// One row of the library's agent list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTypeSummary {
    pub name: String,
    pub description: String,
    pub adapter: AdapterKind,
    pub model: Option<String>,
    pub permission_mode: Option<PermissionMode>,
    pub updated_at: String,
}

impl AgentType {
    pub fn summary(&self) -> AgentTypeSummary {
        AgentTypeSummary {
            name: self.def.name.clone(),
            description: self.def.description.clone(),
            adapter: self.def.adapter,
            model: self.def.model.clone(),
            permission_mode: self.def.permission_mode,
            updated_at: self.updated_at.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definitions::parse_agent;
    use serde_json::json;

    #[test]
    fn a_type_is_its_definition_plus_source_on_the_wire() {
        let source = "name = \"b\"\nadapter = \"claude\"\npermission_mode = \"acceptEdits\"\n";
        let t = AgentType {
            def: parse_agent(source).unwrap().def,
            source: source.into(),
            updated_at: "2026-11-05T10:00:00.000Z".into(),
        };
        assert_eq!(
            serde_json::to_value(&t).unwrap(),
            json!({"name": "b", "description": "", "adapter": "claude", "model": null,
                   "permissionMode": "acceptEdits", "tools": [], "prompt": null, "command": null,
                   "canHandBackTo": null, "requires": [], "source": source, "updatedAt": "2026-11-05T10:00:00.000Z"})
        );
        let keys: Vec<String> = serde_json::to_value(t.summary())
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(
            keys,
            [
                "adapter",
                "description",
                "model",
                "name",
                "permissionMode",
                "updatedAt"
            ]
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentUsage {
    pub agent: String,
    pub runs: u64,
    pub tokens: u64,
}
