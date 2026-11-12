use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use gpui::{
    actions, div, prelude::*, px, AnyElement, Context, Entity, FontWeight, IntoElement,
    MouseDownEvent, PathPromptOptions, Rgba, SharedString, Subscription, Window,
};
use gpui_component::input::{InputEvent, InputState};
use townhall_core::clock::unix_millis;
use townhall_core::model::{ConversationStatus, ServerEvent};

use crate::client::Api;
use crate::composer::{Composer, Started};
use crate::drafts::Drafts;
use crate::home::daemon_error_text;
use crate::home_model::{greeting_text, repos_known_label, starter_prompts};
use crate::prefs::Prefs;
use crate::row_menu::{row_menu_items, MenuAction, MenuItem};
use crate::sections::{normalize_query, Filter};
use crate::sidebar_rows::{build_sections, repo_blocks, FoldState, GroupBlock, RowSections};
use crate::state::{Load, Refetch, State};
use crate::task_rows::{needs_you_count, unfinished_count};
use crate::theme::{self, Theme};
use crate::thread::Thread;
use crate::thread_model::{conversation_status, end_question};

actions!(townhall_root, [FocusSearch]);

pub const SEARCH_DELAY: Duration = Duration::from_millis(200);
pub const AGE_TICK: Duration = Duration::from_secs(60);
const RAIL_MIN_WIDTH: f32 = 1200.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub ended: bool,
    pub selected: bool,
    pub tasks: usize,
    pub thinking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Conversation,
    Home,
    FirstRun,
}

#[derive(Debug, Default)]
struct Matches {
    query: String,
    ids: HashSet<String>,
    error: Option<String>,
}

fn now_ms() -> i64 {
    unix_millis(SystemTime::now())
}

pub struct Root {
    pub(crate) api: Arc<dyn Api>,
    pub(crate) state: State,
    pub(crate) composer: Entity<Composer>,
    start: Entity<Composer>,
    thread: Entity<Thread>,
    pub(crate) search: Entity<InputState>,
    prefs: Option<Prefs>,
    pub(crate) by_repo: bool,
    pub(crate) folded: HashSet<String>,
    pub(crate) query: String,
    matches: Matches,
    search_generation: u64,
    pub(crate) action_error: Option<String>,
    home: bool,
    pub(crate) now_ms: i64,
    retrying: bool,
    rail_open: bool,
    asking_end: bool,
    renders: usize,
    daemon_note: SharedString,
    _subscriptions: Vec<Subscription>,
}

