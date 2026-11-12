use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    actions, div, list, prelude::*, px, App, ClipboardItem, Context, FocusHandle, Focusable,
    FollowMode, IntoElement, KeyBinding, ListAlignment, ListState, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Render, Window,
};
use townhall_core::model::{ConversationDetail, Task, ThreadEntry};

use crate::markdown::Block;
use crate::selectable::{BlockRef, Registry};
use crate::selection::{clamp_to_boundary, selected_text, word_range, BlockKey, Selection};
use crate::thread_model::{display_items, Item};

actions!(townhall_thread, [CopySelection]);

pub const CONTEXT: &str = "Thread";

pub struct Reply(pub String);

impl gpui::EventEmitter<Reply> for Thread {}

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-c", CopySelection, Some(CONTEXT)),
        KeyBinding::new("ctrl-c", CopySelection, Some(CONTEXT)),
    ]);
}

const OVERDRAW: f32 = 600.0;

pub struct Thread {
    pub(crate) conversation_id: Option<String>,
    pub(crate) entries: Vec<ThreadEntry>,
    pub(crate) tasks: Vec<Task>,
    pub(crate) shown: Vec<(Item, Option<ThreadEntry>)>,
    pub(crate) list: ListState,
    pub(crate) selection: Option<Selection>,
    pub(crate) registry: Registry,
    pub(crate) parsed: HashMap<u64, (String, Rc<Vec<Block>>)>,
    pub(crate) pane_width: f32,
    pub(crate) thinking: bool,
    dragging: Option<BlockKey>,
    press: Option<(BlockKey, usize)>,
    focus: FocusHandle,
}

impl Thread {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let list = ListState::new(0, ListAlignment::Top, px(OVERDRAW));
        list.set_follow_mode(FollowMode::Tail);
        Self {
            conversation_id: None,
            entries: Vec::new(),
            tasks: Vec::new(),
            shown: Vec::new(),
            list,
            selection: None,
            registry: Rc::new(RefCell::new(HashMap::new())),
            parsed: HashMap::new(),
            pane_width: 800.0,
            thinking: false,
            dragging: None,
            press: None,
            focus: cx.focus_handle(),
        }
    }

    pub fn sync(
        &mut self,
        detail: Option<&ConversationDetail>,
        thinking: bool,
        cx: &mut Context<Self>,
    ) {
        let (id, entries, tasks, ended) = match detail {
            Some(d) => (
                Some(d.conversation.id.clone()),
                d.entries.clone(),
                d.tasks.clone(),
                d.conversation.ended,
            ),
            None => (None, Vec::new(), Vec::new(), false),
        };
        let same = id == self.conversation_id;
        self.conversation_id = id;
        self.thinking = thinking;
        self.entries = entries;
        self.tasks = tasks;
        let shown: Vec<(Item, Option<ThreadEntry>)> =
            display_items(&self.entries, &self.tasks, thinking, ended)
                .into_iter()
                .map(|item| {
                    let entry = match &item {
                        Item::Entry { index, .. } => self.entries.get(*index).cloned(),
                        _ => None,
                    };
                    (item, entry)
                })
                .collect();
        if same {
            self.splice_changes(&shown);
        } else {
            self.list.reset(shown.len());
            self.list.set_follow_mode(FollowMode::Tail);
            self.selection = None;
            self.dragging = None;
            self.press = None;
            self.registry.borrow_mut().clear();
            self.parsed.clear();
        }
        self.shown = shown;
        cx.notify();
    }

    fn splice_changes(&self, shown: &[(Item, Option<ThreadEntry>)]) {
        let old = &self.shown;
        let prefix = old.iter().zip(shown).take_while(|(a, b)| a == b).count();
        if prefix == old.len() && prefix == shown.len() {
            return;
        }
        let suffix = old[prefix..]
            .iter()
            .rev()
            .zip(shown[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        self.list
            .splice(prefix..old.len() - suffix, shown.len() - prefix - suffix);
    }

    pub fn set_pane_width(&mut self, width: f32, cx: &mut Context<Self>) {
        if (self.pane_width - width).abs() < 1.0 {
            return;
        }
        self.pane_width = width;
        self.list.remeasure();
        cx.notify();
    }

    #[cfg(test)]
    pub fn list(&self) -> &ListState {
        &self.list
    }

    pub fn selected_text(&self) -> Option<String> {
        let selection = self.selection.filter(|s| !s.is_empty())?;
        let registry = self.registry.borrow();
        let block = registry.get(&selection.key)?;
        Some(selected_text(&block.text, selection.range())).filter(|t| !t.is_empty())
    }

    fn index_at(block: &BlockRef, position: gpui::Point<gpui::Pixels>) -> usize {
        let ix = block
            .layout
            .index_for_position(position)
            .unwrap_or_else(|ix| ix);
        clamp_to_boundary(&block.text, ix)
    }

    pub(crate) fn press(
        &mut self,
        key: BlockKey,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        let registry = self.registry.borrow();
        let Some(block) = registry.get(&key) else {
            return;
        };
        let ix = Self::index_at(block, event.position);
        let text = block.text.clone();
        drop(registry);
        let (anchor, head) = match event.click_count {
            0 | 1 => (ix, ix),
            2 => {
                let word = word_range(&text, ix);
                (word.start, word.end)
            }
            _ => (0, text.len()),
        };
        self.selection = Some(Selection { key, anchor, head });
        self.dragging = Some(key);
        self.press = (event.click_count <= 1).then_some((key, ix));
        cx.stop_propagation();
        cx.notify();
    }

    fn drag(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.dragging else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.dragging = None;
            return;
        }
        let ix = {
            let registry = self.registry.borrow();
            let Some(block) = registry.get(&key) else {
                return;
            };
            Self::index_at(block, event.position)
        };
        if let Some(selection) = self.selection.as_mut().filter(|s| s.key == key) {
            if selection.head != ix {
                selection.head = ix;
                cx.notify();
            }
        }
    }

    fn release(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let dragged = self.dragging.take();
        let press = self.press.take();
        let Some(selection) = self.selection else {
            return;
        };
        if !selection.is_empty() {
            return;
        }
        self.selection = None;
        if let (Some(_), Some((key, ix))) = (dragged, press) {
            let url = self
                .registry
                .borrow()
                .get(&key)
                .and_then(|b| b.links.iter().find(|(r, _)| r.contains(&ix)).cloned())
                .map(|(_, url)| url);
            if let Some(url) = url {
                cx.open_url(&url);
            }
        }
        cx.notify();
    }

    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.take().is_some() {
            cx.notify();
        }
    }

    fn copy(&mut self, _: &CopySelection, _: &mut Window, cx: &mut Context<Self>) {
        match self.selected_text() {
            Some(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
            None => cx.propagate(),
        }
    }
}

