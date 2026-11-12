use std::collections::{HashMap, HashSet};

use townhall_core::model::{Conversation, Task};

use crate::repo_group::{group_by_repo, infer_repo, repo_of_conversation, ConversationRepo};
use crate::row_summary::{summarize_row, RowSummary};
use crate::sections::{split_sections, Filter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarRow {
    pub conversation: Conversation,
    pub summary: RowSummary,
    pub repo: ConversationRepo,
    pub auto_repo: ConversationRepo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowSections {
    pub active: Vec<SidebarRow>,
    pub older: Vec<SidebarRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupBlock {
    pub key: String,
    pub label: String,
    pub count: usize,
    pub open: bool,
    pub rows: Vec<SidebarRow>,
}

pub struct FoldState<'a> {
    pub folded: &'a HashSet<String>,
    pub current_id: Option<&'a str>,
    pub searching: bool,
}

pub fn build_sections(
    conversations: &[Conversation],
    tasks: &[Task],
    filter: Option<&Filter>,
) -> RowSections {
    let mut owned: HashMap<&str, Vec<Task>> = HashMap::new();
    for task in tasks {
        owned
            .entry(task.conversation_id.as_str())
            .or_default()
            .push(task.clone());
    }
    let rows: HashMap<&str, SidebarRow> = conversations
        .iter()
        .map(|conversation| {
            let own = owned
                .get(conversation.id.as_str())
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let row = SidebarRow {
                conversation: conversation.clone(),
                summary: summarize_row(conversation, own),
                repo: repo_of_conversation(conversation, own),
                auto_repo: infer_repo(own),
            };
            (conversation.id.as_str(), row)
        })
        .collect();
    let sections = split_sections(
        conversations,
        |c| rows[c.id.as_str()].summary.needs_you,
        filter,
    );
    let row_of = |c: &Conversation| rows[c.id.as_str()].clone();
    RowSections {
        active: sections.active.iter().map(row_of).collect(),
        older: sections.older.iter().map(row_of).collect(),
    }
}

fn shown_rows(rows: Vec<SidebarRow>, open: bool, current_id: Option<&str>) -> Vec<SidebarRow> {
    if open {
        return rows;
    }
    rows.into_iter()
        .filter(|row| Some(row.conversation.id.as_str()) == current_id)
        .collect()
}

pub fn repo_blocks(active: &[SidebarRow], fold: &FoldState) -> Vec<GroupBlock> {
    group_by_repo(active.to_vec(), |row| row.repo.clone())
        .into_iter()
        .map(|group| {
            let open = fold.searching || !fold.folded.contains(&group.key);
            GroupBlock {
                count: group.items.len(),
                rows: shown_rows(group.items, open, fold.current_id),
                key: group.key,
                label: group.label,
                open,
            }
        })
        .collect()
}

pub fn visible_ids(sections: &RowSections, blocks: Option<&[GroupBlock]>) -> Vec<String> {
    let active: Vec<&SidebarRow> = match blocks {
        Some(blocks) => blocks.iter().flat_map(|b| b.rows.iter()).collect(),
        None => sections.active.iter().collect(),
    };
    active
        .into_iter()
        .chain(sections.older.iter())
        .map(|row| row.conversation.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{conversation, task};
    use townhall_core::model::TaskState;

    fn at(minute: u32) -> String {
        format!("2026-11-04T10:{minute:02}:00.000Z")
    }

    fn conversations() -> Vec<Conversation> {
        let mut delta = conversation("d", "delta", &at(6));
        delta.ended = true;
        vec![
            conversation("a", "alpha", &at(9)),
            conversation("b", "beta", &at(8)),
            conversation("c", "gamma", &at(7)),
            delta,
        ]
    }

    fn tasks() -> Vec<Task> {
        let mk = |name: &str, conv: &str, repo: &str, state| {
            let mut t = task(name, state);
            t.conversation_id = conv.into();
            t.repo = repo.into();
            t
        };
        vec![
            mk("t1", "a", "shop", TaskState::Working),
            mk("t2", "b", "site", TaskState::Working),
            mk("t3", "c", "shop", TaskState::NeedsYou),
        ]
    }

    fn ids(rows: &[SidebarRow]) -> Vec<&str> {
        rows.iter().map(|r| r.conversation.id.as_str()).collect()
    }

    #[test]
    fn each_row_is_summarized_from_its_own_tasks_and_what_needs_you_comes_first() {
        let s = build_sections(&conversations(), &tasks(), None);
        assert_eq!(ids(&s.active), ["c", "a", "b"]);
        assert_eq!(ids(&s.older), ["d"]);
        assert_eq!(s.active[0].summary.status, "t3 needs you");
        assert_eq!(s.active[1].repo, ConversationRepo::Repo("shop".into()));
        assert_eq!(s.older[0].repo, ConversationRepo::None);
    }

    #[test]
    fn the_search_filters_the_rows() {
        let matches: HashSet<String> = ["a".to_string()].into();
        let filter = Filter {
            query: "be",
            text_matches: &matches,
        };
        let s = build_sections(&conversations(), &tasks(), Some(&filter));
        assert_eq!(ids(&s.active), ["a", "b"]);
    }

    #[test]
    fn active_rows_group_by_repo_and_a_folded_group_shrinks_to_the_current_row() {
        let s = build_sections(&conversations(), &tasks(), None);
        let folded: HashSet<String> = ["repo:shop".to_string()].into();
        let blocks = repo_blocks(
            &s.active,
            &FoldState {
                folded: &folded,
                current_id: Some("a"),
                searching: false,
            },
        );
        let shown: Vec<(&str, usize, bool, Vec<&str>)> = blocks
            .iter()
            .map(|b| (b.label.as_str(), b.count, b.open, ids(&b.rows)))
            .collect();
        assert_eq!(
            shown,
            [("shop", 2, false, vec!["a"]), ("site", 1, true, vec!["b"])]
        );
        assert_eq!(visible_ids(&s, Some(&blocks)), ["a", "b", "d"]);
    }

    #[test]
    fn every_group_is_open_while_searching() {
        let s = build_sections(&conversations(), &tasks(), None);
        let folded: HashSet<String> = ["repo:shop".to_string()].into();
        let blocks = repo_blocks(
            &s.active,
            &FoldState {
                folded: &folded,
                current_id: None,
                searching: true,
            },
        );
        assert_eq!(blocks[0].rows.len(), 2);
    }

    #[test]
    fn the_flat_order_has_no_groups() {
        let s = build_sections(&conversations(), &tasks(), None);
        assert_eq!(visible_ids(&s, None), ["c", "a", "b", "d"]);
    }
}
