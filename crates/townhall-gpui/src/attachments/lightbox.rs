use std::sync::Arc;

use gpui::{
    actions, div, img, point, prelude::*, px, relative, App, BoxShadow, ClipboardItem, Context,
    EventEmitter, FocusHandle, Focusable, FontWeight, Image, KeyBinding, MouseButton,
    MouseDownEvent, ObjectFit, Render, SharedString, Window,
};

use super::images::{self, Slot};
use super::ui::{ghost_button, kbd, GHOST_GAP};
use crate::theme::{self, Theme};

actions!(
    townhall_lightbox,
    [Close, Previous, Next, CopyImage, Download]
);

pub const CONTEXT: &str = "Lightbox";
const NAV_W: f32 = 36.0;
const NAV_H: f32 = 48.0;
const STAGE_PAD_X: f32 = 48.0;
const STAGE_PAD_Y: f32 = 24.0;
const STRIP_W: f32 = 48.0;
const STRIP_H: f32 = 36.0;
const FOOT_H: f32 = 55.0;

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Close, Some(CONTEXT)),
        KeyBinding::new("left", Previous, Some(CONTEXT)),
        KeyBinding::new("right", Next, Some(CONTEXT)),
        KeyBinding::new("c", CopyImage, Some(CONTEXT)),
    ]);
}

#[derive(Debug, Clone)]
pub enum Source {
    Local(Arc<Image>),
    Remote {
        conversation: String,
        id: String,
        mime: String,
    },
}

#[derive(Debug, Clone)]
pub struct LightboxImage {
    pub source: Source,
    pub name: String,
    pub meta: String,
}

type Resolved = (Arc<Image>, Option<(u32, u32)>);

pub struct Open {
    pub images: Vec<LightboxImage>,
    pub index: usize,
}

pub struct Closed;

impl EventEmitter<Closed> for Lightbox {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notice {
    pub index: usize,
    pub copied: bool,
    pub error: Option<String>,
}

pub struct Lightbox {
    images: Vec<LightboxImage>,
    index: usize,
    notice: Option<Notice>,
    focus: FocusHandle,
}

pub fn clamp_step(index: usize, delta: i32, len: usize) -> usize {
    let last = len.saturating_sub(1) as i32;
    (index as i32 + delta).clamp(0, last) as usize
}

pub fn fit_scale(natural: (u32, u32), available: (f32, f32)) -> f32 {
    let (w, h) = (natural.0.max(1) as f32, natural.1.max(1) as f32);
    (available.0 / w).min(available.1 / h).min(1.0)
}

pub fn meta_line(pixels: Option<(u32, u32)>, meta: &str) -> String {
    let size = pixels.map(|(w, h)| format!("{w} × {h}"));
    [size.as_deref().unwrap_or(""), meta]
        .iter()
        .filter(|part| !part.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" · ")
}

impl Lightbox {
    pub fn new(images: Vec<LightboxImage>, start: usize, cx: &mut Context<Self>) -> Self {
        let last = images.len().saturating_sub(1);
        Self {
            index: start.min(last),
            images,
            notice: None,
            focus: cx.focus_handle(),
        }
    }

    #[cfg(test)]
    pub fn index(&self) -> usize {
        self.index
    }

    #[cfg(test)]
    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    fn current(&self) -> Option<&LightboxImage> {
        self.images.get(self.index)
    }

    fn resolved(&self, cx: &mut App) -> Option<Resolved> {
        match &self.current()?.source {
            Source::Local(image) => Some((image.clone(), images::size_of(cx, image))),
            Source::Remote {
                conversation,
                id,
                mime,
            } => match images::slot(cx, conversation, id, mime) {
                Slot::Ready(loaded) => Some((loaded.image, loaded.size)),
                Slot::Loading | Slot::Failed => None,
            },
        }
    }

    pub fn step(&mut self, delta: i32, cx: &mut Context<Self>) {
        let next = clamp_step(self.index, delta, self.images.len());
        if next != self.index {
            self.index = next;
            cx.notify();
        }
    }

    pub fn pick(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.images.len() && index != self.index {
            self.index = index;
            cx.notify();
        }
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        cx.emit(Closed);
    }

    pub fn copy(&mut self, cx: &mut Context<Self>) {
        let name = self.current().map(|i| i.name.clone()).unwrap_or_default();
        self.notice = Some(match self.resolved(cx) {
            Some((image, _)) => {
                cx.write_to_clipboard(ClipboardItem::new_image(&image));
                Notice {
                    index: self.index,
                    copied: true,
                    error: None,
                }
            }
            None => Notice {
                index: self.index,
                copied: false,
                error: Some(format!(
                    "error: cannot copy {name}: the image has not loaded"
                )),
            },
        });
        cx.notify();
    }

