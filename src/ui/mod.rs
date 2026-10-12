//! 主窗口：左侧导航 + 页面。

mod decomp;
mod keyboard;
mod library;
mod lookup;
mod practice;
mod stats;

use std::rc::Rc;

use gpui_kit::component::{
    ActiveTheme, IconName, h_flex,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    v_flex,
};
use gpui_kit::*;

use crate::store::Store;
use library::{LibraryEvent, LibraryView};
use lookup::LookupView;
use practice::{PracticeEvent, PracticeView};
use stats::{StatsEvent, StatsView};

actions!(wubi_mentor, [Quit]);

pub fn init(cx: &mut App) {
    practice::init(cx);
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Practice,
    Library,
    Stats,
    Lookup,
}

pub struct MainView {
    page: Page,
    practice: Entity<PracticeView>,
    library: Entity<LibraryView>,
    stats: Entity<StatsView>,
    lookup: Entity<LookupView>,
    _subscriptions: Vec<Subscription>,
}

impl MainView {
    pub fn new(store: Rc<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let practice = cx.new(|cx| PracticeView::new(store.clone(), window, cx));
        let library = cx.new(|cx| LibraryView::new(store.clone(), window, cx));
        let stats = cx.new(|cx| StatsView::new(store.clone(), window, cx));
        let lookup = cx.new(|cx| LookupView::new(store.clone(), window, cx));

        let subscriptions = vec![
            cx.subscribe_in(
                &practice,
                window,
                |this, _, event, window, cx| match event {
                    PracticeEvent::OpenLibrary => this.go(Page::Library, window, cx),
                },
            ),
            cx.subscribe_in(&library, window, |this, _, event, window, cx| match event {
                LibraryEvent::Practice(id) => {
                    this.practice.update(cx, |p, cx| p.load_text(*id, cx));
                    this.go(Page::Practice, window, cx);
                }
                LibraryEvent::Changed(id) => {
                    this.practice
                        .update(cx, |p, cx| p.reload_if_current(*id, cx));
                }
            }),
            cx.subscribe_in(&stats, window, |this, _, event, window, cx| match event {
                StatsEvent::Lookup(text) => {
                    this.lookup
                        .update(cx, |l, cx| l.set_query(text, window, cx));
                    this.go(Page::Lookup, window, cx);
                }
                StatsEvent::Practice(id) => {
                    this.practice.update(cx, |p, cx| p.load_text(*id, cx));
                    this.go(Page::Practice, window, cx);
                }
            }),
        ];

        // 退出时把还没结束的练习记录存下来。
        let quit = cx.on_app_quit(|this, cx| {
            this.practice.update(cx, |p, _| p.end_session());
            async {}
        });

        let view = Self {
            page: Page::Practice,
            practice,
            library,
            stats,
            lookup,
            _subscriptions: subscriptions.into_iter().chain([quit]).collect(),
        };
        window.focus(&view.practice.focus_handle(cx), cx);
        view
    }

    pub(crate) fn go(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        if self.page == Page::Practice && page != Page::Practice {
            self.practice.update(cx, |p, _| p.end_session());
        }
        self.page = page;
        match page {
            Page::Practice => window.focus(&self.practice.focus_handle(cx), cx),
            Page::Library => self.library.update(cx, |l, cx| l.reload(window, cx)),
            Page::Stats => self.stats.update(cx, |s, cx| s.reload(cx)),
            Page::Lookup => self.lookup.update(cx, |l, cx| l.focus(window, cx)),
        }
        cx.notify();
    }
}

#[cfg(feature = "snapshot")]
impl MainView {
    pub(crate) fn debug_open_text(
        &mut self,
        title: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = self.library.read(cx).debug_text_id(title);
        if let Some(id) = id {
            self.practice.update(cx, |p, cx| p.load_text(id, cx));
            self.go(Page::Practice, window, cx);
        }
    }

    pub(crate) fn debug_new_text(
        &mut self,
        content: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = self
            .library
            .update(cx, |l, cx| l.debug_insert(content, window, cx));
        self.practice.update(cx, |p, cx| p.load_text(id, cx));
        self.go(Page::Practice, window, cx);
    }

    pub(crate) fn debug_type(&mut self, text: &str, cx: &mut Context<Self>) {
        self.practice.update(cx, |p, cx| {
            for ch in text.chars() {
                p.debug_commit(&ch.to_string(), cx);
            }
        });
    }

    pub(crate) fn debug_mark(&mut self, text: &str, cx: &mut Context<Self>) {
        self.practice.update(cx, |p, cx| p.debug_mark(text, cx));
    }

    pub(crate) fn debug_hint(&mut self, mode: &str, cx: &mut Context<Self>) {
        self.practice.update(cx, |p, cx| p.debug_hint(mode, cx));
    }

    pub(crate) fn debug_inspect(&mut self, index: usize, cx: &mut Context<Self>) {
        self.practice.update(cx, |p, cx| p.debug_inspect(index, cx));
    }

    pub(crate) fn debug_lookup(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.lookup
            .update(cx, |l, cx| l.set_query(text, window, cx));
        self.go(Page::Lookup, window, cx);
    }
}

impl Render for MainView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let item = |label: &'static str, icon: IconName, page: Page, cx: &mut Context<Self>| {
            SidebarMenuItem::new(label)
                .icon(icon)
                .active(self.page == page)
                .on_click(cx.listener(move |this, _, window, cx| this.go(page, window, cx)))
        };
        let sidebar = Sidebar::new("nav")
            .w(px(188.))
            .header(
                SidebarHeader::new().child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .size(px(30.))
                                .rounded(px(8.))
                                .bg(theme.primary)
                                .text_color(theme.primary_foreground)
                                .flex()
                                .items_center()
                                .justify_center()
                                .font_weight(FontWeight::BOLD)
                                .child("五"),
                        )
                        .child(
                            v_flex()
                                .child(div().font_weight(FontWeight::SEMIBOLD).child("五笔学堂"))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("86 版"),
                                ),
                        ),
                ),
            )
            .child(
                SidebarGroup::new("学习").child(
                    SidebarMenu::new()
                        .child(item("练习", IconName::SquareTerminal, Page::Practice, cx))
                        .child(item("文本库", IconName::BookOpen, Page::Library, cx))
                        .child(item("查字", IconName::Search, Page::Lookup, cx))
                        .child(item("统计", IconName::ChartPie, Page::Stats, cx)),
                ),
            );

        let content = match self.page {
            Page::Practice => self.practice.clone().into_any_element(),
            Page::Library => self.library.clone().into_any_element(),
            Page::Stats => self.stats.clone().into_any_element(),
            Page::Lookup => self.lookup.clone().into_any_element(),
        };

        h_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(sidebar)
            .child(div().flex_1().min_w_0().h_full().child(content))
    }
}
