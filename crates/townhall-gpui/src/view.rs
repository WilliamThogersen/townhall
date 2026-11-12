use std::sync::Arc;

use gpui::{
    div, prelude::*, px, AnyElement, ClickEvent, Context, FontWeight, IntoElement, Rgba,
    SharedString, Window,
};
use townhall_core::model::{AgentEvent, ConversationStatus, EntryBody, ServerEvent, ThreadEntry};

use crate::client::Api;
use crate::state::{Load, Refetch, State};
use crate::theme::{self, Theme};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub ended: bool,
    pub selected: bool,
    pub tasks: usize,
    pub thinking: bool,
}

pub struct Root {
    api: Arc<dyn Api>,
    state: State,
    daemon_note: SharedString,
}

impl Root {
    pub fn new(
        api: Arc<dyn Api>,
        events: Option<async_channel::Receiver<ServerEvent>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let root = Self {
            api,
            state: State::new(),
            daemon_note: SharedString::default(),
        };
        root.load_list(true, cx);
        if let Some(events) = events {
            cx.spawn(async move |this, cx| -> gpui::Result<()> {
                while let Ok(event) = events.recv().await {
                    this.update(cx, |this, cx| this.on_event(event, cx))?;
                }
                Ok(())
            })
            .detach_and_log_err(cx);
        }
        root
    }

    pub fn with_daemon_note(mut self, note: impl Into<SharedString>) -> Self {
        self.daemon_note = note.into();
        self
    }

    #[cfg(test)]
    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn active_rows(&self) -> Vec<Row> {
        self.rows(false)
    }

    pub fn older_rows(&self) -> Vec<Row> {
        self.rows(true)
    }

