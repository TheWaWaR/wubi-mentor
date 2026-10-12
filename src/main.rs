mod db;
mod dict;
mod practice;
#[cfg(feature = "snapshot")]
mod snapshot;
mod store;
mod ui;

use std::rc::Rc;

use gpui_kit::*;

fn main() {
    let store = match store::Store::open() {
        Ok(store) => Rc::new(store),
        Err(err) => {
            eprintln!("无法打开数据库: {err:#}");
            std::process::exit(1);
        }
    };

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            ui::init(cx);
            let bounds = Bounds::centered(None, size(px(1280.), px(900.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(1100.), px(760.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("五笔学堂".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let (_window, _main) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| ui::MainView::new(store.clone(), window, cx))
            })
            .expect("failed to open window");
            #[cfg(feature = "snapshot")]
            if let Some(dir) = std::env::var_os("WUBI_SNAPSHOT") {
                snapshot::run(_window, _main, dir.into(), cx);
            }
            cx.activate(true);
        });
}
