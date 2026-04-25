use std::sync::Arc;

use tauri::State;

use crate::perms;
use crate::settings::{sanitize_nickname, Settings, SettingsStore};
use crate::stats::{self, LiveStats, MonthStats, TodayStats, TotalStats, WeekStats, WeekdayStats};
use crate::store::Store;
use crate::telemetry::{self, WorldStats};

pub struct AppState {
    pub store: Arc<Store>,
    pub settings: Arc<SettingsStore>,
}

#[tauri::command]
pub fn get_today(state: State<'_, AppState>) -> Result<TodayStats, String> {
    stats::today(&state.store).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_weekday_avg(state: State<'_, AppState>) -> Result<WeekdayStats, String> {
    stats::weekday_avg(&state.store).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_week(state: State<'_, AppState>, offset: i32) -> Result<WeekStats, String> {
    stats::week(&state.store, offset).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_month(state: State<'_, AppState>, offset: i32) -> Result<MonthStats, String> {
    stats::month(&state.store, offset).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_total(state: State<'_, AppState>) -> Result<TotalStats, String> {
    stats::total(&state.store).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_live(state: State<'_, AppState>) -> Result<LiveStats, String> {
    stats::live(&state.store).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_accessibility() -> bool {
    perms::check_accessibility()
}

#[tauri::command]
pub fn request_accessibility() -> bool {
    perms::request_accessibility()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.get()
}

#[tauri::command]
pub fn set_telemetry_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Settings, String> {
    state
        .settings
        .update(|s| s.telemetry_enabled = enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_nickname(
    state: State<'_, AppState>,
    nickname: Option<String>,
) -> Result<Settings, String> {
    let cleaned = sanitize_nickname(nickname);
    state
        .settings
        .update(|s| s.nickname = cleaned)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mark_consent_shown(
    state: State<'_, AppState>,
    consented: bool,
) -> Result<Settings, String> {
    state
        .settings
        .update(|s| {
            s.first_run_consent_shown = true;
            // 同意しなかった場合のみ telemetry を切る
            if !consented {
                s.telemetry_enabled = false;
            }
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_world_stats() -> Result<WorldStats, String> {
    telemetry::fetch_world().await.map_err(|e| e.to_string())
}
