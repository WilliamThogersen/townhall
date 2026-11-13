use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionVerdict {
    Allow,
    Deny,
    AskUser,
    HardDeny,
    Timeout,
}

impl PermissionVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::AskUser => "ask-user",
            Self::HardDeny => "hard-deny",
            Self::Timeout => "timeout",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::Allow,
            Self::Deny,
            Self::AskUser,
            Self::HardDeny,
            Self::Timeout,
        ]
        .into_iter()
        .find(|v| v.as_str() == text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPermissionDecision {
    pub task: String,
    pub agent_type: String,
    pub command: String,
    pub pattern: String,
    pub verdict: PermissionVerdict,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDecision {
    pub id: i64,
    pub task: String,
    pub agent_type: String,
    pub command: String,
    pub pattern: String,
    pub verdict: PermissionVerdict,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionSuggestion {
    pub agent_type: String,
    pub tool: String,
    pub approvals: u32,
}
