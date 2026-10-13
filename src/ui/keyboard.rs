//! 86 版五笔虚拟键盘：五个区的底色、键名字根、常用字根，以及当前编码的高亮。

use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::dict::{KEY_ROWS, ROOT_FONT, key_name, key_roots, key_roots_frequent};

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
    /// 大键帽，显示该键的全部字根（字根表用）；否则只显示最常用的几个。
    large: bool,
}

impl Keyboard {
    pub fn new() -> Self {
        Self {
            highlights: Vec::new(),
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
}

/// 鼠标停在哪个键（或它展开的浮层）上。
#[derive(Default)]
struct HoverState {
    key: Option<char>,
    overlay: Option<char>,
}

/// 展开浮层的宽度：比键帽宽，盖住相邻的键。
const EXPANDED_WIDTH: f32 = 168.;

impl RenderOnce for Keyboard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let any_highlight = !self.highlights.is_empty();
        let hover = window.use_keyed_state(
            if self.large {
                "keyboard-hover-large"
            } else {
                "keyboard-hover"
            },
            cx,
            |_, _| HoverState::default(),
        );
        let expanded = {
            let state = hover.read(cx);
            state.overlay.or(state.key)
        };
        let key_cap = |row: usize, col: usize, key: char| {
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
            let hidden = if self.large {
                0
            } else {
                key_roots(key)
                    .len()
                    .saturating_sub(1 + key_roots_frequent(key).len())
            };
            let header = |name_size_large: bool, fg: Hsla, sub: Hsla| {
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
                                    if name_size_large {
                                        d.text_xl()
                                    } else {
                                        d.text_base()
                                    }
                                })
                                .font_weight(FontWeight::BOLD)
                                .text_color(fg)
                                .child(name.to_string()),
                        )
                    })
            };
            v_flex()
                .id(SharedString::from(format!("key-{key}")))
                .relative()
                .when(hidden > 0, |this| {
                    let hover = hover.clone();
                    this.on_hover(move |hovered, _, cx| {
                        hover.update(cx, |state, cx| {
                            if *hovered {
                                state.key = Some(key);
                            } else if state.key == Some(key) {
                                state.key = None;
                            }
                            cx.notify();
                        })
                    })
                })
                .w(px(if self.large { 92. } else { 88. }))
                .min_h(px(if self.large { 136. } else { 84. }))
                .px_1p5()
                .py_1()
                .gap_0p5()
                .rounded(px(8.))
                .border_1()
                .border_color(border)
                .bg(bg)
                .child(header(self.large, fg, sub))
                .child(
                    // 第一个是键名字根，已经显示在右上角。
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_x(px(if self.large { 4. } else { 2. }))
                        .pt(px(if self.large { 4. } else { 2. }))
                        .font_family(ROOT_FONT)
                        .text_size(px(if self.large { 18. } else { 13. }))
                        .line_height(px(if self.large { 22. } else { 15. }))
                        .text_color(if self.large { fg } else { sub })
                        .map(|this| {
                            if self.large {
                                this.children(
                                    key_roots(key)
                                        .iter()
                                        .skip(1)
                                        .map(|g| div().child(g.to_string())),
                                )
                            } else {
                                let shown = key_roots_frequent(key);
                                this.children(shown.iter().map(|g| div().child(g.to_string())))
                                    .when(hidden > 0, |this| {
                                        this.child(
                                            div()
                                                .font_family(theme.font_family.clone())
                                                .text_size(px(10.))
                                                .child(format!("+{hidden}")),
                                        )
                                    })
                            }
                        }),
                )
                .when(key == 'Z', |this| {
                    this.child(div().text_size(px(10.)).text_color(sub).child("学习键"))
                })
                // 折叠的键：鼠标停上去时原地展开，盖住相邻的键，显示全部字根。
                .when(expanded == Some(key) && hidden > 0, |this| {
                    let hover = hover.clone();
                    let bottom_row = row == KEY_ROWS.len() - 1;
                    let right_half = col * 2 >= KEY_ROWS[row].len();
                    this.child(
                        deferred(
                            v_flex()
                                .id(SharedString::from(format!("key-expanded-{key}")))
                                .occlude()
                                .absolute()
                                .map(|d| {
                                    if bottom_row {
                                        d.bottom(px(-1.))
                                    } else {
                                        d.top(px(-1.))
                                    }
                                })
                                .map(|d| {
                                    if right_half {
                                        d.right(px(-1.))
                                    } else {
                                        d.left(px(-1.))
                                    }
                                })
                                .w(px(EXPANDED_WIDTH))
                                .px_2()
                                .py_1p5()
                                .gap_1()
                                .rounded(px(8.))
                                .border_1()
                                .border_color(tint)
                                .bg(theme.background)
                                .shadow_lg()
                                .on_hover(move |hovered, _, cx| {
                                    hover.update(cx, |state, cx| {
                                        state.overlay = hovered.then_some(key);
                                        cx.notify();
                                    })
                                })
                                .child(header(false, theme.foreground, theme.muted_foreground))
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .gap_x(px(4.))
                                        .font_family(ROOT_FONT)
                                        .text_size(px(16.))
                                        .line_height(px(20.))
                                        .text_color(theme.foreground)
                                        .children(
                                            key_roots(key)
                                                .iter()
                                                .skip(1)
                                                .map(|g| div().child(g.to_string())),
                                        ),
                                ),
                        )
                        .with_priority(2),
                    )
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
                    .items_stretch()
                    .gap_1p5()
                    .pl(px(row as f32 * if self.large { 30. } else { 22. }))
                    .children(
                        keys.chars()
                            .enumerate()
                            .map(|(col, key)| key_cap(row, col, key)),
                    )
            }))
    }
}
