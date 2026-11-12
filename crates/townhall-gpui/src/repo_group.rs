use townhall_core::model::{Conversation, Task};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversationRepo {
    Repo(String),
    Multiple,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoGroup<T> {
    pub key: String,
    pub label: String,
    pub items: Vec<T>,
}

pub const MULTIPLE_REPOS: &str = "multiple repos";
pub const NO_REPO: &str = "no repo";

const MULTIPLE_KEY: &str = "multiple";
const NONE_KEY: &str = "none";

pub fn infer_repo(tasks: &[Task]) -> ConversationRepo {
    let mut counts: Vec<(String, String, usize)> = Vec::new();
    for task in tasks {
        let key = task.repo.to_lowercase();
        match counts.iter_mut().find(|(k, _, _)| *k == key) {
            Some(seen) => seen.2 += 1,
            None => counts.push((key, task.repo.clone(), 1)),
        }
    }
    counts.sort_by(|a, b| b.2.cmp(&a.2));
    match counts.as_slice() {
        [] => ConversationRepo::None,
        [top, next, ..] if top.2 == next.2 => ConversationRepo::Multiple,
        [top, ..] => ConversationRepo::Repo(top.1.clone()),
    }
}

pub fn repo_of_conversation(conversation: &Conversation, tasks: &[Task]) -> ConversationRepo {
    match &conversation.linked_repo {
        Some(name) => ConversationRepo::Repo(name.clone()),
        None => infer_repo(tasks),
    }
}

pub fn repo_label(repo: &ConversationRepo) -> String {
    match repo {
        ConversationRepo::Repo(name) => name.clone(),
        ConversationRepo::Multiple => MULTIPLE_REPOS.to_string(),
        ConversationRepo::None => NO_REPO.to_string(),
    }
}

pub fn row_repo_text(repo: &ConversationRepo) -> String {
    match repo {
        ConversationRepo::None => String::new(),
        other => repo_label(other),
    }
}

fn group_key(repo: &ConversationRepo) -> String {
    match repo {
        ConversationRepo::Repo(name) => format!("repo:{}", name.to_lowercase()),
        ConversationRepo::Multiple => MULTIPLE_KEY.to_string(),
        ConversationRepo::None => NONE_KEY.to_string(),
    }
}

fn rank(key: &str) -> u8 {
    match key {
        MULTIPLE_KEY => 1,
        NONE_KEY => 2,
        _ => 0,
    }
}

pub fn group_by_repo<T>(
    items: Vec<T>,
    repo_of: impl Fn(&T) -> ConversationRepo,
) -> Vec<RepoGroup<T>> {
    let mut groups: Vec<RepoGroup<T>> = Vec::new();
    for item in items {
        let repo = repo_of(&item);
        let key = group_key(&repo);
        match groups.iter_mut().find(|g| g.key == key) {
            Some(group) => group.items.push(item),
            None => groups.push(RepoGroup {
                key,
                label: repo_label(&repo),
                items: vec![item],
            }),
        }
    }
    groups.sort_by_key(|g| rank(&g.key));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{conversation, task};
    use townhall_core::model::TaskState;

    fn on_repos(repos: &[&str]) -> Vec<Task> {
        repos
            .iter()
            .enumerate()
            .map(|(i, repo)| {
                let mut t = task(&format!("t{i}"), TaskState::Working);
                t.repo = repo.to_string();
                t
            })
            .collect()
    }

    fn named(name: &str) -> ConversationRepo {
        ConversationRepo::Repo(name.into())
    }

    fn conv() -> Conversation {
        conversation("c", "t", "2026-11-12T10:00:00.000Z")
    }

    #[test]
    fn the_repo_set_from_the_row_menu_comes_first() {
        let mut c = conv();
        c.linked_repo = Some("site".into());
        assert_eq!(
            repo_of_conversation(&c, &on_repos(&["shop", "shop"])),
            named("site")
        );
    }

    #[test]
    fn the_most_common_repo_among_its_tasks_wins_ignoring_case() {
        assert_eq!(
            repo_of_conversation(&conv(), &on_repos(&["shop", "site", "Shop"])),
            named("shop")
        );
    }

    #[test]
    fn a_tie_is_multiple_repos_and_no_tasks_is_no_repo() {
        assert_eq!(
            infer_repo(&on_repos(&["shop", "site"])),
            ConversationRepo::Multiple
        );
        assert_eq!(infer_repo(&[]), ConversationRepo::None);
    }

    #[test]
    fn labels_for_a_group_and_for_a_row() {
        assert_eq!(repo_label(&named("shop")), "shop");
        assert_eq!(repo_label(&ConversationRepo::Multiple), "multiple repos");
        assert_eq!(repo_label(&ConversationRepo::None), "no repo");
        assert_eq!(row_repo_text(&ConversationRepo::None), "");
        assert_eq!(row_repo_text(&ConversationRepo::Multiple), "multiple repos");
    }

    #[test]
    fn groups_keep_list_order_and_the_fixed_groups_go_last() {
        let items = vec![
            ("a", ConversationRepo::None),
            ("b", named("shop")),
            ("c", ConversationRepo::Multiple),
            ("d", named("townhall")),
            ("e", named("Shop")),
        ];
        let groups = group_by_repo(items, |(_, repo)| repo.clone());
        let shown: Vec<(String, Vec<&str>)> = groups
            .iter()
            .map(|g| (g.label.clone(), g.items.iter().map(|(id, _)| *id).collect()))
            .collect();
        assert_eq!(
            shown,
            [
                ("shop".to_string(), vec!["b", "e"]),
                ("townhall".to_string(), vec!["d"]),
                ("multiple repos".to_string(), vec!["c"]),
                ("no repo".to_string(), vec!["a"]),
            ]
        );
        let keys: std::collections::HashSet<_> = groups.iter().map(|g| &g.key).collect();
        assert_eq!(keys.len(), 4);
    }
}
