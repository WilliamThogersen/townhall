//! What the thread says about repos so far: the current repo (the latest `repo` entry),
//! a `repoAsk` waiting for its answer, and whether a message answers it.

use super::repo::candidates_of;
use crate::model::{EntryBody, Repo, RepoRule, ThreadEntry};
use crate::redact::redact;
use crate::repo_infer::{self, Inference, Rule};

/// The latest `repo` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentRepo {
    pub repo: String,
    pub base_branch: String,
    pub rule: RepoRule,
}

impl CurrentRepo {
    pub fn is(&self, repo: &Repo) -> bool {
        self.repo.eq_ignore_ascii_case(&repo.name) && self.base_branch == repo.base_branch
    }
}

/// A `repoAsk` right before the latest message, and the message it asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAsk {
    pub candidates: Vec<String>,
    pub text: String,
    pub attachments: Vec<String>,
    /// The entry that message answered, if it was a reply.
    pub reply_to: Option<u64>,
}

/// How a message answered a `repoAsk`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// `bare` when the message is only the repo's name.
    One { repo: Repo, bare: bool },
    /// `both` or `all`.
    All { repos: Vec<Repo>, word: String },
}

pub fn last_repo(entries: &[ThreadEntry]) -> Option<CurrentRepo> {
    entries.iter().rev().find_map(|e| match &e.body {
        EntryBody::Repo {
            repo,
            base_branch,
            rule,
            ..
        } => Some(CurrentRepo {
            repo: repo.clone(),
            base_branch: base_branch.clone(),
            rule: *rule,
        }),
        _ => None,
    })
}

/// Entries the agents add on their own; they do not answer or replace an ask.
fn from_agents(body: &EntryBody) -> bool {
    matches!(
        body,
        EntryBody::NeedsYou { .. }
            | EntryBody::Agent { .. }
            | EntryBody::TasksCreated { .. }
            | EntryBody::Event { .. }
            | EntryBody::Plugin { .. }
    )
}

/// The ask the latest message (the last entry) answers, if the entry before it is one.
pub fn pending_ask(entries: &[ThreadEntry]) -> Option<PendingAsk> {
    let (_latest, earlier) = entries.split_last()?;
    let mut back = earlier.iter().rev().filter(|e| !from_agents(&e.body));
    let candidates = match &back.next()?.body {
        EntryBody::RepoAsk { candidates, .. } => candidates.clone(),
        _ => return None,
    };
    let asked = back.find_map(|e| match &e.body {
        EntryBody::You {
            text,
            attachments,
            reply_to,
        } => Some((text.clone(), attachments.clone(), *reply_to)),
        _ => None,
    });
    let (text, attachments, reply_to) = asked.unwrap_or_default();
    Some(PendingAsk {
        candidates,
        text,
        attachments,
        reply_to,
    })
}

fn known(candidates: &[String], repos: &[Repo]) -> Vec<Repo> {
    candidates
        .iter()
        .filter_map(|c| repos.iter().find(|r| r.name.eq_ignore_ascii_case(c)))
        .cloned()
        .collect()
}

/// `both`, `all`, a candidate's name, or a message that names one candidate.
pub async fn answer(text: &str, candidates: &[String], repos: &[Repo]) -> Option<Answer> {
    let options = known(candidates, repos);
    let word = text.trim().to_lowercase();
    if (word == "both" || word == "all") && !options.is_empty() {
        return Some(Answer::All {
            repos: options,
            word,
        });
    }
    if let Some(repo) = options.iter().find(|r| r.name.to_lowercase() == word) {
        return Some(Answer::One {
            repo: repo.clone(),
            bare: true,
        });
    }
    match repo_infer::infer(text, &candidates_of(&options)).await {
        Ok(Inference::Found {
            repo,
            rule: Rule::Named,
            ..
        }) => known(&[repo], &options)
            .into_iter()
            .next()
            .map(|repo| Answer::One { repo, bare: false }),
        Ok(_) => None,
        Err(e) => {
            tracing::warn!(
                target: "townhall::runner", event = "repo.answer_unreadable",
                error_kind = e.kind(), error = %redact(&e.to_string()),
                "cannot read the answer to a repo question"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AgentEvent;

    fn entry(id: u64, body: EntryBody) -> ThreadEntry {
        ThreadEntry {
            id,
            at: "2026-11-04T10:42:05.000Z".into(),
            body,
        }
    }

    fn you(text: &str) -> EntryBody {
        EntryBody::You {
            text: text.into(),
            attachments: vec!["a1".into()],
            reply_to: None,
        }
    }

    fn ask() -> EntryBody {
        EntryBody::RepoAsk {
            question: "which one?".into(),
            candidates: vec!["shop".into(), "admin".into()],
            offer_all: true,
        }
    }

    fn repo(name: &str) -> Repo {
        Repo {
            name: name.into(),
            path: format!("/nowhere/{name}").into(),
            base_branch: "main".into(),
            github_account: None,
            gitlab_account: None,
        }
    }

    #[test]
    fn an_ask_right_before_the_message_is_pending() {
        let agent = EntryBody::Agent {
            agent: "x".into(),
            event: AgentEvent::Ended,
            text: "".into(),
        };
        let thread = [
            entry(1, you("make it fast")),
            entry(2, ask()),
            entry(3, agent),
            entry(4, you("shop")),
        ];
        let pending = pending_ask(&thread).unwrap();
        assert_eq!(pending.text, "make it fast");
        assert_eq!(pending.attachments, ["a1"]);
        assert_eq!(pending.candidates, ["shop", "admin"]);
        let answered = [
            entry(1, you("make it fast")),
            entry(2, ask()),
            entry(3, EntryBody::Townhall { text: "ok".into() }),
            entry(4, you("shop")),
        ];
        assert!(pending_ask(&answered).is_none());
        assert!(pending_ask(&[]).is_none());
        assert!(last_repo(&answered).is_none());
    }

    #[test]
    fn the_latest_repo_entry_is_current() {
        let noted = |name: &str, rule| EntryBody::Repo {
            repo: name.into(),
            base_branch: "main".into(),
            rule,
            reason: "r".into(),
        };
        let thread = [
            entry(1, noted("shop", RepoRule::Named)),
            entry(2, noted("admin", RepoRule::Linked)),
            entry(3, you("x")),
        ];
        let current = last_repo(&thread).unwrap();
        assert_eq!(current.rule, RepoRule::Linked);
        assert!(current.is(&repo("ADMIN")) && !current.is(&repo("shop")));
    }

    #[tokio::test]
    async fn bare_answers_need_no_git() {
        let repos = [repo("shop"), repo("admin")];
        let names = ["shop".to_string(), "admin".to_string()];
        let both = answer(" Both ", &names, &repos).await.unwrap();
        assert_eq!(
            both,
            Answer::All {
                repos: repos.to_vec(),
                word: "both".into()
            }
        );
        let one = answer("ADMIN", &names, &repos).await.unwrap();
        assert_eq!(
            one,
            Answer::One {
                repo: repo("admin"),
                bare: true
            }
        );
        assert!(answer("neither", &names, &repos).await.is_none());
    }
}
