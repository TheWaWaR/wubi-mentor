//! 练习页：稿纸式文字格，输入法上屏后逐字比对；右侧拆字卡，底部键盘。

use std::{collections::HashMap, ops::Range, rc::Rc, time::Duration};

use gpui_kit::component::{
    ActiveTheme, Disableable as _, Selectable as _, Sizable as _,
    button::{Button, ButtonGroup, ButtonVariants as _},
    h_flex,
    progress::Progress,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::db::{CharStat, HintMode, Mastery, Text, now_ms};
use crate::practice::{Mark, Outcome, Practice};
use crate::store::Store;
use crate::ui::decomp::{big_char, char_card, mastery_color, section_label};
use crate::ui::keyboard::Keyboard;

actions!(practice, [PeekHint, SkipChar]);

pub const CONTEXT: &str = "Practice";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", PeekHint, Some(CONTEXT)),
        KeyBinding::new("cmd-j", SkipChar, Some(CONTEXT)),
    ]);
}

/// 一页大约多少个字；会延伸到下一个句末，避免把句子拆开。
const PAGE_CHARS: usize = 240;
/// 选了“出错再提示”时，停顿这么久也自动显示拆分。
const IDLE_HINT_MS: i64 = 6_000;
/// 停顿这么久就结束本次练习记录。
const SESSION_IDLE_MS: i64 = 90_000;

pub enum PracticeEvent {
    OpenLibrary,
}

impl EventEmitter<PracticeEvent> for PracticeView {}

pub struct PracticeView {
    store: Rc<Store>,
    focus_handle: FocusHandle,
    text: Option<Text>,
    practice: Option<Practice>,
    pages: Vec<Range<usize>>,
    /// 手动翻到的页；`None` 时跟随当前字。
    browse_page: Option<usize>,
    /// 输入法正在组字的编码（未上屏）。
    marked: Option<String>,
    /// 点选查看的字（下标）。
    inspect: Option<usize>,
    /// 当前字的拆分已经被看过（提示模式不是“始终”时，会记为未掌握）。
    peeked: bool,
    hint_mode: HintMode,
    notice: Option<SharedString>,
    stats: HashMap<char, CharStat>,
    recent_misses: Vec<char>,
    finished_summary: Option<crate::practice::SessionTally>,
    cursor_bounds: Option<Bounds<Pixels>>,
    paper_scroll: ScrollHandle,
    _ticker: Task<()>,
}

