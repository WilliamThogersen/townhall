use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    div, img, prelude::*, px, relative, AnyElement, ClipboardEntry, Context, EventEmitter,
    FontWeight, Image, ImageFormat, ObjectFit, PathPromptOptions, SharedString,
};

use super::{Composer, Mode};
use crate::attachments::api::{AttachApi, Cancel};
use crate::attachments::format::format_size;
use crate::attachments::images;
use crate::attachments::lightbox::{LightboxImage, Open, Source};
use crate::attachments::ui::{file_card, ghost_button};
use crate::attachments::uploads::{
    can_retry, done_attachment_ids, is_uploading, latest_error, new_upload, reduce, Action,
    PendingUpload, Status,
};
use crate::attachments::validate::{
    is_image, mime_for, reject_reason, FileMeta, MAX_ATTACHMENT_BYTES,
};
use crate::theme::{self, Theme};

pub const ATTACH_PLACEHOLDER: &str = "reply, ask for more work, or drop files here";
pub const ATTACH_HELP: &str = "⏎ send · ⇧⏎ new line · paste or drop images";

const THUMB_W: f32 = 64.0;
const THUMB_H: f32 = 48.0;
const REMOVE: f32 = 16.0;

impl EventEmitter<Open> for Composer {}

#[derive(Debug, Clone)]
pub struct Picked {
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub bytes: Arc<Vec<u8>>,
}

#[derive(Default)]
pub struct Staging {
    api: Option<Arc<dyn AttachApi>>,
    items: Vec<PendingUpload>,
    previews: HashMap<String, Arc<Image>>,
    cancels: HashMap<String, Cancel>,
    counter: u64,
}

impl Staging {
    pub fn enabled(&self) -> bool {
        self.api.is_some()
    }

    pub fn api(&self) -> Option<Arc<dyn AttachApi>> {
        self.api.clone()
    }

    pub fn ids(&self) -> Vec<String> {
        done_attachment_ids(&self.items)
    }

    pub fn uploading(&self) -> bool {
        is_uploading(&self.items)
    }

    pub fn error(&self) -> Option<String> {
        latest_error(&self.items)
    }

    pub fn clear(&mut self) {
        for cancel in self.cancels.values() {
            cancel.cancel();
        }
        self.cancels.clear();
        self.previews.clear();
        reduce(&mut self.items, Action::Cleared);
    }

    pub fn finish_sent(&mut self) {
        self.cancels.clear();
        self.previews.clear();
        reduce(&mut self.items, Action::Cleared);
    }
}

fn mime_of(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::Webp => "image/webp",
        _ => "image/png",
    }
}

fn extension_of(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Gif => "gif",
        ImageFormat::Webp => "webp",
        _ => "png",
    }
}

pub fn read_picked(path: &std::path::Path) -> Result<Option<Picked>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.is_dir() {
        return Ok(None);
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let size = meta.len();
    let bytes = if size == 0 || size > MAX_ATTACHMENT_BYTES {
        Vec::new()
    } else {
        std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?
    };
    Ok(Some(Picked {
        mime: mime_for(&name),
        name,
        size,
        bytes: Arc::new(bytes),
    }))
}

impl Composer {
    pub fn set_attach(
        &mut self,
        api: Arc<dyn AttachApi>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode != Mode::Reply {
            return;
        }
        self.staging.api = Some(api);
        self.refresh_placeholder(window, cx);
        cx.notify();
    }

    pub(super) fn refresh_placeholder(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let text = if self.thinking {
            super::THINKING_PLACEHOLDER
        } else if self.staging.enabled() {
            ATTACH_PLACEHOLDER
        } else {
            super::PLACEHOLDER
        };
        self.input
            .update(cx, |input, cx| input.set_placeholder(text, window, cx));
    }

    #[cfg(test)]
    pub fn uploads(&self) -> &[PendingUpload] {
        &self.staging.items
    }

