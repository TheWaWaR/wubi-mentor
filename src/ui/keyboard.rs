//! 虚拟键盘：分区底色、键名字根、常用字根（五笔）或辅助字形（仓颉），以及当前编码的高亮。

use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use std::collections::HashSet;

use crate::cangjie;
use crate::dict::{KEY_ROWS, ROOT_FONT, Root, key_name, key_roots, key_roots_frequent};
use crate::scheme::Scheme;

/// 每个区的颜色（色相），键位底色和字根标签共用。
pub fn zone_color(zone: usize, cx: &App) -> Hsla {
    let t = cx.theme();
    [t.blue, t.green, t.yellow, t.magenta, t.cyan][zone % 5]
}

#[derive(IntoElement)]
pub struct Keyboard {
    scheme: Scheme,
    /// 要高亮的键和它在编码中的序号（从 1 开始）。
    highlights: Vec<(char, usize)>,
    /// 激活的字根（五笔是字根字体字形，仓颉是部件字形）和它们所在的键，在键盘上标出来。
    active_glyphs: HashSet<char>,
    active_keys: HashSet<char>,
    /// 大键帽，显示该键的全部字根（字根表用）；否则只显示最常用的几个。
    large: bool,
}

impl Keyboard {
    pub fn new(scheme: Scheme) -> Self {
        Self {
            scheme,
            highlights: Vec::new(),
            active_glyphs: HashSet::new(),
            active_keys: HashSet::new(),
            large: false,
        }
    }

    pub fn highlight(mut self, code: &str) -> Self {
        self.highlights = code.chars().enumerate().map(|(i, k)| (k, i + 1)).collect();
        self
    }

    pub fn active_roots<'a>(mut self, roots: impl IntoIterator<Item = &'a Root>) -> Self {
        for root in roots {
            self.active_keys.insert(root.key);
            self.active_glyphs.extend(root.font_glyph);
        }
        self
    }

    /// 仓颉：激活的部件字形和它们所在的键。
    pub fn active_shapes(mut self, shapes: impl IntoIterator<Item = (char, char)>) -> Self {
        for (glyph, key) in shapes {
            self.active_keys.insert(key);
            self.active_glyphs.insert(glyph);
        }
        self
    }

    /// 只激活键，不标字形（如仓颉占多码的部件）。
    pub fn active_keys(mut self, keys: impl IntoIterator<Item = char>) -> Self {
        self.active_keys.extend(keys);
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
        let any_highlight = !self.highlights.is_empty() || !self.active_keys.is_empty();
        let wubi = self.scheme == Scheme::Wubi86;
        let hover = window.use_keyed_state(
            match (self.large, wubi) {
                (true, true) => "keyboard-hover-large",
                (false, true) => "keyboard-hover",
                (true, false) => "keyboard-hover-large-cj",
                (false, false) => "keyboard-hover-cj",
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
            let zone = self.scheme.zone_of(key);
            let tint = zone
                .map(|z| zone_color(z, cx))
                .unwrap_or(theme.muted_foreground);
            let active_key = self.active_keys.contains(&key);
            let (bg, border) = if lit {
                (tint.opacity(0.9), tint)
            } else if active_key {
                (tint.opacity(0.16), tint)
            } else if any_highlight {
                (tint.opacity(0.05), tint.opacity(0.2))
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
            // 键名字根的字形，以及键上其余的字根（五笔）或辅助字形（仓颉）。
            let (name, name_glyph, all): (Option<char>, Option<char>, &[char]) = if wubi {
                let roots = key_roots(key);
                (
                    key_name(key),
                    roots.first().copied(),
                    roots.get(1..).unwrap_or_default(),
                )
            } else {
                let name = cangjie::root_name(key);
                (name, name, cangjie::key_shapes(key))
            };
            let is_active = |g: char| self.active_glyphs.contains(&g);
            // 小键帽只显示常用字根，再加上激活的字根。
            let shown: Vec<char> = if self.large || !wubi {
                all.to_vec()
            } else {
                let frequent = key_roots_frequent(key);
                all.iter()
                    .copied()
                    .filter(|g| frequent.contains(g) || is_active(*g))
                    .collect()
            };
            let hidden = all.len().saturating_sub(shown.len());
            let glyph_font = if wubi {
                SharedString::from(ROOT_FONT)
            } else {
                theme.font_family.clone()
            };
            // 一个字根字形；激活的用区色实底标出。
            let glyph = |g: char| {
                div()
                    .when(is_active(g) && !lit, |d| {
                        d.px(px(2.))
                            .rounded(px(4.))
                            .bg(tint)
                            .text_color(gpui_kit::white())
                    })
                    .child(g.to_string())
            };
            let name_active = name_glyph.is_some_and(is_active);
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
                    .when_some(name, |this, name| {
                        this.child(
                            div()
                                .when(name_active && !lit, |d| {
                                    d.px(px(3.)).rounded(px(4.)).bg(tint)
                                })
                                .map(|d| {
                                    if name_size_large {
                                        d.text_xl()
                                    } else {
                                        d.text_base()
                                    }
                                })
                                .font_weight(FontWeight::BOLD)
                                .text_color(if name_active && !lit {
                                    gpui_kit::white()
                                } else {
                                    fg
                                })
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
                // 仓颉的辅助字形少，大键帽不用那么高。
                .min_h(px(match (self.large, wubi) {
                    (true, true) => 136.,
                    (true, false) => 96.,
                    _ => 84.,
                }))
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
                        .font_family(glyph_font.clone())
                        .text_size(px(if self.large { 18. } else { 13. }))
                        .line_height(px(if self.large { 22. } else { 15. }))
                        .text_color(if self.large { fg } else { sub })
                        .children(shown.iter().map(|&g| glyph(g)))
                        .when(hidden > 0, |this| {
                            this.child(
                                div()
                                    .font_family(theme.font_family.clone())
                                    .text_size(px(10.))
                                    .child(format!("+{hidden}")),
                            )
                        }),
                )
                .map(|this| match (self.scheme, key) {
                    (Scheme::Wubi86, 'Z') => {
                        this.child(div().text_size(px(10.)).text_color(sub).child("学习键"))
                    }
                    (Scheme::Cangjie5, 'X') => {
                        this.child(div().text_size(px(10.)).text_color(sub).child("难字键"))
                    }
                    (Scheme::Cangjie5, 'Z') => this.opacity(0.4),
                    _ => this,
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
                                        .font_family(glyph_font.clone())
                                        .text_size(px(16.))
                                        .line_height(px(20.))
                                        .text_color(theme.foreground)
                                        .children(all.iter().map(|&g| glyph(g))),
                                ),
                        )
                        .with_priority(2),
                    )
                })
                .when(lit, |this| {
                    // 延后绘制，免得被键帽边框盖住。
                    this.child(
                        deferred(
                            div()
                                .absolute()
                                .top(px(-9.))
                                .right(px(-4.))
                                .h(px(18.))
                                .min_w(px(18.))
                                .px(px(5.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .border_1()
                                .border_color(theme.background)
                                .bg(theme.foreground)
                                .text_color(theme.background)
                                .text_size(px(11.))
                                .line_height(px(16.))
                                .font_weight(FontWeight::BOLD)
                                .child(orders.join("·")),
                        )
                        .with_priority(1),
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
