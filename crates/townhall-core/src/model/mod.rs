//! The API contract: every type the daemon sends or stores, serde camelCase.
//! Mirrored field for field in `web/src/api/types.ts`; see `docs/daemon-api.md`.
//! An `Option` is always present on the wire, as `null` when empty.

mod agent;
mod attachment;
mod conversation;
mod event;
mod repo;
mod settings;
mod task;
mod thread;

pub use agent::{AgentInfo, AgentState};
pub use attachment::{Attachment, AttachmentKind, MAX_ATTACHMENT_BYTES};
pub use conversation::{
    ContextUsage, Conversation, ConversationDetail, ConversationPatch, ConversationStatus,
    NewMessage, NEW_CONVERSATION_TITLE,
};
pub use event::ServerEvent;
pub use repo::{AddRepo, Repo};
pub use settings::{
    NotifyWhen, PermissionMode, PutSetting, RepoUnclear, SettingKey, Settings,
    AGENTS_AT_ONCE_DEFAULT, AGENTS_AT_ONCE_MAX, AGENTS_AT_ONCE_MIN,
};
pub use task::{Checks, CreateTaskInput, NewTask, Pr, Task, TaskState, TASK_BRANCH_PREFIX};
pub use thread::{AgentEvent, EntryBody, RepoRule, ThreadEntry};

/// For `Option<Option<T>>` patch fields: a missing key is `None`, an explicit `null` is `Some(None)`.
pub(crate) fn present<'de, T, D>(d: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(d).map(Some)
}