impl Root {
    pub fn new(
        api: Arc<dyn Api>,
        events: Option<async_channel::Receiver<ServerEvent>>,
        drafts: Drafts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let composer = cx.new(|cx| Composer::new(api.clone(), drafts, window, cx));
        let start = cx.new(|cx| Composer::new_start(api.clone(), window, cx));
        let thread = cx.new(Thread::new);
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("search"));
        let subscriptions = vec![
            cx.subscribe_in(&search, window, Self::on_search),
            cx.subscribe_in(&start, window, Self::on_started),
        ];
        let root = Self {
            api,
            state: State::new(),
            composer,
            start,
            thread,
            search,
            prefs: None,
            by_repo: false,
            folded: HashSet::new(),
            query: String::new(),
            matches: Matches::default(),
            search_generation: 0,
            action_error: None,
            home: false,
            now_ms: now_ms(),
            retrying: false,
            rail_open: true,
            asking_end: false,
            renders: 0,
            daemon_note: SharedString::default(),
            _subscriptions: subscriptions,
        };
        root.composer
            .update(cx, |composer, cx| composer.focus(window, cx));
        root.reload(true, window, cx);
        if let Some(events) = events {
            cx.spawn_in(window, async move |this, cx| -> gpui::Result<()> {
                while let Ok(event) = events.recv().await {
                    this.update_in(cx, |this, window, cx| this.on_event(event, window, cx))?;
                }
                Ok(())
            })
            .detach_and_log_err(cx);
        }
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            loop {
                cx.background_executor().timer(AGE_TICK).await;
                this.update(cx, |this, cx| {
                    this.now_ms = now_ms();
                    cx.notify();
                })?;
            }
        })
        .detach_and_log_err(cx);
        root
    }

    pub fn with_daemon_note(mut self, note: impl Into<SharedString>) -> Self {
        self.daemon_note = note.into();
        self
    }

    pub fn with_prefs(mut self, prefs: Prefs) -> Self {
        self.by_repo = prefs.by_repo();
        self.prefs = Some(prefs);
        self
    }

    #[cfg(test)]
    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn renders(&self) -> usize {
        self.renders
    }

    pub fn screen(&self) -> Screen {
        if !self.home && self.state.selected.is_some() {
            return Screen::Conversation;
        }
        match &self.state.repos {
            Load::Ready(repos) if repos.is_empty() => Screen::FirstRun,
            _ => Screen::Home,
        }
    }

    pub(crate) fn is_current(&self, id: &str) -> bool {
        !self.home && self.state.selected.as_deref() == Some(id)
    }

    fn searching(&self) -> bool {
        !normalize_query(&self.query).is_empty()
    }

    pub fn sections(&self) -> RowSections {
        let Load::Ready(list) = &self.state.conversations else {
            return RowSections {
                active: Vec::new(),
                older: Vec::new(),
            };
        };
        let none = HashSet::new();
        let settled = self.matches.query == self.query.trim();
        let filter = self.searching().then(|| Filter {
            query: &self.query,
            text_matches: if settled { &self.matches.ids } else { &none },
        });
        build_sections(list, self.state.task_list(), filter.as_ref())
    }

    pub fn blocks(&self) -> Option<Vec<GroupBlock>> {
        if !self.by_repo {
            return None;
        }
        let current = self.state.selected.as_deref().filter(|_| !self.home);
        Some(repo_blocks(
            &self.sections().active,
            &FoldState {
                folded: &self.folded,
                current_id: current,
                searching: self.searching(),
            },
        ))
    }

    pub fn active_rows(&self) -> Vec<Row> {
        self.rows(false)
    }

    pub fn older_rows(&self) -> Vec<Row> {
        self.rows(true)
    }

    fn rows(&self, ended: bool) -> Vec<Row> {
        let sections = self.sections();
        let rows = if ended {
            sections.older
        } else {
            sections.active
        };
        rows.into_iter()
            .map(|r| Row {
                selected: self.is_current(&r.conversation.id),
                tasks: r.conversation.tasks.len(),
                thinking: r.conversation.status == ConversationStatus::Thinking,
                ended: r.conversation.ended,
                title: r.conversation.title,
                id: r.conversation.id,
            })
            .collect()
    }

    pub fn toggle_by_repo(&mut self, cx: &mut Context<Self>) {
        self.by_repo = !self.by_repo;
        if let Some(prefs) = self.prefs.as_mut() {
            if let Err(e) = prefs.set_by_repo(self.by_repo) {
                tracing::warn!(
                    target: "townhall::native",
                    event = "prefs.save_failed",
                    error = %e,
                    "cannot save the sidebar choice"
                );
            }
        }
        cx.notify();
    }

    pub fn toggle_group(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.folded.remove(key) {
            self.folded.insert(key.to_string());
        }
        cx.notify();
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |input, cx| input.focus(window, cx));
    }

    pub fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.set_query(String::new(), cx);
        self.composer
            .update(cx, |composer, cx| composer.focus(window, cx));
    }

    pub fn query(&self, cx: &gpui::App) -> String {
        self.search.read(cx).value().to_string()
    }

    pub fn match_error(&self) -> Option<&str> {
        self.matches.error.as_deref().filter(|_| self.searching())
    }

    fn on_search(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Change = event {
            let text = self.search.read(cx).value().to_string();
            self.set_query(text, cx);
        }
    }

    fn set_query(&mut self, text: String, cx: &mut Context<Self>) {
        if self.query == text {
            return;
        }
        self.query = text;
        self.search_generation += 1;
        cx.notify();
        let wanted = self.query.trim().to_string();
        if wanted.is_empty() {
            self.matches = Matches::default();
            return;
        }
        let generation = self.search_generation;
        let api = self.api.clone();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            cx.background_executor().timer(SEARCH_DELAY).await;
            if !this.update(cx, |this, _| this.search_generation == generation)? {
                return Ok(());
            }
            let asked = wanted.clone();
            let result = cx
                .background_executor()
                .spawn(async move { api.search(&asked) })
                .await;
            this.update(cx, |this, cx| {
                if this.search_generation != generation {
                    return;
                }
                this.matches = match result {
                    Ok(hits) => Matches {
                        query: wanted,
                        ids: hits.into_iter().map(|h| h.conversation_id).collect(),
                        error: None,
                    },
                    Err(e) => Matches {
                        query: wanted,
                        ids: HashSet::new(),
                        error: Some(e),
                    },
                };
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    pub fn menu_items(&self, id: &str) -> Vec<MenuItem> {
        let Load::Ready(list) = &self.state.conversations else {
            return Vec::new();
        };
        let sections = build_sections(list, self.state.task_list(), None);
        sections
            .active
            .iter()
            .chain(sections.older.iter())
            .find(|row| row.conversation.id == id)
            .map(|row| row_menu_items(row, self.state.repo_list()))
            .unwrap_or_default()
    }

    pub fn run_menu(&mut self, action: MenuAction, cx: &mut Context<Self>) {
        self.action_error = None;
        let api = self.api.clone();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match action {
                        MenuAction::End(id) => api.end_conversation(&id),
                        MenuAction::Link(id, repo) => api.link_repo(&id, repo.as_deref()),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(conversation) => {
                        this.state
                            .apply(ServerEvent::ConversationUpdated { conversation });
                        this.sync_thread(cx);
                    }
                    Err(e) => this.action_error = Some(e),
                }
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
        cx.notify();
    }

    pub fn action_error(&self) -> Option<&str> {
        self.action_error.as_deref()
    }

    pub fn go_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.home = true;
        self.asking_end = false;
        self.start
            .update(cx, |composer, cx| composer.focus(window, cx));
        cx.notify();
    }

    pub fn starters(&self) -> Vec<String> {
        let repos = match &self.state.repos {
            Load::Ready(repos) => Some(repos.as_slice()),
            _ => None,
        };
        starter_prompts(repos)
    }

    pub fn greeting_at(&self, hour: u32) -> String {
        greeting_text(hour, self.state.user_name())
    }

    pub fn pick_starter(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.start
            .update(cx, |composer, cx| composer.fill(text, window, cx));
    }

    pub fn start_composer(&self) -> &Entity<Composer> {
        &self.start
    }

    fn on_started(
        &mut self,
        _: &Entity<Composer>,
        event: &Started,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let detail = event.0.clone();
        let id = detail.conversation.id.clone();
        self.state.apply(ServerEvent::ConversationUpdated {
            conversation: detail.conversation.clone(),
        });
        self.home = false;
        self.asking_end = false;
        self.state.select(id.clone());
        self.state.set_detail(&id, Ok(detail));
        self.composer.update(cx, |composer, cx| {
            composer.switch_to(Some(id.clone()), window, cx)
        });
        self.sync_thinking(window, cx);
        self.sync_thread(cx);
        cx.notify();
    }

    pub fn add_repo(&mut self, path: String, _window: &mut Window, cx: &mut Context<Self>) {
        self.action_error = None;
        let api = self.api.clone();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            let result = cx
                .background_executor()
                .spawn(async move { api.add_repo(&path) })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(_) => this.load_repos(cx),
                    Err(e) => this.action_error = Some(e),
                }
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
        cx.notify();
    }

    pub fn browse_repo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("add repository".into()),
        });
        cx.spawn_in(window, async move |this, cx| -> gpui::Result<()> {
            if let Ok(Ok(Some(paths))) = picked.await {
                if let Some(path) = paths.first() {
                    let path = path.to_string_lossy().into_owned();
                    this.update_in(cx, |this, window, cx| this.add_repo(path, window, cx))?;
                }
            }
            Ok(())
        })
        .detach_and_log_err(cx);
    }

    pub fn banner(&self) -> Option<String> {
        match &self.state.conversations {
            Load::Failed(e) => Some(daemon_error_text(e)),
            _ => None,
        }
    }

    pub fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.retrying = true;
        self.reload(true, window, cx);
        cx.notify();
    }

    pub fn on_event(&mut self, event: ServerEvent, window: &mut Window, cx: &mut Context<Self>) {
        match self.state.apply(event) {
            Refetch::Nothing => {}
            Refetch::Everything => {
                self.reload(false, window, cx);
                if let Some(id) = self.state.selected.clone() {
                    self.load_detail(id, cx);
                }
            }
        }
        self.sync_thinking(window, cx);
        self.sync_thread(cx);
        cx.notify();
    }

    fn selected_thinking(&self) -> bool {
        match (&self.state.conversations, self.state.selected.as_deref()) {
            (Load::Ready(list), Some(id)) => list
                .iter()
                .any(|c| c.id == id && c.status == ConversationStatus::Thinking),
            _ => false,
        }
    }

    fn sync_thinking(&self, window: &mut Window, cx: &mut Context<Self>) {
        let thinking = self.selected_thinking();
        self.composer.update(cx, |composer, cx| {
            composer.set_thinking(thinking, window, cx)
        });
    }

    fn sync_thread(&self, cx: &mut Context<Self>) {
        let thinking = self.selected_thinking();
        let state = &self.state;
        self.thread.update(cx, |thread, cx| {
            let detail = match &state.detail {
                Load::Ready(detail) => Some(detail),
                _ => None,
            };
            thread.sync(detail, thinking, cx);
        });
    }

    fn reload(&self, select_newest: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.load_list(select_newest, window, cx);
        self.load_tasks(cx);
        self.load_repos(cx);
        self.load_info(cx);
    }

    fn load_list(&self, select_newest: bool, window: &mut Window, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn_in(window, async move |this, cx| -> gpui::Result<()> {
            let result = cx
                .background_executor()
                .spawn(async move { api.conversations() })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.retrying = false;
                this.state.set_list(result);
                if select_newest && this.state.selected.is_none() {
                    if let Some(id) = this.state.newest() {
                        this.select(id, window, cx);
                    }
                }
                this.sync_thinking(window, cx);
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    fn load_tasks(&self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            let result = cx
                .background_executor()
                .spawn(async move { api.tasks() })
                .await;
            this.update(cx, |this, cx| {
                this.state.set_tasks(result);
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    fn load_repos(&self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            let result = cx
                .background_executor()
                .spawn(async move { api.repos() })
                .await;
            this.update(cx, |this, cx| {
                this.state.set_repos(result);
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    fn load_info(&self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            let result = cx
                .background_executor()
                .spawn(async move { api.info() })
                .await;
            this.update(cx, |this, cx| {
                this.state.set_info(result);
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    pub fn select(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.home = false;
        self.asking_end = false;
        self.state.select(id.clone());
        self.composer.update(cx, |composer, cx| {
            composer.switch_to(Some(id.clone()), window, cx)
        });
        self.sync_thinking(window, cx);
        self.sync_thread(cx);
        self.load_detail(id, cx);
        cx.notify();
    }

    fn load_detail(&self, id: String, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx| -> gpui::Result<()> {
            let wanted = id.clone();
            let result = cx
                .background_executor()
                .spawn(async move { api.conversation(&wanted) })
                .await;
            this.update(cx, |this, cx| {
                this.state.set_detail(&id, result);
                this.sync_thread(cx);
                cx.notify();
            })
        })
        .detach_and_log_err(cx);
    }

    fn conversation_header(&self, t: Theme, cx: &mut Context<Self>) -> AnyElement {
        let p = t.palette;
        let Load::Ready(detail) = &self.state.detail else {
            return div()
                .h(px(theme::BAR))
                .flex_none()
                .border_b(px(theme::BORDER))
                .border_color(p.line)
                .into_any_element();
        };
        let conversation = &detail.conversation;
        let status = conversation_status(conversation);
        let unfinished = unfinished_count(&detail.tasks);
        let waiting = needs_you_count(&detail.tasks);
        let id = conversation.id.clone();
        let repo = crate::session_model::repo_line(detail)
            .map(|line| line.label.trim_start_matches("~ ").to_string())
            .unwrap_or_default();
        let ch = theme::TEXT_SM * 0.6;
        let mut actions = div()
            .flex()
            .items_center()
            .gap(px(theme::SPACE_1))
            .ml_auto();
        if self.asking_end {
            let end_id = id.clone();
            actions = actions
                .child(
                    div()
                        .text_size(px(theme::TEXT_SM))
                        .text_color(p.fg_muted)
                        .child(end_question(&conversation.title, unfinished)),
                )
                .child(crate::home::button(
                    &t,
                    "end-cancel",
                    "cancel".to_string(),
                    false,
                    Box::new(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        this.asking_end = false;
                        cx.notify();
                    })),
                ))
                .child(crate::home::button(
                    &t,
                    "end-confirm",
                    "end".to_string(),
                    true,
                    Box::new(cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        this.asking_end = false;
                        this.run_menu(MenuAction::End(end_id.clone()), cx);
                    })),
                ));
        } else {
            let label = if waiting > 0 {
                format!("◆ {waiting} · session")
            } else {
                "session".to_string()
            };
            actions = actions.child(crate::home::button(
                &t,
                "rail-toggle",
                label,
                false,
                Box::new(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.rail_open = !this.rail_open;
                    cx.notify();
                })),
            ));
            if !conversation.ended {
                actions = actions.child(crate::home::button(
                    &t,
                    "end",
                    "end".to_string(),
                    false,
                    Box::new(cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        if unfinished > 0 {
                            this.asking_end = true;
                            cx.notify();
                        } else {
                            this.run_menu(MenuAction::End(id.clone()), cx);
                        }
                    })),
                ));
            }
        }
        let status_color: Rgba = match status {
            "thinking" => t.status_working(),
            "ended" => p.fg_faint,
            _ => p.fg_muted,
        };
        div()
            .h(px(theme::BAR))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(theme::SPACE_2))
            .pl(px(theme::SPACE_4))
            .pr(px(theme::SPACE_2))
            .border_b(px(theme::BORDER))
            .border_color(p.line)
            .bg(p.surface)
            .child(
                div()
                    .truncate()
                    .font_weight(FontWeight::BOLD)
                    .child(conversation.title.clone()),
            )
            .when(!self.asking_end, |d| {
                d.child(
                    div()
                        .truncate()
                        .text_size(px(theme::TEXT_XS))
                        .text_color(p.fg_faint)
                        .child(repo),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .gap(px(ch))
                        .text_size(px(theme::TEXT_SM))
                        .text_color(status_color)
                        .when(status == "thinking", |d| d.child("⠋"))
                        .child(status),
                )
            })
            .child(actions)
            .into_any_element()
    }

    fn conversation_screen(
        &mut self,
        t: Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = t.palette;
        let viewport = f32::from(window.viewport_size().width);
        let rail = self.rail_open && viewport >= RAIL_MIN_WIDTH;
        let pane_width =
            viewport - theme::SIDEBAR - if rail { theme::RAIL } else { 0.0 } - 2.0 * theme::SPACE_4;
        self.thread
            .update(cx, |thread, cx| thread.set_pane_width(pane_width, cx));
        let header = self.conversation_header(t, cx);
        let body: AnyElement = match &self.state.detail {
            Load::Ready(_) => div()
                .flex_1()
                .min_h_0()
                .child(self.thread.clone())
                .into_any_element(),
            Load::Loading => note(p.fg_faint, "loading…").into_any_element(),
            Load::Failed(e) => note(t.status_error(), format!("error: {e}")).into_any_element(),
        };
        let measure = theme::measure(pane_width) + 2.0 * theme::SPACE_4;
        let center = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(header)
            .child(body)
            .child(
                div().w_full().flex_none().flex().justify_center().child(
                    div()
                        .w_full()
                        .max_w(px(measure))
                        .child(self.composer.clone()),
                ),
            );
        let mut row = div().flex_1().min_h_0().flex().child(center);
        if rail {
            if let Load::Ready(detail) = &self.state.detail {
                row = row.child(crate::session::session_panel(detail, &t));
            }
        }
        row.into_any_element()
    }

    fn home_screen(&self, t: Theme, cx: &mut Context<Self>) -> AnyElement {
        let p = t.palette;
        let hour = local_hour();
        let mut starters = div()
            .flex()
            .flex_wrap()
            .justify_center()
            .gap(px(theme::SPACE_2));
        for (i, prompt) in self.starters().into_iter().enumerate() {
            let text = prompt.clone();
            starters = starters.child(crate::home::starter(
                &t,
                SharedString::from(format!("starter-{i}")),
                prompt,
                Box::new(cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    this.pick_starter(&text, window, cx)
                })),
            ));
        }
        let known = match &self.state.repos {
            Load::Ready(repos) if !repos.is_empty() => {
                let names: Vec<&str> = repos.iter().map(|r| r.name.as_str()).collect();
                Some((repos_known_label(repos.len()), names.join(", ")))
            }
            _ => None,
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .p(px(theme::SPACE_5))
            .child(
                div()
                    .w_full()
                    .max_w(px(theme::HOME))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap(px(theme::SPACE_4))
                    .child(crate::home::greeting(&t, self.greeting_at(hour)))
                    .child(div().w_full().child(self.start.clone()))
                    .when_some(known, |d, (label, names)| {
                        d.child(
                            div()
                                .flex()
                                .justify_center()
                                .gap(px(theme::TEXT_XS * 0.6))
                                .text_size(px(theme::TEXT_XS))
                                .text_color(p.fg_faint)
                                .child(div().text_color(p.accent).child("~"))
                                .child(label)
                                .child(format!("({names})")),
                        )
                    })
                    .child(starters),
            )
            .into_any_element()
    }

    fn main_screen(&mut self, t: Theme, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        match self.screen() {
            Screen::Conversation => self.conversation_screen(t, window, cx),
            Screen::Home => self.home_screen(t, cx),
            Screen::FirstRun => {
                crate::home::first_run(
                    &t,
                    self.action_error.clone(),
                    Box::new(cx.listener(|this, _: &MouseDownEvent, window, cx| {
                        this.browse_repo(window, cx)
                    })),
                )
            }
        }
    }
}

