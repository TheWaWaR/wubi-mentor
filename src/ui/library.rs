//! 文本库：管理练习文本，新建、编辑、删除，生成错字重练。

use std::rc::Rc;

use gpui_kit::component::{
    ActiveTheme, Disableable as _, IconName, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    progress::Progress,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::db::{Text, now_ms};
use crate::store::Store;
use crate::ui::decomp::section_label;

pub enum LibraryEvent {
    Practice(i64),
    /// 文本内容被修改或删除。
    Changed(i64),
}

impl EventEmitter<LibraryEvent> for LibraryView {}

struct TextSummary {
    text: Text,
    targets: usize,
    done: usize,
}

pub struct LibraryView {
    store: Rc<Store>,
    texts: Vec<TextSummary>,
    /// 正在编辑的文本；`None` 表示新建。
    editing: Option<i64>,
    title: Entity<InputState>,
    content: Entity<TextareaState>,
    dirty: bool,
    confirm_delete: bool,
    /// 程序填充编辑框时不算修改。
    loading: bool,
    _subscriptions: Vec<Subscription>,
}

impl LibraryView {
    pub fn new(store: Rc<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("标题"));
        let content = cx.new(|cx| {
            TextareaState::new(window, cx).placeholder("把要练习的文字粘贴或输入到这里……")
        });
        let subscriptions = vec![
            cx.subscribe(&title, |this, _, event, cx| this.mark_dirty(event, cx)),
            cx.subscribe(&content, |this, _, event, cx| this.mark_dirty(event, cx)),
        ];
        let mut view = Self {
            store,
            texts: vec![],
            editing: None,
            title,
            content,
            dirty: false,
            confirm_delete: false,
            loading: false,
            _subscriptions: subscriptions,
        };
        view.reload(window, cx);
        view
    }

    #[cfg(feature = "snapshot")]
    pub fn debug_insert(
        &mut self,
        content: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> i64 {
        let id = self.store.db.insert_text("调试", content).expect("insert");
        self.reload(window, cx);
        id
    }

    #[cfg(feature = "snapshot")]
    pub fn debug_text_id(&self, title: &str) -> Option<i64> {
        self.texts
            .iter()
            .find(|t| t.text.title.contains(title))
            .map(|t| t.text.id)
    }

    fn mark_dirty(&mut self, event: &InputEvent, cx: &mut Context<Self>) {
        if matches!(event, InputEvent::Change) && !self.loading {
            self.dirty = true;
            cx.notify();
        }
    }

    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = &self.store;
        self.texts = self
            .store
            .db
            .texts()
            .unwrap_or_default()
            .into_iter()
            .map(|text| {
                let mut targets = 0;
                let mut done = 0;
                for (i, c) in text.content.chars().enumerate() {
                    if store.contains(c) {
                        targets += 1;
                        if i < text.position {
                            done += 1;
                        }
                    }
                }
                TextSummary {
                    text,
                    targets,
                    done,
                }
            })
            .collect();
        let still_exists = self
            .editing
            .is_some_and(|id| self.texts.iter().any(|t| t.text.id == id));
        if !self.dirty && !still_exists {
            let first = self.texts.first().map(|t| t.text.id);
            self.select(first, window, cx);
        }
        cx.notify();
    }

    fn select(&mut self, id: Option<i64>, window: &mut Window, cx: &mut Context<Self>) {
        let text = id
            .and_then(|id| self.texts.iter().find(|t| t.text.id == id))
            .map(|t| t.text.clone());
        self.editing = text.as_ref().map(|t| t.id);
        self.loading = true;
        let (title, content) = text.map(|t| (t.title, t.content)).unwrap_or_default();
        self.title
            .update(cx, |s, cx| s.set_value(title, window, cx));
        self.content
            .update(cx, |s, cx| s.set_value(content, window, cx));
        self.loading = false;
        self.dirty = false;
        self.confirm_delete = false;
        cx.notify();
    }

    fn new_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.select(None, window, cx);
        self.title.update(cx, |s, cx| s.focus(window, cx));
    }

    /// 保存编辑框内容，返回文本 id。
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<i64> {
        let title = self.title.read(cx).value().trim().to_string();
        let content = self.content.read(cx).value().to_string();
        if content.trim().is_empty() {
            window.push_notification("内容是空的，先写点要练习的文字。", cx);
            return None;
        }
        let title = if title.is_empty() {
            content.chars().take(12).collect::<String>()
        } else {
            title
        };
        let id = match self.editing {
            Some(id) => {
                self.store.db.update_text(id, &title, &content).ok()?;
                cx.emit(LibraryEvent::Changed(id));
                id
            }
            None => self.store.db.insert_text(&title, &content).ok()?,
        };
        self.editing = Some(id);
        self.dirty = false;
        self.reload(window, cx);
        Some(id)
    }

    fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.editing else { return };
        if !self.confirm_delete {
            self.confirm_delete = true;
            cx.notify();
            return;
        }
        let _ = self.store.db.delete_text(id);
        cx.emit(LibraryEvent::Changed(id));
        self.editing = None;
        self.dirty = false;
        self.reload(window, cx);
    }

    fn practice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = if self.dirty || self.editing.is_none() {
            self.save(window, cx)
        } else {
            self.editing
        };
        if let Some(id) = id {
            cx.emit(LibraryEvent::Practice(id));
        }
    }

    /// 用薄弱字生成一篇重练文本：每个字连打三遍。
    fn drill_weak(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = self.store.db.weak_chars(40).unwrap_or_default();
        if weak.is_empty() {
            window.push_notification("目前没有薄弱字，继续保持！", cx);
            return;
        }
        let content = weak
            .chunks(8)
            .map(|row| {
                row.iter()
                    .map(|s| s.ch.to_string().repeat(3))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let date = format_date(now_ms());
        if let Ok(id) = self
            .store
            .db
            .insert_text(&format!("错字重练 {date}"), &content)
        {
            self.reload(window, cx);
            self.select(Some(id), window, cx);
        }
    }

    fn render_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        v_flex()
            .id("text-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap_1p5()
            .p_3()
            .children(self.texts.iter().map(|t| {
                let id = t.text.id;
                let selected = self.editing == Some(id);
                let pct = if t.targets == 0 {
                    0.
                } else {
                    t.done as f32 * 100. / t.targets as f32
                };
                let preview: String = t
                    .text
                    .content
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .take(26)
                    .collect();
                v_flex()
                    .id(("text", id as u64))
                    .gap_1()
                    .p_3()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(if selected { theme.ring } else { theme.border })
                    .bg(if selected {
                        theme.secondary
                    } else {
                        theme.background
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.secondary))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.select(Some(id), window, cx)),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(t.text.title.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{} 字", t.targets)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(preview),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .child(Progress::new(("text-progress", id as u64)).value(pct)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .w(px(36.))
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{pct:.0}%")),
                            ),
                    )
            }))
    }
}