    pub fn add_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if !self.staging.enabled() || self.conversation.is_none() {
            return;
        }
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            let read = cx
                .background_executor()
                .spawn(async move {
                    paths
                        .iter()
                        .map(|p| read_picked(p))
                        .collect::<Vec<Result<Option<Picked>, String>>>()
                })
                .await;
            this.update(cx, |this, cx| {
                let mut picked = Vec::new();
                for result in read {
                    match result {
                        Ok(Some(file)) => picked.push(file),
                        Ok(None) => {}
                        Err(e) => this.error = Some(e),
                    }
                }
                this.add_files(picked, cx);
            })
        })
        .detach_and_log_err(cx);
    }

    pub fn add_files(&mut self, files: Vec<Picked>, cx: &mut Context<Self>) {
        if !self.staging.enabled() || self.conversation.is_none() || files.is_empty() {
            return;
        }
        let mut added = Vec::new();
        for file in files {
            self.staging.counter += 1;
            let key = format!("upload-{}", self.staging.counter);
            let meta = FileMeta {
                name: file.name,
                mime: file.mime,
                size: file.size,
            };
            let rejection = reject_reason(&meta);
            if rejection.is_none() && is_image(&meta.mime) {
                self.staging
                    .previews
                    .insert(key.clone(), images::local(&file.bytes, &meta.mime));
            }
            added.push(new_upload(&key, meta, file.bytes, rejection));
        }
        let starting: Vec<String> = added
            .iter()
            .filter(|u| u.status == Status::Uploading)
            .map(|u| u.key.clone())
            .collect();
        reduce(&mut self.staging.items, Action::Added(added));
        for key in starting {
            self.start_upload(key, cx);
        }
        cx.notify();
    }

    fn start_upload(&mut self, key: String, cx: &mut Context<Self>) {
        let (Some(api), Some(conversation)) = (self.staging.api.clone(), self.conversation.clone())
        else {
            return;
        };
        let Some(upload) = self.staging.items.iter().find(|u| u.key == key) else {
            return;
        };
        let meta = upload.meta.clone();
        let bytes = upload.bytes.clone();
        let cancel = Cancel::default();
        self.staging.cancels.insert(key.clone(), cancel.clone());
        let (tx, rx) = async_channel::unbounded::<Action>();
        let upload_key = key.clone();
        cx.background_executor()
            .spawn(async move {
                let progress = tx.clone();
                let progress_key = upload_key.clone();
                let result = api.upload(
                    &conversation,
                    &meta,
                    &bytes,
                    &move |percent| {
                        let _ = progress.send_blocking(Action::Progressed {
                            key: progress_key.clone(),
                            percent,
                        });
                    },
                    &cancel,
                );
                let last = match result {
                    Ok(attachment) => Some(Action::Finished {
                        key: upload_key,
                        attachment,
                    }),
                    Err(e) if e.cancelled => None,
                    Err(e) => Some(Action::Failed {
                        key: upload_key,
                        error: e.message,
                        retryable: e.retryable,
                    }),
                };
                if let Some(action) = last {
                    let _ = tx.send_blocking(action);
                }
            })
            .detach();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            while let Ok(action) = rx.recv().await {
                this.update(cx, |this, cx| {
                    reduce(&mut this.staging.items, action);
                    cx.notify();
                })?;
            }
            Ok(())
        })
        .detach_and_log_err(cx);
    }

    pub fn remove_upload(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(cancel) = self.staging.cancels.remove(key) {
            cancel.cancel();
        }
        self.staging.previews.remove(key);
        reduce(
            &mut self.staging.items,
            Action::Removed {
                key: key.to_string(),
            },
        );
        cx.notify();
    }

    pub fn retry_failed(&mut self, cx: &mut Context<Self>) {
        let keys: Vec<String> = self
            .staging
            .items
            .iter()
            .filter(|u| u.status == Status::Failed && u.retryable)
            .map(|u| u.key.clone())
            .collect();
        for key in keys {
            reduce(
                &mut self.staging.items,
                Action::Restarted { key: key.clone() },
            );
            self.start_upload(key, cx);
        }
        cx.notify();
    }

    pub fn paste(&mut self, item: &gpui::ClipboardItem, cx: &mut Context<Self>) -> bool {
        if !self.staging.enabled() || self.conversation.is_none() {
            return false;
        }
        let mut files = Vec::new();
        let mut has_text = false;
        let mut paths = Vec::new();
        for entry in item.entries() {
            match entry {
                ClipboardEntry::Image(image) => {
                    let mime = mime_of(image.format);
                    files.push(Picked {
                        name: format!("image.{}", extension_of(image.format)),
                        mime: mime.to_string(),
                        size: image.bytes.len() as u64,
                        bytes: Arc::new(image.bytes.clone()),
                    });
                }
                ClipboardEntry::ExternalPaths(external) => {
                    paths.extend(external.paths().iter().cloned())
                }
                ClipboardEntry::String(_) => has_text = true,
            }
        }
        if files.is_empty() && paths.is_empty() {
            return false;
        }
        self.add_files(files, cx);
        self.add_paths(paths, cx);
        !has_text
    }

    pub fn pick_files(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("attach".into()),
        });
        cx.spawn_in(window, async move |this, cx| -> gpui::Result<()> {
            if let Ok(Ok(Some(paths))) = picked.await {
                this.update(cx, |this, cx| this.add_paths(paths, cx))?;
            }
            Ok(())
        })
        .detach_and_log_err(cx);
    }

    pub(super) fn attach_button(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.staging.enabled() {
            return None;
        }
        Some(
            ghost_button(
                t,
                "attach",
                true,
                Box::new({
                    let this = cx.entity();
                    move |_, window, cx| this.update(cx, |this, cx| this.pick_files(window, cx))
                }),
            )
            .debug_selector(|| "attach".to_string())
            .child("+ attach")
            .into_any_element(),
        )
    }

    pub(super) fn retry_button(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !can_retry(&self.staging.items) {
            return None;
        }
        Some(
            ghost_button(
                t,
                "retry-uploads",
                true,
                Box::new({
                    let this = cx.entity();
                    move |_, _, cx| this.update(cx, |this, cx| this.retry_failed(cx))
                }),
            )
            .child("retry")
            .into_any_element(),
        )
    }

    fn open_chip(&self, key: &str, cx: &mut Context<Self>) {
        let Some(upload) = self.staging.items.iter().find(|u| u.key == key) else {
            return;
        };
        let Some(preview) = self.staging.previews.get(key) else {
            return;
        };
        cx.emit(Open {
            images: vec![LightboxImage {
                source: Source::Local(preview.clone()),
                name: upload.meta.name.clone(),
                meta: format_size(upload.meta.size),
            }],
            index: 0,
        });
    }

    fn remove_button(&self, t: &Theme, key: &str, cx: &mut Context<Self>) -> AnyElement {
        let p = t.palette;
        let key = key.to_string();
        div()
            .id(SharedString::from(format!("remove-{key}")))
            .debug_selector({
                let key = key.clone();
                move || format!("remove-{key}")
            })
            .absolute()
            .top(px(2.0))
            .right(px(2.0))
            .w(px(REMOVE))
            .h(px(REMOVE))
            .flex()
            .items_center()
            .justify_center()
            .border(px(theme::BORDER))
            .border_color(p.line_strong)
            .bg(p.bg)
            .text_color(p.fg_muted)
            .text_size(px(10.0))
            .line_height(px(10.0))
            .cursor_pointer()
            .opacity(0.0)
            .group_hover("chip", |s| s.opacity(1.0))
            .hover(move |s| s.text_color(p.fg))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.remove_upload(&key, cx)
                }),
            )
            .child("×")
            .into_any_element()
    }

    fn image_chip(&self, t: &Theme, upload: &PendingUpload, cx: &mut Context<Self>) -> AnyElement {
        let p = t.palette;
        let key = upload.key.clone();
        let failed = upload.status == Status::Failed;
        let uploading = upload.status == Status::Uploading;
        let mut chip = div()
            .id(SharedString::from(format!("chip-{key}")))
            .group("chip")
            .debug_selector({
                let key = key.clone();
                move || format!("chip-{key}")
            })
            .relative()
            .flex_none()
            .w(px(THUMB_W))
            .h(px(THUMB_H))
            .overflow_hidden()
            .border(px(theme::BORDER))
            .border_color(if failed { p.danger } else { p.line_strong })
            .bg(p.surface_2)
            .cursor(gpui::CursorStyle::PointingHand);
        if let Some(preview) = self.staging.previews.get(&key) {
            let picture = img(preview.clone())
                .size_full()
                .object_fit(ObjectFit::Cover);
            chip = chip.child(if uploading {
                picture.opacity(0.5)
            } else {
                picture
            });
        }
        if uploading {
            chip = chip.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .h(px(2.0))
                    .w(relative(f32::from(upload.percent) / 100.0))
                    .bg(p.info),
            );
        }
        if failed {
            chip = chip.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(p.danger_tint)
                    .text_color(p.danger)
                    .font_weight(FontWeight::BOLD)
                    .child("×"),
            );
        }
        let press_key = key.clone();
        chip = chip.on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(move |this, _: &gpui::MouseDownEvent, _, cx| {
                let Some(upload) = this.staging.items.iter().find(|u| u.key == press_key) else {
                    return;
                };
                match upload.status {
                    Status::Done => this.open_chip(&press_key, cx),
                    Status::Failed => this.remove_upload(&press_key, cx),
                    Status::Uploading => {}
                }
            }),
        );
        if !failed {
            chip = chip.child(self.remove_button(t, &key, cx));
        }
        chip.into_any_element()
    }

    fn file_chip(&self, t: &Theme, upload: &PendingUpload, cx: &mut Context<Self>) -> AnyElement {
        let key = upload.key.clone();
        let failed = upload.status == Status::Failed;
        let detail = if upload.status == Status::Uploading {
            format!("{}%", upload.percent)
        } else {
            format_size(upload.meta.size)
        };
        file_card(t, &upload.meta.name, &detail, failed)
            .id(SharedString::from(format!("chip-{key}")))
            .group("chip")
            .debug_selector({
                let key = key.clone();
                move || format!("chip-{key}")
            })
            .child(self.remove_button(t, &key, cx))
            .into_any_element()
    }

    pub(super) fn upload_chips(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.staging.items.is_empty() {
            return None;
        }
        let mut row = div()
            .id("attachments")
            .flex()
            .flex_wrap()
            .gap(px(theme::SPACE_2))
            .px(px(theme::SPACE_2))
            .pt(px(theme::SPACE_2));
        for upload in self.staging.items.clone() {
            row = row.child(if upload.is_image {
                self.image_chip(t, &upload, cx)
            } else {
                self.file_chip(t, &upload, cx)
            });
        }
        Some(row.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attachments::api::UploadError;
    use crate::attachments::images::tests::{FakeAttach, PNG};
    use crate::client::Api;
    use crate::drafts::Drafts;
    use crate::theme::Mode;
    use gpui::{ClipboardItem, Entity, TestAppContext, VisualTestContext};
    use std::sync::Mutex;
    use townhall_core::model::{Conversation, ConversationDetail, Repo, Task, ThreadEntry};

    #[derive(Default)]
    struct NullApi {
        sent: Mutex<Vec<(String, String)>>,
    }

    fn unused<T>() -> Result<T, String> {
        Err("not used".into())
    }

    impl Api for NullApi {
        fn conversations(&self) -> Result<Vec<Conversation>, String> {
            unused()
        }
        fn conversation(&self, _: &str) -> Result<ConversationDetail, String> {
            unused()
        }
        fn send_message(&self, id: &str, text: &str) -> Result<ThreadEntry, String> {
            self.sent.lock().unwrap().push((id.into(), text.into()));
            unused()
        }
        fn tasks(&self) -> Result<Vec<Task>, String> {
            unused()
        }
        fn repos(&self) -> Result<Vec<Repo>, String> {
            unused()
        }
        fn info(&self) -> Result<crate::client::Info, String> {
            unused()
        }
        fn search(&self, _: &str) -> Result<Vec<townhall_core::search::SearchHit>, String> {
            unused()
        }
        fn end_conversation(&self, _: &str) -> Result<Conversation, String> {
            unused()
        }
        fn link_repo(&self, _: &str, _: Option<&str>) -> Result<Conversation, String> {
            unused()
        }
        fn start_conversation(&self, _: &str) -> Result<ConversationDetail, String> {
            unused()
        }
        fn add_repo(&self, _: &str) -> Result<Repo, String> {
            unused()
        }
    }

    fn open(
        cx: &mut TestAppContext,
        attach: Arc<FakeAttach>,
    ) -> (Entity<Composer>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(Theme::new(Mode::Dark));
        });
        let drafts = Drafts::open(tempfile::tempdir().unwrap().keep().as_path()).unwrap();
        let (composer, vcx) = cx.add_window_view(move |window, cx| {
            Composer::new(Arc::new(NullApi::default()), drafts, window, cx)
        });
        composer.update_in(vcx, |c, window, cx| {
            c.set_attach(attach, window, cx);
            c.switch_to(Some("c1".into()), window, cx);
        });
        vcx.run_until_parked();
        (composer, vcx)
    }

    fn picked(name: &str, mime: &str, bytes: &[u8]) -> Picked {
        Picked {
            name: name.into(),
            mime: mime.into(),
            size: bytes.len() as u64,
            bytes: Arc::new(bytes.to_vec()),
        }
    }

    fn add(composer: &Entity<Composer>, files: Vec<Picked>, vcx: &mut VisualTestContext) {
        composer.update(vcx, |c, cx| c.add_files(files, cx));
        vcx.run_until_parked();
    }

    fn uploads(composer: &Entity<Composer>, vcx: &mut VisualTestContext) -> Vec<PendingUpload> {
        composer.read_with(vcx, |c, _| c.uploads().to_vec())
    }

    #[gpui::test]
    fn a_picked_image_uploads_and_ends_up_done_with_its_attachment(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeAttach::default());
        let (composer, vcx) = open(cx, fake.clone());
        add(&composer, vec![picked("shot.png", "image/png", PNG)], vcx);
        let items = uploads(&composer, vcx);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, Status::Done);
        assert_eq!(
            items[0].attachment.as_ref().map(|a| a.id.as_str()),
            Some("id-shot.png")
        );
        assert_eq!(
            *fake.uploaded.lock().unwrap(),
            vec![("c1".to_string(), "shot.png".to_string())]
        );
    }

    #[gpui::test]
    fn a_refused_file_fails_with_the_web_message_and_is_never_uploaded(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeAttach::default());
        let (composer, vcx) = open(cx, fake.clone());
        add(
            &composer,
            vec![
                picked("movie.mp4", "video/mp4", b"x"),
                picked("blank.txt", "text/plain", b""),
            ],
            vcx,
        );
        let items = uploads(&composer, vcx);
        assert_eq!(items[0].status, Status::Failed);
        assert_eq!(
            items[0].error.as_deref(),
            Some("movie.mp4 is not an image, text, code or pdf")
        );
        assert_eq!(items[1].error.as_deref(), Some("blank.txt is empty"));
        assert!(fake.uploaded.lock().unwrap().is_empty());
        let shown = composer.read_with(vcx, |c, _| c.staging.error());
        assert_eq!(shown.as_deref(), Some("blank.txt is empty"));
    }

    #[gpui::test]
    fn sending_with_only_attachments_posts_their_ids_and_clears_the_chips(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeAttach::default());
        let (composer, vcx) = open(cx, fake.clone());
        assert!(!composer.read_with(vcx, |c, cx| c.can_send(cx)));
        add(&composer, vec![picked("a.log", "text/plain", b"x")], vcx);
        assert!(composer.read_with(vcx, |c, cx| c.can_send(cx)));
        composer.update_in(vcx, |c, window, cx| c.submit(window, cx));
        vcx.run_until_parked();
        assert_eq!(
            *fake.sent.lock().unwrap(),
            vec![(
                "c1".to_string(),
                String::new(),
                vec!["id-a.log".to_string()]
            )]
        );
        assert!(uploads(&composer, vcx).is_empty());
    }

    #[gpui::test]
    fn sending_waits_while_an_upload_is_running(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeAttach::default());
        let (composer, vcx) = open(cx, fake);
        composer.update(vcx, |c, _| {
            c.staging.items.push(new_upload(
                "u1",
                FileMeta {
                    name: "a.png".into(),
                    mime: "image/png".into(),
                    size: 1,
                },
                Arc::default(),
                None,
            ));
        });
        assert!(!composer.read_with(vcx, |c, cx| c.can_send(cx)));
    }

    #[gpui::test]
    fn a_dropped_connection_can_be_retried_and_a_refusal_cannot(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeAttach::default());
        *fake.upload_error.lock().unwrap() = Some(UploadError {
            message: "cannot reach the daemon: the upload of a.log stopped".into(),
            retryable: true,
            cancelled: false,
        });
        let (composer, vcx) = open(cx, fake.clone());
        add(&composer, vec![picked("a.log", "text/plain", b"x")], vcx);
        assert_eq!(uploads(&composer, vcx)[0].status, Status::Failed);
        assert!(composer.read_with(vcx, |c, _| can_retry(c.uploads())));
        *fake.upload_error.lock().unwrap() = None;
        composer.update(vcx, |c, cx| c.retry_failed(cx));
        vcx.run_until_parked();
        assert_eq!(uploads(&composer, vcx)[0].status, Status::Done);

        *fake.upload_error.lock().unwrap() = Some(UploadError {
            message: "b.log is over 20 MB".into(),
            retryable: false,
            cancelled: false,
        });
        add(&composer, vec![picked("b.log", "text/plain", b"x")], vcx);
        assert!(!composer.read_with(vcx, |c, _| can_retry(c.uploads())));
    }

    #[gpui::test]
    fn removing_a_chip_drops_it(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeAttach::default());
        let (composer, vcx) = open(cx, fake);
        add(&composer, vec![picked("a.log", "text/plain", b"x")], vcx);
        composer.update(vcx, |c, cx| c.remove_upload("upload-1", cx));
        assert!(uploads(&composer, vcx).is_empty());
    }

    #[gpui::test]
    fn switching_conversation_clears_the_chips(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeAttach::default());
        let (composer, vcx) = open(cx, fake);
        add(&composer, vec![picked("a.log", "text/plain", b"x")], vcx);
        composer.update_in(vcx, |c, window, cx| {
            c.switch_to(Some("c2".into()), window, cx)
        });
        assert!(uploads(&composer, vcx).is_empty());
    }

    #[gpui::test]
    fn pasting_an_image_attaches_it_and_pasting_text_only_does_not(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeAttach::default());
        let (composer, vcx) = open(cx, fake.clone());
        composer.update_in(vcx, |c, window, cx| c.focus(window, cx));
        vcx.write_to_clipboard(ClipboardItem::new_string("just words".into()));
        vcx.dispatch_action(gpui_component::input::Paste);
        vcx.run_until_parked();
        assert!(uploads(&composer, vcx).is_empty());
        assert_eq!(composer.read_with(vcx, |c, cx| c.text(cx)), "just words");

        vcx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
            ImageFormat::Png,
            PNG.to_vec(),
        )));
        vcx.dispatch_action(gpui_component::input::Paste);
        vcx.run_until_parked();
        let items = uploads(&composer, vcx);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].meta.name, "image.png");
        assert_eq!(items[0].status, Status::Done);
        assert_eq!(composer.read_with(vcx, |c, cx| c.text(cx)), "just words");
    }

    #[gpui::test]
    fn dropped_paths_are_read_from_disk_and_folders_are_skipped(cx: &mut TestAppContext) {
        let fake = Arc::new(FakeAttach::default());
        let (composer, vcx) = open(cx, fake.clone());
        let dir = tempfile::tempdir().unwrap();
        let notes = dir.path().join("notes.txt");
        std::fs::write(&notes, "hello").unwrap();
        let blank = dir.path().join("blank.txt");
        std::fs::write(&blank, "").unwrap();
        let folder = dir.path().join("folder");
        std::fs::create_dir(&folder).unwrap();
        composer.update(vcx, |c, cx| c.add_paths(vec![notes, blank, folder], cx));
        vcx.run_until_parked();
        let items = uploads(&composer, vcx);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].meta.name, "notes.txt");
        assert_eq!(items[0].status, Status::Done);
        assert_eq!(items[0].meta.mime, "text/plain");
        assert_eq!(items[1].error.as_deref(), Some("blank.txt is empty"));
    }
}
