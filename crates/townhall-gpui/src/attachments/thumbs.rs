use gpui::{div, img, prelude::*, px, AnyElement, App, ObjectFit, Rgba, SharedString};
use townhall_core::model::{Attachment, AttachmentKind};

use super::format::format_size;
use super::images::{self, Slot};
use super::lightbox::{LightboxImage, Source};
use super::ui::{file_card, Handler};
use crate::theme::{self, Theme};
use crate::thread_model::clock_time;

pub const MAX_IMAGES_IN_FULL: usize = 3;
pub const SHOWN_WHEN_COLLAPSED: usize = 2;
pub const THUMB_HEIGHT: f32 = 104.0;
pub const THUMB_MAX_WIDTH: f32 = 240.0;
pub const MORE_WIDTH: f32 = 72.0;

pub fn shared_in(ids: &[String], attachments: &[Attachment]) -> Vec<Attachment> {
    ids.iter()
        .filter_map(|id| attachments.iter().find(|a| &a.id == id).cloned())
        .collect()
}

pub fn collapsed(count: usize, collapse: bool) -> (usize, usize) {
    let shown = if collapse && count > MAX_IMAGES_IN_FULL {
        SHOWN_WHEN_COLLAPSED
    } else {
        count
    };
    (shown, count - shown)
}

pub fn thumb_width(size: Option<(u32, u32)>) -> f32 {
    match size {
        Some((w, h)) if h > 0 => (THUMB_HEIGHT * w as f32 / h as f32).min(THUMB_MAX_WIDTH),
        _ => THUMB_HEIGHT,
    }
}

pub fn lightbox_images(
    conversation: &str,
    images: &[Attachment],
    from: &str,
) -> Vec<LightboxImage> {
    images
        .iter()
        .map(|image| {
            let clock = clock_time(&image.created_at);
            let meta = [format_size(image.size), format!("from {from}"), clock]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            LightboxImage {
                source: Source::Remote {
                    conversation: conversation.to_string(),
                    id: image.id.clone(),
                    mime: image.mime.clone(),
                },
                name: image.name.clone(),
                meta,
            }
        })
        .collect()
}

pub struct Thumbs<'a> {
    pub conversation: &'a str,
    pub shared: &'a [Attachment],
    pub collapse: bool,
    pub border: Rgba,
}

