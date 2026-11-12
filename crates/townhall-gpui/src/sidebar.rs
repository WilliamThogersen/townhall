use gpui::{
    div, prelude::*, px, AnyElement, Context, FontWeight, MouseButton, MouseDownEvent, Rgba,
    SharedString, Window,
};
use gpui_component::input::{Escape, Input};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};

use crate::age::age_label;
use crate::repo_group::row_repo_text;
use crate::row_menu::MenuItem;
use crate::row_summary::RowState;
use crate::sidebar_rows::{GroupBlock, SidebarRow};
use crate::state::Load;
use crate::theme::{self, Theme};
use crate::view::{note, Root};

const CH: f32 = theme::TEXT_MD * 0.6;

fn status_color(t: &Theme, state: Option<RowState>) -> Rgba {
    match state {
        Some(RowState::Waiting) => t.palette.accent,
        Some(RowState::Error) => t.palette.danger,
        _ => t.palette.fg_faint,
    }
}

fn section(t: &Theme, label: String, action: Option<AnyElement>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(theme::SPACE_2))
        .pt(px(theme::SPACE_3))
        .pb(px(theme::SPACE_1))
        .pl(px(theme::SPACE_4))
        .pr(px(theme::SPACE_3))
        .text_size(px(theme::TEXT_XS))
        .line_height(px(16.0))
        .text_color(t.palette.fg_faint)
        .child(label)
        .child(
            div()
                .flex_1()
                .border_t(px(theme::BORDER))
                .border_color(t.palette.line),
        )
        .children(action)
        .into_any_element()
}

