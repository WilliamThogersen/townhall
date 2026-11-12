mod api;
#[cfg(target_os = "macos")]
mod measure;
mod theme;
mod view;

use gpui::{prelude::*, px, size, App, Application, Bounds, WindowBounds, WindowOptions};

const DEFAULT_BASE: &str = "http://127.0.0.1:7777";

fn main() {
    let mut base = DEFAULT_BASE.to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--url" => base = args.next().expect("--url needs a daemon url"),
            #[cfg(target_os = "macos")]
            "--measure" => {
                let seconds = args
                    .next()
                    .and_then(|s| s.parse().ok())
                    .expect("--measure needs a number of seconds");
                measure::spawn(seconds);
            }
            other => panic!("unknown argument {other}"),
        }
    }
    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| view::Root::new(base, cx)),
        )
        .expect("cannot open the window");
        cx.activate(true);
    });
}
