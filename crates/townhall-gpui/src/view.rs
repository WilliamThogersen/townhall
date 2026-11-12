use gpui::{
    div, prelude::*, px, AnyElement, ClickEvent, Context, FontWeight, IntoElement, SharedString,
    Window,
};
use townhall_core::model::{
    AgentEvent, Conversation, ConversationDetail, ConversationStatus, EntryBody, ThreadEntry,
};

use crate::{api, theme};

enum Load<T> {
    Loading,
    Ready(T),
    Failed(String),
}

pub struct Root {
    base: String,
    conversations: Load<Vec<Conversation>>,
    selected: Option<String>,
    detail: Load<ConversationDetail>,
}

impl Root {
    pub fn new(base: String, cx: &mut Context<Self>) -> Self {
        let root = Self {
            base,
            conversations: Load::Loading,
            selected: None,
            detail: Load::Loading,
        };
        root.load_list(cx);
        root
    }

    fn load_list(&self, cx: &mut Context<Self>) {
        let base = self.base.clone();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            let result = cx
                .background_executor()
                .spawn(async move { api::conversations(&base) })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(mut list) => {
                        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                        let first = list.first().map(|c| c.id.clone());
                        this.conversations = Load::Ready(list);
                        if let Some(id) = first {
                            this.select(id, cx);
                        }
                    }
                    Err(e) => this.conversations = Load::Failed(e),
                }
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    fn select(&mut self, id: String, cx: &mut Context<Self>) {
        self.selected = Some(id.clone());
        self.detail = Load::Loading;
        let base = self.base.clone();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            let result = cx
                .background_executor()
                .spawn(async move { api::conversation(&base, &id).map(|d| (id, d)) })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok((id, detail)) if this.selected.as_deref() == Some(id.as_str()) => {
                        this.detail = Load::Ready(detail)
                    }
                    Ok(_) => {}
                    Err(e) => this.detail = Load::Failed(e),
                }
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (active, older): (Vec<_>, Vec<_>) = match &self.conversations {
            Load::Ready(list) => list.iter().partition(|c| !c.ended),
            _ => (Vec::new(), Vec::new()),
        };
        let mut rows = div().flex().flex_col();
        match &self.conversations {
            Load::Loading => rows = rows.child(note("loading…")),
            Load::Failed(e) => rows = rows.child(note(format!("error: {e}"))),
            Load::Ready(_) => {
                rows = rows.child(section("active", active.len()));
                for c in active {
                    rows = rows.child(self.row(c, cx));
                }
                rows = rows.child(section("older", older.len()));
                for c in older {
                    rows = rows.child(self.row(c, cx));
                }
            }
        }
        div()
            .w(px(theme::SIDEBAR))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme::surface())
            .border_r_1()
            .border_color(theme::line())
            .child(
                div()
                    .h(px(theme::BAR))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme::line())
                    .child(div().text_color(theme::fg()).child("▟"))
                    .child(
                        div()
                            .flex()
                            .font_weight(FontWeight::BOLD)
                            .child(div().text_color(theme::accent()).child("/"))
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

    fn row(&self, c: &Conversation, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.selected.as_deref() == Some(c.id.as_str());
        let (glyph, color) = if c.ended {
            ("○", theme::fg_faint())
        } else if c.status == ConversationStatus::Thinking {
            ("⠋", theme::info())
        } else {
            ("●", theme::ok())
        };
        let id = c.id.clone();
        let title_color = if c.ended {
            theme::fg_muted()
        } else {
            theme::fg()
        };
        let mut title = div()
            .flex_1()
            .overflow_hidden()
            .text_color(title_color)
            .child(c.title.clone());
        if current {
            title = title.font_weight(FontWeight::BOLD);
        }
        let mut row = div()
            .id(SharedString::from(c.id.clone()))
            .h(px(28.0))
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .cursor_pointer()
            .hover(|s| s.bg(theme::surface_2()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.select(id.clone(), cx);
            }))
            .child(div().w(px(16.0)).text_color(color).child(glyph))
            .child(title)
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::fg_faint())
                    .child(format!("{} tasks", c.tasks.len())),
            );
        if current {
            row = row
                .bg(theme::surface_2())
                .border_l_2()
                .border_color(theme::accent());
        }
        row
    }

    fn thread(&self) -> impl IntoElement {
        let (title, body): (String, AnyElement) = match &self.detail {
            Load::Loading => (String::new(), note("loading…").into_any_element()),
            Load::Failed(e) => (
                String::new(),
                note(format!("error: {e}")).into_any_element(),
            ),
            Load::Ready(d) => {
                let mut col = div()
                    .w_full()
                    .max_w(px(theme::MEASURE))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .py_4();
                for entry in &d.entries {
                    col = col.child(entry_view(entry));
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
                    .px_4()
                    .border_b_1()
                    .border_color(theme::line())
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
                    .px_4()
                    .child(body),
            )
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::bg())
            .text_color(theme::fg())
            .font_family(theme::FONT)
            .text_size(px(13.0))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.sidebar(cx))
                    .child(self.thread()),
            )
            .child(
                div()
                    .h(px(theme::KEYBAR))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px_3()
                    .border_t_1()
                    .border_color(theme::line())
                    .bg(theme::surface())
                    .text_size(px(11.0))
                    .text_color(theme::fg_faint())
                    .child("read-only spike"),
            )
    }
}

