use gpui::{
    div, prelude::*, px, AnyElement, App, ElementId, FontWeight, MouseButton, MouseDownEvent,
    Window,
};

use crate::home_model::NO_REPOS_TEXT;
use crate::theme::{self, Theme};

const DAEMON_PREFIX: &str = "cannot reach the daemon: ";

pub fn daemon_error_text(message: &str) -> String {
    if message.starts_with(DAEMON_PREFIX) {
        message.to_string()
    } else {
        format!("{DAEMON_PREFIX}{message}")
    }
}

pub type Click = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

pub fn button(
    t: &Theme,
    id: impl Into<ElementId>,
    label: String,
    primary: bool,
    click: Click,
) -> AnyElement {
    let p = t.palette;
    let base = div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .gap(px(theme::TEXT_SM * 0.6))
        .h(px(theme::ROW))
        .px(px(10.0))
        .border(px(theme::BORDER))
        .text_size(px(theme::TEXT_SM))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, click);
    if primary {
        base.border_color(p.fg)
            .bg(p.fg)
            .text_color(p.on_inverse)
            .font_weight(FontWeight::BOLD)
            .child(label)
            .into_any_element()
    } else {
        base.border_color(p.line_control)
            .text_color(p.fg)
            .hover(move |s| s.bg(p.surface_2).border_color(p.fg_faint))
            .child(label)
            .into_any_element()
    }
}

pub fn starter(t: &Theme, id: impl Into<ElementId>, label: String, click: Click) -> AnyElement {
    let p = t.palette;
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .gap(px(theme::TEXT_SM * 0.6))
        .h(px(theme::CONTROL_SM))
        .px(px(theme::SPACE_2))
        .text_size(px(theme::TEXT_SM))
        .text_color(p.fg)
        .cursor_pointer()
        .hover(move |s| s.bg(p.surface_2))
        .on_mouse_down(MouseButton::Left, click)
        .child(div().text_color(p.fg_faint).child("›"))
        .child(label)
        .into_any_element()
}

pub fn greeting(t: &Theme, text: String) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(theme::SPACE_3))
        .text_size(px(theme::TEXT_XL))
        .line_height(px(theme::LEADING_XL))
        .font_weight(FontWeight::BOLD)
        .child(div().text_color(t.palette.accent).child("▟"))
        .child(text)
        .into_any_element()
}

pub fn banner(t: &Theme, text: String, retrying: bool, retry: Click) -> AnyElement {
    let p = t.palette;
    div()
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(theme::TEXT_SM * 0.6))
        .min_h(px(32.0))
        .py(px(theme::SPACE_1))
        .pl(px(theme::SPACE_3))
        .pr(px(theme::SPACE_2))
        .border_b(px(theme::BORDER))
        .border_l(px(theme::ACCENT_RULE))
        .border_color(p.danger)
        .bg(p.danger_tint)
        .text_size(px(theme::TEXT_SM))
        .child(
            div()
                .text_color(p.danger)
                .font_weight(FontWeight::BOLD)
                .child("error:"),
        )
        .child(div().flex_1().child(text))
        .child(button(
            t,
            "banner-retry",
            if retrying { "retrying…" } else { "retry" }.to_string(),
            false,
            retry,
        ))
        .into_any_element()
}

pub fn first_run(t: &Theme, error: Option<String>, add: Click) -> AnyElement {
    let p = t.palette;
    let mut col = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(theme::SPACE_3))
        .max_w(px(theme::HOME))
        .child(
            div()
                .text_size(px(theme::TEXT_XL))
                .text_color(p.accent)
                .child("▟"),
        )
        .child(
            div()
                .text_size(px(theme::TEXT_LG))
                .line_height(px(theme::LEADING_LG))
                .font_weight(FontWeight::BOLD)
                .child("add your first repository"),
        )
        .child(
            div()
                .text_color(p.fg_muted)
                .text_center()
                .child(NO_REPOS_TEXT),
        )
        .child(button(
            t,
            "add-repository",
            "add repository…".to_string(),
            true,
            add,
        ));
    if let Some(error) = error {
        col = col.child(
            div()
                .text_size(px(theme::TEXT_SM))
                .text_color(p.danger)
                .child(format!("error: {error}")),
        );
    }
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p(px(theme::SPACE_5))
        .child(col)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_daemon_error_says_it_could_not_reach_the_daemon() {
        assert_eq!(
            daemon_error_text("connection refused"),
            "cannot reach the daemon: connection refused"
        );
    }

    #[test]
    fn a_message_that_already_says_so_is_left_alone() {
        assert_eq!(
            daemon_error_text("cannot reach the daemon: timeout"),
            "cannot reach the daemon: timeout"
        );
    }
}
