use std::sync::Arc;

use gpui::{
    div, prelude::*, px, App, Context, Entity, EventEmitter, Focusable, MouseButton, SharedString,
    Subscription, Window,
};
use gpui_component::input::{Enter, InputEvent, Textarea, TextareaState};
use townhall_core::model::ConversationDetail;

use crate::client::Api;
use crate::drafts::Drafts;
use crate::theme::{self, Theme};

#[path = "attachments/composer_attach.rs"]
mod attach;

const MIN_ROWS: usize = 2;
const START_ROWS: usize = 3;
const MAX_ROWS: usize = 8;
const PLACEHOLDER: &str = "reply, or ask for more work";
const START_PLACEHOLDER: &str = "what should we work on? name a repo, or describe the code";
const THINKING_PLACEHOLDER: &str = "townhall is thinking…";
const HELP: &str = "⏎ send · ⇧⏎ new line";
const START_HELP: &str = "⏎ start · ⇧⏎ new line";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Reply,
    Start,
}

pub struct Started(pub ConversationDetail);

impl EventEmitter<Started> for Composer {}

pub struct Composer {
    api: Arc<dyn Api>,
    drafts: Option<Drafts>,
    mode: Mode,
    input: Entity<TextareaState>,
    conversation: Option<String>,
    thinking: bool,
    sending: bool,
    error: Option<String>,
    staging: attach::Staging,
    _subscription: Subscription,
}

impl Composer {
    pub fn new(
        api: Arc<dyn Api>,
        drafts: Drafts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(api, Some(drafts), Mode::Reply, window, cx)
    }

    pub fn new_start(api: Arc<dyn Api>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::build(api, None, Mode::Start, window, cx)
    }

