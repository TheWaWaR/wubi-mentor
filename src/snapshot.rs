//! 开发用：按脚本操作界面，把每一步渲染成 PNG（`ZIGEN_SNAPSHOT=目录`）。

use std::time::Duration;

use gpui_kit::*;

use crate::ui::{MainView, Page};

pub fn run(window: AnyWindowHandle, main: Entity<MainView>, dir: std::path::PathBuf, cx: &mut App) {
    std::fs::create_dir_all(&dir).expect("create snapshot dir");
    cx.spawn(async move |cx| {
        let step =
            async |cx: &mut AsyncApp,
                   name: &str,
                   f: &dyn Fn(&mut MainView, &mut Window, &mut Context<MainView>)| {
                cx.update(|cx| {
                    window
                        .update(cx, |_, window, cx| {
                            main.update(cx, |m, cx| f(m, window, cx))
                        })
                        .ok();
                });
                cx.background_executor()
                    .timer(Duration::from_millis(700))
                    .await;
                cx.update(|cx| {
                    window
                        .update(cx, |_, window, cx| {
                            // 窗口被遮挡时系统不会触发重绘，这里手动画一帧。
                            window.draw(cx).clear(cx);
                            let image = window.render_to_image().expect("render");
                            image.save(dir.join(format!("{name}.png"))).expect("save");
                        })
                        .ok();
                });
            };
        cx.background_executor()
            .timer(Duration::from_millis(1200))
            .await;
        let script = std::env::var("ZIGEN_SNAPSHOT_SCRIPT").unwrap_or_default();
        for (i, cmd) in script
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .enumerate()
        {
            let name = format!("{:02}", i + 1);
            let (verb, arg) = cmd.split_once(' ').unwrap_or((cmd, ""));
            let arg = arg.to_string();
            match verb {
                "page" => {
                    let page = match arg.as_str() {
                        "library" => Page::Library,
                        "stats" => Page::Stats,
                        "lookup" => Page::Lookup,
                        _ => Page::Practice,
                    };
                    step(cx, &name, &move |m, w, cx| m.go(page, w, cx)).await
                }
                "open" => step(cx, &name, &move |m, w, cx| m.debug_open_text(&arg, w, cx)).await,
                "text" => step(cx, &name, &move |m, w, cx| m.debug_new_text(&arg, w, cx)).await,
                "type" => step(cx, &name, &move |m, _, cx| m.debug_type(&arg, cx)).await,
                "mark" => step(cx, &name, &move |m, _, cx| m.debug_mark(&arg, cx)).await,
                "hint" => step(cx, &name, &move |m, _, cx| m.debug_hint(&arg, cx)).await,
                "lookup" => step(cx, &name, &move |m, w, cx| m.debug_lookup(&arg, w, cx)).await,
                "scheme" => step(cx, &name, &move |m, w, cx| m.debug_scheme(&arg, w, cx)).await,
                "inspect" => {
                    step(cx, &name, &move |m, _, cx| {
                        m.debug_inspect(arg.parse().unwrap_or(0), cx)
                    })
                    .await
                }
                "move" => {
                    let mut xy = arg.split_whitespace().filter_map(|v| v.parse::<f32>().ok());
                    let (x, y) = (xy.next().unwrap_or(0.), xy.next().unwrap_or(0.));
                    step(cx, &name, &move |_, w, cx| {
                        w.dispatch_event(
                            PlatformInput::MouseMove(MouseMoveEvent {
                                position: point(px(x), px(y)),
                                pressed_button: None,
                                modifiers: Modifiers::default(),
                            }),
                            cx,
                        );
                    })
                    .await
                }
                "click" => {
                    let mut xy = arg.split_whitespace().filter_map(|v| v.parse::<f32>().ok());
                    let position = point(px(xy.next().unwrap_or(0.)), px(xy.next().unwrap_or(0.)));
                    // 直接派发给窗口：点击的回调可能要更新 MainView，不能套在 `main.update` 里。
                    cx.update(|cx| {
                        window
                            .update(cx, |_, w, cx| {
                                w.dispatch_event(
                                    PlatformInput::MouseDown(MouseDownEvent {
                                        button: MouseButton::Left,
                                        position,
                                        modifiers: Modifiers::default(),
                                        click_count: 1,
                                        first_mouse: false,
                                    }),
                                    cx,
                                );
                                w.dispatch_event(
                                    PlatformInput::MouseUp(MouseUpEvent {
                                        button: MouseButton::Left,
                                        position,
                                        modifiers: Modifiers::default(),
                                        click_count: 1,
                                    }),
                                    cx,
                                );
                            })
                            .ok();
                    });
                    step(cx, &name, &|_, _, _| {}).await
                }
                "shot" => step(cx, &name, &|_, _, _| {}).await,
                other => eprintln!("unknown snapshot command: {other}"),
            }
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}
