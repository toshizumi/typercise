mod buffer;
mod commands;
mod keystroke;
mod perms;
mod stats;
mod store;

use std::sync::Arc;
use std::time::Duration;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

use crate::buffer::Buffer;
use crate::commands::AppState;
use crate::store::Store;

fn toggle_popover(app: &tauri::AppHandle) {
    use tauri_plugin_positioner::{Position, WindowExt};

    if let Some(window) = app.get_webview_window("popover") {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            _ => {
                let _ = window.move_window(Position::TrayCenter);
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .setup(|app| {
            let db_path = app
                .path()
                .app_data_dir()
                .map_err(|e| anyhow::anyhow!("app_data_dir: {e}"))?
                .join("db.sqlite");
            tracing::info!(db = %db_path.display(), "opening store");

            let store = Arc::new(Store::open(&db_path)?);
            let buffer = Arc::new(Buffer::new());

            buffer::spawn_flush_task(Arc::clone(&buffer), Arc::clone(&store));
            keystroke::start(Arc::clone(&buffer));

            // 初回起動時にアクセシビリティ権限プロンプトを発火させ、
            // システム設定の一覧に Typercise を登録させる。
            // 既に許可済みなら no-op（ダイアログは出ない）。
            let trusted = perms::request_accessibility();
            tracing::info!(trusted, "accessibility status at startup");

            app.manage(AppState {
                store: Arc::clone(&store),
            });

            let show_item = MenuItemBuilder::with_id("show", "統計を開く").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "終了").build(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&show_item, &quit_item])
                .build()?;

            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .title("⌨ 0")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "show" => toggle_popover(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_popover(tray.app_handle());
                    }
                })
                .build(app)?;

            if let Some(popover) = app.get_webview_window("popover") {
                let p = popover.clone();
                popover.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = p.hide();
                    }
                });
            }

            let app_handle = app.handle().clone();
            let store_for_tray = Arc::clone(&store);
            tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(Duration::from_secs(1));
                ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    ticker.tick().await;
                    if let Ok(t) = stats::live(&store_for_tray) {
                        if let Some(tray) = app_handle.tray_by_id("main") {
                            let _ = tray.set_title(Some(format!("🔥 {:.2} kcal", t.kcal)));
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_today,
            commands::get_weekday_avg,
            commands::get_week,
            commands::get_month,
            commands::get_total,
            commands::get_live,
            commands::check_accessibility,
            commands::request_accessibility,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

