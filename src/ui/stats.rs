//! 统计：累计数据、速度曲线、练习记录、薄弱字。

use std::rc::Rc;

use gpui_kit::component::{ActiveTheme, chart::LineChart, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::db::{CharStat, Mastery, Session};
use crate::store::Store;
use crate::ui::decomp::{mastery_color, section_label};
use crate::ui::library::civil_from_days;

pub enum StatsEvent {
    Lookup(String),
    Practice(i64),
}

impl EventEmitter<StatsEvent> for StatsView {}

pub struct StatsView {
    store: Rc<Store>,
    sessions: Vec<Session>,
    stats: Vec<CharStat>,
    weak: Vec<CharStat>,
}

#[derive(Clone)]
struct SpeedPoint {
    label: SharedString,
    speed: f64,
    accuracy: f64,
}

impl StatsView {
    pub fn new(store: Rc<Store>, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            store,
            sessions: vec![],
            stats: vec![],
            weak: vec![],
        };
        view.reload(cx);
        view
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.sessions = self.store.db.sessions(200).unwrap_or_default();
        self.stats = self.store.db.char_stats().unwrap_or_default();
        self.weak = self.store.db.weak_chars(60).unwrap_or_default();
        cx.notify();
    }
}

fn format_time(ms: i64) -> String {
    let secs = ms / 1000 + 8 * 3600;
    let (_, m, d) = civil_from_days(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!("{m:02}-{d:02} {:02}:{:02}", s / 3600, s % 3600 / 60)
}

fn format_duration(ms: i64) -> String {
    let mins = ms / 60_000;
    if mins >= 60 {
        format!("{} 小时 {} 分", mins / 60, mins % 60)
    } else {
        format!("{} 分 {} 秒", mins, ms / 1000 % 60)
    }
}

impl Render for StatsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let total_chars: i64 = self.sessions.iter().map(|s| s.chars).sum();
        let total_ms: i64 = self.sessions.iter().map(|s| s.active_ms).sum();
        let recent: Vec<&Session> = self.sessions.iter().take(10).collect();
        let recent_speed = {
            let chars: i64 = recent.iter().map(|s| s.chars).sum();
            let ms: i64 = recent.iter().map(|s| s.active_ms).sum();
            if ms > 0 {
                chars as f64 * 60_000. / ms as f64
            } else {
                0.
            }
        };
        let count = |m: Mastery| self.stats.iter().filter(|s| s.mastery() == m).count();

        let tile = |label: &'static str, value: String, hint: String| {
            v_flex()
                .flex_1()
                .p_4()
                .gap_1()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
                .child(div().text_2xl().font_weight(FontWeight::BOLD).child(value))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(hint),
                )
        };

        let points: Vec<SpeedPoint> = self
            .sessions
            .iter()
            .take(30)
            .rev()
            .map(|s| SpeedPoint {
                label: format_time(s.started_at)[..5].to_string().into(),
                speed: (s.speed() * 10.).round() / 10.,
                accuracy: (s.accuracy() * 1000.).round() / 10.,
            })
            .collect();

        let mastery_bar = {
            let total = self.stats.len().max(1) as f32;
            h_flex()
                .h(px(10.))
                .w_full()
                .rounded_full()
                .overflow_hidden()
                .bg(theme.secondary)
                .children(
                    [Mastery::Mastered, Mastery::Learning, Mastery::Weak].map(|m| {
                        div()
                            .h_full()
                            .w(relative(count(m) as f32 / total))
                            .bg(mastery_color(m, cx))
                    }),
                )
        };

        let header_row = |cols: [&'static str; 6]| {
            h_flex()
                .px_3()
                .py_2()
                .text_xs()
                .text_color(theme.muted_foreground)
                .children(cols.into_iter().enumerate().map(|(i, c)| col(i, c.into())))
        };

        let session_rows = self.sessions.iter().take(50).map(|s| {
            let text_id = s.text_id;
            h_flex()
                .id(("session", s.id as u64))
                .px_3()
                .py_2()
                .text_sm()
                .border_t_1()
                .border_color(theme.border)
                .when_some(text_id, |this, id| {
                    this.cursor_pointer()
                        .hover(|st| st.bg(theme.secondary))
                        .on_click(cx.listener(move |_, _, _, cx| cx.emit(StatsEvent::Practice(id))))
                })
                .child(col(0, format_time(s.started_at).into()))
                .child(col(
                    1,
                    s.text_title
                        .clone()
                        .unwrap_or_else(|| "（已删除）".into())
                        .into(),
                ))
                .child(col(2, s.chars.to_string().into()))
                .child(col(3, format!("{:.1}", s.speed()).into()))
                .child(col(4, format!("{:.0}%", s.accuracy() * 100.).into()))
                .child(col(5, format_duration(s.active_ms).into()))
        });

        v_flex()
            .id("stats")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .gap_6()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("统计"),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(tile(
                        "累计打字",
                        total_chars.to_string(),
                        format!("{} 次练习", self.sessions.len()),
                    ))
                    .child(tile(
                        "累计用时",
                        format_duration(total_ms),
                        "不计停顿".into(),
                    ))
                    .child(tile(
                        "最近 10 次速度",
                        format!("{recent_speed:.1}"),
                        "字 / 分钟".into(),
                    ))
                    .child(tile(
                        "已掌握",
                        format!("{} / {}", count(Mastery::Mastered), self.stats.len()),
                        format!(
                            "练习中 {} · 薄弱 {}",
                            count(Mastery::Learning),
                            count(Mastery::Weak)
                        ),
                    )),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(section_label(
                        "掌握程度（绿：已掌握 / 黄：练习中 / 红：薄弱）",
                        cx,
                    ))
                    .child(mastery_bar),
            )
            .when(points.len() >= 2, |this| {
                this.child(
                    h_flex()
                        .gap_4()
                        .child(
                            v_flex()
                                .flex_1()
                                .gap_2()
                                .child(section_label("速度（字/分钟，最近 30 次）", cx))
                                .child(
                                    div().h(px(200.)).child(
                                        LineChart::new(points.clone())
                                            .x(|p| p.label.clone())
                                            .y(|p| p.speed)
                                            .dot()
                                            .stroke(theme.chart_1),
                                    ),
                                ),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .gap_2()
                                .child(section_label("一次打对率（%）", cx))
                                .child(
                                    div().h(px(200.)).child(
                                        LineChart::new(points)
                                            .x(|p| p.label.clone())
                                            .y(|p| p.accuracy)
                                            .dot()
                                            .stroke(theme.chart_2),
                                    ),
                                ),
                        ),
                )
            })
            .child(
                v_flex()
                    .gap_2()
                    .child(section_label("薄弱字（点击查看拆分）", cx))
                    .map(|this| {
                        if self.weak.is_empty() {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("暂无薄弱字。"),
                            )
                        } else {
                            this.child(h_flex().flex_wrap().gap_1p5().children(
                                self.weak.iter().map(|s| {
                                    let ch = s.ch;
                                    v_flex()
                                        .id(SharedString::from(format!("weak-{ch}")))
                                        .w(px(52.))
                                        .py_1()
                                        .items_center()
                                        .rounded(px(8.))
                                        .border_1()
                                        .border_color(theme.danger.opacity(0.35))
                                        .cursor_pointer()
                                        .hover(|st| st.bg(theme.secondary))
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            cx.emit(StatsEvent::Lookup(ch.to_string()))
                                        }))
                                        .child(div().text_xl().child(ch.to_string()))
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(theme.muted_foreground)
                                                .child(format!("错 {}", s.errors)),
                                        )
                                }),
                            ))
                        }
                    }),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(section_label("练习记录（点击继续练这篇）", cx))
                    .child(
                        v_flex()
                            .rounded(px(10.))
                            .border_1()
                            .border_color(theme.border)
                            .child(header_row([
                                "时间",
                                "文本",
                                "字数",
                                "速度",
                                "一次打对",
                                "用时",
                            ]))
                            .children(session_rows)
                            .when(self.sessions.is_empty(), |this| {
                                this.child(
                                    div()
                                        .p_4()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("还没有练习记录。"),
                                )
                            }),
                    ),
            )
    }
}

fn col(i: usize, text: SharedString) -> impl IntoElement {
    let width = [110., 0., 60., 70., 80., 100.][i];
    div()
        .when(width == 0., |d| d.flex_1().min_w_0())
        .when(width > 0., |d| d.w(px(width)).flex_none())
        .truncate()
        .child(text)
}
