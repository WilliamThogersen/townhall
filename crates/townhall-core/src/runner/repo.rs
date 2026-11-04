//! Which repo a message is about, before its turn starts: the linked repo wins, then an
//! answer to a pending `repoAsk`, then repo inference (2.4). A found repo is noted as a
//! `repo` entry when it changes; an unclear one asks (or guesses, per `repoUnclear`).

use super::answer::{answer, last_repo, pending_ask, Answer, CurrentRepo, PendingAsk};
use super::prompt::RepoNote;
use crate::model::{Conversation, EntryBody, Repo, RepoRule, RepoUnclear};
use crate::repo_infer::{self, Candidate, Inference, Rule};
use crate::store::Store;
use crate::{Context, Result};

const LINKED: &str = "you linked it";
const PICKED: &str = "you picked it";

/// What the turn runs with, or that townhall asked and waits for the answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    Run {
        note: RepoNote,
        /// The text for the session: an answered ask brings back the message it asked about.
        text: String,
        attachments: Vec<String>,
    },
    Asked,
}

/// The message being resolved.
#[derive(Debug, Clone, Copy)]
pub struct Message<'a> {
    pub text: &'a str,
    pub attachments: &'a [String],
}

struct Resolver<'a> {
    store: &'a Store,
    id: &'a str,
    repos: &'a [Repo],
    current: Option<CurrentRepo>,
    pending: Option<PendingAsk>,
    message: Message<'a>,
}

fn find(repos: &[Repo], name: &str) -> Option<Repo> {
    repos
        .iter()
        .find(|r| r.name.eq_ignore_ascii_case(name.trim()))
        .cloned()
}

fn rule_of(rule: Rule) -> RepoRule {
    match rule {
        Rule::Named => RepoRule::Named,
        Rule::Code => RepoRule::Code,
        Rule::Only => RepoRule::Only,
    }
}

/// `two repos could fit. which one?` becomes `guessed: two repos could fit`.
pub fn guess_reason(reason: &str) -> String {
    let why = reason.trim().trim_end_matches("which one?").trim_end();
    format!("guessed: {}", why.trim_end_matches('.'))
}

pub fn candidates_of(repos: &[Repo]) -> Vec<Candidate> {
    repos
        .iter()
        .map(|r| Candidate {
            name: r.name.clone(),
            path: r.path.clone(),
        })
        .collect()
}

impl Resolver<'_> {
    fn run(&self, note: RepoNote, bare: bool) -> Resolved {
        let (text, mut attachments) = match &self.pending {
            Some(p) if bare => (p.text.clone(), p.attachments.clone()),
            Some(p) => (
                format!("{}\n\n{}", p.text, self.message.text),
                p.attachments.clone(),
            ),
            None => (self.message.text.to_string(), vec![]),
        };
        attachments.extend(self.message.attachments.iter().cloned());
        Resolved::Run {
            note,
            text,
            attachments,
        }
    }

    async fn append(&self, repo: &Repo, rule: RepoRule, reason: &str) -> Result<()> {
        let body = EntryBody::Repo {
            repo: repo.name.clone(),
            base_branch: repo.base_branch.clone(),
            rule,
            reason: reason.to_string(),
        };
        self.store.append_entry(self.id, body).await.map(|_| ())
    }

    /// Appends the `repo` entry unless the conversation is already there.
    async fn note(&self, repo: &Repo, rule: RepoRule, reason: &str) -> Result<()> {
        let same = self.current.as_ref().is_some_and(|c| {
            c.is(repo) && (rule != RepoRule::Linked || c.rule == RepoRule::Linked)
        });
        if same {
            return Ok(());
        }
        self.append(repo, rule, reason).await
    }

    async fn picked(&self, picked: Answer) -> Result<Resolved> {
        match picked {
            Answer::One { repo, bare } => {
                self.note(&repo, RepoRule::Picked, PICKED).await?;
                Ok(self.run(RepoNote::One(repo), bare))
            }
            Answer::All { repos, word } => {
                let reason = format!("you picked {word}");
                for repo in &repos {
                    self.append(repo, RepoRule::Picked, &reason).await?;
                }
                Ok(self.run(RepoNote::Several(repos), true))
            }
        }
    }

    async fn unclear(&self, candidates: Vec<String>, reason: String) -> Result<Resolved> {
        let current = self
            .current
            .as_ref()
            .and_then(|c| find(self.repos, &c.repo));
        let keep = current.filter(|r| candidates.iter().any(|c| r.name.eq_ignore_ascii_case(c)));
        if let Some(repo) = keep {
            return Ok(self.run(RepoNote::One(repo), false));
        }
        let first = candidates.first().and_then(|c| find(self.repos, c));
        match (self.store.settings().await?.repo_unclear, first) {
            (RepoUnclear::Guess, Some(repo)) => {
                self.note(&repo, RepoRule::Picked, &guess_reason(&reason))
                    .await?;
                Ok(self.run(RepoNote::One(repo), false))
            }
            _ => {
                let ask = EntryBody::RepoAsk {
                    question: reason,
                    offer_all: candidates.len() > 1,
                    candidates,
                };
                self.store.append_entry(self.id, ask).await?;
                Ok(Resolved::Asked)
            }
        }
    }

    async fn infer(&self) -> Result<Resolved> {
        let inferred = repo_infer::infer(self.message.text, &candidates_of(self.repos)).await;
        match inferred {
            Ok(Inference::Found { repo, rule, reason }) => match find(self.repos, &repo) {
                Some(repo) => {
                    self.note(&repo, rule_of(rule), &reason).await?;
                    Ok(self.run(RepoNote::One(repo), false))
                }
                None => Ok(self.run(RepoNote::Unknown, false)),
            },
            Ok(Inference::Ask { candidates, reason }) => self.unclear(candidates, reason).await,
            Ok(Inference::NoRepos) => Ok(self.run(RepoNote::Unknown, false)),
            Err(e) => {
                eprintln!("warn: repo inference failed: {e}");
                Ok(self.run(RepoNote::Unknown, false))
            }
        }
    }
}

/// Resolves the repo for the latest message of `conversation` (already appended).
pub async fn resolve(
    ctx: &Context,
    conversation: &Conversation,
    message: Message<'_>,
) -> Result<Resolved> {
    let store = ctx.store()?;
    let repos = ctx.repos().await?;
    let entries = store.entries(&conversation.id).await?;
    let r = Resolver {
        store,
        id: &conversation.id,
        repos: &repos,
        current: last_repo(&entries),
        pending: pending_ask(&entries),
        message,
    };
    let linked = conversation.linked_repo.as_deref();
    match linked.map(|name| (name, find(&repos, name))) {
        Some((_, Some(repo))) => {
            r.note(&repo, RepoRule::Linked, LINKED).await?;
            return Ok(r.run(RepoNote::One(repo), false));
        }
        Some((name, None)) => eprintln!("warn: linked repo `{name}` is gone, inferring instead"),
        None => {}
    }
    if let Some(p) = &r.pending {
        if let Some(picked) = answer(message.text, &p.candidates, &repos).await {
            return r.picked(picked).await;
        }
    }
    r.infer().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guess_keeps_the_reason_without_the_question() {
        assert_eq!(
            guess_reason("two repos could fit. which one?"),
            "guessed: two repos could fit"
        );
        assert_eq!(
            guess_reason("you named three repos. which one?"),
            "guessed: you named three repos"
        );
        assert_eq!(guess_reason("odd"), "guessed: odd");
        assert_eq!(rule_of(Rule::Code), RepoRule::Code);
        assert_eq!(rule_of(Rule::Only), RepoRule::Only);
    }
}
