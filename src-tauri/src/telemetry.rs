use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{Duration as CDuration, Local};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::settings::SettingsStore;
use crate::stats::day_summary;
use crate::store::Store;

pub const API_BASE: &str = "https://typercise-api.typersize.workers.dev";

#[derive(Serialize, Debug)]
struct ReportPayload<'a> {
    client_id: &'a str,
    date: String,
    keys: u64,
    corrections: u64,
    kcal: f64,
    peak_kpm: u32,
    avg_kpm: u32,
    active_minutes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    nickname: Option<&'a str>,
    app_version: &'a str,
    os_version: String,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
pub struct WorldStats {
    pub as_of: i64,
    pub participants_7d: u64,
    pub today: WorldToday,
    pub all_time: WorldAllTime,
    pub top_today_keys: Vec<RankKeys>,
    pub top_today_peak_kpm: Vec<RankPeak>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
pub struct WorldToday {
    pub date: String,
    pub keys: u64,
    pub kcal: f64,
    pub active_users: u64,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
pub struct WorldAllTime {
    pub keys: u64,
    pub kcal: f64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RankKeys {
    pub nickname: String,
    pub keys: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RankPeak {
    pub nickname: String,
    pub peak_kpm: u32,
}

#[cfg(target_os = "macos")]
fn detect_os_version() -> String {
    std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "macos".to_string())
}

#[cfg(not(target_os = "macos"))]
fn detect_os_version() -> String {
    std::env::consts::OS.to_string()
}

/// 前日分の集計を 1 度だけサーバへ送る。
/// 既に送信済み・テレメトリ無効・データなし のいずれかなら何もしない。
/// Ok(true) を返した場合のみ実送信が成功。
pub async fn upload_yesterday(
    settings: Arc<SettingsStore>,
    store: Arc<Store>,
) -> Result<bool> {
    let s = settings.get();
    if !s.telemetry_enabled {
        return Ok(false);
    }
    if !s.first_run_consent_shown {
        return Ok(false);
    }

    let today_local = Local::now().date_naive();
    let yesterday = today_local - CDuration::days(1);
    let yesterday_str = yesterday.format("%Y-%m-%d").to_string();

    if let Some(last) = s.last_uploaded_date.as_deref() {
        if last >= yesterday_str.as_str() {
            return Ok(false);
        }
    }

    let summary = day_summary(&store, yesterday)?;
    if summary.keys == 0 && summary.corrections == 0 {
        // 空の日も「送信済み扱い」にして次回は再評価しない
        settings.update(|x| x.last_uploaded_date = Some(yesterday_str.clone()))?;
        return Ok(false);
    }

    let payload = ReportPayload {
        client_id: &s.client_id,
        date: yesterday_str.clone(),
        keys: summary.keys,
        corrections: summary.corrections,
        kcal: summary.kcal,
        peak_kpm: summary.peak_kpm,
        avg_kpm: summary.avg_kpm,
        active_minutes: summary.active_minutes,
        nickname: s.nickname.as_deref(),
        app_version: env!("CARGO_PKG_VERSION"),
        os_version: detect_os_version(),
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let url = format!("{}/api/v1/report", API_BASE);
    let resp = client.post(&url).json(&payload).send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("report failed: status={} body={}", status, body);
    }

    settings.update(|x| x.last_uploaded_date = Some(yesterday_str))?;
    tracing::info!(date = %payload.date, keys = payload.keys, "telemetry uploaded");
    Ok(true)
}

/// 起動時 + 1 時間毎に upload_yesterday を呼ぶバックグラウンドタスク。
pub fn spawn_periodic(settings: Arc<SettingsStore>, store: Arc<Store>) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(3600));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await; // 初回は即tick
            match upload_yesterday(Arc::clone(&settings), Arc::clone(&store)).await {
                Ok(true) => {}
                Ok(false) => tracing::debug!("telemetry skip"),
                Err(e) => tracing::warn!(error = ?e, "telemetry upload failed"),
            }
        }
    });
}

pub async fn fetch_world() -> Result<WorldStats> {
    let url = format!("{}/api/v1/world", API_BASE);
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("world failed: {}", resp.status());
    }
    let stats: WorldStats = resp.json().await?;
    Ok(stats)
}