    fn rows(&self, ended: bool) -> Vec<Row> {
        match &self.state.conversations {
            Load::Ready(list) => list
                .iter()
                .filter(|c| c.ended == ended)
                .map(|c| Row {
                    id: c.id.clone(),
                    title: c.title.clone(),
                    ended: c.ended,
                    selected: self.state.selected.as_deref() == Some(c.id.as_str()),
                    tasks: c.tasks.len(),
                    thinking: c.status == ConversationStatus::Thinking,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn on_event(&mut self, event: ServerEvent, cx: &mut Context<Self>) {
        match self.state.apply(event) {
            Refetch::Nothing => {}
            Refetch::Everything => {
                self.load_list(false, cx);
                if let Some(id) = self.state.selected.clone() {
                    self.load_detail(id, cx);
                }
            }
        }
        cx.notify();
    }

    fn load_list(&self, select_newest: bool, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            let result = cx
                .background_executor()
                .spawn(async move { api.conversations() })
                .await;
            this.update(cx, |this, cx| {
                this.state.set_list(result);
                if select_newest && this.state.selected.is_none() {
                    if let Some(id) = this.state.newest() {
                        this.select(id, cx);
                    }
                }
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    pub fn select(&mut self, id: String, cx: &mut Context<Self>) {
        self.state.select(id.clone());
        self.load_detail(id, cx);
        cx.notify();
    }

    fn load_detail(&self, id: String, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            let wanted = id.clone();
            let result = cx
                .background_executor()
                .spawn(async move { api.conversation(&wanted) })
                .await;
            this.update(cx, |this, cx| {
                this.state.set_detail(&id, result);
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    fn sidebar(&self, t: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let p = t.palette;
        let mut rows = div().flex().flex_col();
        match &self.state.conversations {
            Load::Loading => rows = rows.child(note(p.fg_faint, "loading…")),
            Load::Failed(e) => rows = rows.child(note(t.status_error(), format!("error: {e}"))),
            Load::Ready(_) => {
                let active = self.active_rows();
                rows = rows.child(section(t, "active", active.len()));
                for row in active {
                    rows = rows.child(self.row(t, row, cx));
                }
                let older = self.older_rows();
                rows = rows.child(section(t, "older", older.len()));
                for row in older {
                    rows = rows.child(self.row(t, row, cx));
                }
            }
        }
        div()
            .w(px(theme::SIDEBAR))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(p.surface)
            .border_r(px(theme::BORDER))
            .border_color(p.line)
            .child(
                div()
                    .h(px(theme::BAR))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px(px(theme::SPACE_3))
                    .gap(px(theme::SPACE_2))
                    .border_b(px(theme::BORDER))
                    .border_color(p.line)
                    .child(div().text_color(p.fg).child("▟"))
                    .child(
                        div()
                            .flex()
                            .font_weight(FontWeight::BOLD)
                            .child(div().text_color(p.accent).child("/"))
                            .child("townhall"),
                    ),
            )
            .child(
                div()
                    .id("conversations")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(rows),
            )
    }

    fn row(&self, t: Theme, row: Row, cx: &mut Context<Self>) -> impl IntoElement {
        let p = t.palette;
        let (glyph, color) = if row.ended {
            ("○", t.status_dead())
        } else if row.thinking {
            ("⠋", t.status_working())
        } else {
            ("●", t.status_alive())
        };
        let id = row.id.clone();
        let mut title = div()
            .flex_1()
            .overflow_hidden()
            .text_color(if row.ended { p.fg_muted } else { p.fg })
            .child(row.title.clone());
        if row.selected {
            title = title.font_weight(FontWeight::BOLD);
        }
        let mut el = div()
            .id(SharedString::from(row.id.clone()))
            .h(px(theme::ROW))
            .flex()
            .items_center()
            .gap(px(theme::SPACE_2))
            .px(px(theme::SPACE_3))
            .cursor_pointer()
            .hover(move |s| s.bg(p.surface_2))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.select(id.clone(), cx);
            }))
            .child(div().w(px(theme::SPACE_4)).text_color(color).child(glyph))
            .child(title)
            .child(
                div()
                    .text_size(px(theme::TEXT_XS))
                    .text_color(p.fg_faint)
                    .child(format!("{} tasks", row.tasks)),
            );
        if row.selected {
            el = el
                .bg(p.surface_2)
                .border_l(px(theme::ACCENT_RULE))
                .border_color(p.accent);
        }
        el
    }

    fn thread(&self, t: Theme, pane_width: f32) -> impl IntoElement {
        let p = t.palette;
        let (title, body): (String, AnyElement) = match &self.state.detail {
            Load::Loading => (
                String::new(),
                note(p.fg_faint, "loading…").into_any_element(),
            ),
            Load::Failed(e) => (
                String::new(),
                note(t.status_error(), format!("error: {e}")).into_any_element(),
            ),
            Load::Ready(d) => {
                let mut col = div()
                    .w_full()
                    .max_w(px(theme::measure(pane_width)))
                    .flex()
                    .flex_col()
                    .gap(px(theme::SPACE_3))
                    .py(px(theme::SPACE_4));
                for entry in &d.entries {
                    col = col.child(entry_view(t, entry));
                }
                (d.conversation.title.clone(), col.into_any_element())
            }
        };
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(theme::BAR))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px(px(theme::SPACE_4))
                    .border_b(px(theme::BORDER))
                    .border_color(p.line)
                    .font_weight(FontWeight::BOLD)
                    .child(title),
            )
            .child(
                div()
                    .id("thread")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .items_center()
                    .px(px(theme::SPACE_4))
                    .child(body),
            )
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx);
        let p = t.palette;
        let pane_width =
            f32::from(window.viewport_size().width) - theme::SIDEBAR - 2.0 * theme::SPACE_4;
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.fg)
            .font_family(theme::FONT)
            .font_features(theme::no_ligatures())
            .text_size(px(theme::TEXT_MD))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.sidebar(t, cx))
                    .child(self.thread(t, pane_width)),
            )
            .child(
                div()
                    .h(px(theme::KEYBAR))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px(px(theme::SPACE_3))
                    .border_t(px(theme::BORDER))
                    .border_color(p.line)
                    .bg(p.surface)
                    .text_size(px(theme::TEXT_XS))
                    .text_color(p.fg_faint)
                    .child(format!("read-only · {}", self.daemon_note)),
            )
    }
}

fn note(color: Rgba, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .px(px(theme::SPACE_3))
        .py(px(theme::SPACE_2))
        .text_color(color)
        .child(text.into())
}

fn section(t: Theme, label: &str, count: usize) -> impl IntoElement {
    div()
        .h(px(theme::KEYBAR))
        .flex()
        .items_center()
        .px(px(theme::SPACE_3))
        .text_size(px(theme::TEXT_XS))
        .text_color(t.palette.fg_faint)
        .child(format!("{label} {count}"))
}

fn event_line(t: Theme, glyph: &str, color: Rgba, text: String) -> AnyElement {
    div()
        .flex()
        .justify_center()
        .gap(px(theme::SPACE_2))
        .text_size(px(theme::TEXT_SM))
        .text_color(t.palette.fg_faint)
        .child(div().text_color(color).child(glyph.to_string()))
        .child(text)
        .into_any_element()
}

fn bubble(t: Theme, tint: Rgba, rule: Rgba, from_you: bool, text: String) -> AnyElement {
    let mut el = div()
        .w_full()
        .max_w(gpui::relative(theme::BUBBLE))
        .px(px(theme::SPACE_3))
        .py(px(theme::SPACE_2))
        .bg(tint)
        .text_color(t.palette.fg)
        .child(text);
    el = if from_you {
        el.border_r(px(theme::ACCENT_RULE))
    } else {
        el.border_l(px(theme::ACCENT_RULE))
    };
    let row = div().flex().w_full();
    let row = if from_you { row.justify_end() } else { row };
    row.child(el.border_color(rule)).into_any_element()
}

fn entry_view(t: Theme, entry: &ThreadEntry) -> AnyElement {
    let p = t.palette;
    match &entry.body {
        EntryBody::You { text, .. } => bubble(t, p.surface_2, p.accent, true, text.clone()),
        EntryBody::Townhall { text } => bubble(t, p.info_tint, p.info, false, text.clone()),
        EntryBody::Repo {
            repo,
            base_branch,
            reason,
            ..
        } => event_line(
            t,
            "~",
            p.fg_faint,
            format!("repo {repo} · {base_branch}, {reason}"),
        ),
        EntryBody::RepoAsk { question, .. } => {
            event_line(t, "◆", t.status_waiting(), question.clone())
        }
        EntryBody::TasksCreated { tasks } => event_line(
            t,
            "+",
            p.fg_faint,
            format!("{} tasks: {}", tasks.len(), tasks.join(", ")),
        ),
        EntryBody::NeedsYou {
            agent, question, ..
        } => bubble(
            t,
            p.accent_tint,
            p.accent,
            false,
            format!("◆ {agent} needs you\n{question}"),
        ),
        EntryBody::Agent { agent, event, text } => {
            let (glyph, color) = match event {
                AgentEvent::Sent => ("→", p.fg_faint),
                AgentEvent::Passed => ("✓", p.ok),
                AgentEvent::Failed => ("×", p.danger),
                AgentEvent::Ended => ("○", p.fg_faint),
                AgentEvent::Allowed => ("+", p.fg_faint),
            };
            event_line(t, glyph, color, format!("{agent}: {text}"))
        }
        EntryBody::Event { text } => event_line(t, "◇", p.fg_faint, text.clone()),
        EntryBody::Handoff {
            task,
            from,
            to,
            reason,
        } => event_line(
            t,
            "↩",
            p.fg_faint,
            format!("{task}: {from} handed back to {to}, {reason}"),
        ),
        EntryBody::Plugin { plugin, text } => {
            event_line(t, "◇", p.fg_faint, format!("plugin {plugin}: {text}"))
        }
        EntryBody::Error { message } => event_line(t, "×", p.danger, format!("error: {message}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests::{conversation, detail, entry};
    use gpui::TestAppContext;
    use std::sync::Mutex;
    use townhall_core::model::{Conversation, ConversationDetail};

    struct Fake {
        list: Mutex<Vec<Conversation>>,
        details: Mutex<Vec<ConversationDetail>>,
    }

    impl Api for Fake {
        fn conversations(&self) -> Result<Vec<Conversation>, String> {
            Ok(self.list.lock().unwrap().clone())
        }

        fn conversation(&self, id: &str) -> Result<ConversationDetail, String> {
            self.details
                .lock()
                .unwrap()
                .iter()
                .find(|d| d.conversation.id == id)
                .cloned()
                .ok_or_else(|| format!("no conversation {id}"))
        }
    }

    fn fake() -> Arc<Fake> {
        let mut ended = conversation("b", "finished work", "2026-11-12T07:00:00.000Z");
        ended.ended = true;
        Arc::new(Fake {
            list: Mutex::new(vec![
                conversation("a", "fix the build", "2026-11-12T10:00:00.000Z"),
                ended,
            ]),
            details: Mutex::new(vec![
                detail("a", vec![entry(1, "on it")]),
                detail("b", vec![entry(1, "done")]),
            ]),
        })
    }

    fn titles(rows: Vec<Row>) -> Vec<String> {
        rows.into_iter().map(|r| r.title).collect()
    }

    fn entry_count(root: &Root) -> usize {
        match &root.state().detail {
            Load::Ready(d) => d.entries.len(),
            other => panic!("{other:?}"),
        }
    }

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_global(Theme::new(theme::Mode::Dark)));
    }

    #[gpui::test]
    fn sidebar_lists_conversations_from_the_client_and_opens_the_newest(cx: &mut TestAppContext) {
        init(cx);
        let (tx, rx) = async_channel::unbounded();
        let _keep = tx;
        let (root, vcx) = cx.add_window_view(|_, cx| Root::new(fake(), Some(rx), cx));
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| {
            assert_eq!(titles(root.active_rows()), ["fix the build"]);
            assert_eq!(titles(root.older_rows()), ["finished work"]);
            assert!(root.active_rows()[0].selected);
            assert_eq!(entry_count(root), 1);
        });
    }

    #[gpui::test]
    fn selecting_a_row_loads_its_thread(cx: &mut TestAppContext) {
        init(cx);
        let (root, vcx) = cx.add_window_view(|_, cx| Root::new(fake(), None, cx));
        vcx.run_until_parked();
        root.update(vcx, |root, cx| root.select("b".into(), cx));
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| {
            assert!(root.older_rows()[0].selected);
            match &root.state().detail {
                Load::Ready(d) => assert_eq!(d.conversation.id, "b"),
                other => panic!("{other:?}"),
            }
        });
    }

