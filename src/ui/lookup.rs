//! 查字：输入任意文字，逐字显示拆分，并在字根键盘上标出用到的字根；没有输入时显示字根表。

use std::rc::Rc;

use gpui_kit::component::{
    ActiveTheme, IconName, h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::dict::Root;
use crate::practice::is_cjk;
use crate::scheme::Scheme;
use crate::store::{Lookup, Store};
use crate::ui::decomp::{char_card, section_label};
use crate::ui::keyboard::{Keyboard, zone_color};

const MAX_CHARS: usize = 40;

pub struct LookupView {
    store: Rc<Store>,
    input: Entity<InputState>,
    query: SharedString,
    _subscription: Subscription,
}

impl LookupView {
    pub fn new(store: Rc<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("输入要查的字或一段文字"));
        let subscription = cx.subscribe_in(&input, window, |this, state, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.query = state.read(cx).value();
                cx.notify();
            }
        });
        Self {
            store,
            input,
            query: "".into(),
            _subscription: subscription,
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.input.update(cx, |s, cx| s.focus(window, cx));
    }

    pub fn set_query(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input
            .update(cx, |s, cx| s.set_value(text.to_string(), window, cx));
        self.query = text.to_string().into();
        cx.notify();
    }
}

impl Render for LookupView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut seen = Vec::new();
        for c in self.query.chars().filter(|c| is_cjk(*c)) {
            if !seen.contains(&c) {
                seen.push(c);
            }
        }
        let truncated = seen.len() > MAX_CHARS;
        seen.truncate(MAX_CHARS);

        // 计入编码的字根（仓颉是只占一码的部件），在键盘上标出来。
        let scheme = self.store.scheme();
        let mut active_roots: Vec<Root> = Vec::new();
        let mut active_shapes: Vec<(char, char)> = Vec::new();
        let mut active_keys: Vec<char> = Vec::new();
        for lookup in seen.iter().filter_map(|&c| self.store.lookup(c)) {
            match lookup {
                Lookup::Wubi(entry) => active_roots.extend(
                    entry
                        .coded_root_indexes()
                        .into_iter()
                        .filter_map(|i| entry.roots.get(i).cloned()),
                ),
                Lookup::Cangjie(entry) => {
                    active_shapes.extend(entry.single_key_shapes());
                    active_keys.extend(entry.part_keys());
                }
            }
        }

        let cards = seen.iter().map(|&c| {
            let body = match self.store.lookup(c) {
                Some(entry) => {
                    let stat = self.store.db.char_stat(c).unwrap_or_default();
                    // 没练过的字不显示统计行，省点空间。
                    char_card(entry, (stat.seen > 0).then_some(stat))
                }
                None => div()
                    .text_color(theme.muted_foreground)
                    .child(format!("字典里没有「{c}」"))
                    .into_any_element(),
            };
            div()
                .id(SharedString::from(format!("lookup-{c}")))
                .w(px(380.))
                .p_4()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .child(body)
        });

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .px_6()
                    .py_3()
                    .gap_3()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("查字"),
                    )
                    .child(
                        div().flex_1().max_w(px(520.)).child(
                            Input::new(&self.input)
                                .prefix(IconName::Search)
                                .cleanable(true),
                        ),
                    ),
            )
            .child(
                div()
                    .id("lookup-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_6()
                    .map(|this| {
                        if seen.is_empty() {
                            this.child(root_chart(scheme, cx))
                        } else {
                            this.child(
                                v_flex()
                                    .gap_3()
                                    .child(
                                        h_flex().flex_wrap().gap_4().items_start().children(cards),
                                    )
                                    .when(truncated, |this| {
                                        this.child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.muted_foreground)
                                                .child(format!("只显示前 {MAX_CHARS} 个字")),
                                        )
                                    })
                                    .child(
                                        v_flex()
                                            .pt_4()
                                            .gap_3()
                                            .items_center()
                                            .child(
                                                Keyboard::new(scheme)
                                                    .large(true)
                                                    .active_roots(&active_roots)
                                                    .active_shapes(active_shapes)
                                                    .active_keys(active_keys),
                                            )
                                            .child(zone_legend(scheme, cx)),
                                    ),
                            )
                        }
                    }),
            )
    }
}

fn root_chart(scheme: Scheme, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let (title, subtitle, rules): (_, _, &[&str]) = match scheme {
        Scheme::Wubi86 => (
            "86 版五笔字根键盘",
            "每个键左上角是字母，右上角是键名字根，下面是这个键上的全部字根和变形（不含繁体）。",
            &[
                "键名字根（王土大木工……）：连击四下本键。",
                "成字字根：先“报户口”打所在键，再打首笔、次笔、末笔。",
                "合体字：按书写顺序拆成字根，取第一、二、三和最后一个字根。",
                "不足四个字根：在后面补“末笔识别码”，末笔笔画定区（横竖撇捺折 → 1~5 区），字型定位（左右 1、上下 2、杂合 3）。",
                "拆字原则：取大优先、兼顾直观、能散不连、能连不交。",
            ],
        ),
        Scheme::Cangjie5 => (
            "仓颉第五代字根键盘",
            "每个键左上角是字母，右上角是字根，下面是这个键上常见的辅助字形。",
            &[
                "24 个字根分四类：哲理类 日月金木水火土、笔画类 竹戈十大中一弓、人体类 人心手口、字形类 尸廿山女田卜；X 是难字键。",
                "组合字分成字首和字身：字首是左、上或外框部分，最多取首尾两码；字身是其余部分，最多取首、次、尾三码。",
                "字身还能再分时，分成次字首、次字身，照样取码；一个字最多五码。",
                "整体字（不能分字首字身的字，如 东、五）：按笔顺取首、次、三、尾码，最多四码。",
                "部件在字里的取码可能和它单独成字时不同（如 我 的 手 取 HQ），以字里的形状为准。",
                "速成（简易）输入法只取全码的首尾两码。",
            ],
        ),
    };
    v_flex()
        .gap_5()
        .items_center()
        .child(
            v_flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(subtitle),
                ),
        )
        .child(Keyboard::new(scheme).large(true))
        .child(zone_legend(scheme, cx))
        .child(
            v_flex()
                .max_w(px(640.))
                .gap_2()
                .p_4()
                .rounded(px(12.))
                .bg(theme.secondary)
                .child(section_label("取码规则速记", cx))
                .children(
                    rules
                        .iter()
                        .map(|s| div().text_sm().line_height(px(20.)).child(*s)),
                ),
        )
}

fn zone_legend(scheme: Scheme, cx: &App) -> impl IntoElement {
    h_flex()
        .gap_4()
        .children(scheme.zone_names().iter().enumerate().map(|(i, name)| {
            h_flex()
                .gap_1p5()
                .items_center()
                .child(div().size(px(12.)).rounded(px(3.)).bg(zone_color(i, cx)))
                .child(div().text_sm().child(format!("{} {}", i + 1, name)))
        }))
}
