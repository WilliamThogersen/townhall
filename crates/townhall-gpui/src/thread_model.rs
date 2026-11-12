use std::collections::HashMap;

use townhall_core::clock::parse_rfc3339;
use townhall_core::model::{
    AgentEvent, Conversation, ConversationStatus, EntryBody, RepoRule, Task, TaskState, ThreadEntry,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyTarget {
    pub id: u64,
    pub author: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskChoices {
    pub candidates: Vec<String>,
    pub all: Option<&'static str>,
}

pub const EXCERPT_CHARS: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Entry {
        index: usize,
        open: bool,
        answer: Option<String>,
    },
    Thinking,
    Ended,
}

pub fn display_items(
    entries: &[ThreadEntry],
    tasks: &[Task],
    thinking: bool,
    ended: bool,
) -> Vec<Item> {
    let pairs = ask_answers(entries);
    let hidden: std::collections::HashSet<usize> = pairs.values().copied().collect();
    let answer_of = |i: usize| match pairs.get(&i).and_then(|at| entries.get(*at)) {
        Some(ThreadEntry {
            body: EntryBody::Agent { text, .. },
            ..
        }) => Some(text.clone()),
        _ => None,
    };
    let mut items: Vec<Item> = entries
        .iter()
        .enumerate()
        .filter(|(i, _)| !hidden.contains(i))
        .map(|(index, entry)| Item::Entry {
            index,
            open: match entry.body {
                EntryBody::NeedsYou { .. } => is_needs_you_open(entries, index, tasks),
                EntryBody::RepoAsk { .. } => is_repo_ask_open(entries, index),
                _ => false,
            },
            answer: answer_of(index),
        })
        .collect();
    if thinking {
        items.push(Item::Thinking);
    } else if ended {
        items.push(Item::Ended);
    }
    items
}

pub fn clock_time(iso: &str) -> String {
    let Some(ms) = parse_rfc3339(iso) else {
        return String::new();
    };
    let secs = ms.div_euclid(1000) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let filled = unsafe { libc::localtime_r(&secs, &mut tm) };
    if filled.is_null() {
        return String::new();
    }
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}

pub fn excerpt_of(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = joined.chars().collect();
    if chars.len() > EXCERPT_CHARS {
        format!("{}…", chars[..EXCERPT_CHARS].iter().collect::<String>())
    } else {
        joined
    }
}

fn said(body: &EntryBody) -> Option<(String, String)> {
    let line = |author: &str, text: &str| Some((author.to_string(), text.to_string()));
    match body {
        EntryBody::You { text, .. } => line("you", text),
        EntryBody::Townhall { text } => line("townhall", text),
        EntryBody::RepoAsk { question, .. } => line("townhall", question),
        EntryBody::NeedsYou {
            agent, question, ..
        } => line(agent, question),
        EntryBody::Agent { agent, event, text } => {
            let words = match event {
                AgentEvent::Sent => "sent",
                AgentEvent::Passed => "passed",
                AgentEvent::Failed => "failed",
                AgentEvent::Ended => "ended",
                AgentEvent::Allowed => "allowed",
            };
            line(agent, if text.is_empty() { words } else { text })
        }
        EntryBody::Event { text } => line("task event", text),
        EntryBody::Handoff { from, reason, .. } => line(from, reason),
        EntryBody::Plugin { plugin, text } => line(&format!("plugin {plugin}"), text),
        EntryBody::Error { message } => line("townhall", message),
        EntryBody::Repo { .. } | EntryBody::TasksCreated { .. } => None,
    }
}

pub fn reply_target(entry: &ThreadEntry) -> Option<ReplyTarget> {
    let (author, text) = said(&entry.body)?;
    Some(ReplyTarget {
        id: entry.id,
        author,
        excerpt: excerpt_of(&text),
    })
}

pub fn repo_reason(reason: &str, rule: RepoRule) -> String {
    if !reason.trim().is_empty() {
        return reason.to_string();
    }
    match rule {
        RepoRule::Named => "you named it",
        RepoRule::Code => "found in its code",
        RepoRule::Only => "only repo",
        RepoRule::Picked => "you picked it",
        RepoRule::Linked => "set by hand",
    }
    .to_string()
}

pub fn is_needs_you_open(entries: &[ThreadEntry], index: usize, tasks: &[Task]) -> bool {
    let Some(EntryBody::NeedsYou { agent, .. }) = entries.get(index).map(|e| &e.body) else {
        return false;
    };
    if let Some(task) = tasks.iter().find(|t| &t.name == agent) {
        if task.state != TaskState::NeedsYou {
            return false;
        }
    }
    !entries[index + 1..].iter().any(|later| match &later.body {
        EntryBody::Agent {
            agent: other,
            event: AgentEvent::Sent,
            ..
        } => other == agent,
        EntryBody::NeedsYou { agent: other, .. } => other == agent,
        _ => false,
    })
}

pub fn ask_answers(entries: &[ThreadEntry]) -> HashMap<usize, usize> {
    let mut pairs = HashMap::new();
    for (i, entry) in entries.iter().enumerate() {
        let EntryBody::NeedsYou { agent, .. } = &entry.body else {
            continue;
        };
        let later = &entries[i + 1..];
        let end = later
            .iter()
            .position(|e| matches!(&e.body, EntryBody::NeedsYou { agent: a, .. } if a == agent))
            .unwrap_or(later.len());
        let answer = later[..end].iter().position(|e| {
            matches!(
                &e.body,
                EntryBody::Agent { agent: a, event: AgentEvent::Sent, .. } if a == agent
            )
        });
        if let Some(answer) = answer {
            pairs.insert(i, i + 1 + answer);
        }
    }
    pairs
}

pub fn is_repo_ask_open(entries: &[ThreadEntry], index: usize) -> bool {
    !entries[index + 1..]
        .iter()
        .any(|later| matches!(later.body, EntryBody::You { .. }))
}

pub fn ask_choices(candidates: &[String], offer_all: bool) -> AskChoices {
    let all = match (offer_all, candidates.len()) {
        (false, _) => None,
        (true, 2) => Some("both"),
        (true, _) => Some("all"),
    };
    AskChoices {
        candidates: candidates.to_vec(),
        all,
    }
}

pub fn conversation_status(conversation: &Conversation) -> &'static str {
    if conversation.status == ConversationStatus::Thinking {
        "thinking"
    } else if conversation.ended {
        "ended"
    } else {
        "your turn"
    }
}