impl Focusable for Thread {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Thread {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("thread")
            .size_full()
            .text_size(px(crate::theme::TEXT_MD))
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.clear_selection(cx)),
            )
            .on_mouse_move(cx.listener(Self::drag))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::release))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::release))
            .child(list(self.list.clone(), cx.processor(Self::render_item)).size_full())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{conversation, detail_of, entry_of, you};
    use crate::theme::{Mode, Theme};
    use gpui::{
        Entity, KeyBinding, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext,
        VisualTestContext,
    };
    use townhall_core::model::{EntryBody, ThreadEntry};

    const CHAR: f32 = 13.0 * 0.6;

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            cx.set_global(Theme::new(Mode::Dark));
            cx.bind_keys([KeyBinding::new("cmd-c", CopySelection, Some("Thread"))]);
        });
    }

    fn townhall(id: u64, text: &str) -> ThreadEntry {
        entry_of(id, EntryBody::Townhall { text: text.into() })
    }

    fn detail(id: &str, entries: Vec<ThreadEntry>) -> ConversationDetail {
        detail_of(
            conversation(id, "t", "2026-11-12T10:00:00.000Z"),
            entries,
            vec![],
        )
    }

    fn tall(n: u64) -> Vec<ThreadEntry> {
        (1..=n)
            .map(|i| townhall(i, &format!("entry {i}\n\nline\n\nline\n\nline")))
            .collect()
    }

    fn open(
        cx: &mut TestAppContext,
        first: ConversationDetail,
    ) -> (Entity<Thread>, &mut VisualTestContext) {
        init(cx);
        let (thread, vcx) = cx.add_window_view(|_, cx| Thread::new(cx));
        thread.update(vcx, |t, cx| t.sync(Some(&first), false, cx));
        settle(vcx);
        (thread, vcx)
    }

    fn settle(vcx: &mut VisualTestContext) {
        for _ in 0..3 {
            vcx.run_until_parked();
            vcx.update(|window, _| window.refresh());
        }
        vcx.run_until_parked();
    }

    fn sync(thread: &Entity<Thread>, vcx: &mut VisualTestContext, d: &ConversationDetail) {
        thread.update(vcx, |t, cx| t.sync(Some(d), false, cx));
        settle(vcx);
    }

    fn at_end(thread: &Entity<Thread>, vcx: &mut VisualTestContext) -> bool {
        thread.read_with(vcx, |t, _| {
            let list = t.list();
            let last = list.item_count() - 1;
            list.bounds_for_item(last)
                .is_some_and(|b| b.bottom() <= list.viewport_bounds().bottom() + px(1.0))
        })
    }

    fn following(thread: &Entity<Thread>, vcx: &mut VisualTestContext) -> bool {
        thread.read_with(vcx, |t, _| t.list().is_following_tail())
    }

    #[gpui::test]
    fn a_long_thread_opens_at_the_bottom(cx: &mut TestAppContext) {
        let (thread, vcx) = open(cx, detail("a", tall(60)));
        assert!(following(&thread, vcx));
        assert!(at_end(&thread, vcx));
    }

    #[gpui::test]
    fn a_new_entry_keeps_a_thread_at_the_bottom_stuck_there(cx: &mut TestAppContext) {
        let mut entries = tall(60);
        let (thread, vcx) = open(cx, detail("a", entries.clone()));
        entries.push(townhall(61, "fresh\n\nmore\n\nmore"));
        sync(&thread, vcx, &detail("a", entries));
        assert!(following(&thread, vcx));
        assert!(at_end(&thread, vcx));
        assert_eq!(thread.read_with(vcx, |t, _| t.list().item_count()), 61);
    }

    #[gpui::test]
    fn scrolling_up_keeps_the_position_when_an_entry_arrives(cx: &mut TestAppContext) {
        let mut entries = tall(60);
        let (thread, vcx) = open(cx, detail("a", entries.clone()));
        thread.read_with(vcx, |t, _| t.list().scroll_by(px(-400.0)));
        settle(vcx);
        assert!(!following(&thread, vcx));
        let before = thread.read_with(vcx, |t, _| t.list().logical_scroll_top());
        entries.push(townhall(61, "fresh\n\nmore\n\nmore"));
        sync(&thread, vcx, &detail("a", entries));
        let after = thread.read_with(vcx, |t, _| t.list().logical_scroll_top());
        assert_eq!(
            (before.item_ix, before.offset_in_item),
            (after.item_ix, after.offset_in_item)
        );
        assert!(!at_end(&thread, vcx));
        assert!(!following(&thread, vcx));
    }

    #[gpui::test]
    fn scrolling_back_to_the_bottom_sticks_again(cx: &mut TestAppContext) {
        let mut entries = tall(60);
        let (thread, vcx) = open(cx, detail("a", entries.clone()));
        thread.read_with(vcx, |t, _| t.list().scroll_by(px(-400.0)));
        settle(vcx);
        thread.read_with(vcx, |t, _| t.list().scroll_to_end());
        settle(vcx);
        assert!(following(&thread, vcx));
        entries.push(townhall(61, "fresh\n\nmore"));
        sync(&thread, vcx, &detail("a", entries));
        assert!(at_end(&thread, vcx));
    }

    #[gpui::test]
    fn opening_another_conversation_starts_at_its_bottom(cx: &mut TestAppContext) {
        let (thread, vcx) = open(cx, detail("a", tall(60)));
        thread.read_with(vcx, |t, _| t.list().scroll_by(px(-400.0)));
        settle(vcx);
        sync(&thread, vcx, &detail("b", tall(40)));
        assert!(following(&thread, vcx));
        assert!(at_end(&thread, vcx));
        assert_eq!(thread.read_with(vcx, |t, _| t.list().item_count()), 40);
    }

    #[gpui::test]
    fn a_changed_entry_replaces_its_row_without_moving_the_reader(cx: &mut TestAppContext) {
        let mut entries = tall(60);
        let (thread, vcx) = open(cx, detail("a", entries.clone()));
        thread.read_with(vcx, |t, _| t.list().scroll_by(px(-400.0)));
        settle(vcx);
        let before = thread.read_with(vcx, |t, _| t.list().logical_scroll_top());
        entries[59] = townhall(60, "rewritten\n\nlonger\n\nlonger\n\nlonger");
        sync(&thread, vcx, &detail("a", entries));
        let after = thread.read_with(vcx, |t, _| t.list().logical_scroll_top());
        assert_eq!(before.item_ix, after.item_ix);
        assert_eq!(thread.read_with(vcx, |t, _| t.list().item_count()), 60);
    }

    fn block_bounds(vcx: &mut VisualTestContext, name: &'static str) -> gpui::Bounds<gpui::Pixels> {
        vcx.debug_bounds(name)
            .unwrap_or_else(|| panic!("{name} is not on screen"))
    }

    fn press(vcx: &mut VisualTestContext, x: f32, y: f32, clicks: usize) {
        vcx.simulate_event(MouseDownEvent {
            position: gpui::point(px(x), px(y)),
            modifiers: Modifiers::none(),
            button: MouseButton::Left,
            click_count: clicks,
            first_mouse: false,
        });
    }

    fn release(vcx: &mut VisualTestContext, x: f32, y: f32) {
        vcx.simulate_event(MouseUpEvent {
            position: gpui::point(px(x), px(y)),
            modifiers: Modifiers::none(),
            button: MouseButton::Left,
            click_count: 1,
        });
    }

    fn drag(vcx: &mut VisualTestContext, from: (f32, f32), to: (f32, f32)) {
        press(vcx, from.0, from.1, 1);
        vcx.simulate_mouse_move(
            gpui::point(px(to.0), px(to.1)),
            MouseButton::Left,
            Modifiers::none(),
        );
        release(vcx, to.0, to.1);
    }

    fn selected(thread: &Entity<Thread>, vcx: &mut VisualTestContext) -> Option<String> {
        thread.read_with(vcx, |t, _| t.selected_text())
    }

    fn two_blocks() -> ConversationDetail {
        detail(
            "a",
            vec![
                townhall(1, "hello world again"),
                you(2, "second block here"),
            ],
        )
    }

    #[gpui::test]
    fn dragging_over_text_selects_it_and_cmd_c_copies_it(cx: &mut TestAppContext) {
        let (thread, vcx) = open(cx, two_blocks());
        let b = block_bounds(vcx, "block-1-0");
        let y = f32::from(b.origin.y) + 4.0;
        let x0 = f32::from(b.origin.x);
        drag(vcx, (x0 + CHAR * 6.0 + 1.0, y), (x0 + CHAR * 11.0 + 1.0, y));
        assert_eq!(selected(&thread, vcx).as_deref(), Some("world"));
        vcx.update(|window, cx| thread.focus_handle(cx).focus(window, cx));
        vcx.simulate_keystrokes("cmd-c");
        assert_eq!(
            vcx.read_from_clipboard().and_then(|c| c.text()),
            Some("world".to_string())
        );
    }

    #[gpui::test]
    fn dragging_backwards_selects_the_same_text(cx: &mut TestAppContext) {
        let (thread, vcx) = open(cx, two_blocks());
        let b = block_bounds(vcx, "block-1-0");
        let y = f32::from(b.origin.y) + 4.0;
        let x0 = f32::from(b.origin.x);
        drag(vcx, (x0 + CHAR * 11.0 + 1.0, y), (x0 + CHAR * 6.0 + 1.0, y));
        assert_eq!(selected(&thread, vcx).as_deref(), Some("world"));
    }

    #[gpui::test]
    fn a_selection_stays_inside_the_block_it_started_in(cx: &mut TestAppContext) {
        let (thread, vcx) = open(cx, two_blocks());
        let first = block_bounds(vcx, "block-1-0");
        let second = block_bounds(vcx, "block-2-0");
        let x0 = f32::from(first.origin.x);
        drag(
            vcx,
            (x0 + CHAR * 6.0 + 1.0, f32::from(first.origin.y) + 4.0),
            (x0 + CHAR * 3.0, f32::from(second.origin.y) + 4.0),
        );
        assert_eq!(selected(&thread, vcx).as_deref(), Some("world again"));
    }

    #[gpui::test]
    fn pressing_in_another_block_moves_the_selection_there(cx: &mut TestAppContext) {
        let (thread, vcx) = open(cx, two_blocks());
        let first = block_bounds(vcx, "block-1-0");
        let second = block_bounds(vcx, "block-2-0");
        let x0 = f32::from(first.origin.x);
        drag(
            vcx,
            (x0 + 1.0, f32::from(first.origin.y) + 4.0),
            (x0 + CHAR * 5.0 + 1.0, f32::from(first.origin.y) + 4.0),
        );
        assert_eq!(selected(&thread, vcx).as_deref(), Some("hello"));
        let x1 = f32::from(second.origin.x);
        drag(
            vcx,
            (x1 + CHAR * 7.0 + 1.0, f32::from(second.origin.y) + 4.0),
            (x1 + CHAR * 12.0 + 1.0, f32::from(second.origin.y) + 4.0),
        );
        assert_eq!(selected(&thread, vcx).as_deref(), Some("block"));
    }

    #[gpui::test]
    fn a_double_click_selects_a_word_and_a_triple_click_the_block(cx: &mut TestAppContext) {
        let (thread, vcx) = open(cx, two_blocks());
        let b = block_bounds(vcx, "block-1-0");
        let y = f32::from(b.origin.y) + 4.0;
        let x = f32::from(b.origin.x) + CHAR * 7.0;
        press(vcx, x, y, 2);
        release(vcx, x, y);
        assert_eq!(selected(&thread, vcx).as_deref(), Some("world"));
        press(vcx, x, y, 3);
        release(vcx, x, y);
        assert_eq!(selected(&thread, vcx).as_deref(), Some("hello world again"));
    }

    #[gpui::test]
    fn a_click_without_a_drag_selects_nothing_and_copies_nothing(cx: &mut TestAppContext) {
        let (thread, vcx) = open(cx, two_blocks());
        let b = block_bounds(vcx, "block-1-0");
        let (x, y) = (f32::from(b.origin.x) + 20.0, f32::from(b.origin.y) + 4.0);
        press(vcx, x, y, 1);
        release(vcx, x, y);
        assert_eq!(selected(&thread, vcx), None);
        vcx.write_to_clipboard(gpui::ClipboardItem::new_string("keep".into()));
        vcx.update(|window, cx| thread.focus_handle(cx).focus(window, cx));
        vcx.simulate_keystrokes("cmd-c");
        assert_eq!(
            vcx.read_from_clipboard().and_then(|c| c.text()),
            Some("keep".to_string())
        );
    }

    fn with_link() -> ConversationDetail {
        detail(
            "a",
            vec![townhall(1, "see [the docs](https://example.com/x) now")],
        )
    }

    #[gpui::test]
    fn clicking_a_link_opens_it(cx: &mut TestAppContext) {
        let (_thread, vcx) = open(cx, with_link());
        let b = block_bounds(vcx, "block-1-0");
        let (x, y) = (
            f32::from(b.origin.x) + CHAR * 6.0,
            f32::from(b.origin.y) + 4.0,
        );
        press(vcx, x, y, 1);
        release(vcx, x, y);
        assert_eq!(vcx.opened_url().as_deref(), Some("https://example.com/x"));
    }

    #[gpui::test]
    fn dragging_across_a_link_selects_and_does_not_open_it(cx: &mut TestAppContext) {
        let (thread, vcx) = open(cx, with_link());
        let b = block_bounds(vcx, "block-1-0");
        let y = f32::from(b.origin.y) + 4.0;
        let x0 = f32::from(b.origin.x);
        drag(vcx, (x0 + CHAR * 4.0 + 1.0, y), (x0 + CHAR * 12.0 + 1.0, y));
        assert_eq!(selected(&thread, vcx).as_deref(), Some("the docs"));
        assert_eq!(vcx.opened_url(), None);
    }

    #[gpui::test]
    fn a_click_outside_every_block_clears_the_selection(cx: &mut TestAppContext) {
        let (thread, vcx) = open(cx, two_blocks());
        let b = block_bounds(vcx, "block-1-0");
        let y = f32::from(b.origin.y) + 4.0;
        let x0 = f32::from(b.origin.x);
        drag(vcx, (x0 + 1.0, y), (x0 + CHAR * 5.0 + 1.0, y));
        assert!(selected(&thread, vcx).is_some());
        press(vcx, 2.0, 2.0, 1);
        release(vcx, 2.0, 2.0);
        assert_eq!(selected(&thread, vcx), None);
    }
}