    pub fn download(&mut self, cx: &mut Context<Self>) {
        let name = self.current().map(|i| i.name.clone()).unwrap_or_default();
        let result = match self.resolved(cx) {
            None => Err("the image has not loaded".to_string()),
            Some((image, _)) => images::download_path(&name)
                .ok_or_else(|| "no downloads folder".to_string())
                .and_then(|path| {
                    std::fs::write(&path, &image.bytes)
                        .map(|()| path)
                        .map_err(|e| e.to_string())
                }),
        };
        self.notice = Some(match result {
            Ok(path) => {
                cx.reveal_path(&path);
                Notice {
                    index: self.index,
                    copied: false,
                    error: None,
                }
            }
            Err(e) => Notice {
                index: self.index,
                copied: false,
                error: Some(format!("error: cannot download {name}: {e}")),
            },
        });
        cx.notify();
    }

    fn thumb(&self, t: &Theme, n: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let p = t.palette;
        let item = &self.images[n];
        let current = n == self.index;
        let source = match &item.source {
            Source::Local(image) => Some(image.clone()),
            Source::Remote {
                conversation,
                id,
                mime,
            } => match images::slot(cx, conversation, id, mime) {
                Slot::Ready(loaded) => Some(loaded.image),
                Slot::Loading | Slot::Failed => None,
            },
        };
        let mut cell = div()
            .id(SharedString::from(format!("strip-{n}")))
            .debug_selector(move || format!("strip-{n}"))
            .relative()
            .flex_none()
            .w(px(STRIP_W))
            .h(px(STRIP_H))
            .overflow_hidden()
            .border(px(theme::BORDER))
            .border_color(if current { p.accent } else { p.line_strong })
            .bg(p.surface_2)
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| this.pick(n, cx)),
            );
        if current {
            cell = cell.shadow(vec![BoxShadow {
                color: p.accent.into(),
                offset: point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(1.0),
                inset: false,
            }]);
        }
        if let Some(image) = source {
            cell = cell.child(img(image).size_full().object_fit(ObjectFit::Cover));
        }
        cell.into_any_element()
    }

    fn nav(
        &self,
        t: &Theme,
        prev: bool,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let p = t.palette;
        let mut button = div()
            .id(if prev { "nav-prev" } else { "nav-next" })
            .debug_selector(move || if prev { "nav-prev" } else { "nav-next" }.to_string())
            .absolute()
            .top(relative(0.5))
            .mt(px(-NAV_H / 2.0))
            .w(px(NAV_W))
            .h(px(NAV_H))
            .flex()
            .items_center()
            .justify_center()
            .border(px(theme::BORDER))
            .border_color(p.line_strong)
            .bg(p.surface)
            .text_size(px(theme::TEXT_LG))
            .text_color(p.fg_muted);
        button = if prev {
            button.left(px(theme::SPACE_3))
        } else {
            button.right(px(theme::SPACE_3))
        };
        if enabled {
            button = button
                .cursor_pointer()
                .hover(move |s| s.border_color(p.fg_muted).text_color(p.fg))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        this.step(if prev { -1 } else { 1 }, cx)
                    }),
                );
        } else {
            button = button.opacity(0.4);
        }
        button
            .child(if prev { "‹" } else { "›" })
            .into_any_element()
    }
}

