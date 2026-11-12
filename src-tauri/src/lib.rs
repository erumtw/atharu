//! Atharu.app (T18, D7): a Tauri 2 shell that runs the same axum router in-process and points one
//! system-WebView window at it. No Tauri IPC: the pages stay plain HTTP + htmx, like `cargo run`.
use atharu::{
    app,
    config::{self, FileConfig},
    db,
    layout::Layout,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{
    AppHandle, LogicalPosition, Manager, PhysicalPosition, Rect, RunEvent, TitleBarStyle,
    WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
    image::Image,
    menu::{Menu, MenuBuilder, MenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    window::{Effect, EffectState, EffectsBuilder},
};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

#[cfg(feature = "recording")]
pub mod recording;

/// Quick popover (T81) size in points; DESIGN.md §6 "Quick popover" and quick.css `.menu.quick` match it.
const QUICK: (f64, f64) = (360.0, 480.0);

/// Popover bookkeeping between tray clicks, the shortcut and blur.
#[derive(Default)]
struct Quick {
    /// Last tray icon rect: the shortcut has no click to anchor to, so it opens where the tray last did.
    tray: Mutex<Option<Rect>>,
    /// When blur last hid the popover. Clicking the tray to close it blurs it first; without this the same click
    /// would open it again.
    hidden_at: Mutex<Option<Instant>>,
}

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
                        toggle_quick(app, None);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let url = serve(app.handle())?;
            app.manage(Quick::default());
            let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url.clone()))
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
                // Settings › About's Restart links here after a DB change: the new DB opens only at start.
                .on_navigation({
                    let h = app.handle().clone();
                    move |url| {
                        if to_browser(url) {
                            return false;
                        }
                        if url.path() == "/app/restart" {
                            h.request_restart();
                            return false;
                        }
                        true
                    }
                })
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
            quick(app.handle(), &url)?;
            tray(app.handle())?;
            tray_state(app.handle());
            // Meetings also start / end / record from a browser on the shared DB (D52), with no request
            // through this app, so the icon polls too (one cheap query a second: the recording clock ticks).
            let h = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let h2 = h.clone();
                    let _ = h.run_on_main_thread(move || tray_state(&h2));
                }
            });
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

