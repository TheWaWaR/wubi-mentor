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
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            highlights: Vec::new(),
            show_roots: true,
        }
    }

    pub fn highlight(mut self, code: &str) -> Self {
        self.highlights = code.chars().enumerate().map(|(i, k)| (k, i + 1)).collect();
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
                .w(px(68.))
                .h(px(if self.show_roots { 76. } else { 48. }))
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
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(fg)
                                    .child(name.to_string()),
                            )
                        }),
                )
                .when(self.show_roots, |this| {
                    this.child(
                        div()
                            .text_size(px(10.))
                            .line_height(px(12.))
                            .text_color(sub)
                            .child(key_roots(key)),
                    )
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
                    .pl(px(row as f32 * 22.))
                    .children(keys.chars().map(key_cap))
            }))
    }
}