fn note(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .px_3()
        .py_2()
        .text_color(theme::fg_faint())
        .child(text.into())
}

fn section(label: &str, count: usize) -> impl IntoElement {
    div()
        .h(px(24.0))
        .flex()
        .items_center()
        .px_3()
        .text_size(px(11.0))
        .text_color(theme::fg_faint())
        .child(format!("{label} {count}"))
}

fn event_line(glyph: &str, color: gpui::Rgba, text: String) -> AnyElement {
    div()
        .flex()
        .justify_center()
        .gap_2()
        .text_size(px(12.0))
        .text_color(theme::fg_faint())
        .child(div().text_color(color).child(glyph.to_string()))
        .child(text)
        .into_any_element()
}

fn entry_view(entry: &ThreadEntry) -> AnyElement {
    match &entry.body {
        EntryBody::You { text, .. } => div()
            .flex()
            .justify_end()
            .child(
                div()
                    .max_w(px(theme::MEASURE * 0.85))
                    .px_3()
                    .py_2()
                    .bg(theme::surface_2())
                    .border_r_2()
                    .border_color(theme::accent())
                    .child(text.clone()),
            )
            .into_any_element(),
        EntryBody::Townhall { text } => div()
            .flex()
            .child(
                div()
                    .max_w(px(theme::MEASURE * 0.85))
                    .px_3()
                    .py_2()
                    .bg(theme::info_tint())
                    .border_l_2()
                    .border_color(theme::info())
                    .child(text.clone()),
            )
            .into_any_element(),
        EntryBody::Repo {
            repo,
            base_branch,
            reason,
            ..
        } => event_line(
            "~",
            theme::fg_faint(),
            format!("repo {repo} · {base_branch}, {reason}"),
        ),
        EntryBody::RepoAsk { question, .. } => event_line("◆", theme::accent(), question.clone()),
        EntryBody::TasksCreated { tasks } => event_line(
            "+",
            theme::fg_faint(),
            format!("{} tasks: {}", tasks.len(), tasks.join(", ")),
        ),
        EntryBody::NeedsYou {
            agent, question, ..
        } => div()
            .flex()
            .child(
                div()
                    .max_w(px(theme::MEASURE * 0.85))
                    .px_3()
                    .py_2()
                    .bg(theme::accent_tint())
                    .border_l_2()
                    .border_color(theme::accent())
                    .child(format!("◆ {agent} needs you\n{question}")),
            )
            .into_any_element(),
        EntryBody::Agent { agent, event, text } => {
            let (glyph, color) = match event {
                AgentEvent::Sent => ("→", theme::fg_faint()),
                AgentEvent::Passed => ("✓", theme::ok()),
                AgentEvent::Failed => ("×", theme::danger()),
                AgentEvent::Ended => ("○", theme::fg_faint()),
                AgentEvent::Allowed => ("+", theme::fg_faint()),
            };
            event_line(glyph, color, format!("{agent}: {text}"))
        }
        EntryBody::Event { text } => event_line("◇", theme::fg_faint(), text.clone()),
        EntryBody::Handoff {
            task,
            from,
            to,
            reason,
        } => event_line(
            "↩",
            theme::fg_faint(),
            format!("{task}: {from} handed back to {to}, {reason}"),
        ),
        EntryBody::Plugin { plugin, text } => {
            event_line("◇", theme::fg_faint(), format!("plugin {plugin}: {text}"))
        }
        EntryBody::Error { message } => {
            event_line("×", theme::danger(), format!("error: {message}"))
        }
    }
}