/// An http(s) URL off this machine goes to the default browser, a System Settings pane to System Settings
/// (`true` = handed off, cancel it here).
fn to_browser(url: &tauri::Url) -> bool {
    let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    // "Open System Settings" on the permission steps: a pane URL only Launch Services can open.
    let settings = url.scheme() == "x-apple.systempreferences";
    if !settings && (local || !matches!(url.scheme(), "http" | "https")) {
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
    // Not ./data like `cargo run` (that is wherever Finder launched us from): see config::resolve_db (D52).
    let app_dir = handle.path().app_data_dir()?;
    let pointer = app_dir.join(config::DB_POINTER);
    let db_path = config::resolve_db(
        std::env::var_os("ATHARU_DB").map(PathBuf::from),
        &app_dir,
        handle.path().home_dir().ok().as_deref(),
    );
    // SAFETY: setup runs on the main thread before the server starts; nothing of ours reads the env yet.
    // A restart inherits it unchanged, which is the same path again.
    unsafe { std::env::set_var(config::DB_POINTER_ENV, &pointer) };
    let conn = db::open(&db_path)?;
    // Whatever was recording when the app last quit has no process now (D33 crash-safety).
    #[cfg(feature = "recording")]
    atharu::recording::interrupt_orphans(&conn)?;
    let mut state = app::AppState::new(conn);
    // The model is the app's, not the DB's: it stays put when ATHARU_DB points elsewhere.
    #[cfg(feature = "recording")]
    let mac = {
        let m = Arc::new(atharu::recording::Mac::new(app_dir.join("models")));
        let _ = m.ask_screen.set(Box::new(recording::screen_or_ask));
        state.mac = Some(m.clone());
        m
    };
    if let Some(dir) = db_path.parent() {
        state.settings = Arc::new(FileConfig::load(&dir.join("config.toml")));
    }
    look_icon(&state);
    // The tray menu reads the running meeting and recent series through it.
    handle.manage(state.clone());

    // Bound here (std, blocking) so the port is known before the window opens.
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let url = format!("http://{}/", listener.local_addr()?).parse()?;
    #[cfg(feature = "recording")]
    {
        let rec = Arc::new(recording::Recorder::new(listener.local_addr()?, mac));
        handle.manage(rec.clone());
        // The DB is shared (D52): a browser on the server can End the meeting without any POST reaching us,
        // so poll too. Our own Record / Stop / End still sync at once through the middleware below.
        let st = state.clone();
        std::thread::spawn(move || {
            loop {
                rec.sync(&st);
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        });
    }

    let (h, st) = (handle.clone(), state.clone());
    // The look picker is a plain POST /settings; re-read the look after it so the Dock follows. Any meeting write
    // (start, end, delete, undo, from either window or the menu) rebuilds the tray menu, which names them.
    let router = app::router(state.clone()).layer(axum::middleware::from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            let (h, st) = (h.clone(), st.clone());
            async move {
                let post = req.method() == axum::http::Method::POST;
                let path = req.uri().path();
                let look_saved = post && path == "/settings";
                let meetings = post && path.starts_with("/meetings");
                // A write from the popover (or the tray menu, which posts through it): the main window would keep
                // showing the old state, so tell it to re-fetch its live parts (`atharu-sync` refreshers).
                let from_quick = post
                    && req
                        .headers()
                        .get(axum::http::header::REFERER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|r| r.parse::<axum::http::Uri>().ok())
                        .is_some_and(|u| u.path().starts_with("/quick"));
                let res = next.run(req).await;
                if from_quick && let Some(w) = h.get_webview_window("main") {
                    let _ = w.eval("document.body.dispatchEvent(new CustomEvent('atharu-sync'))");
                }
                if meetings {
                    #[cfg(feature = "recording")]
                    sync_recording(&h);
                }
                if look_saved {
                    let st = st.clone();
                    let _ = h.run_on_main_thread(move || look_icon(&st));
                }
                if meetings {
                    let h2 = h.clone();
                    let _ = h.run_on_main_thread(move || refresh_menu(&h2));
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

/// Start / stop capture to match the recording rows (Record, Stop, End just changed one). Blocking SCK calls,
/// so off the async runtime.
#[cfg(feature = "recording")]
fn sync_recording(h: &AppHandle) {
    let (rec, st) = (
        h.state::<Arc<recording::Recorder>>().inner().clone(),
        h.state::<app::AppState>().inner().clone(),
    );
    std::thread::spawn(move || rec.sync(&st));
}

fn tray(handle: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id("main")
        // MENU.I from docs/design/icon/options.html, black on clear at 22 pt @2x; macOS tints templates.
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("Atharu")
        // Left click = the popover; the menu is the right click only.
        .menu(&tray_menu(handle)?)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = e
            {
                toggle_quick(tray.app_handle(), Some(rect));
            }
        })
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show(app),
            "quit" => app.exit(0),
            id => {
                // Through the popover's webview, so the write takes the same HTTP path (and middleware) as a click.
                let post = |path: String, body: String| {
                    if let Some(w) = app.get_webview_window("quick") {
                        let _ = w.eval(format!(
                            "fetch({path:?}, {{method: 'POST', body: new URLSearchParams({body})}})"
                        ));
                    }
                };
                if let Some(name) = id.strip_prefix("meet:") {
                    // Series names are slugs ([a-z0-9-]); Debug quoting is a valid JS string for them.
                    post("/meetings".into(), format!("{{name: {name:?}}}"));
                } else if let Some(mid) = id.strip_prefix("end:") {
                    post(format!("/meetings/{mid}/end"), "{}".into());
                }
            }
        })
        .build(handle)?;
    Ok(())
}

/// Right-click menu: Open Atharu · Start meeting › recent series (or End meeting · name) · Quit.
fn tray_menu(h: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let atharu::quick::TrayMenu {
        running, series, ..
    } = {
        let st = h.state::<app::AppState>();
        let conn = st.conn();
        atharu::quick::tray_menu(&conn).unwrap_or_default()
    };
    let mut menu = MenuBuilder::new(h).text("open", "Open Atharu");
    menu = match running {
        Some((id, name)) => menu.text(format!("end:{id}"), format!("End meeting · {name}")),
        None => {
            let mut sub = SubmenuBuilder::new(h, "Start meeting").enabled(!series.is_empty());
            for s in &series {
                sub = sub.text(format!("meet:{s}"), s);
            }
            menu.item(&sub.build()?)
        }
    };
    let quit = MenuItem::with_id(h, "quit", "Quit Atharu", true, Some("CmdOrCtrl+Q"))?;
    menu.separator().item(&quit).build()
}

fn refresh_menu(h: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (h.tray_by_id("main"), tray_menu(h)) {
        let _ = tray.set_menu(Some(menu));
    }
    tray_state(h);
}

/// Tray icon state: 0 idle, 1 meeting running, 2 recording. Only a change touches the icon.
static TRAY_STATE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Menu-bar icon shows the meeting state: a badge dot while a meeting runs, the whole mark red while recording,
/// with the recording's elapsed time as the item's title. Red can't be a template image (macOS would tint it), so
/// that one is drawn in #ff3b30, readable on light and dark bars.
fn tray_state(h: &AppHandle) {
    let (state, tip, title) = {
        let st = h.state::<app::AppState>();
        let m = atharu::quick::tray_menu(&st.conn()).unwrap_or_default();
        match (m.running, m.recording) {
            (Some((_, name)), Some(started)) => {
                let secs = atharu::recording::secs_between(&started, &atharu::recording::now());
                (
                    2,
                    format!("Atharu · Recording · {name}"),
                    Some(atharu::recording::elapsed(secs)),
                )
            }
            (Some((_, name)), None) => (1, format!("Atharu · Meeting · {name}"), None),
            (None, _) => (0, "Atharu".to_string(), None),
        }
    };
    let Some(tray) = h.tray_by_id("main") else {
        return;
    };
    let changed = TRAY_STATE.swap(state, std::sync::atomic::Ordering::Relaxed) != state;
    if changed {
        let bytes: &[u8] = match state {
            2 => include_bytes!("../icons/tray-rec.png"),
            1 => include_bytes!("../icons/tray-meeting.png"),
            _ => include_bytes!("../icons/tray.png"),
        };
        if let Ok(img) = Image::from_bytes(bytes) {
            let _ = tray.set_icon(Some(img));
            let _ = tray.set_icon_as_template(state != 2);
        }
    }
    // Every tick while recording (the clock moves); otherwise only on a change of state. After the icon: a new image
    // must not reset the title.
    match title {
        Some(t) => red_title(&tray, t),
        None if changed => {
            let _ = tray.set_title(None::<&str>);
        }
        None => {}
    }
    if changed {
        let _ = tray.set_tooltip(Some(tip));
    }
}

/// The recording clock in system red, like the icon. Tauri's set_title is plain text (the bar's own colour),
/// so this sets the status item button's attributed title; tabular digits keep the item from jiggling each second.
#[cfg(target_os = "macos")]
fn red_title(tray: &tauri::tray::TrayIcon, text: String) {
    let _ = tray.with_inner_tray_icon(move |inner| {
        use objc2::{AllocAnyThread, MainThreadMarker, runtime::AnyObject};
        use objc2_app_kit::{
            NSColor, NSFont, NSFontAttributeName, NSFontWeightRegular,
            NSForegroundColorAttributeName,
        };
        use objc2_foundation::{NSAttributedString, NSDictionary, NSString};
        let (Some(item), Some(mtm)) = (inner.ns_status_item(), MainThreadMarker::new()) else {
            return;
        };
        let Some(button) = item.button(mtm) else {
            return;
        };
        let color = NSColor::systemRedColor();
        let font =
            unsafe { NSFont::monospacedDigitSystemFontOfSize_weight(13.0, NSFontWeightRegular) };
        let (kc, kf) = unsafe { (NSForegroundColorAttributeName, NSFontAttributeName) };
        let attrs = NSDictionary::<NSString, AnyObject>::from_slices(
            &[kc, kf],
            &[color.as_ref() as &AnyObject, font.as_ref() as &AnyObject],
        );
        let s = unsafe {
            NSAttributedString::initWithString_attributes(
                NSAttributedString::alloc(),
                &NSString::from_str(&format!(" {text}")),
                Some(&attrs),
            )
        };
        button.setAttributedTitle(&s);
    });
}

#[cfg(not(target_os = "macos"))]
fn red_title(tray: &tauri::tray::TrayIcon, text: String) {
    let _ = tray.set_title(Some(text));
}

fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// The popover window: built hidden once, then shown under the tray icon (`toggle_quick`).
fn quick(app: &AppHandle, base: &tauri::Url) -> Result<(), Box<dyn std::error::Error>> {
    let h = app.clone();
    let win = WebviewWindowBuilder::new(app, "quick", WebviewUrl::External(base.join("quick")?))
        .title("Atharu")
        .inner_size(QUICK.0, QUICK.1)
        .resizable(false)
        .decorations(false)
        // html.q is transparent: the native material is the panel's blur (quick.css drops the CSS one).
        .transparent(true)
        .shadow(true)
        .skip_taskbar(true)
        .always_on_top(true)
        // Opens on the current Space instead of jumping to the one it was built on.
        .visible_on_all_workspaces(true)
        .visible(false)
        // Our glass is light-only: keep the material light under a dark menu bar.
        .theme(Some(tauri::Theme::Light))
        .effects(
            EffectsBuilder::new()
                .effect(Effect::Popover)
                .state(EffectState::Active)
                .radius(18.0)
                .build(),
        )
        .initialization_script("document.documentElement.dataset.app = 'mac'")
        .on_navigation(move |url| quick_nav(&h, url))
        .on_new_window(|url, _| {
            if to_browser(&url) {
                tauri::webview::NewWindowResponse::Deny
            } else {
                tauri::webview::NewWindowResponse::Allow
            }
        })
        .build()?;
    let h = app.clone();
    win.on_window_event(move |e| {
        if let WindowEvent::Focused(false) = e {
            hide_quick(&h);
        }
    });
    Ok(())
}

/// The popover only ever shows /quick. Esc navigates to /quick/close (quick.ts) = hide; any other app page
/// (Open Atharu, a meeting, + N more) opens in the main window; off-machine links go to the browser.
fn quick_nav(h: &AppHandle, url: &tauri::Url) -> bool {
    if to_browser(url) {
        return false;
    }
    if !matches!(url.scheme(), "http" | "https") {
        return true;
    }
    match url.path() {
        "/quick/close" => hide_quick(h),
        "/quick" => return true,
        _ => {
            if let Some(m) = h.get_webview_window("main") {
                let _ = m.navigate(url.clone());
            }
            show(h);
            hide_quick(h);
        }
    }
    false
}

fn hide_quick(h: &AppHandle) {
    if let Some(w) = h.get_webview_window("quick")
        && w.is_visible().unwrap_or(false)
    {
        let _ = w.hide();
        *h.state::<Quick>().hidden_at.lock().unwrap() = Some(Instant::now());
    }
}

/// Tray left click (`tray` = its rect) or the shortcut (`None`): open the popover focused on the line, or close it.
fn toggle_quick(h: &AppHandle, tray: Option<Rect>) {
    let Some(w) = h.get_webview_window("quick") else {
        return;
    };
    let q = h.state::<Quick>();
    if tray.is_some() {
        *q.tray.lock().unwrap() = tray;
    }
    if w.is_visible().unwrap_or(false) {
        hide_quick(h);
        return;
    }
    let just_hidden = q
        .hidden_at
        .lock()
        .unwrap()
        .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
    if tray.is_some() && just_hidden {
        return;
    }
    place(&w, *q.tray.lock().unwrap());
    // Fresh to-dos and meeting state each time (the main window may have changed them); #line is autofocus.
    let _ = w.eval("location.replace('/quick')");
    let _ = w.show();
    let _ = w.set_focus();
}

/// 6 pt under the tray icon, left edge on it, clamped 8 pt inside the screen's right edge. Without a tray rect yet
/// (shortcut first), top-right of the main screen, under the menu bar.
fn place(w: &WebviewWindow, tray: Option<Rect>) {
    let guess = w.scale_factor().unwrap_or(2.0);
    let anchor = tray.map(|r| {
        let p = r.position.to_physical::<f64>(guess);
        let s = r.size.to_physical::<f64>(guess);
        (p.x, p.y + s.height)
    });
    let monitor = match anchor {
        Some((x, y)) => w.monitor_from_point(x, y).ok().flatten(),
        None => None,
    }
    .or_else(|| w.primary_monitor().ok().flatten());
    let Some(m) = monitor else {
        return;
    };
    let k = m.scale_factor();
    let area = m.work_area();
    let right = (area.position.x + area.size.width as i32) as f64 - (QUICK.0 + 8.0) * k;
    let (x, y) = match anchor {
        Some((x, bottom)) => (x.min(right), bottom + 6.0 * k),
        None => (right, area.position.y as f64 + 6.0 * k),
    };
    let _ = w.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
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
