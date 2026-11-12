use std::rc::Rc;

use gpui::{
    div, prelude::*, px, relative, AnyElement, Context, FontWeight, MouseButton, MouseDownEvent,
    Rgba, SharedString, Window,
};
use townhall_core::model::{AgentEvent, EntryBody, ThreadEntry};

use crate::attachments::lightbox::Open;
use crate::attachments::markers::strip_attachment_markers;
use crate::attachments::thumbs;
use crate::glyph::Glyph;
use crate::markdown::{self, Align, Block, Inline, ListItem};
use crate::selectable::SelectableText;
use crate::selection::BlockKey;
use crate::task_rows::{task_block_head, task_words};
use crate::theme::{self, Theme};
use crate::thread::{Reply, Thread};
use crate::thread_model::{ask_choices, clock_time, reply_target, repo_reason, AskChoices, Item};

const CH: f32 = theme::TEXT_MD * 0.6;
const MAX_COLUMN_CH: usize = 60;

struct Keys {
    entry: u64,
    next: usize,
}

impl Keys {
    fn next(&mut self) -> BlockKey {
        let key = BlockKey {
            entry: self.entry,
            block: self.next,
        };
        self.next += 1;
        key
    }
}

fn paragraphs(text: &str) -> Vec<Inline> {
    let mut parts = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in text.split('\n') {
        if line.trim().is_empty() {
            if !current.is_empty() {
                parts.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        parts.push(current.join("\n"));
    }
    parts.iter().map(|p| markdown::plain(p)).collect()
}

fn time_el(t: &Theme, at: &str) -> AnyElement {
    div()
        .flex_none()
        .pl(px(2.0 * CH))
        .text_size(px(theme::TEXT_XS))
        .text_color(t.palette.fg_faint)
        .child(clock_time(at))
        .into_any_element()
}

fn bubble(
    t: &Theme,
    background: Rgba,
    rule: Rgba,
    from_you: bool,
    content: AnyElement,
    at: &str,
) -> AnyElement {
    let inner = div()
        .flex()
        .items_baseline()
        .max_w(relative(theme::BUBBLE))
        .py(px(6.0))
        .pr(px(theme::SPACE_3))
        .pl(px(if from_you { theme::SPACE_3 } else { 3.0 * CH }))
        .bg(background)
        .border_color(rule)
        .child(div().child(content))
        .child(time_el(t, at));
    let inner = if from_you {
        inner.border_r(px(theme::ACCENT_RULE))
    } else {
        inner.border_l(px(theme::ACCENT_RULE))
    };
    let row = div().flex().w_full();
    let row = if from_you { row.justify_end() } else { row };
    row.child(inner).into_any_element()
}

impl Thread {
    pub(crate) fn render_item(
        &mut self,
        ix: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = Theme::of(cx);
        let Some((item, entry)) = self.shown.get(ix).cloned() else {
            return div().into_any_element();
        };
        let last = ix + 1 == self.shown.len();
        let body = match (item, entry) {
            (Item::Entry { open, answer, .. }, Some(entry)) => {
                self.entry_view(&entry, open, answer.as_deref(), &t, cx)
            }
            (Item::Thinking, _) => self.note_line(&t, "⠋", t.status_working(), "thinking"),
            (Item::Ended, _) => self.note_line(
                &t,
                "○",
                t.palette.fg_faint,
                "info: ended. a message continues it",
            ),
            _ => div().into_any_element(),
        };
        div()
            .w_full()
            .flex()
            .justify_center()
            .px(px(theme::SPACE_4))
            .pt(px(if ix == 0 { theme::SPACE_5 } else { 0.0 }))
            .pb(px(if last { theme::SPACE_6 } else { theme::SPACE_4 }))
            .child(
                div()
                    .w_full()
                    .max_w(px(theme::measure(self.pane_width)))
                    .child(body),
            )
            .into_any_element()
    }

    fn note_line(&self, t: &Theme, sign: &str, color: Rgba, text: &str) -> AnyElement {
        div()
            .flex()
            .justify_center()
            .gap(px(CH))
            .text_size(px(theme::TEXT_SM))
            .text_color(t.palette.fg_faint)
            .child(div().text_color(color).child(sign.to_string()))
            .child(text.to_string())
            .into_any_element()
    }

    fn text_block(
        &self,
        key: BlockKey,
        inline: &Inline,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.selection.filter(|s| s.key == key).map(|s| s.range());
        let text = SelectableText::new(key, inline, selected, t, self.registry.clone());
        div()
            .debug_selector(move || format!("block-{}-{}", key.entry, key.block))
            .cursor_text()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.press(key, event, window, cx)
                }),
            )
            .child(text)
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn event_line(
        &self,
        keys: &mut Keys,
        t: &Theme,
        sign: &str,
        sign_color: Rgba,
        text: &str,
        at: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = keys.next();
        div()
            .flex()
            .justify_center()
            .items_baseline()
            .gap(px(CH))
            .text_size(px(theme::TEXT_SM))
            .text_color(t.palette.fg_faint)
            .child(div().text_color(sign_color).child(sign.to_string()))
            .child(self.text_block(key, &markdown::plain(text), t, cx))
            .child(time_el(t, at))
            .into_any_element()
    }

    fn blocks_of(&mut self, entry: &ThreadEntry, text: &str) -> Rc<Vec<Block>> {
        if let Some((source, blocks)) = self.parsed.get(&entry.id) {
            if source == text {
                return blocks.clone();
            }
        }
        let blocks = Rc::new(markdown::parse(text));
        self.parsed
            .insert(entry.id, (text.to_string(), blocks.clone()));
        blocks
    }

    fn entry_view(
        &mut self,
        entry: &ThreadEntry,
        open: bool,
        answer: Option<&str>,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = t.palette;
        let mut keys = Keys {
            entry: entry.id,
            next: 0,
        };
        let at = entry.at.as_str();
        match &entry.body {
            EntryBody::You {
                text,
                reply_to,
                attachments,
            } => {
                let shared = thumbs::shared_in(attachments, &self.attachments);
                let names: Vec<String> = shared.iter().map(|a| a.name.clone()).collect();
                let stripped = strip_attachment_markers(text, &names);
                let text = &stripped;
                let mut col = div().flex().flex_col().gap(px(theme::SPACE_2));
                if let Some(quote) = reply_to
                    .and_then(|id| self.entries.iter().find(|e| e.id == id))
                    .and_then(reply_target)
                {
                    col = col.child(
                        div()
                            .text_size(px(theme::TEXT_XS))
                            .text_color(p.fg_faint)
                            .child(format!("↳ {}: {}", quote.author, quote.excerpt)),
                    );
                }
                for inline in paragraphs(text) {
                    let key = keys.next();
                    col = col.child(self.text_block(key, &inline, t, cx));
                }
                if let Some(conversation) = self.conversation_id.clone() {
                    let block = thumbs::Thumbs {
                        conversation: &conversation,
                        shared: &shared,
                        collapse: true,
                        border: p.line_control,
                    };
                    let images = block.images();
                    let all = thumbs::lightbox_images(&conversation, &images, "you");
                    let opens = cx.entity();
                    for row in block.render(t, cx, |index| {
                        let all = all.clone();
                        let opens = opens.clone();
                        Box::new(move |_, _, cx| {
                            let images = all.clone();
                            opens.update(cx, |_, cx| cx.emit(Open { images, index }))
                        })
                    }) {
                        col = col.child(row);
                    }
                }
                bubble(t, p.surface_2, p.accent, true, col.into_any_element(), at)
            }
            EntryBody::Townhall { text } => {
                let blocks = self.blocks_of(entry, text);
                let content = self.md_blocks(&mut keys, &blocks, t, cx);
                bubble(t, p.info_tint, p.info, false, content, at)
            }
            EntryBody::Repo {
                repo,
                base_branch,
                rule,
                reason,
            } => self.event_line(
                &mut keys,
                t,
                "~",
                p.fg_faint,
                &format!(
                    "repo {repo} · {base_branch}, {}",
                    repo_reason(reason, *rule)
                ),
                at,
                cx,
            ),
            EntryBody::RepoAsk {
                question,
                candidates,
                offer_all,
            } => self.ask_block(
                &mut keys,
                t,
                open,
                "townhall",
                question,
                answer,
                Some(ask_choices(candidates, *offer_all)),
                at,
                cx,
            ),
            EntryBody::NeedsYou {
                agent, question, ..
            } => self.ask_block(&mut keys, t, open, agent, question, answer, None, at, cx),
            EntryBody::TasksCreated { tasks } => self.tasks_block(tasks, t, at),
            EntryBody::Agent { agent, event, text } => {
                let (sign, color) = match event {
                    AgentEvent::Sent => ("→", p.fg_faint),
                    AgentEvent::Passed => ("✓", p.ok),
                    AgentEvent::Failed => ("×", p.danger),
                    AgentEvent::Ended => ("○", p.fg_faint),
                    AgentEvent::Allowed => ("+", p.fg_faint),
                };
                self.event_line(
                    &mut keys,
                    t,
                    sign,
                    color,
                    &agent_text(agent, *event, text),
                    at,
                    cx,
                )
            }
            EntryBody::Event { text } => {
                self.event_line(&mut keys, t, "◇", p.fg_faint, text, at, cx)
            }
            EntryBody::Handoff {
                task,
                from,
                to,
                reason,
            } => self.event_line(
                &mut keys,
                t,
                "↩",
                p.fg_faint,
                &format!("{task}: {from} handed back to {to}, {reason}"),
                at,
                cx,
            ),
            EntryBody::Plugin { plugin, text } => self.event_line(
                &mut keys,
                t,
                "◇",
                p.fg_faint,
                &format!("plugin {plugin}: {text}"),
                at,
                cx,
            ),
            EntryBody::Error { message } => self.event_line(
                &mut keys,
                t,
                "×",
                p.danger,
                &format!("error: {message}"),
                at,
                cx,
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn ask_block(
        &self,
        keys: &mut Keys,
        t: &Theme,
        open: bool,
        who: &str,
        question: &str,
        answer: Option<&str>,
        choices: Option<AskChoices>,
        at: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = t.palette;
        let head = if open {
            format!("{who} needs you")
        } else {
            format!("{who} asked")
        };
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(if open { theme::SPACE_2 } else { 2.0 }))
            .child(
                div()
                    .text_size(px(theme::TEXT_SM))
                    .text_color(if open { p.accent } else { p.fg_muted })
                    .when(open, |d| d.font_weight(FontWeight::BOLD))
                    .child(head),
            );
        if !question.is_empty() {
            let key = keys.next();
            col = col.child(
                div()
                    .text_color(if open { p.fg } else { p.fg_muted })
                    .child(self.text_block(key, &markdown::plain(question), t, cx)),
            );
        }
        if let Some(answer) = answer {
            col = col.child(
                div()
                    .flex()
                    .gap(px(CH))
                    .text_color(p.fg_faint)
                    .child("→")
                    .child(div().text_color(p.fg).child(answer.to_string())),
            );
        }
        if let (true, Some(choices)) = (open, choices) {
            let names = choices
                .candidates
                .iter()
                .cloned()
                .chain(choices.all.map(String::from));
            let mut row = div().flex().flex_wrap().gap(px(theme::SPACE_2));
            for name in names {
                row = row.child(self.reply_button(t, name, cx));
            }
            col = col.child(row);
        }
        let sign = if open { "◆" } else { "◇" };
        let sign_color = if open { p.accent } else { p.fg_faint };
        div()
            .flex()
            .items_baseline()
            .max_w(relative(theme::BUBBLE))
            .py(px(if open { 8.0 } else { 6.0 }))
            .pr(px(theme::SPACE_3))
            .when(open, |d| d.bg(p.accent_tint))
            .border_l(px(theme::ACCENT_RULE))
            .border_color(if open { p.accent } else { p.line_strong })
            .child(
                div()
                    .flex_none()
                    .w(px(3.0 * CH))
                    .text_color(sign_color)
                    .flex()
                    .justify_center()
                    .child(sign),
            )
            .child(div().child(col))
            .child(time_el(t, at))
            .into_any_element()
    }

    fn reply_button(&self, t: &Theme, name: String, cx: &mut Context<Self>) -> AnyElement {
        let p = t.palette;
        let key = format!("ask-{name}");
        let selector = key.clone();
        if self.thinking {
            return div()
                .debug_selector(move || selector)
                .flex()
                .items_center()
                .h(px(theme::ROW))
                .px(px(10.0))
                .border(px(theme::BORDER))
                .border_color(p.line)
                .text_size(px(theme::TEXT_SM))
                .text_color(p.fg_faint)
                .child(name)
                .into_any_element();
        }
        let reply = name.clone();
        div()
            .debug_selector(move || selector)
            .child(crate::home::button(
                t,
                SharedString::from(key),
                name,
                false,
                Box::new(
                    cx.listener(move |_, _: &MouseDownEvent, _, cx| cx.emit(Reply(reply.clone()))),
                ),
            ))
            .into_any_element()
    }

    fn tasks_block(&self, names: &[String], t: &Theme, at: &str) -> AnyElement {
        let p = t.palette;
        let mine: Vec<_> = names
            .iter()
            .filter_map(|name| self.tasks.iter().find(|task| &task.name == name).cloned())
            .collect();
        let head = task_block_head(&mine, names.len());
        let mut lines = div().flex().flex_col();
        for (i, name) in names.iter().enumerate() {
            let task = mine.iter().find(|task| &task.name == name);
            let (glyph, words, title) = match task {
                Some(task) => (
                    crate::task_rows::task_view(task, &self.tasks).glyph,
                    task_words(task, &self.tasks),
                    task.title.clone(),
                ),
                None => (Glyph::Queued, "queued".to_string(), String::new()),
            };
            lines = lines.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(CH))
                    .min_h(px(theme::ROW))
                    .px(px(theme::SPACE_2))
                    .text_size(px(theme::TEXT_SM))
                    .when(i > 0, |d| {
                        d.border_t(px(theme::BORDER)).border_color(p.line)
                    })
                    .child(
                        div()
                            .w(px(2.0 * CH))
                            .text_color(glyph.color(t))
                            .child(glyph.symbol()),
                    )
                    .child(div().font_weight(FontWeight::BOLD).child(name.clone()))
                    .child(div().text_color(p.fg_faint).child(words))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(p.fg_muted)
                            .child(title),
                    ),
            );
        }
        div()
            .w_full()
            .max_w(relative(theme::BUBBLE))
            .border(px(theme::BORDER))
            .border_color(p.line_strong)
            .bg(p.surface)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(CH))
                    .h(px(theme::CONTROL_SM))
                    .px(px(theme::SPACE_3))
                    .border_b(px(theme::BORDER))
                    .border_color(p.line)
                    .text_size(px(theme::TEXT_XS))
                    .text_color(p.fg_faint)
                    .child(div().flex_1().min_w_0().truncate().child(head))
                    .child(clock_time(at)),
            )
            .child(lines)
            .into_any_element()
    }

    fn md_blocks(
        &self,
        keys: &mut Keys,
        blocks: &[Block],
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut col = div().flex().flex_col().gap(px(theme::SPACE_2));
        for block in blocks {
            col = col.child(self.md_block(keys, block, t, cx));
        }
        col.into_any_element()
    }

    fn md_block(
        &self,
        keys: &mut Keys,
        block: &Block,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = t.palette;
        match block {
            Block::Paragraph(inline) => {
                let key = keys.next();
                self.text_block(key, inline, t, cx)
            }
            Block::Heading { level, text } => {
                let key = keys.next();
                let big = *level <= 2;
                div()
                    .mt(px(theme::SPACE_1))
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(if big { theme::TEXT_LG } else { theme::TEXT_MD }))
                    .line_height(px(if big {
                        theme::LEADING_LG
                    } else {
                        theme::LEADING_MD
                    }))
                    .child(self.text_block(key, text, t, cx))
                    .into_any_element()
            }
            Block::Code { text, .. } => {
                let key = keys.next();
                let id = SharedString::from(format!("code-{}-{}", key.entry, key.block));
                div()
                    .id(id)
                    .overflow_x_scroll()
                    .px(px(theme::SPACE_3))
                    .py(px(theme::SPACE_2))
                    .border(px(theme::BORDER))
                    .border_color(p.line)
                    .bg(p.surface)
                    .child(div().whitespace_nowrap().child(self.text_block(
                        key,
                        &markdown::plain(text),
                        t,
                        cx,
                    )))
                    .into_any_element()
            }
            Block::Quote(inner) => div()
                .pl(px(CH))
                .border_l(px(theme::ACCENT_RULE))
                .border_color(p.line_strong)
                .text_color(p.fg_muted)
                .child(self.md_blocks(keys, inner, t, cx))
                .into_any_element(),
            Block::List { start, items } => {
                let mut col = div().flex().flex_col().pl(px(CH));
                for (i, item) in items.iter().enumerate() {
                    col = col.child(self.list_item(keys, *start, i, item, t, cx));
                }
                col.into_any_element()
            }
            Block::Table { align, head, rows } => self.table(keys, align, head, rows, t, cx),
            Block::Rule => div()
                .h(px(theme::BORDER))
                .my(px(theme::SPACE_1))
                .bg(p.line_strong)
                .into_any_element(),
        }
    }

    fn list_item(
        &self,
        keys: &mut Keys,
        start: Option<u64>,
        index: usize,
        item: &ListItem,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = t.palette;
        let marker = match (item.task, start) {
            (Some(true), _) => "☑".to_string(),
            (Some(false), _) => "☐".to_string(),
            (None, Some(n)) => format!("{}.", n + index as u64),
            (None, None) => "•".to_string(),
        };
        div()
            .flex()
            .items_baseline()
            .child(
                div()
                    .flex_none()
                    .w(px(3.0 * CH))
                    .flex()
                    .justify_end()
                    .pr(px(CH))
                    .text_color(p.fg_faint)
                    .child(marker),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(self.md_blocks(keys, &item.blocks, t, cx)),
            )
            .into_any_element()
    }

    fn table(
        &self,
        keys: &mut Keys,
        align: &[Align],
        head: &[Inline],
        rows: &[Vec<Inline>],
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = t.palette;
        let columns = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
        let widths: Vec<f32> = (0..columns)
            .map(|c| {
                let longest = std::iter::once(head.get(c))
                    .chain(rows.iter().map(|r| r.get(c)))
                    .flatten()
                    .flat_map(|cell| cell.text.lines().map(|l| l.chars().count()))
                    .max()
                    .unwrap_or(0);
                (longest.clamp(3, MAX_COLUMN_CH) as f32 + 2.0) * theme::TEXT_SM * 0.6
            })
            .collect();
        let total: f32 = widths.iter().sum();
        let key = keys.next();
        let id = SharedString::from(format!("table-{}-{}", key.entry, key.block));
        let mut grid = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(total + theme::BORDER))
            .border_l(px(theme::BORDER))
            .border_t(px(theme::BORDER))
            .border_color(p.line_strong)
            .text_size(px(theme::TEXT_SM))
            .line_height(px(theme::LEADING_SM));
        let mut all: Vec<(&[Inline], bool)> = vec![(head, true)];
        all.extend(rows.iter().map(|r| (r.as_slice(), false)));
        for (cells, is_head) in all {
            let mut row = div().flex();
            for (c, width) in widths.iter().enumerate() {
                let content = match cells.get(c) {
                    Some(inline) => {
                        let key = keys.next();
                        self.text_block(key, inline, t, cx)
                    }
                    None => div().into_any_element(),
                };
                let cell = div()
                    .flex_none()
                    .w(px(*width))
                    .px(px(theme::TEXT_SM * 0.6))
                    .py(px(3.0))
                    .border_r(px(theme::BORDER))
                    .border_b(px(theme::BORDER))
                    .border_color(p.line_strong)
                    .flex()
                    .when(is_head, |d| d.bg(p.surface_2).font_weight(FontWeight::BOLD))
                    .map(|d| match align.get(c) {
                        Some(Align::Center) => d.justify_center(),
                        Some(Align::Right) => d.justify_end(),
                        _ => d,
                    })
                    .child(content);
                row = row.child(cell);
            }
            grid = grid.child(row);
        }
        div()
            .id(id)
            .overflow_x_scroll()
            .child(grid)
            .into_any_element()
    }
}

