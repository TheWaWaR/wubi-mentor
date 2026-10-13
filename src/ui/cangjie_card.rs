//! 仓颉拆字卡：大字、逐码讲解、字首／字身部件、速成码和熟练度。

use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::cangjie::{Entry, Kind, Part, root_name};
use crate::db::CharStat;
use crate::scheme::Scheme;
use crate::ui::decomp::{big_char, key_tint, section_label, stat_row};

#[derive(IntoElement)]
pub struct CangjieCard {
    entry: Entry,
    stat: Option<CharStat>,
}

impl CangjieCard {
    pub fn new(entry: Entry) -> Self {
        Self { entry, stat: None }
    }

    pub fn stat(mut self, stat: CharStat) -> Self {
        self.stat = Some(stat);
        self
    }
}

fn tint(key: char, cx: &App) -> Hsla {
    key_tint(Scheme::Cangjie5, key, cx)
}

fn glyph_text(part: &Part) -> String {
    part.glyph.map(String::from).unwrap_or_else(|| "？".into())
}

/// 部件标签：字形、在这个字里取的码，以及角色（字首、字身……）。
fn part_chip(part: &Part, role: &'static str, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let first = part.code.chars().next().unwrap_or('?');
    let border = tint(first, cx);
    v_flex()
        .items_center()
        .min_w(px(52.))
        .px_1p5()
        .py_1()
        .gap_0p5()
        .rounded(px(8.))
        .border_1()
        .border_color(border)
        .bg(border.opacity(0.10))
        .when(!role.is_empty(), |this| {
            this.child(
                div()
                    .text_size(px(10.))
                    .text_color(theme.muted_foreground)
                    .child(role),
            )
        })
        .child(
            div()
                .h(px(30.))
                .flex()
                .items_center()
                .text_size(px(24.))
                .text_color(if part.glyph.is_some() {
                    theme.foreground
                } else {
                    theme.muted_foreground
                })
                .child(glyph_text(part)),
        )
        .child(h_flex().gap(px(2.)).children(part.code.chars().map(|k| {
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(tint(k, cx))
                .child(k.to_string())
        })))
        .when_some(part.standalone.clone(), |this, code| {
            this.child(
                div()
                    .text_size(px(10.))
                    .text_color(theme.warning)
                    .child(format!("单独打 {code}")),
            )
        })
}

fn rule_text(entry: &Entry) -> String {
    match entry.kind {
        Kind::Whole if entry.code.len() <= 1 => {
            format!("「{}」本身就是字根，打一下 {} 即可。", entry.ch, entry.code)
        }
        Kind::Whole => format!(
            "「{}」是整体字，不分字首字身：按笔顺取首、次、三、尾码，最多四码。",
            entry.ch
        ),
        Kind::Split => {
            let roles = entry.part_roles();
            let pieces: Vec<String> = entry
                .parts
                .iter()
                .zip(roles)
                .map(|(p, role)| format!("{role}「{}」取 {}", glyph_text(p), p.code))
                .collect();
            format!("{}。", pieces.join("，"))
        }
        Kind::Unknown => "这个字的结构数据和编码对不上，只按编码逐码讲解。组合字的字首最多取两码，字身最多取三码。".into(),
    }
}

impl RenderOnce for CangjieCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let entry = &self.entry;

        // 每一码属于哪个部件，用部件字形作说明。
        let owners: Vec<Option<char>> = entry
            .parts
            .iter()
            .flat_map(|p| p.code.chars().map(move |_| p.glyph))
            .collect();
        let code_row = h_flex()
            .gap_1p5()
            .children(entry.code.chars().enumerate().map(|(i, key)| {
                let t = tint(key, cx);
                v_flex()
                    .id(("cj-code", i))
                    .items_center()
                    .gap_0p5()
                    .child(
                        div()
                            .size(px(38.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(8.))
                            .bg(t)
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui_kit::white())
                            .child(key.to_string()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(root_name(key).map(String::from).unwrap_or("难".into())),
                    )
                    .when(entry.kind == Kind::Split, |this| {
                        this.child(
                            div()
                                .text_size(px(10.))
                                .text_color(theme.muted_foreground)
                                .child(
                                    owners
                                        .get(i)
                                        .copied()
                                        .flatten()
                                        .map(String::from)
                                        .unwrap_or_default(),
                                ),
                        )
                    })
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
                                    .child(format!("全码 {}", entry.code)),
                            ),
                    )
                    .child(code_row),
            );

        let roles = entry.part_roles();
        let parts_row = h_flex().flex_wrap().items_start().gap_1().children(
            entry
                .parts
                .iter()
                .zip(roles)
                .map(|(p, role)| part_chip(p, role, cx)),
        );

        let quick = entry.quick_code();
        v_flex()
            .gap_3()
            .child(header)
            // 整体字的部件就是它自己，不必再列。
            .when(entry.kind == Kind::Split, |this| {
                this.child(
                    v_flex()
                        .gap_1p5()
                        .child(section_label("字首 · 字身", cx))
                        .child(parts_row),
                )
            })
            .child(
                div()
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(theme.foreground)
                    .child(rule_text(entry)),
            )
            .when(entry.parts.iter().any(|p| p.standalone.is_some()), |this| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("部件在字里的取码和它单独成字时不一样，注意区分。"),
                )
            })
            .when(quick != entry.code, |this| {
                this.child(
                    v_flex()
                        .gap_1p5()
                        .items_start()
                        .child(section_label("速成", cx))
                        .child(
                            h_flex()
                                .gap_1()
                                .px_1p5()
                                .py_0p5()
                                .rounded(px(6.))
                                .bg(theme.secondary)
                                .text_xs()
                                .child(
                                    div()
                                        .text_color(theme.muted_foreground)
                                        .child("速成（取首尾两码）"),
                                )
                                .child(div().font_weight(FontWeight::BOLD).child(quick)),
                        ),
                )
            })
            .when_some(self.stat, |this, stat| this.child(stat_row(&stat, cx)))
    }
}