pub fn format_date(ms: i64) -> String {
    // 只需要本地的月-日，按 UTC+8 近似即可避免引入时区依赖。
    let days = (ms / 1000 + 8 * 3600).div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let _ = y;
    format!("{m:02}-{d:02}")
}

/// 把 1970-01-01 起的天数转成公历日期（Howard Hinnant 的算法）。
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

impl Render for LibraryView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let content = self.content.read(cx).value();
        let store = &self.store;
        let total = content.chars().filter(|c| !c.is_whitespace()).count();
        let targets = content.chars().filter(|c| store.contains(*c)).count();
        let editing_title = match (self.editing, self.dirty) {
            (None, _) => "新建文本",
            (Some(_), true) => "编辑中 · 未保存",
            (Some(_), false) => "编辑文本",
        };

        h_flex()
            .size_full()
            .items_stretch()
            .child(
                v_flex()
                    .w(px(320.))
                    .flex_none()
                    .border_r_1()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .child(
                        h_flex()
                            .px_4()
                            .py_3()
                            .gap_2()
                            .items_center()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(div().flex_1().text_lg().font_weight(FontWeight::SEMIBOLD).child("文本库"))
                            .child(
                                Button::new("drill")
                                    .small()
                                    .ghost()
                                    .label("错字重练")
                                    .tooltip("用最近的薄弱字生成一篇练习")
                                    .on_click(cx.listener(|this, _, window, cx| this.drill_weak(window, cx))),
                            )
                            .child(
                                Button::new("new")
                                    .small()
                                    .primary()
                                    .icon(IconName::Plus)
                                    .label("新建")
                                    .on_click(cx.listener(|this, _, window, cx| this.new_text(window, cx))),
                            ),
                    )
                    .child(self.render_list(cx)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .p_6()
                    .gap_3()
                    .child(section_label(editing_title, cx))
                    .child(Input::new(&self.title).large())
                    .child(Textarea::new(&self.content).flex_1().min_h_0().text_lg())
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("共 {total} 字，其中 {targets} 个汉字需要打（标点等自动跳过）")),
                            )
                            .when(self.editing.is_some(), |this| {
                                this.child(
                                    Button::new("delete")
                                        .ghost()
                                        .when(self.confirm_delete, |b| b.danger())
                                        .label(if self.confirm_delete { "确认删除？" } else { "删除" })
                                        .on_click(cx.listener(|this, _, window, cx| this.delete(window, cx))),
                                )
                            })
                            .child(
                                Button::new("save")
                                    .label("保存")
                                    .disabled(!self.dirty && self.editing.is_some())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("practice")
                                    .primary()
                                    .icon(IconName::Play)
                                    .label("开始练习")
                                    .disabled(targets == 0)
                                    .on_click(cx.listener(|this, _, window, cx| this.practice(window, cx))),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_739), (2026, 10, 13));
    }
}