fn agent_text(agent: &str, event: AgentEvent, text: &str) -> String {
    let text = text.trim();
    match event {
        AgentEvent::Allowed => {
            let (tool, kind) = match text.rfind(" to ") {
                Some(cut) => (&text[..cut], &text[cut + 4..]),
                None => (text, ""),
            };
            if kind.is_empty() {
                format!("{agent} allowed {tool}")
            } else {
                format!("{agent} allowed {tool} for agent type {kind}")
            }
        }
        AgentEvent::Sent if text.is_empty() => format!("sent to {agent}"),
        AgentEvent::Sent => format!("sent to {agent}: {text}"),
        other => {
            let words = match other {
                AgentEvent::Passed => "passed",
                AgentEvent::Failed => "failed",
                _ => "ended",
            };
            format!("{agent} {}", if text.is_empty() { words } else { text })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_split_on_blank_lines_and_keep_single_line_breaks() {
        let texts: Vec<String> = paragraphs("one\ntwo\n\n  \nthree\n")
            .into_iter()
            .map(|p| p.text)
            .collect();
        assert_eq!(texts, ["one\ntwo", "three"]);
        assert!(paragraphs("  \n ").is_empty());
    }

    #[test]
    fn agent_lines_read_as_the_web_app_words_them() {
        assert_eq!(agent_text("fix", AgentEvent::Sent, "y"), "sent to fix: y");
        assert_eq!(agent_text("fix", AgentEvent::Sent, " "), "sent to fix");
        assert_eq!(
            agent_text("fix", AgentEvent::Failed, "tests red"),
            "fix tests red"
        );
        assert_eq!(agent_text("fix", AgentEvent::Ended, ""), "fix ended");
        assert_eq!(
            agent_text("fix", AgentEvent::Allowed, "Bash(npm test) to Coder"),
            "fix allowed Bash(npm test) for agent type Coder"
        );
        assert_eq!(
            agent_text("fix", AgentEvent::Allowed, "Bash(npm test)"),
            "fix allowed Bash(npm test)"
        );
    }
}
