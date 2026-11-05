use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use townhall_core::redact::redact;

const SHOW_ID: &str = "show";
const QUIT_ID: &str = "quit";

pub fn show_main(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        tracing::warn!(
            target: "townhall::app",
            event = "window.missing",
            "main window missing"
        );
        return;
    };
    for result in [win.show(), win.unminimize(), win.set_focus()] {
        if let Err(e) = result {
            tracing::warn!(
                target: "townhall::app",
                event = "window.show_failed",
                error = %redact(&e.to_string()),
                "cannot show window"
            );
        }
    }
    tracing::info!(target: "townhall::app", event = "window.show", "window shown");
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, SHOW_ID, "Show window", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("Townhall")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            SHOW_ID => show_main(app),
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    // Black on transparent, rendered as a template so it adapts to light and dark menu bars.
    builder = builder
        .icon(tauri::include_image!("icons/tray.png"))
        .icon_as_template(true);
    builder.build(app)?;
    Ok(())
}