impl Thumbs<'_> {
    pub fn images(&self) -> Vec<Attachment> {
        self.shared
            .iter()
            .filter(|a| a.kind == AttachmentKind::Image)
            .cloned()
            .collect()
    }

    pub fn render(
        &self,
        t: &Theme,
        cx: &mut App,
        open: impl Fn(usize) -> Handler,
    ) -> Vec<AnyElement> {
        let p = t.palette;
        let images = self.images();
        let files: Vec<&Attachment> = self
            .shared
            .iter()
            .filter(|a| a.kind == AttachmentKind::File)
            .collect();
        let mut rows = Vec::new();
        if !images.is_empty() {
            let (shown, hidden) = collapsed(images.len(), self.collapse);
            let mut row = div()
                .flex()
                .flex_wrap()
                .gap(px(theme::SPACE_2))
                .id("msg-images");
            for (n, image) in images.iter().take(shown).enumerate() {
                let slot = images::slot(cx, self.conversation, &image.id, &image.mime);
                let (width, picture) = match slot {
                    Slot::Ready(loaded) => (thumb_width(loaded.size), Some(loaded.image)),
                    Slot::Loading | Slot::Failed(_) => (THUMB_HEIGHT, None),
                };
                let border = self.border;
                let mut cell = div()
                    .id(SharedString::from(format!("thumb-{}", image.id)))
                    .debug_selector({
                        let id = image.id.clone();
                        move || format!("thumb-{id}")
                    })
                    .flex_none()
                    .w(px(width))
                    .h(px(THUMB_HEIGHT))
                    .overflow_hidden()
                    .border(px(theme::BORDER))
                    .border_color(border)
                    .bg(p.surface_2)
                    .cursor_pointer()
                    .hover(move |s| s.border_color(p.fg_muted))
                    .on_mouse_down(gpui::MouseButton::Left, {
                        let handler = open(n);
                        move |event, window, cx| handler(event, window, cx)
                    });
                if let Some(picture) = picture {
                    cell = cell.child(img(picture).size_full().object_fit(ObjectFit::Cover));
                }
                row = row.child(cell);
            }
            if hidden > 0 {
                let border = self.border;
                row = row.child(
                    div()
                        .id("thumb-more")
                        .debug_selector(|| "thumb-more".to_string())
                        .flex_none()
                        .w(px(MORE_WIDTH))
                        .h(px(THUMB_HEIGHT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .border(px(theme::BORDER))
                        .border_color(border)
                        .bg(p.surface_2)
                        .text_size(px(theme::TEXT_SM))
                        .text_color(p.fg_muted)
                        .cursor_pointer()
                        .hover(move |s| s.border_color(p.fg_muted))
                        .on_mouse_down(gpui::MouseButton::Left, {
                            let handler = open(shown);
                            move |event, window, cx| handler(event, window, cx)
                        })
                        .child(format!("+{hidden}")),
                );
            }
            rows.push(row.into_any_element());
        }
        if !files.is_empty() {
            let mut row = div()
                .flex()
                .flex_wrap()
                .gap(px(theme::SPACE_2))
                .id("msg-files");
            for file in files {
                row = row.child(
                    file_card(t, &file.name, &format_size(file.size), false)
                        .id(SharedString::from(format!("file-{}", file.id)))
                        .cursor_pointer()
                        .hover(move |s| s.border_color(p.fg_muted))
                        .on_mouse_down(gpui::MouseButton::Left, {
                            let (conversation, id, name) = (
                                self.conversation.to_string(),
                                file.id.clone(),
                                file.name.clone(),
                            );
                            move |_, _, cx| images::save_attachment(cx, &conversation, &id, &name)
                        }),
                );
            }
            rows.push(row.into_any_element());
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attachment(id: &str, kind: &str, size: u64) -> Attachment {
        serde_json::from_value(serde_json::json!({
            "id": id, "conversationId": "c1", "name": format!("{id}.png"),
            "mime": "image/png", "size": size, "kind": kind,
            "createdAt": "2026-11-12T10:00:00.000Z"
        }))
        .unwrap()
    }

    #[test]
    fn shared_attachments_follow_the_entrys_ids_and_skip_unknown_ones() {
        let all = [attachment("a", "image", 1), attachment("b", "file", 2)];
        let ids = ["b".to_string(), "zz".to_string(), "a".to_string()];
        let found: Vec<String> = shared_in(&ids, &all).into_iter().map(|a| a.id).collect();
        assert_eq!(found, ["b", "a"]);
    }

    #[test]
    fn more_than_three_images_collapse_to_two_and_a_count() {
        assert_eq!(collapsed(3, true), (3, 0));
        assert_eq!(collapsed(4, true), (2, 2));
        assert_eq!(collapsed(7, true), (2, 5));
        assert_eq!(collapsed(7, false), (7, 0));
    }

    #[test]
    fn a_thumbnail_keeps_its_aspect_at_104_tall_and_stops_at_240_wide() {
        assert_eq!(thumb_width(Some((208, 104))), 208.0);
        assert_eq!(thumb_width(Some((1000, 100))), 240.0);
        assert_eq!(thumb_width(None), 104.0);
    }

    #[test]
    fn the_lightbox_meta_names_the_size_the_sender_and_the_time() {
        let one = [attachment("a", "image", 4096)];
        let images = lightbox_images("c1", &one, "you");
        assert!(images[0].meta.starts_with("4 KB · from you"), "{}", images[0].meta);
        assert_eq!(images[0].name, "a.png");
    }
}