impl Root {
    pub(crate) fn sidebar(&self, t: Theme, cx: &mut Context<Self>) -> AnyElement {
        let p = t.palette;
        let top = div()
            .id("brand")
            .h(px(theme::BAR))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(theme::SPACE_2))
            .px(px(theme::SPACE_4))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| this.go_home(window, cx)),
            )
            .child(div().text_color(p.fg).child("▟"))
            .child(
                div()
                    .flex()
                    .font_weight(FontWeight::BOLD)
                    .child(div().text_color(p.accent).child("/"))
                    .child("townhall"),
            );
        let new_conversation = div()
            .id("new-conversation")
            .h(px(theme::ROW))
            .flex()
            .items_center()
            .gap(px(CH))
            .px(px(theme::SPACE_2))
            .cursor_pointer()
            .hover(move |s| s.bg(p.surface_2))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| this.go_home(window, cx)),
            )
            .child(
                div()
                    .w(px(2.0 * CH))
                    .text_color(p.fg_faint)
                    .flex()
                    .justify_center()
                    .child("+"),
            )
            .child(div().flex_1().child("new conversation"));
        let search = div()
            .h(px(theme::ROW))
            .flex()
            .items_center()
            .gap(px(CH))
            .px(px(theme::SPACE_2))
            .on_action(cx.listener(|this, _: &Escape, window, cx| this.clear_search(window, cx)))
            .child(
                div()
                    .w(px(2.0 * CH))
                    .text_color(p.fg_faint)
                    .flex()
                    .justify_center()
                    .child("/"),
            )
            .child(
                div().flex_1().min_w_0().text_color(p.fg).child(
                    Input::new(&self.search)
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false),
                ),
            )
            .child(
                div()
                    .flex_none()
                    .px(px(4.0))
                    .border(px(theme::BORDER))
                    .border_color(p.line)
                    .text_size(px(theme::TEXT_XS))
                    .text_color(p.fg_faint)
                    .child("esc"),
            );
        div()
            .debug_selector(|| "sidebar".to_string())
            .w(px(theme::SIDEBAR))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(p.surface)
            .border_r(px(theme::BORDER))
            .border_color(p.line)
            .child(top)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .px(px(theme::SPACE_2))
                    .pb(px(theme::SPACE_2))
                    .child(new_conversation)
                    .child(search),
            )
            .child(
                div()
                    .id("conversations")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .border_t(px(theme::BORDER))
                    .border_color(p.line)
                    .child(self.conversation_list(t, cx)),
            )
            .into_any_element()
    }

    fn notes(&self, t: &Theme) -> Vec<AnyElement> {
        let p = t.palette;
        let mut notes = Vec::new();
        let total = match &self.state.conversations {
            Load::Ready(list) => list.len(),
            _ => 0,
        };
        let sections = self.sections();
        let shown = sections.active.len() + sections.older.len();
        match &self.state.conversations {
            Load::Loading => notes.push(note(p.fg_faint, "loading…").into_any_element()),
            Load::Failed(e) => {
                notes.push(note(t.status_error(), format!("error: {e}")).into_any_element())
            }
            Load::Ready(_) => {}
        }
        if let Some(e) = self.action_error() {
            notes.push(note(t.status_error(), format!("error: {e}")).into_any_element());
        }
        if let Some(e) = self.match_error() {
            notes.push(
                note(p.fg_faint, format!("warn: message search failed: {e}")).into_any_element(),
            );
        }
        if matches!(self.state.conversations, Load::Ready(_)) && total == 0 {
            notes.push(note(p.fg_faint, "no conversations yet").into_any_element());
        }
        if total > 0 && shown == 0 && !self.query.trim().is_empty() {
            notes.push(
                note(
                    p.fg_faint,
                    format!("no conversations match {}", self.query.trim()),
                )
                .into_any_element(),
            );
        }
        notes
    }

    fn conversation_list(&self, t: Theme, cx: &mut Context<Self>) -> AnyElement {
        let p = t.palette;
        let sections = self.sections();
        let by_repo_label = {
            let on = self.by_repo;
            div()
                .id("by-repo")
                .px(px(2.0))
                .cursor_pointer()
                .text_size(px(theme::TEXT_XS))
                .text_color(if on { p.fg } else { p.fg_faint })
                .hover(move |s| s.text_color(p.fg))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, _, cx| this.toggle_by_repo(cx)),
                )
                .child("by repo")
                .into_any_element()
        };
        let mut list = div().flex().flex_col().pb(px(theme::SPACE_2));
        list = list.child(section(
            &t,
            format!("active {}", sections.active.len()),
            Some(by_repo_label),
        ));
        match self.blocks() {
            Some(blocks) => {
                for block in blocks {
                    list = list.child(self.group(&t, &block, cx));
                    for row in &block.rows {
                        list = list.child(self.row(&t, row, false, cx));
                    }
                }
            }
            None => {
                for row in &sections.active {
                    list = list.child(self.row(&t, row, true, cx));
                }
            }
        }
        for note in self.notes(&t) {
            list = list.child(note);
        }
        list = list.child(section(&t, format!("older {}", sections.older.len()), None));
        for row in &sections.older {
            list = list.child(self.row(&t, row, true, cx));
        }
        list.into_any_element()
    }

    fn group(&self, t: &Theme, block: &GroupBlock, cx: &mut Context<Self>) -> AnyElement {
        let p = t.palette;
        let key = block.key.clone();
        div()
            .id(SharedString::from(format!("group-{}", block.key)))
            .w_full()
            .h(px(theme::CONTROL_SM))
            .flex()
            .items_center()
            .gap(px(theme::TEXT_XS * 0.6))
            .pl(px(theme::SPACE_4))
            .pr(px(theme::SPACE_3))
            .text_size(px(theme::TEXT_XS))
            .text_color(p.fg_muted)
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| this.toggle_group(&key, cx)),
            )
            .child(
                div()
                    .text_color(p.fg_faint)
                    .child(if block.open { "▾" } else { "▸" }),
            )
            .child(div().child(block.label.clone()))
            .child(
                div()
                    .ml_auto()
                    .text_color(p.fg_faint)
                    .child(block.count.to_string()),
            )
            .into_any_element()
    }

    fn row(
        &self,
        t: &Theme,
        row: &SidebarRow,
        show_repo: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = t.palette;
        let conversation = &row.conversation;
        let summary = &row.summary;
        let current = self.is_current(&conversation.id);
        let id = conversation.id.clone();
        let open_id = id.clone();
        let menu_items = self.menu_items(&id);
        let root = cx.entity();
        let waiting = summary.state == Some(RowState::Waiting);
        let ended = summary.state == Some(RowState::Ended);
        let title_color = if ended { p.fg_muted } else { p.fg };
        let repo = if show_repo {
            row_repo_text(&row.repo)
        } else {
            String::new()
        };
        div()
            .id(SharedString::from(format!("row-{id}")))
            .debug_selector({
                let id = id.clone();
                move || format!("row-{id}")
            })
            .w_full()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .py(px(6.0))
            .pl(px(theme::SPACE_4 - CH))
            .pr(px(theme::SPACE_3))
            .cursor_pointer()
            .when(waiting, |d| d.bg(p.accent_tint))
            .when(current, |d| {
                d.bg(p.surface_2)
                    .border_l(px(theme::ACCENT_RULE))
                    .border_color(p.accent)
            })
            .hover(move |s| s.bg(p.surface_2))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    this.select(open_id.clone(), window, cx)
                }),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(CH))
                    .child(
                        div()
                            .w(px(2.0 * CH))
                            .flex_none()
                            .text_color(summary.glyph.color(t))
                            .child(summary.glyph.symbol()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(title_color)
                            .when(current, |d| d.font_weight(FontWeight::BOLD))
                            .child(conversation.title.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(theme::TEXT_XS))
                            .text_color(p.fg_faint)
                            .child(age_label(&conversation.updated_at, self.now_ms)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(CH))
                    .pl(px(3.0 * CH))
                    .text_size(px(theme::TEXT_XS))
                    .line_height(px(16.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(status_color(t, summary.state))
                            .child(summary.status.clone()),
                    )
                    .child(div().flex_none().text_color(p.fg_faint).child(repo)),
            )
            .context_menu(move |menu, _window, _cx| {
                let mut menu = menu.min_w(px(240.0));
                for item in &menu_items {
                    menu = menu.item(menu_entry(item, &root));
                }
                menu
            })
            .into_any_element()
    }
}

fn menu_entry(item: &MenuItem, root: &gpui::Entity<Root>) -> PopupMenuItem {
    let label = item.label.clone();
    let detail = item.detail.clone();
    let mut entry = PopupMenuItem::element(move |_, cx| {
        let faint = Theme::of(cx).palette.fg_faint;
        div()
            .flex()
            .items_center()
            .gap(px(CH))
            .child(div().child(label.clone()))
            .children(detail.clone().map(|d| {
                div()
                    .text_size(px(theme::TEXT_XS))
                    .text_color(faint)
                    .child(d)
            }))
    })
    .checked(item.checked == Some(true));
    if let Some(action) = item.action.clone() {
        let root = root.clone();
        entry = entry.on_click(move |_, _: &mut Window, cx| {
            root.update(cx, |root, cx| root.run_menu(action.clone(), cx));
        });
    }
    entry
}