impl Focusable for Lightbox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Lightbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::of(cx);
        let p = t.palette;
        let Some(image) = self.current().cloned() else {
            return div().into_any_element();
        };
        let resolved = self.resolved(cx);
        let pixels = resolved.as_ref().and_then(|(_, size)| *size);
        let meta = meta_line(pixels, &image.meta);
        let viewport = window.viewport_size();
        let room = (
            f32::from(viewport.width) - 2.0 * STAGE_PAD_X,
            f32::from(viewport.height) - theme::BAR - FOOT_H - 2.0 * STAGE_PAD_Y,
        );
        let last = self.images.len() - 1;
        let notice = self
            .notice
            .clone()
            .filter(|n| n.index == self.index)
            .unwrap_or_default();
        let copy_label = if notice.copied { "copied" } else { "copy" };
        let mut actions = div()
            .flex()
            .items_center()
            .gap(px(theme::SPACE_1))
            .ml_auto()
            .flex_none();
        if let Some(error) = &notice.error {
            actions = actions.child(
                div()
                    .text_size(px(theme::TEXT_XS))
                    .text_color(p.danger)
                    .child(error.clone()),
            );
        }
        let actions = actions
            .child(
                div()
                    .text_size(px(theme::TEXT_XS))
                    .text_color(p.fg_faint)
                    .child(format!("{} / {}", self.index + 1, self.images.len())),
            )
            .child(
                ghost_button(
                    &t,
                    "lightbox-copy",
                    true,
                    Box::new({
                        let this = cx.entity();
                        move |_, _, cx| this.update(cx, |this, cx| this.copy(cx))
                    }),
                )
                .debug_selector(|| "lightbox-copy".to_string())
                .child(copy_label),
            )
            .child(
                ghost_button(
                    &t,
                    "lightbox-download",
                    true,
                    Box::new({
                        let this = cx.entity();
                        move |_, _, cx| this.update(cx, |this, cx| this.download(cx))
                    }),
                )
                .debug_selector(|| "lightbox-download".to_string())
                .child("download"),
            )
            .child(
                ghost_button(
                    &t,
                    "lightbox-close",
                    true,
                    Box::new({
                        let this = cx.entity();
                        move |_, _, cx| this.update(cx, |this, cx| this.close(cx))
                    }),
                )
                .debug_selector(|| "lightbox-close".to_string())
                .child("close")
                .child(div().opacity(0.6).child(kbd(&t, "esc"))),
            );
        let bar = div()
            .h(px(theme::BAR))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(GHOST_GAP))
            .pl(px(theme::SPACE_4))
            .pr(px(theme::SPACE_2))
            .border_b(px(theme::BORDER))
            .border_color(p.line)
            .bg(p.surface)
            .text_size(px(theme::TEXT_SM))
            .whitespace_nowrap()
            .child(
                div()
                    .truncate()
                    .font_weight(FontWeight::BOLD)
                    .child(image.name.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(theme::TEXT_XS))
                    .text_color(p.fg_faint)
                    .child(meta),
            )
            .child(actions);
        let mut stage = div()
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .px(px(STAGE_PAD_X))
            .py(px(STAGE_PAD_Y));
        if let Some((data, Some(natural))) = &resolved {
            let scale = fit_scale(*natural, room);
            stage = stage.child(
                img(data.clone())
                    .w(px(natural.0 as f32 * scale))
                    .h(px(natural.1 as f32 * scale))
                    .border(px(theme::BORDER))
                    .border_color(p.line_strong)
                    .bg(p.bg)
                    .object_fit(ObjectFit::Contain),
            );
        }
        let stage = stage
            .child(self.nav(&t, true, self.index > 0, cx))
            .child(self.nav(&t, false, self.index < last, cx));
        let mut strip = div().flex().gap(px(theme::SPACE_1));
        for n in 0..self.images.len() {
            strip = strip.child(self.thumb(&t, n, cx));
        }
        let foot = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(theme::SPACE_4))
            .px(px(theme::SPACE_4))
            .py(px(theme::SPACE_2))
            .border_t(px(theme::BORDER))
            .border_color(p.line)
            .text_size(px(theme::TEXT_XS))
            .text_color(p.fg_faint)
            .child(strip)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(theme::TEXT_XS * 0.6))
                    .child(kbd(&t, "←"))
                    .child(kbd(&t, "→"))
                    .child("step ·")
                    .child(kbd(&t, "c"))
                    .child("copy ·")
                    .child(kbd(&t, "esc"))
                    .child("close"),
            );
        div()
            .id("lightbox")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .flex()
            .flex_col()
            .bg(p.lightbox_bg)
            .text_color(p.fg)
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Close, _, cx| this.close(cx)))
            .on_action(cx.listener(|this, _: &Previous, _, cx| this.step(-1, cx)))
            .on_action(cx.listener(|this, _: &Next, _, cx| this.step(1, cx)))
            .on_action(cx.listener(|this, _: &CopyImage, _, cx| this.copy(cx)))
            .child(bar)
            .child(stage)
            .child(foot)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attachments::images::tests::{FakeAttach, PNG};
    use crate::theme::{Mode, Theme};
    use gpui::{Entity, TestAppContext, VisualTestContext};

    #[test]
    fn stepping_stops_at_both_ends() {
        assert_eq!(clamp_step(0, -1, 3), 0);
        assert_eq!(clamp_step(0, 1, 3), 1);
        assert_eq!(clamp_step(2, 1, 3), 2);
        assert_eq!(clamp_step(1, -1, 3), 0);
    }

    #[test]
    fn a_small_image_is_not_scaled_up_and_a_big_one_fits() {
        assert_eq!(fit_scale((100, 50), (800.0, 600.0)), 1.0);
        assert_eq!(fit_scale((1600, 400), (800.0, 600.0)), 0.5);
        assert_eq!(fit_scale((400, 1200), (800.0, 600.0)), 0.5);
    }

    #[test]
    fn the_meta_line_joins_pixels_and_the_rest_with_dots() {
        assert_eq!(
            meta_line(Some((640, 480)), "4 KB · from you"),
            "640 × 480 · 4 KB · from you"
        );
        assert_eq!(meta_line(None, "4 KB"), "4 KB");
        assert_eq!(meta_line(None, ""), "");
    }

    fn local(name: &str) -> LightboxImage {
        LightboxImage {
            source: Source::Local(images::local(PNG, "image/png")),
            name: name.into(),
            meta: "1 B".into(),
        }
    }

    fn open<'a>(
        cx: &'a mut TestAppContext,
        names: &[&str],
        start: usize,
    ) -> (Entity<Lightbox>, &'a mut VisualTestContext) {
        cx.update(|cx| {
            cx.set_global(Theme::new(Mode::Dark));
            images::install(cx, Arc::new(FakeAttach::default()));
            bind_keys(cx);
        });
        let list: Vec<_> = names.iter().map(|n| local(n)).collect();
        let (lightbox, vcx) = cx.add_window_view(|_, cx| Lightbox::new(list, start, cx));
        lightbox.update_in(vcx, |l, window, cx| l.focus(window, cx));
        vcx.run_until_parked();
        (lightbox, vcx)
    }

    fn index(l: &Entity<Lightbox>, vcx: &mut VisualTestContext) -> usize {
        l.read_with(vcx, |l, _| l.index())
    }

    #[gpui::test]
    fn the_arrow_keys_step_through_the_images_and_stop_at_the_ends(cx: &mut TestAppContext) {
        let (lightbox, vcx) = open(cx, &["a.png", "b.png", "c.png"], 1);
        vcx.simulate_keystrokes("right");
        assert_eq!(index(&lightbox, vcx), 2);
        vcx.simulate_keystrokes("right");
        assert_eq!(index(&lightbox, vcx), 2);
        vcx.simulate_keystrokes("left left left");
        assert_eq!(index(&lightbox, vcx), 0);
    }

    #[gpui::test]
    fn escape_closes_the_lightbox(cx: &mut TestAppContext) {
        let (lightbox, vcx) = open(cx, &["a.png"], 0);
        let closed = std::rc::Rc::new(std::cell::Cell::new(false));
        let seen = closed.clone();
        vcx.update(|_, cx| {
            cx.subscribe(&lightbox, move |_, _: &Closed, _| seen.set(true))
                .detach()
        });
        vcx.simulate_keystrokes("escape");
        assert!(closed.get());
    }

    #[gpui::test]
    fn c_copies_the_image_to_the_clipboard_and_says_copied(cx: &mut TestAppContext) {
        let (lightbox, vcx) = open(cx, &["a.png"], 0);
        vcx.simulate_keystrokes("c");
        let item = vcx.read_from_clipboard().expect("clipboard");
        assert!(matches!(
            item.entries().first(),
            Some(gpui::ClipboardEntry::Image(_))
        ));
        let copied = lightbox.read_with(vcx, |l, _| l.notice().map(|n| n.copied));
        assert_eq!(copied, Some(true));
    }

    #[gpui::test]
    fn a_remote_image_that_failed_to_load_cannot_be_copied_and_says_why(cx: &mut TestAppContext) {
        let api = Arc::new(FakeAttach::default());
        *api.fail.lock().unwrap() = Some("gone".into());
        cx.update(|cx| {
            cx.set_global(Theme::new(Mode::Dark));
            images::install(cx, api);
            bind_keys(cx);
        });
        let remote = LightboxImage {
            source: Source::Remote {
                conversation: "c1".into(),
                id: "a1".into(),
                mime: "image/png".into(),
            },
            name: "shot.png".into(),
            meta: String::new(),
        };
        let (lightbox, vcx) = cx.add_window_view(|_, cx| Lightbox::new(vec![remote], 0, cx));
        lightbox.update_in(vcx, |l, window, cx| {
            l.copy(cx);
            l.focus(window, cx)
        });
        let error = lightbox.read_with(vcx, |l, _| l.notice().and_then(|n| n.error.clone()));
        assert_eq!(
            error.as_deref(),
            Some("error: cannot copy shot.png: the image has not loaded")
        );
        assert!(vcx.read_from_clipboard().is_none());
    }
}
