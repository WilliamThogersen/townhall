mod client;
mod daemon;
mod fonts;
#[cfg(target_os = "macos")]
mod measure;
mod state;
mod theme;
mod view;

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{prelude::*, px, size, App, Bounds, WindowBounds, WindowOptions};
use townhall_core::redact::redact;
use townhall_core::Home;

fn extend_path() {
    let extra = ["/opt/homebrew/bin", "/usr/local/bin"];
    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut parts: Vec<PathBuf> = std::env::split_paths(&current).collect();
    for dir in extra {
        let dir = PathBuf::from(dir);
        if !parts.contains(&dir) {
            parts.push(dir);
        }
    }
    match std::env::join_paths(parts) {
        Ok(joined) => std::env::set_var("PATH", joined),
        Err(e) => tracing::warn!(
            target: "townhall::native",
            event = "path.extend_failed",
            error = %redact(&e.to_string()),
            "cannot extend PATH"
        ),
    }
}

fn init_logging(home: &Home) -> Option<townhall_daemon::logging::Guard> {
    let logs = home
        .logs_dir()
        .map_err(|e| eprintln!("townhall-native: logging disabled: {e}"))
        .ok()?;
    let guard = townhall_daemon::logging::init(townhall_daemon::logging::LogConfig {
        dir: logs,
        prefix: "native".into(),
        stderr_level: tracing::Level::WARN,
    });
    tracing::info!(
        target: "townhall::native",
        event = "app.start",
        version = townhall_core::version::VERSION,
        "app started"
    );
    Some(guard)
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("townhall-native: {message}");
    std::process::exit(1)
}

fn main() {
    let mut url: Option<String> = None;
    let mut home_dir: Option<PathBuf> = None;
    let mut bind = daemon::BIND.to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                bind = args
                    .next()
                    .unwrap_or_else(|| fail("--bind needs host:port"))
            }
            "--home" => {
                home_dir = Some(PathBuf::from(
                    args.next().unwrap_or_else(|| fail("--home needs a folder")),
                ))
            }
            "--url" => {
                url = Some(
                    args.next()
                        .unwrap_or_else(|| fail("--url needs a daemon url")),
                )
            }
            #[cfg(target_os = "macos")]
            "--measure" => {
                let seconds = args
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| fail("--measure needs a number of seconds"));
                measure::spawn(seconds);
            }
            other => fail(format!("unknown argument {other}")),
        }
    }

    let home = match home_dir {
        Some(dir) => Home::at(dir),
        None => Home::from_env().unwrap_or_else(|e| fail(format!("townhall home: {e}"))),
    };
    let _log_guard = init_logging(&home);
    extend_path();

    let (base, daemon, note) = match url {
        Some(url) => (url, None, "daemon given by --url".to_string()),
        None => {
            let addr = bind
                .parse()
                .unwrap_or_else(|e| fail(format!("bind address {bind}: {e}")));
            let daemon = daemon::Daemon::start(addr, home)
                .unwrap_or_else(|e| fail(format!("cannot start the daemon: {e}")));
            let note = match daemon.mode() {
                daemon::Mode::Embedded => format!("daemon in-process on {bind}"),
                daemon::Mode::External => format!("adopted the daemon on {bind}"),
            };
            (daemon.base_url(), Some(daemon), note)
        }
    };
    let events = client::subscribe(&base).unwrap_or_else(|e| fail(e));
    let api = Arc::new(client::Client::new(base));

    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(e) = fonts::register(cx) {
            fail(e);
        }
        cx.set_global(theme::Theme::for_appearance(cx.window_appearance()));
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                move |window, cx| {
                    window
                        .observe_window_appearance(|window, cx| {
                            cx.set_global(theme::Theme::for_appearance(window.appearance()));
                            window.refresh();
                        })
                        .detach();
                    cx.new(|cx| view::Root::new(api, Some(events), cx).with_daemon_note(note))
                },
            )
            .unwrap_or_else(|e| fail(format!("cannot open the window: {e}")));
        let _ = window;
        cx.activate(true);
    });

    if let Some(daemon) = daemon {
        daemon.stop();
    }
}