impl PracticeView {
    pub fn new(store: Rc<Store>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hint_mode = store.db.hint_mode().unwrap_or(HintMode::Always);
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |this, cx| this.on_tick(cx)).is_err() {
                    break;
                }
            }
        });
        let stats = store
            .db
            .char_stats()
            .unwrap_or_default()
            .into_iter()
            .map(|s| (s.ch, s))
            .collect();
        let mut view = Self {
            store,
            focus_handle: cx.focus_handle(),
            text: None,
            practice: None,
            pages: vec![],
            browse_page: None,
            marked: None,
            inspect: None,
            peeked: false,
            hint_mode,
            notice: None,
            stats,
            recent_misses: vec![],
            finished_summary: None,
            cursor_bounds: None,
            paper_scroll: ScrollHandle::new(),
            _ticker: ticker,
        };
        if let Ok(Some(id)) = view.store.db.current_text_id() {
            view.load_text(id, cx);
        }
        view
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus_handle, cx);
    }

    pub fn text_id(&self) -> Option<i64> {
        self.text.as_ref().map(|t| t.id)
    }

    pub fn load_text(&mut self, id: i64, cx: &mut Context<Self>) {
        self.end_session();
        let Ok(Some(text)) = self.store.db.text(id) else {
            return;
        };
        let store = self.store.clone();
        let practice = Practice::new(&text.content, text.position, |c| store.contains(c));
        self.pages = paginate(practice.chars());
        self.practice = Some(practice);
        self.text = Some(text);
        self.browse_page = None;
        self.inspect = None;
        self.peeked = false;
        self.marked = None;
        self.notice = None;
        self.recent_misses.clear();
        self.finished_summary = None;
        let _ = self.store.db.set_current_text_id(id);
        self.scroll_to_current();
        cx.notify();
    }

    /// 切换方案：存下这次练习，按新方案重新读入统计、当前文本和进度（要打的字也可能不同）。
    pub fn scheme_changed(&mut self, cx: &mut Context<Self>) {
        self.end_session();
        self.stats = self
            .store
            .db
            .char_stats()
            .unwrap_or_default()
            .into_iter()
            .map(|s| (s.ch, s))
            .collect();
        // 每个方案有自己的当前文本和进度。
        match self.store.db.current_text_id().ok().flatten().or(self.text_id()) {
            Some(id) => self.load_text(id, cx),
            None => cx.notify(),
        }
    }

    /// 文本被编辑或删除后，重新读取当前文本。
    pub fn reload_if_current(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.text_id() != Some(id) {
            return;
        }
        match self.store.db.text(id) {
            Ok(Some(_)) => self.load_text(id, cx),
            _ => {
                self.end_session();
                self.text = None;
                self.practice = None;
                cx.notify();
            }
        }
    }

    pub fn end_session(&mut self) -> Option<crate::practice::SessionTally> {
        let text_id = self.text_id();
        let tally = self.practice.as_mut()?.take_session()?;
        let _ = self.store.db.insert_session(
            text_id,
            tally.started_at.unwrap_or(tally.last_at),
            tally.last_at,
            tally.active_ms,
            tally.chars,
            tally.missed,
        );
        Some(tally)
    }

    fn on_tick(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.practice.as_ref() else {
            return;
        };
        let Some(idle) = p.idle_ms(now_ms()) else {
            return;
        };
        if idle > SESSION_IDLE_MS {
            self.end_session();
            cx.notify();
        } else if self.hint_mode == HintMode::OnMiss && idle > IDLE_HINT_MS && !self.peeked {
            self.peeked = true;
            cx.notify();
        } else if p.session().chars > 0 {
            // 刷新速度显示。
            cx.notify();
        }
    }

    fn set_hint_mode(&mut self, mode: HintMode, cx: &mut Context<Self>) {
        self.hint_mode = mode;
        let _ = self.store.db.set_hint_mode(mode);
        cx.notify();
    }

    fn hint_visible(&self) -> bool {
        let Some(p) = self.practice.as_ref() else {
            return false;
        };
        match self.hint_mode {
            HintMode::Always => true,
            HintMode::OnMiss => self.peeked || p.errors_here() > 0,
            HintMode::Never => self.peeked,
        }
    }

    fn peek(&mut self, _: &PeekHint, _: &mut Window, cx: &mut Context<Self>) {
        if self.inspect.take().is_none() {
            self.peeked = true;
        }
        cx.notify();
    }

    fn skip(&mut self, _: &SkipChar, _: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.practice.as_mut() else {
            return;
        };
        if let Some(ch) = p.skip() {
            let _ = self.store.db.record_char(ch, 0, 1);
            self.refresh_stat(ch);
            self.push_miss(ch);
        }
        self.after_move(cx);
    }

    fn after_move(&mut self, cx: &mut Context<Self>) {
        self.peeked = false;
        self.inspect = None;
        self.browse_page = None;
        self.scroll_to_current();
        if let (Some(p), Some(text)) = (self.practice.as_ref(), self.text.as_ref()) {
            let _ = self.store.db.set_position(text.id, p.position());
            if p.is_finished() && self.finished_summary.is_none() {
                self.finished_summary = self.end_session().or(Some(Default::default()));
            }
        }
        cx.notify();
    }

    /// 让当前字保持在可视区域内。
    fn scroll_to_current(&self) {
        let Some(p) = self.practice.as_ref() else {
            return;
        };
        if let Some(page) = self.pages.iter().find(|r| r.contains(&p.position())) {
            self.paper_scroll.scroll_to_item(p.position() - page.start);
        }
    }

    fn refresh_stat(&mut self, ch: char) {
        if let Ok(stat) = self.store.db.char_stat(ch) {
            self.stats.insert(ch, stat);
        }
    }

    fn push_miss(&mut self, ch: char) {
        self.recent_misses.retain(|c| *c != ch);
        self.recent_misses.insert(0, ch);
        self.recent_misses.truncate(16);
    }

    fn commit(&mut self, text: &str, cx: &mut Context<Self>) {
        let Some(p) = self.practice.as_mut() else {
            return;
        };
        if p.is_finished() {
            return;
        }
        if !text.is_empty() && text.chars().all(|c| c.is_ascii_alphabetic()) {
            self.notice = Some(
                format!(
                    "打出来的是英文字母，请先切换到系统的{}输入法。",
                    self.store.scheme().ime()
                )
                .into(),
            );
            cx.notify();
            return;
        }
        self.notice = None;
        let peeked = self.peeked && self.hint_mode != HintMode::Always;
        let mut moved = false;
        let mut first = true;
        for outcome in p.input(text, now_ms()) {
            match outcome {
                Outcome::Correct { ch, ms, errors, .. } => {
                    // 看过提示只算在第一个字上。
                    let errors = errors + u32::from(first && peeked);
                    first = false;
                    let _ = self.store.db.record_char(ch, ms, errors);
                    self.refresh_stat(ch);
                    if errors > 0 {
                        self.push_miss(ch);
                    }
                    moved = true;
                }
                Outcome::Wrong { expected, .. } => {
                    self.push_miss(expected);
                }
                Outcome::Finished => {}
            }
        }
        if moved {
            self.after_move(cx);
        } else {
            cx.notify();
        }
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.text_id() {
            let _ = self.store.db.set_position(id, 0);
            self.load_text(id, cx);
        }
    }

    fn current_page(&self) -> usize {
        if let Some(page) = self.browse_page {
            return page.min(self.pages.len().saturating_sub(1));
        }
        let pos = self.practice.as_ref().map(|p| p.position()).unwrap_or(0);
        self.pages
            .iter()
            .position(|r| r.contains(&pos))
            .unwrap_or(self.pages.len().saturating_sub(1))
    }
}

