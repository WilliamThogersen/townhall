use townhall_core::model::Repo;

use crate::repo_group::repo_label;
use crate::sidebar_rows::SidebarRow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    End(String),
    Link(String, Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
    pub checked: Option<bool>,
    pub action: Option<MenuAction>,
}

pub fn row_menu_items(row: &SidebarRow, repos: &[Repo]) -> Vec<MenuItem> {
    let conversation = &row.conversation;
    let id = conversation.id.clone();
    let linked = conversation.linked_repo.as_ref().map(|r| r.to_lowercase());
    let choose = |repo: Option<&str>| {
        let same = repo.map(str::to_lowercase) == linked;
        (!same).then(|| MenuAction::Link(id.clone(), repo.map(String::from)))
    };
    let mut items = Vec::new();
    if !conversation.ended {
        items.push(MenuItem {
            id: "end".into(),
            label: "end conversation".into(),
            detail: None,
            checked: None,
            action: Some(MenuAction::End(id.clone())),
        });
    }
    items.push(MenuItem {
        id: "auto".into(),
        label: "repo: auto".into(),
        detail: Some(repo_label(&row.auto_repo)),
        checked: Some(linked.is_none()),
        action: choose(None),
    });
    for repo in repos {
        items.push(MenuItem {
            id: format!("repo:{}", repo.name),
            label: format!("repo: {}", repo.name),
            detail: Some(repo.base_branch.clone()),
            checked: Some(linked.as_deref() == Some(repo.name.to_lowercase().as_str())),
            action: choose(Some(&repo.name)),
        });
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glyph::Glyph;
    use crate::repo_group::ConversationRepo;
    use crate::row_summary::RowSummary;
    use crate::testkit::{conversation, repo};

    fn row(linked: Option<&str>, ended: bool) -> SidebarRow {
        let mut c = conversation("c1", "t", "2026-11-12T10:00:00.000Z");
        c.linked_repo = linked.map(String::from);
        c.ended = ended;
        SidebarRow {
            conversation: c,
            summary: RowSummary {
                glyph: Glyph::Dead,
                state: None,
                status: "your turn".into(),
                needs_you: false,
            },
            repo: ConversationRepo::Repo("shop".into()),
            auto_repo: ConversationRepo::Repo("shop".into()),
        }
    }

    fn repos() -> Vec<Repo> {
        vec![repo("shop", "develop"), repo("townhall", "main")]
    }

    #[test]
    fn offers_end_repo_auto_with_the_inferred_repo_and_one_item_per_repo() {
        let items = row_menu_items(&row(None, false), &repos());
        let shown: Vec<(&str, &str, Option<bool>)> = items
            .iter()
            .map(|i| {
                (
                    i.label.as_str(),
                    i.detail.as_deref().unwrap_or(""),
                    i.checked,
                )
            })
            .collect();
        assert_eq!(
            shown,
            [
                ("end conversation", "", None),
                ("repo: auto", "shop", Some(true)),
                ("repo: shop", "develop", Some(false)),
                ("repo: townhall", "main", Some(false)),
            ]
        );
    }

    #[test]
    fn ends_and_links_through_actions() {
        let items = row_menu_items(&row(None, false), &repos());
        assert_eq!(items[0].action, Some(MenuAction::End("c1".into())));
        assert_eq!(
            items[3].action,
            Some(MenuAction::Link("c1".into(), Some("townhall".into())))
        );
    }

    #[test]
    fn checks_the_linked_repo_ignoring_case_and_sets_auto_back() {
        let items = row_menu_items(&row(Some("Shop"), false), &repos());
        let checked: Vec<&str> = items
            .iter()
            .filter(|i| i.checked == Some(true))
            .map(|i| i.label.as_str())
            .collect();
        assert_eq!(checked, ["repo: shop"]);
        assert_eq!(items[2].action, None);
        assert_eq!(items[1].action, Some(MenuAction::Link("c1".into(), None)));
    }

    #[test]
    fn leaves_end_out_of_an_ended_conversation() {
        let items = row_menu_items(&row(None, true), &repos());
        assert_eq!(items[0].label, "repo: auto");
    }
}
