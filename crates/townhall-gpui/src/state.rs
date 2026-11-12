use townhall_core::model::{
    Conversation, ConversationDetail, Repo, ServerEvent, Task, ThreadEntry,
};

use crate::client::Info;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Load<T> {
    Loading,
    Ready(T),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refetch {
    Nothing,
    Everything,
}

#[derive(Debug)]
pub struct State {
    pub conversations: Load<Vec<Conversation>>,
    pub selected: Option<String>,
    pub detail: Load<ConversationDetail>,
    pub tasks: Load<Vec<Task>>,
    pub repos: Load<Vec<Repo>>,
    pub info: Option<Info>,
}

impl State {
    pub fn new() -> Self {
        Self {
            conversations: Load::Loading,
            selected: None,
            detail: Load::Loading,
            tasks: Load::Loading,
            repos: Load::Loading,
            info: None,
        }
    }

    pub fn set_tasks(&mut self, result: Result<Vec<Task>, String>) {
        self.tasks = match result {
            Ok(tasks) => Load::Ready(tasks),
            Err(e) => Load::Failed(e),
        };
    }

    pub fn set_repos(&mut self, result: Result<Vec<Repo>, String>) {
        self.repos = match result {
            Ok(repos) => Load::Ready(repos),
            Err(e) => Load::Failed(e),
        };
    }

    pub fn set_info(&mut self, result: Result<Info, String>) {
        self.info = result.ok();
    }

    pub fn task_list(&self) -> &[Task] {
        match &self.tasks {
            Load::Ready(tasks) => tasks,
            _ => &[],
        }
    }

    pub fn repo_list(&self) -> &[Repo] {
        match &self.repos {
            Load::Ready(repos) => repos,
            _ => &[],
        }
    }

    pub fn user_name(&self) -> Option<&str> {
        self.info.as_ref().and_then(|i| i.user_name.as_deref())
    }

    pub fn set_list(&mut self, result: Result<Vec<Conversation>, String>) {
        self.conversations = match result {
            Ok(mut list) => {
                sort_newest_first(&mut list);
                Load::Ready(list)
            }
            Err(e) => Load::Failed(e),
        };
    }

    pub fn newest(&self) -> Option<String> {
        match &self.conversations {
            Load::Ready(list) => list.first().map(|c| c.id.clone()),
            _ => None,
        }
    }

    pub fn select(&mut self, id: String) {
        self.selected = Some(id);
        self.detail = Load::Loading;
    }

    pub fn set_detail(&mut self, id: &str, result: Result<ConversationDetail, String>) {
        if self.selected.as_deref() != Some(id) {
            return;
        }
        self.detail = match result {
            Ok(detail) => Load::Ready(detail),
            Err(e) => Load::Failed(e),
        };
    }

    pub fn apply(&mut self, event: ServerEvent) -> Refetch {
        match event {
            ServerEvent::ConversationUpdated { conversation } => {
                self.upsert_conversation(conversation);
                Refetch::Nothing
            }
            ServerEvent::EntryAppended {
                conversation_id,
                entry,
            } => {
                self.upsert_entry(&conversation_id, entry);
                Refetch::Nothing
            }
            ServerEvent::TaskUpdated { task } => {
                if let Load::Ready(tasks) = &mut self.tasks {
                    match tasks.iter_mut().find(|t| t.name == task.name) {
                        Some(slot) => *slot = task.clone(),
                        None => tasks.push(task.clone()),
                    }
                }
                if let Load::Ready(detail) = &mut self.detail {
                    if detail.conversation.id == task.conversation_id {
                        match detail.tasks.iter_mut().find(|t| t.name == task.name) {
                            Some(slot) => *slot = task,
                            None => detail.tasks.push(task),
                        }
                    }
                }
                Refetch::Nothing
            }
            ServerEvent::AgentUpdated { .. } => Refetch::Nothing,
            ServerEvent::Resync => Refetch::Everything,
        }
    }

    fn upsert_conversation(&mut self, conversation: Conversation) {
        if let Load::Ready(detail) = &mut self.detail {
            if detail.conversation.id == conversation.id {
                detail.conversation = conversation.clone();
            }
        }
        if let Load::Ready(list) = &mut self.conversations {
            match list.iter_mut().find(|c| c.id == conversation.id) {
                Some(slot) => *slot = conversation,
                None => list.push(conversation),
            }
            sort_newest_first(list);
        }
    }

    fn upsert_entry(&mut self, conversation_id: &str, entry: ThreadEntry) {
        let Load::Ready(detail) = &mut self.detail else {
            return;
        };
        if detail.conversation.id != conversation_id {
            return;
        }
        match detail.entries.iter_mut().find(|e| e.id == entry.id) {
            Some(slot) => *slot = entry,
            None => {
                detail.entries.push(entry);
                detail.entries.sort_by_key(|e| e.id);
            }
        }
    }
}

fn sort_newest_first(list: &mut [Conversation]) {
    list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use serde_json::json;
    use townhall_core::model::{EntryBody, TaskState};

    pub fn conversation(id: &str, title: &str, updated: &str) -> Conversation {
        serde_json::from_value(json!({
            "id": id,
            "sessionId": "s",
            "title": title,
            "status": "yourTurn",
            "tasks": [],
            "linkedRepo": null,
            "ended": false,
            "createdAt": "2026-11-12T09:00:00.000Z",
            "updatedAt": updated
        }))
        .unwrap()
    }

    pub fn entry(id: u64, text: &str) -> ThreadEntry {
        ThreadEntry {
            id,
            at: "2026-11-12T10:00:00.000Z".into(),
            body: EntryBody::Townhall { text: text.into() },
        }
    }

    pub fn detail(id: &str, entries: Vec<ThreadEntry>) -> ConversationDetail {
        ConversationDetail {
            conversation: conversation(id, "t", "2026-11-12T10:00:00.000Z"),
            entries,
            tasks: vec![],
            attachments: vec![],
        }
    }

    fn ready() -> State {
        let mut s = State::new();
        s.set_list(Ok(vec![
            conversation("old", "old one", "2026-11-12T08:00:00.000Z"),
            conversation("new", "new one", "2026-11-12T10:00:00.000Z"),
        ]));
        s.select("new".into());
        s.set_detail("new", Ok(detail("new", vec![entry(1, "one")])));
        s
    }

    fn ids(s: &State) -> Vec<String> {
        match &s.conversations {
            Load::Ready(list) => list.iter().map(|c| c.id.clone()).collect(),
            other => panic!("{other:?}"),
        }
    }

    fn entry_ids(s: &State) -> Vec<u64> {
        match &s.detail {
            Load::Ready(d) => d.entries.iter().map(|e| e.id).collect(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn list_is_sorted_newest_first() {
        let s = ready();
        assert_eq!(ids(&s), ["new", "old"]);
        assert_eq!(s.newest().as_deref(), Some("new"));
    }

    #[test]
    fn conversation_updated_replaces_and_resorts() {
        let mut s = ready();
        let refetch = s.apply(ServerEvent::ConversationUpdated {
            conversation: conversation("old", "renamed", "2026-11-12T11:00:00.000Z"),
        });
        assert_eq!(refetch, Refetch::Nothing);
        assert_eq!(ids(&s), ["old", "new"]);
    }

    #[test]
    fn conversation_updated_adds_an_unknown_conversation() {
        let mut s = ready();
        s.apply(ServerEvent::ConversationUpdated {
            conversation: conversation("fresh", "f", "2026-11-12T12:00:00.000Z"),
        });
        assert_eq!(ids(&s), ["fresh", "new", "old"]);
    }

    #[test]
    fn conversation_updated_refreshes_the_open_thread_header() {
        let mut s = ready();
        s.apply(ServerEvent::ConversationUpdated {
            conversation: conversation("new", "renamed", "2026-11-12T11:00:00.000Z"),
        });
        match &s.detail {
            Load::Ready(d) => assert_eq!(d.conversation.title, "renamed"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn entry_appended_adds_in_id_order_and_replaces_a_known_id() {
        let mut s = ready();
        s.apply(ServerEvent::EntryAppended {
            conversation_id: "new".into(),
            entry: entry(3, "three"),
        });
        s.apply(ServerEvent::EntryAppended {
            conversation_id: "new".into(),
            entry: entry(2, "two"),
        });
        assert_eq!(entry_ids(&s), [1, 2, 3]);
        s.apply(ServerEvent::EntryAppended {
            conversation_id: "new".into(),
            entry: entry(2, "two again"),
        });
        assert_eq!(entry_ids(&s), [1, 2, 3]);
        match &s.detail {
            Load::Ready(d) => assert_eq!(d.entries[1], entry(2, "two again")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn entry_appended_for_another_conversation_is_ignored() {
        let mut s = ready();
        s.apply(ServerEvent::EntryAppended {
            conversation_id: "old".into(),
            entry: entry(9, "elsewhere"),
        });
        assert_eq!(entry_ids(&s), [1]);
    }

    fn named_tasks(s: &State) -> Vec<(String, TaskState)> {
        s.task_list()
            .iter()
            .map(|t| (t.name.clone(), t.state))
            .collect()
    }

    #[test]
    fn tasks_repos_and_info_are_kept_when_they_load_and_empty_when_they_do_not() {
        let mut s = State::new();
        assert!(s.task_list().is_empty() && s.repo_list().is_empty());
        assert_eq!(s.user_name(), None);
        s.set_tasks(Ok(vec![crate::testkit::task("a", TaskState::Working)]));
        s.set_repos(Ok(vec![crate::testkit::repo("shop", "main")]));
        s.set_info(Ok(Info {
            user_name: Some("Ada".into()),
        }));
        assert_eq!(named_tasks(&s), [("a".to_string(), TaskState::Working)]);
        assert_eq!(s.repo_list()[0].name, "shop");
        assert_eq!(s.user_name(), Some("Ada"));
        s.set_tasks(Err("down".into()));
        s.set_repos(Err("down".into()));
        s.set_info(Err("down".into()));
        assert!(s.task_list().is_empty() && s.repo_list().is_empty());
        assert_eq!(s.tasks, Load::Failed("down".into()));
        assert_eq!(s.user_name(), None);
    }

    #[test]
    fn task_updated_replaces_a_known_task_and_adds_an_unknown_one_in_the_global_list() {
        let mut s = ready();
        s.set_tasks(Ok(vec![crate::testkit::task("a", TaskState::Working)]));
        s.apply(ServerEvent::TaskUpdated {
            task: crate::testkit::task("a", TaskState::Merged),
        });
        s.apply(ServerEvent::TaskUpdated {
            task: crate::testkit::task("b", TaskState::Queued),
        });
        assert_eq!(
            named_tasks(&s),
            [
                ("a".to_string(), TaskState::Merged),
                ("b".to_string(), TaskState::Queued)
            ]
        );
    }

    #[test]
    fn a_task_update_before_the_task_list_loaded_is_not_lost_to_the_open_thread() {
        let mut s = ready();
        let mut task = crate::testkit::task("a", TaskState::Working);
        task.conversation_id = "new".into();
        s.apply(ServerEvent::TaskUpdated { task });
        match &s.detail {
            Load::Ready(d) => assert_eq!(d.tasks.len(), 1),
            other => panic!("{other:?}"),
        }
        assert!(s.task_list().is_empty());
    }

    #[test]
    fn resync_asks_for_everything() {
        let mut s = ready();
        assert_eq!(s.apply(ServerEvent::Resync), Refetch::Everything);
    }

    #[test]
    fn a_late_detail_for_a_deselected_conversation_is_dropped() {
        let mut s = ready();
        s.select("old".into());
        s.set_detail("new", Ok(detail("new", vec![])));
        assert_eq!(s.detail, Load::Loading);
    }
}