fn paginate(chars: &[char]) -> Vec<Range<usize>> {
    let mut pages = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let mut end = (start + PAGE_CHARS).min(chars.len());
        if end < chars.len() {
            let limit = (end + 80).min(chars.len());
            if let Some(off) = chars[end..limit]
                .iter()
                .position(|c| "\n。！？；…".contains(*c))
            {
                end += off + 1;
            }
        }
        pages.push(start..end);
        start = end;
    }
    if pages.is_empty() {
        pages.push(0..0);
    }
    pages
}

impl EntityInputHandler for PracticeView {
    fn text_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let marked = self.marked.clone().unwrap_or_default();
        actual_range.replace(0..marked.encode_utf16().count());
        Some(marked)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let len = self
            .marked
            .as_ref()
            .map(|m| m.encode_utf16().count())
            .unwrap_or(0);
        Some(UTF16Selection {
            range: len..len,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|m| 0..m.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = None;
        self.commit(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = (!new_text.is_empty()).then(|| new_text.to_string());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(self.cursor_bounds.unwrap_or(element_bounds))
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl Focusable for PracticeView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(feature = "snapshot")]
impl PracticeView {
    pub fn debug_commit(&mut self, text: &str, cx: &mut Context<Self>) {
        self.marked = None;
        self.commit(text, cx);
    }

    pub fn debug_mark(&mut self, text: &str, cx: &mut Context<Self>) {
        self.marked = (!text.is_empty()).then(|| text.to_string());
        cx.notify();
    }

    pub fn debug_hint(&mut self, mode: &str, cx: &mut Context<Self>) {
        self.set_hint_mode(HintMode::parse(mode), cx);
    }

    pub fn debug_inspect(&mut self, index: usize, cx: &mut Context<Self>) {
        self.inspect = Some(index);
        cx.notify();
    }
}

// ---- rendering ----

impl PracticeView {
    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (Some(text), Some(p)) = (self.text.as_ref(), self.practice.as_ref()) else {
            return div().into_any_element();
        };
        let done = p.targets_done();
        let total = p.target_count().max(1);
        let session = p.session();
        let stat = |label: &'static str, value: String| {
            v_flex()
                .items_end()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(value),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
        };
        h_flex()
            .gap_6()
            .items_center()
            .px_6()
            .py_3()
            .border_b_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_baseline()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(text.title.clone()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{done} / {} 字", p.target_count())),
                            ),
                    )
                    .child(Progress::new("text-progress").value(done as f32 * 100. / total as f32)),
            )
            .child(stat("字/分钟", format!("{:.0}", session.speed())))
            .child(stat(
                "一次打对",
                format!("{:.0}%", session.accuracy() * 100.),
            ))
            .child(stat("本次字数", session.chars.to_string()))
            .child(
                ButtonGroup::new("hint-mode")
                    .small()
                    .outline()
                    .children(HintMode::ALL.map(|mode| {
                        Button::new(mode.as_str())
                            .label(mode.label())
                            .selected(self.hint_mode == mode)
                    }))
                    .on_click(cx.listener(|this, selected: &Vec<usize>, _, cx| {
                        if let Some(&i) = selected.first() {
                            this.set_hint_mode(HintMode::ALL[i], cx);
                        }
                    })),
            )
            .into_any_element()
    }

    fn render_cell(
        &self,
        index: usize,
        ch: char,
        p: &Practice,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        if ch == '\n' {
            return div().w_full().h(px(6.)).into_any_element();
        }
        if ch.is_whitespace() {
            return div().w(px(18.)).h(px(64.)).into_any_element();
        }
        if !p.is_target(index) {
            return div()
                .w(px(24.))
                .h(px(64.))
                .pt(px(8.))
                .flex()
                .justify_center()
                .text_size(px(26.))
                .text_color(theme.muted_foreground.opacity(0.7))
                .child(ch.to_string())
                .into_any_element();
        }

        let is_current = index == p.position();
        let is_inspected = self.inspect == Some(index);
        let mark = p.mark(index);
        let mastery = self
            .stats
            .get(&ch)
            .map(|s| s.mastery())
            .unwrap_or(Mastery::New);
        let done = index < p.position();

        let text_color = match mark {
            Some(Mark::Clean) => theme.foreground.opacity(0.38),
            Some(Mark::Fixed) => theme.warning,
            Some(Mark::Skipped) => theme.danger,
            None => theme.foreground,
        };
        let entity = cx.entity();
        let mut cell = v_flex()
            .id(("cell", index))
            .relative()
            .w(px(44.))
            .h(px(64.))
            .items_center()
            .pt(px(8.))
            .rounded(px(8.))
            .cursor_pointer()
            .text_size(px(26.))
            .line_height(px(34.))
            .text_color(text_color)
            .hover(|s| s.bg(theme.secondary))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.inspect = if this.inspect == Some(index) {
                    None
                } else {
                    Some(index)
                };
                this.focus(window, cx);
                cx.notify();
            }))
            .child(ch.to_string());

        if is_current {
            cell = cell
                .bg(theme.blue.opacity(0.12))
                .border_b_2()
                .border_color(theme.blue)
                .child(
                    canvas(
                        move |bounds, _, cx| {
                            entity.update(cx, |this, _| this.cursor_bounds = Some(bounds));
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                );
            if let Some(wrong) = p.last_wrong() {
                cell = cell.child(
                    div()
                        .absolute()
                        .top(px(-4.))
                        .right(px(-4.))
                        .px_1()
                        .rounded(px(4.))
                        .bg(theme.danger)
                        .text_color(theme.danger_foreground)
                        .text_size(px(11.))
                        .line_height(px(15.))
                        .child(wrong.to_string()),
                );
            }
            if let Some(marked) = self.marked.as_ref() {
                cell = cell.child(
                    deferred(
                        div()
                            .absolute()
                            .top(px(58.))
                            .left(px(-8.))
                            .px_1p5()
                            .rounded(px(5.))
                            .bg(theme.foreground)
                            .text_color(theme.background)
                            .font_family("Menlo")
                            .whitespace_nowrap()
                            .text_size(px(14.))
                            .line_height(px(20.))
                            .child(marked.to_uppercase()),
                    )
                    .with_priority(1),
                );
            }
        } else if is_inspected {
            cell = cell.border_1().border_color(theme.ring).bg(theme.secondary);
        }

        // 还没打的薄弱字，右上角一个小红点提醒。
        if !done && !is_current && mastery == Mastery::Weak {
            cell = cell.child(
                div()
                    .absolute()
                    .top(px(6.))
                    .right(px(4.))
                    .size(px(5.))
                    .rounded_full()
                    .bg(mastery_color(mastery, cx)),
            );
        }
        if mark == Some(Mark::Fixed) || mark == Some(Mark::Skipped) {
            cell = cell.child(
                div()
                    .absolute()
                    .bottom(px(10.))
                    .left(px(12.))
                    .right(px(12.))
                    .h(px(2.))
                    .rounded_full()
                    .bg(text_color.opacity(0.6)),
            );
        }
        cell.into_any_element()
    }

    fn render_paper(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let entity = cx.entity();
        let focus = self.focus_handle.clone();
        let focused = self.focus_handle.is_focused(window);
        let Some(p) = self.practice.as_ref() else {
            return v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3()
                .child(div().text_lg().child("还没有选择练习文本"))
                .child(
                    Button::new("open-library")
                        .primary()
                        .label("去文本库选一篇")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(PracticeEvent::OpenLibrary))),
                )
                .into_any_element();
        };
        let page = self.current_page();
        let range = self.pages[page].clone();
        let chars = p.chars();
        let cells: Vec<AnyElement> = range
            .clone()
            .map(|i| self.render_cell(i, chars[i], p, cx))
            .collect();
        let page_count = self.pages.len();

        v_flex()
            .id("paper-wrap")
            .flex_1()
            .min_h_0()
            .relative()
            .child(
                div()
                    .id("paper")
                    .key_context(CONTEXT)
                    .track_focus(&self.focus_handle)
                    .on_action(cx.listener(Self::peek))
                    .on_action(cx.listener(Self::skip))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.paper_scroll)
                    .px_6()
                    .py_5()
                    .flex()
                    .flex_wrap()
                    .content_start()
                    .gap_x_0p5()
                    .gap_y_1()
                    // 文字格必须是滚动容器的直接子元素，`scroll_to_item` 才能定位当前字。
                    .children(cells)
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, cx| {
                                window.handle_input(
                                    &focus,
                                    ElementInputHandler::new(bounds, entity),
                                    cx,
                                );
                            },
                        )
                        .absolute()
                        .size_full(),
                    ),
            )
            .child(
                h_flex()
                    .px_6()
                    .py_2()
                    .gap_2()
                    .items_center()
                    .border_t_1()
                    .border_color(theme.border)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(if focused {
                        div().child(format!(
                            "用{}输入法直接打字 · Tab 看拆分 · ⌘J 跳过 · 点字查看",
                            self.store.scheme().ime()
                        ))
                    } else {
                        div()
                            .text_color(theme.warning)
                            .child("点一下文字区域开始打字")
                    })
                    .when_some(self.notice.clone(), |this, notice| {
                        this.child(div().text_color(theme.danger).child(notice))
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("prev-page")
                            .ghost()
                            .xsmall()
                            .icon(gpui_kit::component::IconName::ChevronLeft)
                            .disabled(page == 0)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.browse_page = Some(page.saturating_sub(1));
                                cx.notify();
                            })),
                    )
                    .child(format!("第 {} / {} 页", page + 1, page_count))
                    .child(
                        Button::new("next-page")
                            .ghost()
                            .xsmall()
                            .icon(gpui_kit::component::IconName::ChevronRight)
                            .disabled(page + 1 >= page_count)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.browse_page = Some(page + 1);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("skip")
                            .ghost()
                            .xsmall()
                            .label("跳过此字")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.skip(&SkipChar, window, cx)),
                            ),
                    )
                    .child(
                        Button::new("restart")
                            .ghost()
                            .xsmall()
                            .label("从头开始")
                            .on_click(cx.listener(|this, _, _, cx| this.restart(cx))),
                    ),
            )
            .when_some(self.finished_summary, |this, s| {
                this.child(
                    v_flex()
                        .absolute()
                        .inset_0()
                        .items_center()
                        .justify_center()
                        .bg(theme.background.opacity(0.92))
                        .gap_3()
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .child("这一篇练完了！"),
                        )
                        .when(s.chars > 0, |this| {
                            this.child(div().text_color(theme.muted_foreground).child(format!(
                                "本次 {} 字 · {:.0} 字/分钟 · 一次打对 {:.0}%",
                                s.chars,
                                s.speed(),
                                s.accuracy() * 100.
                            )))
                        })
                        .child(
                            h_flex()
                                .gap_2()
                                .child(Button::new("again").primary().label("再练一遍").on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.restart(cx);
                                        this.focus(window, cx);
                                    }),
                                ))
                                .child(Button::new("to-library").label("换一篇").on_click(
                                    cx.listener(|_, _, _, cx| cx.emit(PracticeEvent::OpenLibrary)),
                                )),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_side(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let p = self.practice.as_ref();
        let index = self
            .inspect
            .or_else(|| p.filter(|p| !p.is_finished()).map(|p| p.position()));
        let ch = index.and_then(|i| p.and_then(|p| p.chars().get(i).copied()));
        let entry = ch.and_then(|c| self.store.lookup(c));
        let show = self.inspect.is_some() || self.hint_visible();

        let body =
            match (entry, show) {
                (Some(entry), true) => {
                    let stat = self.stats.get(&entry.ch()).cloned().unwrap_or(CharStat {
                        ch: entry.ch(),
                        ..Default::default()
                    });
                    char_card(entry, Some(stat))
                }
                (Some(entry), false) => v_flex()
                    .gap_3()
                    .items_start()
                    .child(big_char(entry.ch(), 96., cx))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("先自己想想怎么拆。"),
                    )
                    .child(Button::new("peek").small().label("看拆分（Tab）").on_click(
                        cx.listener(|this, _, window, cx| this.peek(&PeekHint, window, cx)),
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("看过拆分的字这次会记为“未掌握”。"),
                    )
                    .into_any_element(),
                _ => div().into_any_element(),
            };

        v_flex()
            .id("side")
            .w(px(360.))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .border_l_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .p_5()
            .gap_5()
            .child(
                h_flex()
                    .justify_between()
                    .child(section_label(
                        if self.inspect.is_some() {
                            "查看"
                        } else {
                            "当前字"
                        },
                        cx,
                    ))
                    .when_some(self.inspect, |this, index| {
                        this.child(
                            h_flex()
                                .gap_1()
                                .when(
                                    p.is_some_and(|p| p.is_target(index) && index != p.position()),
                                    |this| {
                                        this.child(
                                            Button::new("start-here")
                                                .xsmall()
                                                .ghost()
                                                .label("从这里开始练")
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        if let Some(p) = this.practice.as_mut() {
                                                            p.jump_to(index);
                                                        }
                                                        this.after_move(cx);
                                                        this.focus(window, cx);
                                                    },
                                                )),
                                        )
                                    },
                                )
                                .child(
                                    Button::new("back-to-current")
                                        .xsmall()
                                        .ghost()
                                        .label("回到当前字")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.inspect = None;
                                            cx.notify();
                                        })),
                                ),
                        )
                    }),
            )
            .child(body)
            .when(!self.recent_misses.is_empty(), |this| {
                this.child(
                    v_flex()
                        .gap_2()
                        .child(section_label("这次打错的字", cx))
                        .child(h_flex().flex_wrap().gap_1().children(
                            self.recent_misses.iter().map(|&c| {
                                let target = p.and_then(|p| p.chars().iter().position(|x| *x == c));
                                div()
                                    .id(SharedString::from(format!("miss-{c}")))
                                    .size(px(34.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(theme.danger.opacity(0.4))
                                    .text_lg()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.secondary))
                                    .child(c.to_string())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.inspect = target;
                                        cx.notify();
                                    }))
                            }),
                        )),
                )
            })
    }
}

impl Render for PracticeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let p = self.practice.as_ref();
        let index = self
            .inspect
            .or_else(|| p.filter(|p| !p.is_finished()).map(|p| p.position()));
        let code = index
            .and_then(|i| p.and_then(|p| p.chars().get(i).copied()))
            .and_then(|c| self.store.lookup(c))
            .filter(|_| self.inspect.is_some() || self.hint_visible())
            .map(|e| e.code().to_string())
            .unwrap_or_default();

        v_flex()
            .size_full()
            .child(self.render_header(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(self.render_paper(window, cx))
                    .child(self.render_side(cx)),
            )
            .child(
                div()
                    .flex_none()
                    .py_4()
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(Keyboard::new(self.store.scheme()).highlight(&code)),
            )
    }
}