    #[gpui::test]
    fn events_update_the_views_without_refetching(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let (tx, rx) = async_channel::unbounded();
        let (root, vcx) = cx.add_window_view(|_, cx| Root::new(api.clone(), Some(rx), cx));
        vcx.run_until_parked();

        api.list.lock().unwrap().clear();
        tx.send_blocking(ServerEvent::EntryAppended {
            conversation_id: "a".into(),
            entry: entry(2, "second"),
        })
        .unwrap();
        tx.send_blocking(ServerEvent::ConversationUpdated {
            conversation: conversation("c", "brand new", "2026-11-12T11:00:00.000Z"),
        })
        .unwrap();
        vcx.run_until_parked();

        root.read_with(vcx, |root, _| {
            assert_eq!(entry_count(root), 2);
            assert_eq!(titles(root.active_rows()), ["brand new", "fix the build"]);
        });
    }

    #[gpui::test]
    fn resync_refetches_the_list_and_the_open_thread(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let (tx, rx) = async_channel::unbounded();
        let (root, vcx) = cx.add_window_view(|_, cx| Root::new(api.clone(), Some(rx), cx));
        vcx.run_until_parked();

        api.list.lock().unwrap().push(conversation(
            "z",
            "appeared while offline",
            "2026-11-12T12:00:00.000Z",
        ));
        api.details.lock().unwrap()[0]
            .entries
            .push(entry(2, "missed"));
        tx.send_blocking(ServerEvent::Resync).unwrap();
        vcx.run_until_parked();

        root.read_with(vcx, |root, _| {
            assert_eq!(
                titles(root.active_rows()),
                ["appeared while offline", "fix the build"]
            );
            assert_eq!(entry_count(root), 2);
        });
    }

    #[gpui::test]
    fn a_failing_client_shows_an_error_instead_of_rows(cx: &mut TestAppContext) {
        init(cx);
        struct Down;
        impl Api for Down {
            fn conversations(&self) -> Result<Vec<Conversation>, String> {
                Err("connection refused".into())
            }
            fn conversation(&self, _: &str) -> Result<ConversationDetail, String> {
                Err("connection refused".into())
            }
        }
        let (root, vcx) = cx.add_window_view(|_, cx| Root::new(Arc::new(Down), None, cx));
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| {
            assert_eq!(
                root.state().conversations,
                Load::Failed("connection refused".into())
            );
            assert!(root.active_rows().is_empty());
        });
    }
}