    fn build(
        api: Arc<dyn Api>,
        drafts: Option<Drafts>,
        mode: Mode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (rows, placeholder) = match mode {
            Mode::Reply => (MIN_ROWS, PLACEHOLDER),
            Mode::Start => (START_ROWS, START_PLACEHOLDER),
        };
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(rows, MAX_ROWS)
                .submit_on_enter(true)
                .placeholder(placeholder)
        });
        let subscription = cx.subscribe_in(&input, window, Self::on_input);
        Self {
            api,
            drafts,
            mode,
            input,
            conversation: None,
            thinking: false,
            sending: false,
            error: None,
            staging: attach::Staging::default(),
            _subscription: subscription,
        }
    }

    pub fn fill(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.set_value(text.to_string(), window, cx)
        });
        self.focus(window, cx);
    }

    fn on_input(
        &mut self,
        _: &Entity<TextareaState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                self.error = None;
                self.save_draft(cx);
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {}
            InputEvent::Focus | InputEvent::Blur => cx.notify(),
        }
    }

    fn save_draft(&mut self, cx: &App) {
        let Some(id) = self.conversation.clone() else {
            return;
        };
        let text = self.text(cx);
        let Some(drafts) = self.drafts.as_mut() else {
            return;
        };
        if let Err(e) = drafts.set(&id, &text) {
            tracing::warn!(
                target: "townhall::native",
                event = "drafts.save_failed",
                error = %e,
                "cannot save the draft"
            );
        }
    }

    #[cfg(test)]
    pub fn input(&self) -> &Entity<TextareaState> {
        &self.input
    }

    pub fn text(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }

    #[cfg(test)]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    #[cfg(test)]
    pub fn is_sending(&self) -> bool {
        self.sending
    }

    pub fn can_send(&self, cx: &App) -> bool {
        let has_text = !self.text(cx).trim().is_empty();
        match self.mode {
            Mode::Reply => {
                let has_content = has_text || !self.staging.ids().is_empty();
                self.conversation.is_some()
                    && !self.thinking
                    && !self.sending
                    && !self.staging.uploading()
                    && has_content
            }
            Mode::Start => !self.sending && has_text,
        }
    }

    pub fn switch_to(&mut self, id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if self.conversation == id {
            return;
        }
        self.conversation = id;
        self.error = None;
        self.staging.clear();
        let draft = match (self.conversation.as_deref(), self.drafts.as_ref()) {
            (Some(id), Some(drafts)) => drafts.get(id).to_string(),
            _ => String::new(),
        };
        self.input
            .update(cx, |input, cx| input.set_value(draft, window, cx));
        self.focus(window, cx);
        cx.notify();
    }

    pub fn set_thinking(&mut self, thinking: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.thinking == thinking {
            return;
        }
        self.thinking = thinking;
        self.refresh_placeholder(window, cx);
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.focus(window, cx));
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.text(cx).trim().to_string();
        self.sending = true;
        self.error = None;
        self.input
            .update(cx, |input, cx| input.set_readonly(true, cx));
        cx.notify();
        let api = self.api.clone();
        cx.spawn_in(window, async move |this, cx| -> gpui::Result<()> {
            let result = cx
                .background_executor()
                .spawn(async move { api.start_conversation(&text) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.sending = false;
                this.input
                    .update(cx, |input, cx| input.set_readonly(false, cx));
                match result {
                    Ok(detail) => {
                        this.input
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        cx.emit(Started(detail));
                    }
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_send(cx) {
            return;
        }
        if self.mode == Mode::Start {
            self.start(window, cx);
            return;
        }
        let Some(id) = self.conversation.clone() else {
            return;
        };
        let text = self.text(cx).trim().to_string();
        self.sending = true;
        self.error = None;
        self.input
            .update(cx, |input, cx| input.set_readonly(true, cx));
        cx.notify();
        let api = self.api.clone();
        let ids = self.staging.ids();
        let attach = if ids.is_empty() {
            None
        } else {
            self.staging.api()
        };
        cx.spawn_in(window, async move |this, cx| -> gpui::Result<()> {
            let sent_to = id.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    match attach {
                        Some(attach) => attach.send(&sent_to, &text, &ids),
                        None => api.send_message(&sent_to, &text),
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.sending = false;
                this.input
                    .update(cx, |input, cx| input.set_readonly(false, cx));
                match result {
                    Ok(_) => this.sent(&id, window, cx),
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    fn sent(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.conversation.as_deref() == Some(id) {
            self.input
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.staging.finish_sent();
        }
        let Some(drafts) = self.drafts.as_mut() else {
            return;
        };
        if let Err(e) = drafts.set(id, "") {
            tracing::warn!(
                target: "townhall::native",
                event = "drafts.save_failed",
                error = %e,
                "cannot clear the draft"
            );
        }
    }

    fn button(&self, t: Theme, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let p = t.palette;
        let label = match (self.mode, self.sending) {
            (Mode::Reply, false) => "send",
            (Mode::Reply, true) => "sending…",
            (Mode::Start, false) => "start",
            (Mode::Start, true) => "starting…",
        };
        let mut button = div()
            .id("send")
            .h(px(theme::ROW - theme::SPACE_2))
            .px(px(theme::SPACE_3))
            .flex()
            .items_center()
            .border(px(theme::BORDER))
            .text_size(px(theme::TEXT_SM));
        if enabled {
            button = button
                .border_color(p.line_control)
                .text_color(p.fg)
                .cursor_pointer()
                .hover(move |s| s.bg(p.surface_2).border_color(p.fg_faint))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.submit(window, cx)),
                );
        } else {
            button = button.border_color(p.line).text_color(p.fg_faint);
        }
        button.child(label)
    }
}

impl Render for Composer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx);
        let p = t.palette;
        let focused = self.input.focus_handle(cx).contains_focused(window, cx);
        let enabled = self.can_send(cx);
        let shown_error = self.error.clone().or_else(|| self.staging.error());
        let (help, help_color): (SharedString, _) = match &shown_error {
            Some(e) => (format!("error: {e}").into(), p.danger),
            None if self.mode == Mode::Start => (START_HELP.into(), p.fg_faint),
            None if self.staging.enabled() => (attach::ATTACH_HELP.into(), p.fg_faint),
            None => (HELP.into(), p.fg_faint),
        };
        let paste_target = cx.entity();
        let chips = self.upload_chips(&t, cx);
        let attach_button = self.attach_button(&t, cx);
        let retry_button = self.retry_button(&t, cx);
        div()
            .w_full()
            .flex_none()
            .px(px(theme::SPACE_4))
            .pb(px(theme::SPACE_3))
            .on_action(cx.listener(|this, _: &Enter, window, cx| this.submit(window, cx)))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .bg(p.surface)
                    .border(px(theme::BORDER))
                    .border_color(if focused { p.accent } else { p.line_control })
                    .when_some(chips, |d, chips| d.child(chips))
                    .child(
                        div()
                            .flex()
                            .px(px(theme::SPACE_3))
                            .pt(px(theme::SPACE_2))
                            .gap(px(theme::SPACE_2))
                            .child(
                                div()
                                    .text_color(if focused { p.accent } else { p.fg_faint })
                                    .child("›"),
                            )
                            .child(
                                div().flex_1().min_w_0().child(
                                    Textarea::new(&self.input)
                                        .appearance(false)
                                        .bordered(false)
                                        .text_color(p.fg)
                                        .on_paste(move |item, _, cx| {
                                            paste_target
                                                .update(cx, |this, cx| this.paste(item, cx))
                                        }),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(theme::SPACE_3))
                            .px(px(theme::SPACE_3))
                            .pb(px(theme::SPACE_2))
                            .when_some(attach_button, |d, button| d.child(button))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_size(px(theme::TEXT_XS))
                                    .text_color(help_color)
                                    .child(help),
                            )
                            .when_some(retry_button, |d, button| d.child(button))
                            .child(self.button(t, enabled, cx)),
                    ),
            )
    }
}
