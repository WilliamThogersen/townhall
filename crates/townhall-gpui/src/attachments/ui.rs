use gpui::{
    div, prelude::*, px, transparent_black, Div, MouseButton, MouseDownEvent, SharedString,
    Stateful, Window,
};

use crate::theme::{self, Theme};

pub const GHOST_GAP: f32 = theme::TEXT_SM * 0.6;

pub fn kbd(t: &Theme, key: &str) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .min_w(px(18.0))
        .h(px(18.0))
        .px(px(theme::SPACE_1))
        .border(px(theme::BORDER))
        .border_color(t.palette.line_strong)
        .text_size(px(theme::TEXT_XS))
        .text_color(t.palette.fg_muted)
        .line_height(px(theme::TEXT_XS))
        .child(key.to_string())
}

pub type Handler = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut gpui::App)>;

pub fn ghost_button(
    t: &Theme,
    id: impl Into<SharedString>,
    enabled: bool,
    on_press: Handler,
) -> Stateful<Div> {
    let p = t.palette;
    let button = div()
        .id(id.into())
        .h(px(theme::CONTROL_SM))
        .px(px(theme::SPACE_2))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(GHOST_GAP))
        .border(px(theme::BORDER))
        .border_color(transparent_black())
        .text_size(px(theme::TEXT_XS))
        .whitespace_nowrap();
    if enabled {
        button
            .text_color(p.fg_muted)
            .cursor_pointer()
            .hover(move |s| s.text_color(p.fg))
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                on_press(event, window, cx)
            })
    } else {
        button.text_color(p.fg_faint)
    }
}