pub fn end_question(title: &str, unfinished: usize) -> String {
    let tasks = if unfinished == 1 {
        "1 task stops and its worktree is deleted".to_string()
    } else {
        format!("{unfinished} tasks stop and their worktrees are deleted")
    };
    format!("end {title}? {tasks}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{conversation, entry_of, task, you};
    use townhall_core::clock::from_unix_millis;
    use townhall_core::model::{AgentEvent, ConversationStatus, EntryBody, TaskState};

    fn ask() -> EntryBody {
        EntryBody::NeedsYou {
            agent: "fix-login".into(),
            question: "commit? (y/n)".into(),
            choices: vec!["y".into(), "n".into()],
        }
    }

    fn sent(agent: &str, text: &str) -> EntryBody {
        EntryBody::Agent {
            agent: agent.into(),
            event: AgentEvent::Sent,
            text: text.into(),
        }
    }

    fn local_iso(hour: i32, minute: i32) -> String {
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        tm.tm_year = 2026 - 1900;
        tm.tm_mon = 10;
        tm.tm_mday = 4;
        tm.tm_hour = hour;
        tm.tm_min = minute;
        tm.tm_isdst = -1;
        let secs = unsafe { libc::mktime(&mut tm) };
        from_unix_millis(secs as i64 * 1000)
    }

    #[test]
    fn the_clock_shows_the_local_hour_and_minute() {
        assert_eq!(clock_time(&local_iso(9, 5)), "09:05");
        assert_eq!(clock_time(&local_iso(23, 59)), "23:59");
    }

    #[test]
    fn the_clock_shows_nothing_for_a_broken_time() {
        assert_eq!(clock_time("not a time"), "");
    }

    fn target(body: EntryBody) -> Option<ReplyTarget> {
        reply_target(&entry_of(7, body))
    }

    fn who(body: EntryBody) -> (String, String) {
        let t = target(body).unwrap();
        (t.author, t.excerpt)
    }

    fn pair(a: &str, b: &str) -> (String, String) {
        (a.to_string(), b.to_string())
    }

    #[test]
    fn a_reply_target_names_who_said_it_and_what() {
        assert_eq!(
            target(EntryBody::You {
                text: "use the new login".into(),
                attachments: vec![],
                reply_to: None
            }),
            Some(ReplyTarget {
                id: 7,
                author: "you".into(),
                excerpt: "use the new login".into()
            })
        );
        assert_eq!(
            who(EntryBody::Townhall {
                text: "which repo?".into()
            }),
            pair("townhall", "which repo?")
        );
        assert_eq!(who(ask()), pair("fix-login", "commit? (y/n)"));
        assert_eq!(
            who(EntryBody::RepoAsk {
                question: "shop or admin?".into(),
                candidates: vec![],
                offer_all: false
            }),
            pair("townhall", "shop or admin?")
        );
        assert_eq!(
            who(EntryBody::Agent {
                agent: "fix-login".into(),
                event: AgentEvent::Failed,
                text: "tests red".into()
            }),
            pair("fix-login", "tests red")
        );
        assert_eq!(
            who(EntryBody::Agent {
                agent: "fix-login".into(),
                event: AgentEvent::Ended,
                text: "".into()
            })
            .1,
            "ended"
        );
        assert_eq!(
            who(EntryBody::Event {
                text: "task fix ended".into()
            }),
            pair("task event", "task fix ended")
        );
        assert_eq!(
            who(EntryBody::Handoff {
                task: "t".into(),
                from: "shipper".into(),
                to: "coder".into(),
                reason: "lint fails".into()
            }),
            pair("shipper", "lint fails")
        );
        assert_eq!(
            who(EntryBody::Plugin {
                plugin: "lint@acme".into(),
                text: "run the audit".into()
            }),
            pair("plugin lint@acme", "run the audit")
        );
        assert_eq!(
            who(EntryBody::Error {
                message: "claude exited with 1".into()
            }),
            pair("townhall", "claude exited with 1")
        );
    }

    #[test]
    fn repo_notes_and_task_blocks_have_nothing_to_reply_to() {
        assert_eq!(
            target(EntryBody::Repo {
                repo: "shop".into(),
                base_branch: "main".into(),
                rule: RepoRule::Only,
                reason: "".into()
            }),
            None
        );
        assert_eq!(
            target(EntryBody::TasksCreated {
                tasks: vec!["a".into()]
            }),
            None
        );
    }

    #[test]
    fn an_excerpt_joins_lines_and_cuts_a_long_text() {
        assert_eq!(
            who(EntryBody::Townhall {
                text: "first\n\n  second   line".into()
            })
            .1,
            "first second line"
        );
        assert_eq!(
            who(EntryBody::Townhall {
                text: "x".repeat(300)
            })
            .1,
            format!("{}…", "x".repeat(120))
        );
        assert_eq!(
            excerpt_of(&"ø".repeat(121)),
            format!("{}…", "ø".repeat(120))
        );
    }

    #[test]
    fn the_repo_reason_is_the_daemons_else_one_per_rule() {
        assert_eq!(
            repo_reason("it has src/a.ts", RepoRule::Code),
            "it has src/a.ts"
        );
        assert_eq!(repo_reason("", RepoRule::Named), "you named it");
        assert_eq!(repo_reason("", RepoRule::Code), "found in its code");
        assert_eq!(repo_reason("", RepoRule::Only), "only repo");
        assert_eq!(repo_reason("", RepoRule::Picked), "you picked it");
        assert_eq!(repo_reason("", RepoRule::Linked), "set by hand");
    }

    #[test]
    fn a_question_is_open_until_the_agent_got_an_answer() {
        let tasks = vec![task("fix-login", TaskState::NeedsYou)];
        let entries = vec![entry_of(1, ask())];
        assert!(is_needs_you_open(&entries, 0, &tasks));
        let answered = vec![entry_of(1, ask()), entry_of(2, sent("fix-login", "y"))];
        assert!(!is_needs_you_open(&answered, 0, &tasks));
    }

    #[test]
    fn a_question_is_closed_once_a_newer_one_came_or_the_task_moved_on() {
        let two = vec![entry_of(1, ask()), entry_of(2, ask())];
        assert!(!is_needs_you_open(&two, 0, &[]));
        assert!(is_needs_you_open(&two, 1, &[]));
        assert!(!is_needs_you_open(
            &[entry_of(1, ask())],
            0,
            &[task("fix-login", TaskState::Working)]
        ));
    }

    #[test]
    fn answers_to_other_agents_do_not_close_a_question() {
        let entries = vec![entry_of(1, ask()), entry_of(2, sent("other", "y"))];
        assert!(is_needs_you_open(&entries, 0, &[]));
    }

    #[test]
    fn a_repo_ask_is_open_until_you_write_again() {
        let repo_ask = entry_of(
            2,
            EntryBody::RepoAsk {
                question: "which?".into(),
                candidates: vec!["a".into(), "b".into()],
                offer_all: true,
            },
        );
        assert!(is_repo_ask_open(&[you(1, "hi"), repo_ask.clone()], 1));
        assert!(!is_repo_ask_open(&[you(1, "hi"), repo_ask, you(3, "a")], 1));
    }

    #[test]
    fn both_is_offered_for_two_candidates_and_all_for_more() {
        let list = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        assert_eq!(ask_choices(&list(&["a", "b"]), true).all, Some("both"));
        assert_eq!(ask_choices(&list(&["a", "b", "c"]), true).all, Some("all"));
        let none = ask_choices(&list(&["a", "b"]), false);
        assert_eq!((none.candidates, none.all), (list(&["a", "b"]), None));
    }

    #[test]
    fn the_status_is_thinking_then_ended_else_your_turn() {
        let mut c = conversation("c", "t", "2026-11-12T10:00:00.000Z");
        c.status = ConversationStatus::Thinking;
        c.ended = true;
        assert_eq!(conversation_status(&c), "thinking");
        c.status = ConversationStatus::YourTurn;
        assert_eq!(conversation_status(&c), "ended");
        c.ended = false;
        assert_eq!(conversation_status(&c), "your turn");
    }

    #[test]
    fn the_end_question_names_the_conversation_and_the_tasks_that_stop() {
        assert_eq!(
            end_question("login loop", 1),
            "end login loop? 1 task stops and its worktree is deleted"
        );
        assert_eq!(
            end_question("login loop", 2),
            "end login loop? 2 tasks stop and their worktrees are deleted"
        );
    }

    fn entry_item(index: usize, open: bool, answer: Option<&str>) -> Item {
        Item::Entry {
            index,
            open,
            answer: answer.map(String::from),
        }
    }

    #[test]
    fn an_answered_question_hides_the_sent_line_and_carries_its_text() {
        let entries = vec![
            you(1, "go"),
            entry_of(2, ask()),
            entry_of(3, sent("fix-login", "y")),
        ];
        let tasks = vec![task("fix-login", TaskState::Working)];
        assert_eq!(
            display_items(&entries, &tasks, false, false),
            [entry_item(0, false, None), entry_item(1, false, Some("y")),]
        );
    }

    #[test]
    fn an_open_question_and_an_open_repo_ask_are_marked_open() {
        let repo_ask = EntryBody::RepoAsk {
            question: "which?".into(),
            candidates: vec!["a".into()],
            offer_all: false,
        };
        let entries = vec![entry_of(1, repo_ask), entry_of(2, ask())];
        let tasks = vec![task("fix-login", TaskState::NeedsYou)];
        assert_eq!(
            display_items(&entries, &tasks, false, false),
            [entry_item(0, true, None), entry_item(1, true, None)]
        );
    }

    #[test]
    fn thinking_adds_a_tail_row_and_wins_over_ended() {
        let entries = vec![you(1, "go")];
        assert_eq!(
            display_items(&entries, &[], true, true),
            [entry_item(0, false, None), Item::Thinking]
        );
        assert_eq!(
            display_items(&entries, &[], false, true),
            [entry_item(0, false, None), Item::Ended]
        );
        assert_eq!(display_items(&entries, &[], false, false).len(), 1);
    }

    #[test]
    fn a_question_pairs_with_the_first_answer_its_agent_got() {
        let entries = vec![
            entry_of(1, ask()),
            entry_of(2, sent("other", "x")),
            entry_of(3, sent("fix-login", "y")),
            entry_of(4, sent("fix-login", "later")),
        ];
        assert_eq!(ask_answers(&entries), HashMap::from([(0, 2)]));
    }

    #[test]
    fn a_question_gets_no_answer_once_a_newer_one_from_its_agent_came() {
        let entries = vec![
            entry_of(1, ask()),
            entry_of(2, ask()),
            entry_of(3, sent("fix-login", "y")),
        ];
        assert_eq!(ask_answers(&entries), HashMap::from([(1, 2)]));
    }
}
