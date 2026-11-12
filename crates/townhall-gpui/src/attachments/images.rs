use std::collections::HashMap;
use std::sync::Arc;

use gpui::{App, Global, Image, ImageFormat};

use super::api::AttachApi;

#[derive(Debug, Clone)]
pub struct Loaded {
    pub image: Arc<Image>,
    pub size: Option<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub enum Slot {
    Loading,
    Ready(Loaded),
    Failed(String),
}

struct Store {
    api: Arc<dyn AttachApi>,
    slots: HashMap<(String, String), Slot>,
}

impl Global for Store {}

pub fn install(cx: &mut App, api: Arc<dyn AttachApi>) {
    cx.set_global(Store {
        api,
        slots: HashMap::new(),
    });
}

pub fn format_of(mime: &str) -> ImageFormat {
    match mime {
        "image/jpeg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::Webp,
        _ => ImageFormat::Png,
    }
}

pub fn local(bytes: &[u8], mime: &str) -> Arc<Image> {
    Arc::new(Image::from_bytes(format_of(mime), bytes.to_vec()))
}

pub fn size_of(cx: &App, image: &Image) -> Option<(u32, u32)> {
    let render = match image.to_image_data(cx.svg_renderer()) {
        Ok(render) => render,
        Err(e) => {
            tracing::warn!(
                target: "townhall::native",
                event = "attachments.decode_failed",
                error = %e,
                "cannot read the size of an image"
            );
            return None;
        }
    };
    let size = render.size(0);
    Some((size.width.0 as u32, size.height.0 as u32))
}

pub fn slot(cx: &mut App, conversation: &str, id: &str, mime: &str) -> Slot {
    let key = (conversation.to_string(), id.to_string());
    let Some(store) = cx.try_global::<Store>() else {
        return Slot::Failed("no daemon".into());
    };
    if let Some(found) = store.slots.get(&key) {
        return found.clone();
    }
    let api = store.api.clone();
    cx.global_mut::<Store>()
        .slots
        .insert(key.clone(), Slot::Loading);
    let mime = mime.to_string();
    cx.spawn(async move |cx| {
        let (conversation, id) = key.clone();
        let fetched = cx
            .background_executor()
            .spawn(async move { api.fetch(&conversation, &id) })
            .await;
        cx.update(|cx| {
            let next = match fetched {
                Ok(bytes) => {
                    let image = local(&bytes, &mime);
                    let size = size_of(cx, &image);
                    Slot::Ready(Loaded { image, size })
                }
                Err(e) => Slot::Failed(e),
            };
            cx.global_mut::<Store>().slots.insert(key, next);
            cx.refresh_windows();
        });
    })
    .detach();
    Slot::Loading
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::attachments::api::{Cancel, UploadError};
    use crate::attachments::validate::FileMeta;
    use gpui::TestAppContext;
    use std::sync::Mutex;
    use townhall_core::model::{Attachment, ThreadEntry};

    pub const PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x10,
        0x50, 0x30, 0x00, 0x00, 0x00, 0xA4, 0x00, 0x61, 0x34, 0x66, 0x7D, 0x72, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[derive(Default)]
    pub struct FakeAttach {
        pub fetched: Mutex<Vec<(String, String)>>,
        pub fail: Mutex<Option<String>>,
    }

    impl AttachApi for FakeAttach {
        fn upload(
            &self,
            _: &str,
            _: &FileMeta,
            _: &[u8],
            _: &dyn Fn(u8),
            _: &Cancel,
        ) -> Result<Attachment, UploadError> {
            unreachable!("not used here")
        }

        fn fetch(&self, conversation: &str, id: &str) -> Result<Vec<u8>, String> {
            self.fetched
                .lock()
                .unwrap()
                .push((conversation.into(), id.into()));
            match self.fail.lock().unwrap().clone() {
                Some(e) => Err(e),
                None => Ok(PNG.to_vec()),
            }
        }

        fn send(&self, _: &str, _: &str, _: &[String]) -> Result<ThreadEntry, String> {
            unreachable!("not used here")
        }
    }

    #[gpui::test]
    fn an_image_loads_once_and_then_serves_from_the_cache(cx: &mut TestAppContext) {
        let api = Arc::new(FakeAttach::default());
        cx.update(|cx| install(cx, api.clone()));
        assert!(matches!(
            cx.update(|cx| slot(cx, "c1", "a1", "image/png")),
            Slot::Loading
        ));
        cx.run_until_parked();
        let Slot::Ready(loaded) = cx.update(|cx| slot(cx, "c1", "a1", "image/png")) else {
            panic!("not ready");
        };
        assert_eq!(loaded.size, Some((1, 1)));
        assert_eq!(api.fetched.lock().unwrap().len(), 1);
    }

    #[gpui::test]
    fn a_failed_fetch_is_remembered_with_its_reason(cx: &mut TestAppContext) {
        let api = Arc::new(FakeAttach::default());
        *api.fail.lock().unwrap() = Some("gone".into());
        cx.update(|cx| install(cx, api.clone()));
        cx.update(|cx| slot(cx, "c1", "a1", "image/png"));
        cx.run_until_parked();
        match cx.update(|cx| slot(cx, "c1", "a1", "image/png")) {
            Slot::Failed(e) => assert_eq!(e, "gone"),
            other => panic!("{other:?}"),
        }
    }
}
