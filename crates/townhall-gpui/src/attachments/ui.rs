use gpui::{
    canvas, div, prelude::*, px, transparent_black, Div, MouseButton, MouseDownEvent, PathBuilder,
    Rgba, SharedString, Stateful, Window,
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

pub fn file_icon(color: Rgba) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let origin = bounds.origin;
            let at = |x: f32, y: f32| origin + gpui::point(px(x), px(y));
            let mut outline = PathBuilder::stroke(px(1.0));
            outline.move_to(at(1.5, 1.5));
            outline.line_to(at(9.5, 1.5));
            outline.line_to(at(14.5, 6.5));
            outline.line_to(at(14.5, 18.5));
            outline.line_to(at(1.5, 18.5));
            outline.close();
            if let Ok(path) = outline.build() {
                window.paint_path(path, color);
            }
            let mut fold = PathBuilder::stroke(px(1.0));
            fold.move_to(at(9.5, 1.5));
            fold.line_to(at(9.5, 6.5));
            fold.line_to(at(14.5, 6.5));
            if let Ok(path) = fold.build() {
                window.paint_path(path, color);
            }
        },
    )
    .flex_none()
    .w(px(16.0))
    .h(px(20.0))
}

pub fn drop_zone(t: &Theme) -> Div {
    let p = t.palette;
    div()
        .debug_selector(|| "drop-zone".to_string())
        .absolute()
        .top(px(theme::SPACE_2))
        .left(px(theme::SPACE_2))
        .right(px(theme::SPACE_2))
        .bottom(px(theme::SPACE_2))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(theme::SPACE_1))
        .border(px(theme::BORDER))
        .border_dashed()
        .border_color(p.accent)
        .bg(p.accent_tint)
        .text_color(p.accent)
        .text_size(px(theme::TEXT_SM))
        .child("drop to attach")
        .child(
            div()
                .text_size(px(theme::TEXT_XS))
                .text_color(p.fg_muted)
                .child("images, text, code, pdf · up to 20 MB each"),
        )
}

pub const THUMB_H: f32 = 48.0;
pub const FILE_W: f32 = 220.0;

pub fn file_card(t: &Theme, name: &str, detail: &str, failed: bool) -> Div {
    let p = t.palette;
    div()
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(theme::SPACE_2))
        .w(px(FILE_W))
        .max_w_full()
        .h(px(THUMB_H))
        .px(px(theme::SPACE_2))
        .border(px(theme::BORDER))
        .border_color(if failed { p.danger } else { p.line_strong })
        .bg(p.surface_2)
        .text_color(p.fg)
        .text_size(px(theme::TEXT_SM))
        .child(file_icon(p.fg_muted))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().truncate().child(name.to_string()))
                .child(
                    div()
                        .truncate()
                        .text_size(px(theme::TEXT_XS))
                        .text_color(if failed { p.danger } else { p.fg_faint })
                        .child(format!(
                            "{} · {detail}",
                            super::format::extension_tag(name).to_uppercase()
                        )),
                ),
        )
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
