use std::sync::Arc;

use gpui::{
    point, px, Entity, ExternalPaths, FileDropEvent, Modifiers, TestAppContext, VisualTestContext,
};
use townhall_core::model::{
    Attachment, Conversation, ConversationDetail, EntryBody, Repo, Task, ThreadEntry,
};

use crate::attachments::images::tests::FakeAttach;
use crate::client::Api;
use crate::drafts::Drafts;
use crate::testkit::{conversation, detail_of, entry_of, repo};
use crate::theme::{self, Theme};
use crate::view::Root;

struct FakeApi {
    detail: ConversationDetail,
}

fn unused<T>() -> Result<T, String> {
    Err("not used".into())
}

impl Api for FakeApi {
    fn conversations(&self) -> Result<Vec<Conversation>, String> {
        Ok(vec![self.detail.conversation.clone()])
    }
    fn conversation(&self, _: &str) -> Result<ConversationDetail, String> {
        Ok(self.detail.clone())
    }
    fn send_message(&self, _: &str, _: &str) -> Result<ThreadEntry, String> {
        unused()
    }
    fn tasks(&self) -> Result<Vec<Task>, String> {
        Ok(vec![])
    }
    fn repos(&self) -> Result<Vec<Repo>, String> {
        Ok(vec![repo("shop", "main")])
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

fn image(id: &str) -> Attachment {
    serde_json::from_value(serde_json::json!({
        "id": id, "conversationId": "c1", "name": format!("{id}.png"),
        "mime": "image/png", "size": 4096, "kind": "image",
        "createdAt": "2026-11-12T10:00:00.000Z"
    }))
    .unwrap()
}

fn detail_with_images() -> ConversationDetail {
    let mut detail = detail_of(
        conversation("c1", "login loop", "2026-11-12T10:00:00.000Z"),
        vec![entry_of(
            1,
            EntryBody::You {
                text: "[Image #1] it loops".into(),
                attachments: vec!["a1".into(), "a2".into()],
                reply_to: None,
            },
        )],
        vec![],
    );
    detail.attachments = vec![image("a1"), image("a2")];
    detail
}

fn open(
    cx: &mut TestAppContext,
    detail: ConversationDetail,
    fake: Arc<FakeAttach>,
) -> (Entity<Root>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(Theme::new(theme::Mode::Dark));
        crate::view::bind_keys(cx);
        crate::attachments::lightbox::bind_keys(cx);
    });
    let home = tempfile::tempdir().unwrap().keep();
    let drafts = Drafts::open(&home).unwrap();
    let api = Arc::new(FakeApi { detail });
    let (root, vcx) = cx.add_window_view(move |window, cx| {
        Root::new(api, None, drafts, window, cx).with_attachments(fake, window, cx)
    });
    vcx.run_until_parked();
    vcx.run_until_parked();
    (root, vcx)
}

fn click(vcx: &mut VisualTestContext, selector: &'static str) {
    let at = vcx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not on screen"))
        .center();
    vcx.simulate_click(at, Modifiers::none());
    vcx.run_until_parked();
}

#[gpui::test]
fn a_thumbnail_in_a_sent_message_opens_the_lightbox_on_that_image(cx: &mut TestAppContext) {
    let (root, vcx) = open(cx, detail_with_images(), Arc::new(FakeAttach::default()));
    assert!(root.read_with(vcx, |r, _| r.lightbox().is_none()));
    click(vcx, "thumb-a2");
    let lightbox = root
        .read_with(vcx, |r, _| r.lightbox().cloned())
        .expect("the lightbox is open");
    assert_eq!(lightbox.read_with(vcx, |l, _| l.index()), 1);
}

#[gpui::test]
fn the_arrow_keys_step_and_escape_closes_the_lightbox_from_the_thread(cx: &mut TestAppContext) {
    let (root, vcx) = open(cx, detail_with_images(), Arc::new(FakeAttach::default()));
    click(vcx, "thumb-a1");
    let lightbox = root.read_with(vcx, |r, _| r.lightbox().cloned()).unwrap();
    vcx.simulate_keystrokes("right");
    assert_eq!(lightbox.read_with(vcx, |l, _| l.index()), 1);
    vcx.simulate_keystrokes("left");
    assert_eq!(lightbox.read_with(vcx, |l, _| l.index()), 0);
    vcx.simulate_keystrokes("escape");
    assert!(root.read_with(vcx, |r, _| r.lightbox().is_none()));
}

#[gpui::test]
fn the_session_panels_files_open_the_lightbox_on_every_image(cx: &mut TestAppContext) {
    let (root, vcx) = open(cx, detail_with_images(), Arc::new(FakeAttach::default()));
    vcx.simulate_resize(gpui::size(px(1300.0), px(800.0)));
    vcx.run_until_parked();
    click(vcx, "panel-thumb-a2");
    let lightbox = root
        .read_with(vcx, |r, _| r.lightbox().cloned())
        .expect("the lightbox is open");
    assert_eq!(lightbox.read_with(vcx, |l, _| l.index()), 1);
    assert!(vcx.debug_bounds("strip-0").is_some() && vcx.debug_bounds("strip-1").is_some());
}

#[gpui::test]
fn the_lightbox_shows_the_pixel_size_once_the_image_has_loaded(cx: &mut TestAppContext) {
    let (_root, vcx) = open(cx, detail_with_images(), Arc::new(FakeAttach::default()));
    click(vcx, "thumb-a1");
    vcx.run_until_parked();
    assert!(vcx.debug_bounds("lightbox-copy").is_some());
    assert!(vcx.debug_bounds("strip-1").is_some());
}

#[gpui::test]
fn dragging_files_over_the_window_shows_the_drop_zone_and_dropping_attaches_them(
    cx: &mut TestAppContext,
) {
    let fake = Arc::new(FakeAttach::default());
    let (root, vcx) = open(cx, detail_with_images(), fake.clone());
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, "hello").unwrap();
    let over = point(px(600.0), px(300.0));
    assert!(vcx.debug_bounds("drop-zone").is_none());
    vcx.simulate_event(FileDropEvent::Entered {
        position: over,
        paths: ExternalPaths([file.clone()].into_iter().collect()),
    });
    vcx.simulate_mouse_move(over, None, Modifiers::none());
    vcx.run_until_parked();
    assert!(vcx.debug_bounds("drop-zone").is_some());
    vcx.simulate_event(FileDropEvent::Submit { position: over });
    vcx.run_until_parked();
    assert!(vcx.debug_bounds("drop-zone").is_none());
    let composer = root.read_with(vcx, |r, _| r.composer.clone());
    let names: Vec<String> = composer.read_with(vcx, |c, _| {
        c.uploads().iter().map(|u| u.meta.name.clone()).collect()
    });
    assert_eq!(names, ["notes.txt"]);
    assert_eq!(fake.uploaded.lock().unwrap().len(), 1);
}
