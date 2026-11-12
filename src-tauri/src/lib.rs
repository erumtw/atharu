//! Atharu.app (T18, D7): a Tauri 2 shell that runs the same axum router in-process and points one
//! system-WebView window at it. No Tauri IPC: the pages stay plain HTTP + htmx, like `cargo run`.
use atharu::{app, config::FileConfig, db, layout::Layout};
use std::{path::PathBuf, sync::Arc};
use tauri::{
    AppHandle, LogicalPosition, Manager, RunEvent, TitleBarStyle, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
    image::Image,
    menu::{MenuBuilder, MenuItem},
    tray::TrayIconBuilder,
    window::{Effect, EffectState, EffectsBuilder},
};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

/// Quick-capture shortcut, Ctrl+Opt+Space: Spotlight, input sources and Raycast/Alfred defaults all use
/// Cmd/Ctrl+Space, not this. Change it here only; the README names it.
const CAPTURE: (Modifiers, Code) = (Modifiers::CONTROL.union(Modifiers::ALT), Code::Space);

/// Traffic lights inside the glass sidebar (x = close button's left edge, y ≈ its centre); looks.css
/// `html[data-app=mac]` leaves this room, so move both together.
const LIGHTS: (f64, f64) = (20.0, 30.0);

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcut(Shortcut::new(Some(CAPTURE.0), CAPTURE.1))
                .expect("valid shortcut")
                .with_handler(|app, _, ev| {
                    if ev.state == ShortcutState::Pressed {
                        capture(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let url = serve(app.handle())?;
            let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Atharu")
                .inner_size(1180.0, 780.0)
                .min_inner_size(390.0, 500.0)
                // No title bar strip: the wallpaper runs to the top and the lights float over the sidebar.
                .title_bar_style(TitleBarStyle::Overlay)
                .hidden_title(true)
                .traffic_light_position(LogicalPosition::new(LIGHTS.0, LIGHTS.1))
                // Transparent webview over the sidebar material: looks.css lets the desktop through (data-app).
                .transparent(true)
                .effects(
                    EffectsBuilder::new()
                        .effect(Effect::Sidebar)
                        .state(EffectState::FollowsWindowActiveState)
                        .build(),
                )
                .initialization_script("document.documentElement.dataset.app = 'mac'")
                // Jira / PR / Slack links belong in the real browser, not inside the app window.
                .on_navigation(|url| !to_browser(url))
                .on_new_window(|url, _| {
                    if to_browser(&url) {
                        tauri::webview::NewWindowResponse::Deny
                    } else {
                        tauri::webview::NewWindowResponse::Allow
                    }
                })
                .build()?;
            // A menu-bar app: closing hides, the tray / shortcut / Dock click bring it back.
            let w = win.clone();
            win.on_window_event(move |e| {
                if let WindowEvent::CloseRequested { api, .. } = e {
                    api.prevent_close();
                    let _ = w.hide();
                }
            });
            tray(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("start Atharu")
        .run(|app, e| {
            if let RunEvent::Reopen { .. } = e {
                show(app);
            }
        });
}

/// An http(s) URL off this machine goes to the default browser (`true` = handed off, cancel it here).
fn to_browser(url: &tauri::Url) -> bool {
    let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if local || !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    // `open` hands the URL to Launch Services; no shell, so the URL can't inject a command.
    // Reaped on a thread: waiting here would stall the main thread, not waiting leaves a zombie.
    if let Ok(mut child) = std::process::Command::new("open").arg(url.as_str()).spawn() {
        std::thread::spawn(move || child.wait());
    }
    true
}

/// Open the DB, start axum on a free 127.0.0.1 port, set the Dock icon; returns the window URL.
fn serve(handle: &AppHandle) -> Result<tauri::Url, Box<dyn std::error::Error>> {
    // Same precedence as `cargo run`: ATHARU_DB wins, else the app's own folder (not ./data, which is
    // wherever Finder launched us from).
    let db_path = match std::env::var_os("ATHARU_DB") {
        Some(p) => PathBuf::from(p),
        None => handle.path().app_data_dir()?.join("atharu.db"),
    };
    let conn = db::open(&db_path)?;
    let mut state = app::AppState::new(conn);
    if let Some(dir) = db_path.parent() {
        state.settings = Arc::new(FileConfig::load(&dir.join("config.toml")));
    }
    look_icon(&state);

    // Bound here (std, blocking) so the port is known before the window opens.
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let url = format!("http://{}/", listener.local_addr()?).parse()?;

    let (h, st) = (handle.clone(), state.clone());
    // The look picker is a plain POST /settings; re-read the look after it so the Dock follows.
    let router = app::router(state.clone()).layer(axum::middleware::from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            let (h, st) = (h.clone(), st.clone());
            async move {
                let look_saved =
                    req.method() == axum::http::Method::POST && req.uri().path() == "/settings";
                let res = next.run(req).await;
                if look_saved {
                    let st = st.clone();
                    let _ = h.run_on_main_thread(move || look_icon(&st));
                }
                res
            }
        },
    ));
    tauri::async_runtime::spawn(async move {
        app::spawn_refreshers(&state);
        let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
        if let Err(e) = axum::serve(listener, router).await {
            eprintln!("atharu server stopped: {e}");
        }
    });
    Ok(url)
}

fn tray(handle: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(handle, "open", "Open Atharu", true, None::<&str>)?;
    let cap = MenuItem::with_id(handle, "capture", "Quick capture", true, None::<&str>)?;
    let quit = MenuItem::with_id(handle, "quit", "Quit Atharu", true, None::<&str>)?;
    let menu = MenuBuilder::new(handle)
        .items(&[&open, &cap])
        .separator()
        .item(&quit)
        .build()?;
    TrayIconBuilder::with_id("main")
        // MENU.I from docs/design/icon/options.html, black on clear at 22 pt @2x; macOS tints templates.
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("Atharu")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show(app),
            "capture" => capture(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(handle)?;
    Ok(())
}

fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Bring the window up on the capture bar. Today and To-dos have `#line`; elsewhere go to Today,
/// whose `#line` is `autofocus`.
fn capture(app: &AppHandle) {
    show(app);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval("(document.getElementById('line') || location.assign('/'))?.focus()");
    }
}

/// Dock icon + bundle icon = the current look (spec "Looks").
fn look_icon(state: &app::AppState) {
    let look = Layout::load(&state.conn(), "", "", vec![]).map_or("sumi", |l| l.look);
    #[cfg(target_os = "macos")]
    {
        let png: &[u8] = match look {
            "sora" => include_bytes!("../icons/looks/sora.png"),
            "wakaba" => include_bytes!("../icons/looks/wakaba.png"),
            "glass" => include_bytes!("../icons/looks/glass.png"),
            _ => include_bytes!("../icons/looks/sumi.png"),
        };
        set_icons(look, png);
    }
}

/// Sets the running Dock icon, then writes the look onto the .app itself so Dock and Finder keep it after Quit.
// ponytail: setIcon:forFile: adds `Icon\r` + a FinderInfo flag inside the bundle, which breaks a code signature;
// fine while the build is unsigned. If it is ever signed, switch to alternate app icons (Assets.car) instead.
#[cfg(target_os = "macos")]
fn set_icons(look: &str, png: &[u8]) {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage, NSWorkspace, NSWorkspaceIconCreationOptions};
    use objc2_foundation::{NSData, NSString};
    // AppKit is main-thread only; callers hop there via run_on_main_thread.
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(img) = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(png)) else {
        return;
    };
    // SAFETY: on the main thread (checked above) with a valid, retained NSImage.
    unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&img)) };

    // Only a real bundle (`cargo tauri dev` runs a bare binary). The marker lives in the bundle so a freshly
    // copied .app (bundled Sumi icon, no marker) gets the look reapplied on its first start.
    let Some(bundle) = std::env::current_exe().ok().and_then(|exe| {
        exe.ancestors()
            .nth(3)
            .filter(|p| p.extension().is_some_and(|e| e == "app"))
            .map(PathBuf::from)
    }) else {
        return;
    };
    let marker = bundle.join(".atharu-look");
    let applied = std::fs::read_to_string(&marker).unwrap_or_else(|_| "sumi".into());
    if applied == look {
        return;
    }
    // Sumi = the bundled icon: clear the custom one rather than stamp a copy of it.
    let custom = (look != "sumi").then_some(&*img);
    let path = NSString::from_str(&bundle.to_string_lossy());
    if NSWorkspace::sharedWorkspace().setIcon_forFile_options(
        custom,
        &path,
        NSWorkspaceIconCreationOptions::empty(),
    ) {
        // A read-only /Applications copy just keeps its icon; the Dock icon above still follows the look.
        let _ = std::fs::write(&marker, look);
    }
}
