use std::sync::Arc;

use tauri::State;

use crate::perms;
use crate::stats::{self, LiveStats, MonthStats, TodayStats, TotalStats, WeekStats, WeekdayStats};
use crate::store::Store;

pub struct AppState {
    pub store: Arc<Store>,
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
