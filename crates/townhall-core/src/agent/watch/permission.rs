use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::sync::Change;
use super::Key;
use crate::context::Context;
use crate::model::{AgentInfo, AgentState, EntryBody, Task};
use crate::permissions::{self, Outcome};
use crate::Workspace;

struct Finished {
    key: Key,
    question: String,
    outcome: Outcome,
}

pub(super) enum Gate {
    Pass,
    Hold,
    Ask,
}

pub(super) struct Decisions {
    deciding: HashMap<Key, String>,
    escalated: HashMap<Key, (String, String)>,
    answered: HashMap<Key, String>,
    tx: UnboundedSender<Finished>,
    rx: UnboundedReceiver<Finished>,
}

impl Decisions {
    pub(super) fn new() -> Self {
        let (tx, rx) = unbounded_channel();
        Self {
            deciding: HashMap::new(),
            escalated: HashMap::new(),
            answered: HashMap::new(),
            tx,
            rx,
        }
    }

    pub(super) fn collect(&mut self) {
        while let Ok(done) = self.rx.try_recv() {
            if self.deciding.get(&done.key) == Some(&done.question) {
                self.deciding.remove(&done.key);
            }
            match done.outcome {
                Outcome::Answered => {
                    self.answered.insert(done.key, done.question);
                }
                Outcome::Ask { advice } => {
                    self.escalated.insert(done.key, (done.question, advice));
                }
                Outcome::Gone => {}
            }
        }
    }

    pub(super) fn leave(&mut self, key: &Key) {
        self.deciding.remove(key);
        self.escalated.remove(key);
        self.answered.remove(key);
    }

    pub(super) fn gate(&mut self, key: &Key, state: &AgentState) -> Gate {
        let AgentState::Waiting { question } = state else {
            self.escalated.remove(key);
            self.answered.remove(key);
            return Gate::Pass;
        };
        if permissions::prompt_of(question, None).is_none() {
            return Gate::Pass;
        }
        let same = |seen: Option<&String>| seen == Some(question);
        if same(self.escalated.get(key).map(|(q, _)| q)) || same(self.answered.get(key)) {
            return Gate::Pass;
        }
        match same(self.deciding.get(key)) {
            true => Gate::Hold,
            false => Gate::Ask,
        }
    }

    pub(super) fn advice(&self, key: &Key, state: &AgentState) -> Option<&str> {
        let AgentState::Waiting { question } = state else {
            return None;
        };
        match self.escalated.get(key) {
            Some((asked, advice)) if asked == question => Some(advice),
            _ => None,
        }
    }

    pub(super) fn start(
        &mut self,
        key: Key,
        ctx: Context,
        ws: Workspace,
        info: AgentInfo,
        task: Task,
        limit: Duration,
    ) {
        let AgentState::Waiting { question } = info.state.clone() else {
            return;
        };
        self.deciding.insert(key.clone(), question.clone());
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let outcome = permissions::decide(&ctx, &ws, &info, &task, &question, limit).await;
            let _ = tx.send(Finished {
                key,
                question,
                outcome,
            });
        });
    }
}

pub(super) fn attach(change: Option<Change>, advice: Option<&str>) -> Option<Change> {
    match (change, advice) {
        (Some(mut change), Some(text)) => {
            if let Some(EntryBody::NeedsYou { advice: slot, .. }) = change.entry.as_mut() {
                *slot = Some(text.to_string());
            }
            Some(change)
        }
        (change, _) => change,
    }
}
