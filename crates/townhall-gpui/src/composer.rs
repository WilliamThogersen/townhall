use std::sync::Arc;

use gpui::{
    div, prelude::*, px, App, Context, Entity, Focusable, MouseButton, SharedString, Subscription,
    Window,
};
use gpui_component::input::{Enter, InputEvent, Textarea, TextareaState};

use crate::client::Api;
use crate::drafts::Drafts;
use crate::theme::{self, Theme};

const MIN_ROWS: usize = 2;
const MAX_ROWS: usize = 8;
const PLACEHOLDER: &str = "reply, or ask for more work";
const THINKING_PLACEHOLDER: &str = "townhall is thinking…";
const HELP: &str = "⏎ send · ⇧⏎ new line";

pub struct Composer {
    api: Arc<dyn Api>,
    drafts: Drafts,
    input: Entity<TextareaState>,
    conversation: Option<String>,
    thinking: bool,
    sending: bool,
    error: Option<String>,
    _subscription: Subscription,
}

impl Composer {
    pub fn new(
        api: Arc<dyn Api>,
        drafts: Drafts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(MIN_ROWS, MAX_ROWS)
                .submit_on_enter(true)
                .placeholder(PLACEHOLDER)
        });
        let subscription = cx.subscribe_in(&input, window, Self::on_input);
        Self {
            api,
            drafts,
            input,
            conversation: None,
            thinking: false,
            sending: false,
            error: None,
            _subscription: subscription,
        }
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
        if let Err(e) = self.drafts.set(&id, &text) {
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
        self.conversation.is_some()
            && !self.thinking
            && !self.sending
            && !self.text(cx).trim().is_empty()
    }

    pub fn switch_to(&mut self, id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if self.conversation == id {
            return;
        }
        self.conversation = id;
        self.error = None;
        let draft = self
            .conversation
            .as_deref()
            .map(|id| self.drafts.get(id).to_string())
            .unwrap_or_default();
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
        let placeholder = if thinking {
            THINKING_PLACEHOLDER
        } else {
            PLACEHOLDER
        };
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.focus(window, cx));
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_send(cx) {
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
        cx.spawn_in(window, async move |this, cx| -> gpui::Result<()> {
            let sent_to = id.clone();
            let result = cx
                .background_executor()
                .spawn(async move { api.send_message(&sent_to, &text) })
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
        }
        if let Err(e) = self.drafts.set(id, "") {
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
        let label = if self.sending { "sending…" } else { "send" };
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
        let (help, help_color): (SharedString, _) = match &self.error {
            Some(e) => (format!("error: {e}").into(), p.danger),
            None => (HELP.into(), p.fg_faint),
        };
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
                                        .text_color(p.fg),
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
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_size(px(theme::TEXT_XS))
                                    .text_color(help_color)
                                    .child(help),
                            )
                            .child(self.button(t, enabled, cx)),
                    ),
            )
    }
}
