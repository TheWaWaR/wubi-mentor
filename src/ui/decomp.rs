//! 拆字卡：大字、逐码讲解、字根序列、简码和熟练度。

use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::db::{CharStat, Mastery};
use crate::dict::{CharKind, CodePart, Entry, ROOT_FONT, Root, display_glyph, is_single_stroke};
use crate::ui::keyboard::{zone_color, zone_of};

#[derive(IntoElement)]
pub struct DecompCard {
    entry: Entry,
    stat: Option<CharStat>,
}

impl DecompCard {
    pub fn new(entry: Entry) -> Self {
        Self { entry, stat: None }
    }

    pub fn stat(mut self, stat: CharStat) -> Self {
        self.stat = Some(stat);
        self
    }
}

fn key_tint(key: char, cx: &App) -> Hsla {
    zone_of(key)
        .map(|z| zone_color(z, cx))
        .unwrap_or(cx.theme().muted_foreground)
}

/// 田字格里的大字。
pub fn big_char(ch: char, size: f32, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let guide = theme.border.opacity(0.7);
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .rounded(px(10.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .child(
            div()
                .absolute()
                .left(px(size / 2.))
                .top_0()
                .bottom_0()
                .w(px(1.))
                .bg(guide),
        )
        .child(
            div()
                .absolute()
                .top(px(size / 2.))
                .left_0()
                .right_0()
                .h(px(1.))
                .bg(guide),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(size * 0.68))
                .line_height(px(size))
                .text_color(theme.foreground)
                .child(ch.to_string()),
        )
}

/// 字根字体里的一个字形。
pub fn root_glyph(glyph: char, size: f32) -> Div {
    div()
        .font_family(ROOT_FONT)
        .text_size(px(size))
        .line_height(px(size * 1.2))
        .child(glyph.to_string())
}

/// 字根标签：字形 + 键位。优先用字根字体；没有时用 Unicode 字形，伪字根（如“牛前”）用小字描述。
pub fn root_chip(root: &Root, coded: bool, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let key = root.key;
    let tint = key_tint(key, cx);
    let glyph = display_glyph(&root.glyph);
    let descriptive = glyph.chars().count() > 1;
    v_flex()
        .items_center()
        .min_w(px(46.))
        .px_1p5()
        .py_1()
        .rounded(px(8.))
        .border_1()
        .border_color(if coded { tint } else { theme.border })
        .bg(if coded {
            tint.opacity(0.12)
        } else {
            theme.background
        })
        .when(!coded, |this| this.opacity(0.55))
        .child(
            div()
                .h(px(30.))
                .flex()
                .items_center()
                .text_color(theme.foreground)
                .map(|this| match root.font_glyph {
                    Some(g) => this.child(root_glyph(g, 26.)),
                    None if descriptive => this.text_sm().child(glyph.to_string()),
                    None => this.text_size(px(24.)).child(glyph.to_string()),
                }),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(if coded { tint } else { theme.muted_foreground })
                .child(key.to_string()),
        )
}

fn part_caption(part: &CodePart, entry: &Entry) -> String {
    match part {
        CodePart::Root { index, .. } => entry
            .roots
            .get(*index)
            .map(|r| display_glyph(&r.glyph).to_string())
            .unwrap_or_default(),
        CodePart::Ident { .. } => "识别码".into(),
        CodePart::Stroke { stroke, .. } => (*stroke).into(),
        CodePart::KeyRepeat { .. } => "重复".into(),
    }
}

fn rule_text(entry: &Entry) -> String {
    match entry.kind {
        CharKind::KeyName => format!(
            "「{}」是 {} 键的键名字根，连击四下 {}。",
            entry.ch,
            entry.full_code.chars().next().unwrap_or('?'),
            entry.full_code
        ),
        CharKind::Radical if is_single_stroke(entry.ch) => {
            format!("「{}」是单笔画：打两下它所在的键，再补两个 L。", entry.ch)
        }
        CharKind::Radical => format!(
            "「{}」是成字字根：先“报户口”（打它所在的键），再打首笔、次笔、末笔；不足四码补空格。",
            entry.ch
        ),
        CharKind::Compound => {
            let n = entry.roots.len();
            if n == 4 {
                format!("正好 4 个字根，依次取码。")
            } else if n > 4 {
                format!("共 {n} 个字根，多于四个：取第一、二、三和最后一个字根。")
            } else if let Some(key) = entry.ident {
                let (stroke, shape) = crate::dict::ident_meaning(key);
                format!("只有 {n} 个字根，补末笔识别码：末笔「{stroke}」+ {shape} → {key}。")
            } else {
                format!("{n} 个字根，按书写顺序依次取码。")
            }
        }
    }
}

impl RenderOnce for DecompCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let entry = &self.entry;
        let parts = entry.code_parts();
        let coded = entry.coded_root_indexes();

        let code_row = h_flex()
            .gap_1p5()
            .children(parts.iter().enumerate().map(|(i, part)| {
                let key = part.key();
                let tint = key_tint(key, cx);
                let is_extra = !matches!(part, CodePart::Root { .. });
                v_flex()
                    .id(("code-part", i))
                    .items_center()
                    .gap_0p5()
                    .child(
                        div()
                            .size(px(38.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(8.))
                            .bg(if is_extra { theme.background } else { tint })
                            .border_1()
                            .border_color(tint)
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_extra { tint } else { gpui_kit::white() })
                            .child(key.to_string()),
                    )
                    .map(|this| match (entry.code_glyphs.get(i), part) {
                        // 字根字体里有这一码的字形：字根直接画出来，识别码、笔画再加一行说明。
                        (Some(&g), CodePart::Root { .. }) => {
                            this.child(root_glyph(g, 18.).text_color(theme.muted_foreground))
                        }
                        (Some(&g), _) => this
                            .child(root_glyph(g, 18.).text_color(theme.muted_foreground))
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(theme.muted_foreground)
                                    .child(part_caption(part, entry)),
                            ),
                        (None, _) => this.child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(part_caption(part, entry)),
                        ),
                    })
            }));

        let roots_row = h_flex()
            .flex_wrap()
            .gap_1()
            .children(entry.roots.iter().enumerate().map(|(i, root)| {
                root_chip(
                    root,
                    coded.contains(&i) || entry.kind != CharKind::Compound,
                    cx,
                )
            }));

        let shorts = h_flex()
            .gap_1()
            .flex_wrap()
            .children(entry.short_codes.iter().map(|c| {
                let level = match c.len() {
                    1 => "一级简码",
                    2 => "二级简码",
                    _ => "三级简码",
                };
                h_flex()
                    .gap_1()
                    .px_1p5()
                    .py_0p5()
                    .rounded(px(6.))
                    .bg(theme.secondary)
                    .text_xs()
                    .child(div().text_color(theme.muted_foreground).child(level))
                    .child(div().font_weight(FontWeight::BOLD).child(c.clone()))
            }));

        let header = h_flex()
            .gap_3()
            .items_start()
            .child(big_char(entry.ch, 96., cx))
            .child(
                v_flex()
                    .gap_2()
                    .min_w_0()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .px_1p5()
                                    .rounded(px(4.))
                                    .bg(theme.accent)
                                    .text_color(theme.accent_foreground)
                                    .child(entry.kind.label()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("全码 {}", entry.full_code)),
                            ),
                    )
                    .child(code_row),
            );

        v_flex()
            .gap_3()
            .child(header)
            .child(
                v_flex()
                    .gap_1p5()
                    .child(section_label("字根拆分", cx))
                    .child(roots_row),
            )
            .map(|this| {
                this.child(
                    div()
                        .text_sm()
                        .line_height(px(20.))
                        .text_color(theme.foreground)
                        .child(rule_text(entry)),
                )
            })
            .when(!entry.short_codes.is_empty(), |this| {
                this.child(
                    v_flex()
                        .gap_1p5()
                        .child(section_label("简码", cx))
                        .child(shorts),
                )
            })
            .when(!entry.verified, |this| {
                this.child(
                    div()
                        .text_xs()
                        .px_2()
                        .py_1()
                        .rounded(px(6.))
                        .bg(theme.warning.opacity(0.15))
                        .border_1()
                        .border_color(theme.warning.opacity(0.5))
                        .text_color(theme.foreground)
                        .child("这个字的拆分数据和编码对不上，拆分仅供参考，以编码为准。"),
                )
            })
            .when_some(self.stat, |this, stat| this.child(stat_row(&stat, cx)))
    }
}

pub fn section_label(text: &'static str, cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

pub fn mastery_color(m: Mastery, cx: &App) -> Hsla {
    let t = cx.theme();
    match m {
        Mastery::New => t.muted_foreground,
        Mastery::Weak => t.danger,
        Mastery::Learning => t.warning,
        Mastery::Mastered => t.success,
    }
}

fn stat_row(stat: &CharStat, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let m = stat.mastery();
    let text = if stat.seen == 0 {
        "还没练过这个字".to_string()
    } else {
        format!(
            "练过 {} 次 · 出错 {} 次 · 平均 {:.1} 秒 · 连对 {}",
            stat.seen,
            stat.errors,
            stat.avg_ms().unwrap_or(0) as f64 / 1000.,
            stat.streak
        )
    };
    h_flex()
        .gap_2()
        .items_center()
        .pt_2()
        .border_t_1()
        .border_color(theme.border)
        .child(
            div()
                .text_xs()
                .px_1p5()
                .rounded(px(4.))
                .border_1()
                .border_color(mastery_color(m, cx))
                .text_color(mastery_color(m, cx))
                .child(m.label()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(text),
        )
}
