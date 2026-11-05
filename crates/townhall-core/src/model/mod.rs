//! The API contract: every type the daemon sends or stores, serde camelCase.
//! Mirrored field for field in `web/src/api/types.ts`; see `docs/daemon-api.md`.
//! An `Option` is always present on the wire, as `null` when empty.

mod agent;
mod artifact;
mod attachment;
mod conversation;
mod event;
mod library;
mod repo;
mod settings;
mod task;
mod thread;

pub use agent::{AgentInfo, AgentKey, AgentState};
pub use artifact::{
    Artifact, ArtifactKind, AuthorRole, NewArtifact, MAX_ARTIFACTS_PER_TASK, MAX_ARTIFACT_BYTES,
    MAX_CAPTION_CHARS,
};
pub use attachment::{Attachment, AttachmentKind, MAX_ATTACHMENT_BYTES};
pub use conversation::{
    ContextUsage, Conversation, ConversationDetail, ConversationPatch, ConversationStatus,
    NewMessage, NEW_CONVERSATION_TITLE,
};
pub use event::ServerEvent;
pub use library::{AgentType, AgentTypeSummary, AgentUsage, DEFAULT_AGENT_TYPE};
pub use repo::{AddRepo, Repo};
pub use settings::{
    NotifyWhen, PermissionMode, PutSetting, RepoUnclear, SettingKey, Settings, TownhallModel,
    AGENTS_AT_ONCE_DEFAULT, AGENTS_AT_ONCE_MAX, AGENTS_AT_ONCE_MIN,
};
pub use task::{
    Checks, CreateTaskInput, Handoff, NewTask, Pr, Task, TaskState, MAX_HANDOFFS,
    TASK_BRANCH_PREFIX,
};
pub use thread::{AgentEvent, EntryBody, RepoRule, ThreadEntry};

/// For `Option<Option<T>>` patch fields: a missing key is `None`, an explicit `null` is `Some(None)`.
pub(crate) fn present<'de, T, D>(d: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(d).map(Some)
}
