//! 86 版五笔虚拟键盘：五个区的底色、键名字根、常用字根，以及当前编码的高亮。

use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::dict::{KEY_ROWS, key_name, key_roots};

/// 五笔的五个区：横、竖、撇、捺、折。
pub fn zone_of(key: char) -> Option<usize> {
    ["GFDSA", "HJKLM", "TREWQ", "YUIOP", "NBVCX"]
        .iter()
        .position(|z| z.contains(key))
}

pub const ZONE_NAMES: [&str; 5] = ["横区", "竖区", "撇区", "捺区", "折区"];

/// 每个区的颜色（色相），键位底色和字根标签共用。
pub fn zone_color(zone: usize, cx: &App) -> Hsla {
    let t = cx.theme();
    [t.blue, t.green, t.yellow, t.magenta, t.cyan][zone % 5]
}

#[derive(IntoElement)]
pub struct Keyboard {
    /// 要高亮的键和它在编码中的序号（从 1 开始）。
    highlights: Vec<(char, usize)>,
    show_roots: bool,
    /// 大键帽，显示该键的全部字根（字根表用）；否则只显示最常用的几个。
    large: bool,
}

/// 小键盘上每个键最多显示几个字根。
const COMPACT_ROOTS: usize = 6;

impl Keyboard {
    pub fn new() -> Self {
        Self {
            highlights: Vec::new(),
            show_roots: true,
            large: false,
        }
    }

    pub fn highlight(mut self, code: &str) -> Self {
        self.highlights = code.chars().enumerate().map(|(i, k)| (k, i + 1)).collect();
        self
    }

    pub fn large(mut self, large: bool) -> Self {
        self.large = large;
        self
    }

    pub fn show_roots(mut self, show: bool) -> Self {
        self.show_roots = show;
        self
    }
}

impl RenderOnce for Keyboard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let any_highlight = !self.highlights.is_empty();
        let key_cap = |key: char| {
            let orders: Vec<String> = self
                .highlights
                .iter()
                .filter(|(k, _)| *k == key)
                .map(|(_, i)| i.to_string())
                .collect();
            let lit = !orders.is_empty();
            let zone = zone_of(key);
            let tint = zone
                .map(|z| zone_color(z, cx))
                .unwrap_or(theme.muted_foreground);
            let (bg, border) = if lit {
                (tint.opacity(0.9), tint)
            } else if any_highlight {
                (theme.background, theme.border)
            } else {
                (tint.opacity(0.08), tint.opacity(0.35))
            };
            let fg = if lit {
                gpui_kit::white()
            } else {
                theme.foreground
            };
            let sub = if lit {
                gpui_kit::white().opacity(0.85)
            } else {
                theme.muted_foreground
            };
            v_flex()
                .id(SharedString::from(format!("key-{key}")))
                .relative()
                .w(px(if self.large { 92. } else { 68. }))
                .h(px(match (self.large, self.show_roots) {
                    (true, _) => 136.,
                    (false, true) => 76.,
                    (false, false) => 48.,
                }))
                .px_1p5()
                .py_1()
                .gap_0p5()
                .rounded(px(8.))
                .border_1()
                .border_color(border)
                .bg(bg)
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(sub)
                                .child(key.to_string()),
                        )
                        .when_some(key_name(key), |this, name| {
                            this.child(
                                div()
                                    .map(|d| {
                                        if self.large {
                                            d.text_xl()
                                        } else {
                                            d.text_base()
                                        }
                                    })
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(fg)
                                    .child(name.to_string()),
                            )
                        }),
                )
                .when(self.show_roots && !self.large, |this| {
                    // 第一个是键名字根，已经显示在右上角。
                    let roots = key_roots(key).get(1..).unwrap_or_default();
                    let mut text = roots[..roots.len().min(COMPACT_ROOTS)].join(" ");
                    if roots.len() > COMPACT_ROOTS {
                        text.push_str(" …");
                    }
                    this.child(
                        div()
                            .text_size(px(10.))
                            .line_height(px(12.))
                            .text_color(sub)
                            .child(text),
                    )
                })
                .when(self.large, |this| {
                    this.child(div().flex().flex_wrap().gap_x_1p5().pt_1().children(
                        key_roots(key).iter().skip(1).map(|root| {
                            // 文字描述的伪字根用小字，和真字形区分开。
                            let descriptive = root.chars().count() > 1;
                            div()
                                .text_color(if descriptive { sub } else { fg })
                                .map(|d| {
                                    if descriptive {
                                        d.text_xs()
                                    } else {
                                        d.text_base()
                                    }
                                })
                                .line_height(px(22.))
                                .child(*root)
                        }),
                    ))
                })
                .when(key == 'Z', |this| {
                    this.child(div().text_size(px(10.)).text_color(sub).child("学习键"))
                })
                .when(lit, |this| {
                    this.child(
                        div()
                            .absolute()
                            .top(px(-8.))
                            .right(px(-6.))
                            .px_1()
                            .rounded_full()
                            .bg(theme.foreground)
                            .text_color(theme.background)
                            .text_size(px(10.))
                            .font_weight(FontWeight::BOLD)
                            .child(orders.join("·")),
                    )
                })
        };

        v_flex()
            .gap_1p5()
            .items_center()
            .children(KEY_ROWS.iter().enumerate().map(|(row, keys)| {
                h_flex()
                    .gap_1p5()
                    .pl(px(row as f32 * if self.large { 30. } else { 22. }))
                    .children(keys.chars().map(key_cap))
            }))
    }
}