fn local_hour() -> u32 {
    let secs = (now_ms() / 1000) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
        return 12;
    }
    tm.tm_hour as u32
}

pub(crate) fn note(color: Rgba, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .px(px(theme::SPACE_3))
        .py(px(theme::SPACE_2))
        .text_color(color)
        .child(text.into())
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        let t = Theme::of(cx);
        let p = t.palette;
        let banner = self.banner().map(|text| {
            crate::home::banner(
                &t,
                text,
                self.retrying,
                Box::new(
                    cx.listener(|this, _: &MouseDownEvent, window, cx| this.retry(window, cx)),
                ),
            )
        });
        let main = self.main_screen(t, window, cx);
        let sidebar = self.sidebar(t, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.fg)
            .font_family(theme::FONT)
            .font_features(theme::no_ligatures())
            .text_size(px(theme::TEXT_MD))
            .on_action(
                cx.listener(|this, _: &FocusSearch, window, cx| this.focus_search(window, cx)),
            )
            .child(
                div().flex_1().min_h_0().flex().child(sidebar).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .when_some(banner, |d, b| d.child(b))
                        .child(div().flex_1().min_h_0().child(main)),
                ),
            )
            .child(
                div()
                    .h(px(theme::KEYBAR))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px(px(theme::SPACE_3))
                    .border_t(px(theme::BORDER))
                    .border_color(p.line)
                    .bg(p.surface)
                    .text_size(px(theme::TEXT_XS))
                    .text_color(p.fg_faint)
                    .child(self.daemon_note.clone()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests::{conversation, detail, entry};
    use gpui::TestAppContext;
    use gpui::{EntityInputHandler, VisualTestContext};
    use std::sync::Mutex;
    use townhall_core::model::{Conversation, ConversationDetail, ThreadEntry};

    #[derive(Default)]
    struct Fake {
        list: Mutex<Vec<Conversation>>,
        details: Mutex<Vec<ConversationDetail>>,
        sent: Mutex<Vec<(String, String)>>,
        send_error: Mutex<Option<String>>,
        tasks: Mutex<Vec<townhall_core::model::Task>>,
        repos: Mutex<Vec<townhall_core::model::Repo>>,
        user_name: Mutex<Option<String>>,
        hits: Mutex<Vec<townhall_core::search::SearchHit>>,
        searched: Mutex<Vec<String>>,
        ended: Mutex<Vec<String>>,
        linked: Mutex<Vec<(String, Option<String>)>>,
        started: Mutex<Vec<String>>,
        added: Mutex<Vec<String>>,
        action_error: Mutex<Option<String>>,
        down: Mutex<bool>,
    }

    impl Fake {
        fn check_up(&self) -> Result<(), String> {
            if *self.down.lock().unwrap() {
                Err("connection refused".into())
            } else {
                Ok(())
            }
        }

        fn check_action(&self) -> Result<(), String> {
            self.check_up()?;
            match self.action_error.lock().unwrap().clone() {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }

        fn find(&self, id: &str) -> Result<Conversation, String> {
            self.list
                .lock()
                .unwrap()
                .iter()
                .find(|c| c.id == id)
                .cloned()
                .ok_or_else(|| format!("no conversation {id}"))
        }
    }

    impl Api for Fake {
        fn conversations(&self) -> Result<Vec<Conversation>, String> {
            self.check_up()?;
            Ok(self.list.lock().unwrap().clone())
        }

        fn conversation(&self, id: &str) -> Result<ConversationDetail, String> {
            self.check_up()?;
            self.details
                .lock()
                .unwrap()
                .iter()
                .find(|d| d.conversation.id == id)
                .cloned()
                .ok_or_else(|| format!("no conversation {id}"))
        }

        fn send_message(&self, id: &str, text: &str) -> Result<ThreadEntry, String> {
            if let Some(e) = self.send_error.lock().unwrap().clone() {
                return Err(e);
            }
            self.sent
                .lock()
                .unwrap()
                .push((id.to_string(), text.to_string()));
            Ok(entry(99, text))
        }

        fn tasks(&self) -> Result<Vec<townhall_core::model::Task>, String> {
            self.check_up()?;
            Ok(self.tasks.lock().unwrap().clone())
        }

        fn repos(&self) -> Result<Vec<townhall_core::model::Repo>, String> {
            self.check_up()?;
            Ok(self.repos.lock().unwrap().clone())
        }

        fn info(&self) -> Result<crate::client::Info, String> {
            self.check_up()?;
            Ok(crate::client::Info {
                user_name: self.user_name.lock().unwrap().clone(),
            })
        }

        fn search(&self, query: &str) -> Result<Vec<townhall_core::search::SearchHit>, String> {
            self.check_up()?;
            self.searched.lock().unwrap().push(query.to_string());
            Ok(self.hits.lock().unwrap().clone())
        }

        fn end_conversation(&self, id: &str) -> Result<Conversation, String> {
            self.check_action()?;
            self.ended.lock().unwrap().push(id.to_string());
            let mut c = self.find(id)?;
            c.ended = true;
            Ok(c)
        }

        fn link_repo(&self, id: &str, repo: Option<&str>) -> Result<Conversation, String> {
            self.check_action()?;
            self.linked
                .lock()
                .unwrap()
                .push((id.to_string(), repo.map(String::from)));
            let mut c = self.find(id)?;
            c.linked_repo = repo.map(String::from);
            Ok(c)
        }

        fn start_conversation(&self, text: &str) -> Result<ConversationDetail, String> {
            self.check_action()?;
            self.started.lock().unwrap().push(text.to_string());
            let mut c = conversation("new", text, "2026-11-12T11:00:00.000Z");
            c.session_started = true;
            Ok(detail_for(c, vec![entry(1, "on it")]))
        }

        fn add_repo(&self, path: &str) -> Result<townhall_core::model::Repo, String> {
            self.check_action()?;
            self.added.lock().unwrap().push(path.to_string());
            let repo = crate::testkit::repo("shop", "main");
            self.repos.lock().unwrap().push(repo.clone());
            Ok(repo)
        }
    }

    fn detail_for(conversation: Conversation, entries: Vec<ThreadEntry>) -> ConversationDetail {
        crate::testkit::detail_of(conversation, entries, vec![])
    }

    fn fake() -> Arc<Fake> {
        let mut ended = conversation("b", "finished work", "2026-11-12T07:00:00.000Z");
        ended.ended = true;
        Arc::new(Fake {
            list: Mutex::new(vec![
                conversation("a", "fix the build", "2026-11-12T10:00:00.000Z"),
                ended,
            ]),
            details: Mutex::new(vec![
                detail("a", vec![entry(1, "on it")]),
                detail("b", vec![entry(1, "done")]),
            ]),
            repos: Mutex::new(vec![crate::testkit::repo("shop", "develop")]),
            ..Fake::default()
        })
    }

    fn drafts() -> Drafts {
        Drafts::open(tempfile::tempdir().unwrap().keep().as_path()).unwrap()
    }

    fn titles(rows: Vec<Row>) -> Vec<String> {
        rows.into_iter().map(|r| r.title).collect()
    }

    fn entry_count(root: &Root) -> usize {
        match &root.state().detail {
            Load::Ready(d) => d.entries.len(),
            other => panic!("{other:?}"),
        }
    }

    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(Theme::new(theme::Mode::Dark));
        });
    }

    fn open<'a>(
        cx: &'a mut TestAppContext,
        api: Arc<Fake>,
        home: &std::path::Path,
    ) -> (Entity<Root>, &'a mut VisualTestContext) {
        let drafts = Drafts::open(home).unwrap();
        let prefs = crate::prefs::Prefs::open(home).unwrap();
        let (root, vcx) = cx.add_window_view(move |window, cx| {
            Root::new(api, None, drafts, window, cx).with_prefs(prefs)
        });
        vcx.run_until_parked();
        (root, vcx)
    }

    fn composer(root: &Entity<Root>, vcx: &mut VisualTestContext) -> Entity<Composer> {
        root.read_with(vcx, |root, _| root.composer.clone())
    }

    fn text(composer: &Entity<Composer>, vcx: &mut VisualTestContext) -> String {
        composer.read_with(vcx, |c, cx| c.text(cx))
    }

    fn select(root: &Entity<Root>, id: &str, vcx: &mut VisualTestContext) {
        root.update_in(vcx, |root, window, cx| root.select(id.into(), window, cx));
        vcx.run_until_parked();
    }

    #[gpui::test]
    fn sidebar_lists_conversations_from_the_client_and_opens_the_newest(cx: &mut TestAppContext) {
        init(cx);
        let (tx, rx) = async_channel::unbounded();
        let _keep = tx;
        let (root, vcx) =
            cx.add_window_view(|window, cx| Root::new(fake(), Some(rx), drafts(), window, cx));
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| {
            assert_eq!(titles(root.active_rows()), ["fix the build"]);
            assert_eq!(titles(root.older_rows()), ["finished work"]);
            assert!(root.active_rows()[0].selected);
            assert_eq!(entry_count(root), 1);
        });
    }

    #[gpui::test]
    fn selecting_a_row_loads_its_thread(cx: &mut TestAppContext) {
        init(cx);
        let (root, vcx) =
            cx.add_window_view(|window, cx| Root::new(fake(), None, drafts(), window, cx));
        vcx.run_until_parked();
        select(&root, "b", vcx);
        root.read_with(vcx, |root, _| {
            assert!(root.older_rows()[0].selected);
            match &root.state().detail {
                Load::Ready(d) => assert_eq!(d.conversation.id, "b"),
                other => panic!("{other:?}"),
            }
        });
    }

    #[gpui::test]
    fn events_update_the_views_without_refetching(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let (tx, rx) = async_channel::unbounded();
        let (root, vcx) =
            cx.add_window_view(|window, cx| Root::new(api.clone(), Some(rx), drafts(), window, cx));
        vcx.run_until_parked();

        api.list.lock().unwrap().clear();
        tx.send_blocking(ServerEvent::EntryAppended {
            conversation_id: "a".into(),
            entry: entry(2, "second"),
        })
        .unwrap();
        tx.send_blocking(ServerEvent::ConversationUpdated {
            conversation: conversation("c", "brand new", "2026-11-12T11:00:00.000Z"),
        })
        .unwrap();
        vcx.run_until_parked();

        root.read_with(vcx, |root, _| {
            assert_eq!(entry_count(root), 2);
            assert_eq!(titles(root.active_rows()), ["brand new", "fix the build"]);
        });
    }

    #[gpui::test]
    fn resync_refetches_the_list_and_the_open_thread(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let (tx, rx) = async_channel::unbounded();
        let (root, vcx) =
            cx.add_window_view(|window, cx| Root::new(api.clone(), Some(rx), drafts(), window, cx));
        vcx.run_until_parked();

        api.list.lock().unwrap().push(conversation(
            "z",
            "appeared while offline",
            "2026-11-12T12:00:00.000Z",
        ));
        api.details.lock().unwrap()[0]
            .entries
            .push(entry(2, "missed"));
        tx.send_blocking(ServerEvent::Resync).unwrap();
        vcx.run_until_parked();

        root.read_with(vcx, |root, _| {
            assert_eq!(
                titles(root.active_rows()),
                ["appeared while offline", "fix the build"]
            );
            assert_eq!(entry_count(root), 2);
        });
    }

    #[gpui::test]
    fn a_failing_client_shows_an_error_instead_of_rows(cx: &mut TestAppContext) {
        init(cx);
        struct Down;
        impl Api for Down {
            fn conversations(&self) -> Result<Vec<Conversation>, String> {
                Err("connection refused".into())
            }
            fn conversation(&self, _: &str) -> Result<ConversationDetail, String> {
                Err("connection refused".into())
            }
            fn send_message(&self, _: &str, _: &str) -> Result<ThreadEntry, String> {
                Err("connection refused".into())
            }
            fn tasks(&self) -> Result<Vec<townhall_core::model::Task>, String> {
                Err("connection refused".into())
            }
            fn repos(&self) -> Result<Vec<townhall_core::model::Repo>, String> {
                Err("connection refused".into())
            }
            fn info(&self) -> Result<crate::client::Info, String> {
                Err("connection refused".into())
            }
            fn search(&self, _: &str) -> Result<Vec<townhall_core::search::SearchHit>, String> {
                Err("connection refused".into())
            }
            fn end_conversation(&self, _: &str) -> Result<Conversation, String> {
                Err("connection refused".into())
            }
            fn link_repo(&self, _: &str, _: Option<&str>) -> Result<Conversation, String> {
                Err("connection refused".into())
            }
            fn start_conversation(&self, _: &str) -> Result<ConversationDetail, String> {
                Err("connection refused".into())
            }
            fn add_repo(&self, _: &str) -> Result<townhall_core::model::Repo, String> {
                Err("connection refused".into())
            }
        }
        let (root, vcx) =
            cx.add_window_view(|window, cx| Root::new(Arc::new(Down), None, drafts(), window, cx));
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| {
            assert_eq!(
                root.state().conversations,
                Load::Failed("connection refused".into())
            );
            assert!(root.active_rows().is_empty());
        });
    }

    fn fresh_home() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[gpui::test]
    fn enter_sends_the_trimmed_text_and_clears_the_composer(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        let composer = composer(&root, vcx);
        vcx.simulate_input("  fix the build  ");
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert_eq!(
            *api.sent.lock().unwrap(),
            [("a".to_string(), "fix the build".to_string())]
        );
        assert_eq!(text(&composer, vcx), "");
        assert_eq!(
            composer.read_with(vcx, |c, _| c.error().map(String::from)),
            None
        );
        assert_eq!(Drafts::open(home.path()).unwrap().get("a"), "");
    }

    #[gpui::test]
    fn shift_enter_adds_a_line_and_does_not_send(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        let composer = composer(&root, vcx);
        vcx.simulate_input("one");
        vcx.simulate_keystrokes("shift-enter");
        vcx.simulate_input("two");
        vcx.run_until_parked();
        assert_eq!(text(&composer, vcx), "one\ntwo");
        assert!(api.sent.lock().unwrap().is_empty());
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert_eq!(api.sent.lock().unwrap()[0].1, "one\ntwo");
    }

    #[gpui::test]
    fn an_empty_or_blank_message_is_not_sent(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        let composer = composer(&root, vcx);
        assert!(!composer.read_with(vcx, |c, cx| c.can_send(cx)));
        vcx.simulate_keystrokes("enter");
        vcx.simulate_input("   ");
        assert!(!composer.read_with(vcx, |c, cx| c.can_send(cx)));
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert!(api.sent.lock().unwrap().is_empty());
        assert_eq!(text(&composer, vcx), "   ");
    }

    #[gpui::test]
    fn the_composer_is_in_the_sending_state_until_the_daemon_answers(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        let composer = composer(&root, vcx);
        vcx.simulate_input("go");
        composer.update_in(vcx, |c, window, cx| c.submit(window, cx));
        assert!(composer.read_with(vcx, |c, _| c.is_sending()));
        assert!(!composer.read_with(vcx, |c, cx| c.can_send(cx)));
        composer.update_in(vcx, |c, window, cx| c.submit(window, cx));
        vcx.run_until_parked();
        assert!(!composer.read_with(vcx, |c, _| c.is_sending()));
        assert_eq!(api.sent.lock().unwrap().len(), 1);
    }

    #[gpui::test]
    fn a_failed_send_shows_the_error_keeps_the_text_and_clears_on_edit(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        *api.send_error.lock().unwrap() = Some("daemon is down".into());
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        let composer = composer(&root, vcx);
        vcx.simulate_input("hello");
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert_eq!(
            composer.read_with(vcx, |c, _| c.error().map(String::from)),
            Some("daemon is down".to_string())
        );
        assert_eq!(text(&composer, vcx), "hello");
        assert!(!composer.read_with(vcx, |c, _| c.is_sending()));
        assert!(composer.read_with(vcx, |c, cx| c.can_send(cx)));
        vcx.simulate_input("!");
        vcx.run_until_parked();
        assert_eq!(
            composer.read_with(vcx, |c, _| c.error().map(String::from)),
            None
        );
    }

    #[gpui::test]
    fn nothing_is_sent_while_townhall_is_thinking(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        api.list.lock().unwrap()[0].status = ConversationStatus::Thinking;
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        let composer = composer(&root, vcx);
        vcx.simulate_input("wait");
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert!(api.sent.lock().unwrap().is_empty());
        assert!(!composer.read_with(vcx, |c, cx| c.can_send(cx)));
        assert_eq!(text(&composer, vcx), "wait");
    }

    #[gpui::test]
    fn a_draft_is_restored_when_switching_back_and_after_a_restart(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        {
            let (root, vcx) = open(cx, api.clone(), home.path());
            let composer = composer(&root, vcx);
            vcx.simulate_input("half a thought");
            select(&root, "b", vcx);
            assert_eq!(text(&composer, vcx), "");
            vcx.simulate_input("other");
            select(&root, "a", vcx);
            assert_eq!(text(&composer, vcx), "half a thought");
            select(&root, "b", vcx);
            assert_eq!(text(&composer, vcx), "other");
        }
        let (root, vcx) = open(cx, api, home.path());
        let composer = composer(&root, vcx);
        assert_eq!(text(&composer, vcx), "half a thought");
        select(&root, "b", vcx);
        assert_eq!(text(&composer, vcx), "other");
    }

    #[gpui::test]
    fn a_sent_draft_is_not_restored(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        let (root, vcx) = open(cx, api, home.path());
        let composer = composer(&root, vcx);
        vcx.simulate_input("sent");
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        select(&root, "b", vcx);
        select(&root, "a", vcx);
        assert_eq!(text(&composer, vcx), "");
    }

    #[gpui::test]
    fn the_composer_grows_with_its_text_up_to_eight_lines(cx: &mut TestAppContext) {
        init(cx);
        let home = fresh_home();
        let (root, vcx) = open(cx, fake(), home.path());
        let composer = composer(&root, vcx);
        let input = composer.read_with(vcx, |c, _| c.input().clone());
        let height = |vcx: &mut VisualTestContext| {
            vcx.run_until_parked();
            vcx.update(|window, _| window.refresh());
            vcx.run_until_parked();
            input.read_with(vcx, |i, _| f32::from(i.input_bounds().size.height))
        };
        let two = height(vcx);
        let lines = |n: usize| {
            (0..n)
                .map(|i| format!("line {i}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        input.update_in(vcx, |i, window, cx| i.set_value(lines(5), window, cx));
        let five = height(vcx);
        input.update_in(vcx, |i, window, cx| i.set_value(lines(8), window, cx));
        let eight = height(vcx);
        input.update_in(vcx, |i, window, cx| i.set_value(lines(30), window, cx));
        let thirty = height(vcx);
        assert!(two > 0.0, "{two}");
        assert!(five > two, "{five} {two}");
        assert!(eight > five, "{eight} {five}");
        assert_eq!(thirty, eight);
    }

    #[gpui::test]
    fn ime_composition_marks_text_and_commits_it(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        let composer = composer(&root, vcx);
        let input = composer.read_with(vcx, |c, _| c.input().clone());
        input.update_in(vcx, |i, window, cx| {
            i.replace_and_mark_text_in_range(None, "n", None, window, cx);
            i.replace_and_mark_text_in_range(None, "ni", None, window, cx);
        });
        assert_eq!(text(&composer, vcx), "ni");
        let marked = input.update_in(vcx, |i, window, cx| i.marked_text_range(window, cx));
        assert_eq!(marked, Some(0..2));
        input.update_in(vcx, |i, window, cx| {
            i.replace_text_in_range(None, "你", window, cx);
        });
        assert_eq!(text(&composer, vcx), "你");
        let marked = input.update_in(vcx, |i, window, cx| i.marked_text_range(window, cx));
        assert_eq!(marked, None);
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert_eq!(api.sent.lock().unwrap()[0].1, "你");
    }

    #[gpui::test]
    fn a_committed_composition_is_saved_as_the_draft(cx: &mut TestAppContext) {
        init(cx);
        let home = fresh_home();
        let (root, vcx) = open(cx, fake(), home.path());
        let composer = composer(&root, vcx);
        let input = composer.read_with(vcx, |c, _| c.input().clone());
        input.update_in(vcx, |i, window, cx| {
            i.replace_and_mark_text_in_range(None, "あ", None, window, cx);
            i.replace_text_in_range(None, "愛", window, cx);
        });
        vcx.run_until_parked();
        assert_eq!(Drafts::open(home.path()).unwrap().get("a"), "愛");
    }

    #[gpui::test]
    fn pasted_text_lands_in_the_composer(cx: &mut TestAppContext) {
        init(cx);
        let home = fresh_home();
        let (root, vcx) = open(cx, fake(), home.path());
        let composer = composer(&root, vcx);
        vcx.write_to_clipboard(gpui::ClipboardItem::new_string("pasted\ntext".into()));
        vcx.simulate_keystrokes("cmd-v");
        vcx.run_until_parked();
        assert_eq!(text(&composer, vcx), "pasted\ntext");
        assert_eq!(Drafts::open(home.path()).unwrap().get("a"), "pasted\ntext");
    }

    use crate::repo_group::ConversationRepo;
    use crate::row_menu::MenuAction;
    use crate::sidebar_rows::SidebarRow;
    use std::time::Duration;
    use townhall_core::model::{Task, TaskState};
    use townhall_core::search::{SearchHit, SearchMatch};

    fn task_on(name: &str, conversation: &str, repo: &str, state: TaskState) -> Task {
        let mut t = crate::testkit::task(name, state);
        t.conversation_id = conversation.into();
        t.repo = repo.into();
        t
    }

    fn row_ids(rows: &[SidebarRow]) -> Vec<String> {
        rows.iter().map(|r| r.conversation.id.clone()).collect()
    }

    fn message_hit(id: &str) -> SearchHit {
        SearchHit {
            conversation_id: id.into(),
            title: "t".into(),
            matched: SearchMatch::Message,
            entry_id: Some(3),
            snippet: Some("…".into()),
            updated_at: "2026-11-12T10:00:00.000Z".into(),
        }
    }

    fn three_active() -> Arc<Fake> {
        let api = fake();
        api.list.lock().unwrap().push(conversation(
            "c",
            "flaky ci on main",
            "2026-11-12T09:00:00.000Z",
        ));
        api.details
            .lock()
            .unwrap()
            .push(detail("c", vec![entry(1, "looking")]));
        api
    }

    #[gpui::test]
    fn rows_carry_the_status_and_repo_of_their_own_tasks(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        *api.tasks.lock().unwrap() = vec![task_on("t1", "a", "shop", TaskState::NeedsYou)];
        let home = fresh_home();
        let (root, vcx) = open(cx, api, home.path());
        root.read_with(vcx, |root, _| {
            let rows = root.sections().active;
            assert_eq!(rows[0].summary.status, "t1 needs you");
            assert_eq!(rows[0].repo, ConversationRepo::Repo("shop".into()));
        });
    }

    #[gpui::test]
    fn a_task_event_updates_the_row_and_the_open_session(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        *api.tasks.lock().unwrap() = vec![task_on("t1", "a", "shop", TaskState::Queued)];
        let (tx, rx) = async_channel::unbounded();
        let (root, vcx) =
            cx.add_window_view(|window, cx| Root::new(api, Some(rx), drafts(), window, cx));
        vcx.run_until_parked();
        tx.send_blocking(ServerEvent::TaskUpdated {
            task: task_on("t1", "a", "shop", TaskState::Working),
        })
        .unwrap();
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| {
            assert_eq!(
                root.sections().active[0].summary.status,
                "your turn · 1 running"
            );
        });
    }

    #[gpui::test]
    fn by_repo_groups_the_active_rows_folds_to_the_current_one_and_is_remembered(
        cx: &mut TestAppContext,
    ) {
        init(cx);
        let api = three_active();
        *api.tasks.lock().unwrap() = vec![
            task_on("t1", "a", "shop", TaskState::Working),
            task_on("t2", "c", "shop", TaskState::Working),
        ];
        let home = fresh_home();
        let (root, vcx) = open(cx, api, home.path());
        root.read_with(vcx, |root, _| assert!(root.blocks().is_none()));
        root.update(vcx, |root, cx| root.toggle_by_repo(cx));
        root.read_with(vcx, |root, _| {
            let blocks = root.blocks().unwrap();
            assert_eq!(blocks.len(), 1);
            assert_eq!((blocks[0].label.as_str(), blocks[0].count), ("shop", 2));
            assert_eq!(row_ids(&blocks[0].rows), ["a", "c"]);
        });
        root.update(vcx, |root, cx| root.toggle_group("repo:shop", cx));
        root.read_with(vcx, |root, _| {
            let blocks = root.blocks().unwrap();
            assert!(!blocks[0].open);
            assert_eq!(row_ids(&blocks[0].rows), ["a"]);
        });
        assert!(crate::prefs::Prefs::open(home.path()).unwrap().by_repo());
        root.update(vcx, |root, cx| root.toggle_by_repo(cx));
        assert!(!crate::prefs::Prefs::open(home.path()).unwrap().by_repo());
    }

    #[gpui::test]
    fn typing_in_the_search_filters_titles_at_once_and_adds_message_matches_later(
        cx: &mut TestAppContext,
    ) {
        init(cx);
        let api = three_active();
        *api.hits.lock().unwrap() = vec![message_hit("c")];
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        root.update_in(vcx, |root, window, cx| root.focus_search(window, cx));
        vcx.simulate_input("build");
        root.read_with(vcx, |root, cx| {
            assert_eq!(root.query(cx), "build");
            assert_eq!(row_ids(&root.sections().active), ["a"]);
        });
        assert!(api.searched.lock().unwrap().is_empty());
        vcx.executor().advance_clock(Duration::from_millis(200));
        vcx.run_until_parked();
        assert_eq!(*api.searched.lock().unwrap(), ["build"]);
        root.read_with(vcx, |root, _| {
            assert_eq!(row_ids(&root.sections().active), ["a", "c"]);
        });
    }

    #[gpui::test]
    fn only_the_settled_search_text_is_sent_to_the_daemon(cx: &mut TestAppContext) {
        init(cx);
        let api = three_active();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        root.update_in(vcx, |root, window, cx| root.focus_search(window, cx));
        vcx.simulate_input("b");
        vcx.executor().advance_clock(Duration::from_millis(100));
        vcx.simulate_input("ui");
        vcx.executor().advance_clock(Duration::from_millis(100));
        vcx.run_until_parked();
        assert!(api.searched.lock().unwrap().is_empty());
        vcx.executor().advance_clock(Duration::from_millis(100));
        vcx.run_until_parked();
        assert_eq!(*api.searched.lock().unwrap(), ["bui"]);
    }

    #[gpui::test]
    fn clearing_the_search_brings_every_row_back(cx: &mut TestAppContext) {
        init(cx);
        let home = fresh_home();
        let (root, vcx) = open(cx, three_active(), home.path());
        root.update_in(vcx, |root, window, cx| root.focus_search(window, cx));
        vcx.simulate_input("zzz");
        root.read_with(vcx, |root, _| assert!(root.sections().active.is_empty()));
        root.update_in(vcx, |root, window, cx| root.clear_search(window, cx));
        vcx.run_until_parked();
        root.read_with(vcx, |root, cx| {
            assert_eq!(root.query(cx), "");
            assert_eq!(root.sections().active.len(), 2);
        });
    }

    #[gpui::test]
    fn a_failed_message_search_keeps_the_title_matches_and_says_why(cx: &mut TestAppContext) {
        init(cx);
        let api = three_active();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        *api.down.lock().unwrap() = true;
        root.update_in(vcx, |root, window, cx| root.focus_search(window, cx));
        vcx.simulate_input("build");
        vcx.executor().advance_clock(Duration::from_millis(200));
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| {
            assert_eq!(root.match_error(), Some("connection refused"));
            assert_eq!(row_ids(&root.sections().active), ["a"]);
        });
    }

    #[gpui::test]
    fn the_row_menu_offers_what_the_web_menu_does(cx: &mut TestAppContext) {
        init(cx);
        let home = fresh_home();
        let (root, vcx) = open(cx, fake(), home.path());
        root.read_with(vcx, |root, _| {
            let labels: Vec<String> = root.menu_items("a").into_iter().map(|i| i.label).collect();
            assert_eq!(labels, ["end conversation", "repo: auto", "repo: shop"]);
            let labels: Vec<String> = root.menu_items("b").into_iter().map(|i| i.label).collect();
            assert_eq!(labels, ["repo: auto", "repo: shop"]);
            assert!(root.menu_items("nope").is_empty());
        });
    }

    #[gpui::test]
    fn ending_a_conversation_from_the_menu_moves_it_under_older(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        root.update(vcx, |root, cx| {
            root.run_menu(MenuAction::End("a".into()), cx)
        });
        vcx.run_until_parked();
        assert_eq!(*api.ended.lock().unwrap(), ["a"]);
        root.read_with(vcx, |root, _| {
            assert!(root.active_rows().is_empty());
            assert_eq!(
                titles(root.older_rows()),
                ["fix the build", "finished work"]
            );
            assert_eq!(root.action_error(), None);
        });
    }

    #[gpui::test]
    fn linking_a_repo_from_the_menu_sets_it_and_auto_clears_it(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        root.update(vcx, |root, cx| {
            root.run_menu(MenuAction::Link("a".into(), Some("shop".into())), cx)
        });
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| {
            assert_eq!(
                root.sections().active[0].repo,
                ConversationRepo::Repo("shop".into())
            );
        });
        root.update(vcx, |root, cx| {
            root.run_menu(MenuAction::Link("a".into(), None), cx)
        });
        vcx.run_until_parked();
        assert_eq!(
            *api.linked.lock().unwrap(),
            [
                ("a".to_string(), Some("shop".to_string())),
                ("a".to_string(), None)
            ]
        );
        root.read_with(vcx, |root, _| {
            assert_eq!(root.sections().active[0].repo, ConversationRepo::None);
        });
    }

    #[gpui::test]
    fn a_failed_menu_action_shows_its_error_until_the_next_one_works(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        *api.action_error.lock().unwrap() = Some("boom".into());
        root.update(vcx, |root, cx| {
            root.run_menu(MenuAction::End("a".into()), cx)
        });
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| assert_eq!(root.action_error(), Some("boom")));
        *api.action_error.lock().unwrap() = None;
        root.update(vcx, |root, cx| {
            root.run_menu(MenuAction::End("a".into()), cx)
        });
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| assert_eq!(root.action_error(), None));
    }

    #[gpui::test]
    fn the_app_opens_the_newest_conversation_and_home_is_one_click_away(cx: &mut TestAppContext) {
        init(cx);
        let home = fresh_home();
        let (root, vcx) = open(cx, fake(), home.path());
        root.read_with(vcx, |root, _| {
            assert_eq!(root.screen(), Screen::Conversation)
        });
        root.update_in(vcx, |root, window, cx| root.go_home(window, cx));
        root.read_with(vcx, |root, _| {
            assert_eq!(root.screen(), Screen::Home);
            assert!(root.active_rows().iter().all(|r| !r.selected));
        });
        select(&root, "b", vcx);
        root.read_with(vcx, |root, _| {
            assert_eq!(root.screen(), Screen::Conversation)
        });
    }

    #[gpui::test]
    fn home_greets_by_first_name_and_suggests_work_on_known_repos(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        *api.user_name.lock().unwrap() = Some("Ada Lovelace".into());
        let home = fresh_home();
        let (root, vcx) = open(cx, api, home.path());
        root.read_with(vcx, |root, _| {
            assert_eq!(root.greeting_at(9), "good morning, Ada");
            assert_eq!(
                root.starters(),
                [
                    "fix the failing tests on shop develop",
                    "find dead code in shop",
                    "tidy the readme in shop"
                ]
            );
        });
    }

    #[gpui::test]
    fn picking_a_starter_fills_the_hero_composer(cx: &mut TestAppContext) {
        init(cx);
        let home = fresh_home();
        let (root, vcx) = open(cx, fake(), home.path());
        root.update_in(vcx, |root, window, cx| {
            root.go_home(window, cx);
            root.pick_starter("find dead code in shop", window, cx)
        });
        let start = root.read_with(vcx, |root, _| root.start_composer().clone());
        assert_eq!(text(&start, vcx), "find dead code in shop");
    }

    #[gpui::test]
    fn starting_from_home_starts_a_conversation_and_opens_it(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        root.update_in(vcx, |root, window, cx| {
            root.go_home(window, cx);
            root.pick_starter("tidy the readme in shop", window, cx)
        });
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert_eq!(*api.started.lock().unwrap(), ["tidy the readme in shop"]);
        root.read_with(vcx, |root, _| {
            assert_eq!(root.screen(), Screen::Conversation);
            assert_eq!(root.state().selected.as_deref(), Some("new"));
            assert!(titles(root.active_rows()).contains(&"tidy the readme in shop".to_string()));
        });
        let start = root.read_with(vcx, |root, _| root.start_composer().clone());
        assert_eq!(text(&start, vcx), "");
    }

    #[gpui::test]
    fn nothing_starts_from_blank_text_and_a_failed_start_keeps_the_text(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        root.update_in(vcx, |root, window, cx| root.go_home(window, cx));
        let start = root.read_with(vcx, |root, _| root.start_composer().clone());
        vcx.update(|window, cx| start.update(cx, |c, cx| c.focus(window, cx)));
        vcx.simulate_keystrokes("enter");
        vcx.simulate_input("  ");
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert!(api.started.lock().unwrap().is_empty());
        *api.action_error.lock().unwrap() = Some("no repo".into());
        vcx.simulate_input("go");
        vcx.simulate_keystrokes("enter");
        vcx.run_until_parked();
        assert_eq!(text(&start, vcx), "  go");
        assert_eq!(
            start.read_with(vcx, |c, _| c.error().map(String::from)),
            Some("no repo".to_string())
        );
        root.read_with(vcx, |root, _| assert_eq!(root.screen(), Screen::Home));
    }

    #[gpui::test]
    fn with_no_repos_and_no_conversations_the_first_run_view_shows_until_a_repo_is_added(
        cx: &mut TestAppContext,
    ) {
        init(cx);
        let api = Arc::new(Fake::default());
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        root.read_with(vcx, |root, _| assert_eq!(root.screen(), Screen::FirstRun));
        root.update_in(vcx, |root, window, cx| {
            root.add_repo("/work/shop".into(), window, cx)
        });
        vcx.run_until_parked();
        assert_eq!(*api.added.lock().unwrap(), ["/work/shop"]);
        root.read_with(vcx, |root, _| {
            assert_eq!(root.screen(), Screen::Home);
            assert_eq!(root.action_error(), None);
        });
    }

    #[gpui::test]
    fn a_failed_add_repo_stays_on_first_run_and_says_why(cx: &mut TestAppContext) {
        init(cx);
        let api = Arc::new(Fake::default());
        *api.action_error.lock().unwrap() = Some("not a git repo".into());
        let home = fresh_home();
        let (root, vcx) = open(cx, api, home.path());
        root.update_in(vcx, |root, window, cx| {
            root.add_repo("/work/x".into(), window, cx)
        });
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| {
            assert_eq!(root.screen(), Screen::FirstRun);
            assert_eq!(root.action_error(), Some("not a git repo"));
        });
    }

    #[gpui::test]
    fn an_unreachable_daemon_shows_a_banner_and_retry_clears_it(cx: &mut TestAppContext) {
        init(cx);
        let api = fake();
        *api.down.lock().unwrap() = true;
        let home = fresh_home();
        let (root, vcx) = open(cx, api.clone(), home.path());
        root.read_with(vcx, |root, _| {
            assert_eq!(
                root.banner().as_deref(),
                Some("cannot reach the daemon: connection refused")
            );
        });
        *api.down.lock().unwrap() = false;
        root.update_in(vcx, |root, window, cx| root.retry(window, cx));
        vcx.run_until_parked();
        root.read_with(vcx, |root, _| {
            assert_eq!(root.banner(), None);
            assert_eq!(titles(root.active_rows()), ["fix the build"]);
        });
    }

    #[gpui::test]
    fn ages_tick_once_a_minute_and_nothing_else_redraws_while_idle(cx: &mut TestAppContext) {
        init(cx);
        cx.update(|cx| theme::apply_to_components(&Theme::new(theme::Mode::Dark), cx));
        let home = fresh_home();
        let (root, vcx) = open(cx, fake(), home.path());
        vcx.executor().advance_clock(Duration::from_secs(5));
        vcx.run_until_parked();
        let before = root.read_with(vcx, |root, _| root.renders());
        vcx.executor().advance_clock(Duration::from_secs(54));
        vcx.run_until_parked();
        assert_eq!(root.read_with(vcx, |root, _| root.renders()), before);
        vcx.executor().advance_clock(Duration::from_secs(1));
        vcx.run_until_parked();
        assert_eq!(root.read_with(vcx, |root, _| root.renders()), before + 1);
    }
}
