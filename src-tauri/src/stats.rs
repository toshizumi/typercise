use std::collections::HashMap;

use chrono::{Datelike, Duration as CDuration, Local, NaiveDate, TimeZone, Timelike};
use serde::Serialize;

use crate::store::Store;

pub const KCAL_PER_KEY: f64 = 0.0014;
const WEEKDAY_LOOKBACK_DAYS: i64 = 56; // 8 weeks

#[derive(Serialize, Clone)]
pub struct TodayStats {
    pub total: u64,
    pub per_hour: [u64; 24],
    pub corrections: u64,
    pub rework: f64,
    pub kcal: f64,
    /// 打鍵があった分の数（活動時間）
    pub active_minutes: u32,
    /// 活動中の平均速度（keys per minute、訂正も含めた総打鍵ベース）
    pub avg_kpm: u32,
    /// ピーク速度（1分単位の最大打鍵数）
    pub peak_kpm: u32,
}

#[derive(Serialize, Clone)]
pub struct WeekdayStats {
    pub avg: [u64; 7], // Mon..Sun
}

#[derive(Serialize, Clone)]
pub struct WeekStats {
    pub total: u64,
    pub per_day: [u64; 7],
    pub start_date: String,
    pub kcal: f64,
}

#[derive(Serialize, Clone)]
pub struct MonthStats {
    pub total: u64,
    pub per_day: Vec<u64>,
    pub year: i32,
    pub month: u32,
    pub kcal: f64,
}

#[derive(Serialize, Clone)]
pub struct TotalStats {
    pub total: u64,
    pub since_ts: Option<i64>,
    pub kcal: f64,
}

#[derive(Serialize, Clone)]
pub struct LiveStats {
    pub today: u64,
    pub corrections: u64,
    pub rework: f64,
    pub kcal: f64,
}

/// 訂正率 = 訂正キー数 / (通常キー + 訂正キー)。分母 0 の時は 0.0。
pub fn rework_rate(keys: u64, corrections: u64) -> f64 {
    let denom = keys + corrections;
    if denom == 0 {
        0.0
    } else {
        corrections as f64 / denom as f64
    }
}

fn local_midnight(date: NaiveDate) -> i64 {
    let naive = date.and_hms_opt(0, 0, 0).unwrap();
    Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .expect("local midnight")
        .timestamp()
}

fn minute_of(ts: i64) -> i64 {
    ts.div_euclid(60)
}

#[derive(Serialize, Clone, Debug)]
pub struct DaySummary {
    pub date: String, // YYYY-MM-DD
    pub keys: u64,
    pub corrections: u64,
    pub kcal: f64,
    pub peak_kpm: u32,
    pub avg_kpm: u32,
    pub active_minutes: u32,
}

pub fn day_summary(store: &Store, date: NaiveDate) -> anyhow::Result<DaySummary> {
    let start = minute_of(local_midnight(date));
    let end = minute_of(local_midnight(date + CDuration::days(1)));
    let rows = store.rows_in_range(start, end)?;
    let mut keys = 0u64;
    let mut corrections = 0u64;
    let mut active_minutes = 0u32;
    let mut peak_kpm = 0u32;
    let mut total_keystrokes = 0u64;
    for (_m, c, r) in rows {
        keys += c as u64;
        corrections += r as u64;
        let minute_keystrokes = (c as u64 + r as u64) as u32;
        if minute_keystrokes > 0 {
            active_minutes += 1;
            total_keystrokes += minute_keystrokes as u64;
            if minute_keystrokes > peak_kpm {
                peak_kpm = minute_keystrokes;
            }
        }
    }
    let avg_kpm = if active_minutes > 0 {
        (total_keystrokes / active_minutes as u64) as u32
    } else {
        0
    };
    Ok(DaySummary {
        date: date.format("%Y-%m-%d").to_string(),
        keys,
        corrections,
        kcal: keys as f64 * KCAL_PER_KEY,
        peak_kpm,
        avg_kpm,
        active_minutes,
    })
}

pub fn today(store: &Store) -> anyhow::Result<TodayStats> {
    let today = Local::now().date_naive();
    let start = minute_of(local_midnight(today));
    let end = minute_of(local_midnight(today + CDuration::days(1)));
    let rows = store.rows_in_range(start, end)?;
    let mut per_hour = [0u64; 24];
    let mut total = 0u64;
    let mut corrections = 0u64;
    let mut active_minutes = 0u32;
    let mut peak_kpm = 0u32;
    let mut total_keystrokes = 0u64; // keys + corrections
    for (m, c, r) in rows {
        if let Some(dt) = Local.timestamp_opt(m * 60, 0).single() {
            let h = dt.hour() as usize;
            per_hour[h] += c as u64;
        }
        total += c as u64;
        corrections += r as u64;
        let minute_keystrokes = (c as u64 + r as u64) as u32;
        if minute_keystrokes > 0 {
            active_minutes += 1;
            total_keystrokes += minute_keystrokes as u64;
            if minute_keystrokes > peak_kpm {
                peak_kpm = minute_keystrokes;
            }
        }
    }
    let avg_kpm = if active_minutes > 0 {
        (total_keystrokes / active_minutes as u64) as u32
    } else {
        0
    };
    Ok(TodayStats {
        total,
        per_hour,
        corrections,
        rework: rework_rate(total, corrections),
        kcal: total as f64 * KCAL_PER_KEY,
        active_minutes,
        avg_kpm,
        peak_kpm,
    })
}

pub fn weekday_avg(store: &Store) -> anyhow::Result<WeekdayStats> {
    let today = Local::now().date_naive();
    let start_date = today - CDuration::days(WEEKDAY_LOOKBACK_DAYS);
    let start = minute_of(local_midnight(start_date));
    let end = minute_of(local_midnight(today + CDuration::days(1)));
    let rows = store.rows_in_range(start, end)?;

    let mut daily: HashMap<NaiveDate, u64> = HashMap::new();
    for (m, c, _r) in rows {
        if let Some(dt) = Local.timestamp_opt(m * 60, 0).single() {
            let d = dt.date_naive();
            *daily.entry(d).or_insert(0) += c as u64;
        }
    }

    let mut sums = [0u64; 7];
    let mut counts = [0u64; 7];
    for (date, n) in daily {
        let wd = date.weekday().num_days_from_monday() as usize;
        sums[wd] += n;
        counts[wd] += 1;
    }
    let mut avg = [0u64; 7];
    for i in 0..7 {
        if counts[i] > 0 {
            avg[i] = sums[i] / counts[i];
        }
    }
    Ok(WeekdayStats { avg })
}

pub fn week(store: &Store, offset: i32) -> anyhow::Result<WeekStats> {
    let today = Local::now().date_naive();
    let monday_this = today - CDuration::days(today.weekday().num_days_from_monday() as i64);
    let week_start = monday_this + CDuration::weeks(offset as i64);
    let start = minute_of(local_midnight(week_start));
    let end = minute_of(local_midnight(week_start + CDuration::days(7)));
    let rows = store.rows_in_range(start, end)?;

    let mut per_day = [0u64; 7];
    let mut total = 0u64;
    for (m, c, _r) in rows {
        if let Some(dt) = Local.timestamp_opt(m * 60, 0).single() {
            let d = dt.date_naive();
            let idx = d.signed_duration_since(week_start).num_days() as usize;
            if idx < 7 {
                per_day[idx] += c as u64;
                total += c as u64;
            }
        }
    }
    Ok(WeekStats {
        total,
        per_day,
        start_date: week_start.format("%Y-%m-%d").to_string(),
        kcal: total as f64 * KCAL_PER_KEY,
    })
}

pub fn month(store: &Store, offset: i32) -> anyhow::Result<MonthStats> {
    let today = Local::now().date_naive();
    let mut m = today.month() as i32 + offset;
    let mut y = today.year();
    while m < 1 {
        m += 12;
        y -= 1;
    }
    while m > 12 {
        m -= 12;
        y += 1;
    }
    let target_year = y;
    let target_month = m as u32;

    let first = NaiveDate::from_ymd_opt(target_year, target_month, 1).unwrap();
    let next_first = if target_month == 12 {
        NaiveDate::from_ymd_opt(target_year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(target_year, target_month + 1, 1).unwrap()
    };
    let days = next_first.signed_duration_since(first).num_days() as usize;
    let start = minute_of(local_midnight(first));
    let end = minute_of(local_midnight(next_first));
    let rows = store.rows_in_range(start, end)?;

    let mut per_day = vec![0u64; days];
    let mut total = 0u64;
    for (m, c, _r) in rows {
        if let Some(dt) = Local.timestamp_opt(m * 60, 0).single() {
            let d = dt.date_naive();
            let idx = d.signed_duration_since(first).num_days() as usize;
            if idx < days {
                per_day[idx] += c as u64;
                total += c as u64;
            }
        }
    }
    Ok(MonthStats {
        total,
        per_day,
        year: target_year,
        month: target_month,
        kcal: total as f64 * KCAL_PER_KEY,
    })
}

pub fn total(store: &Store) -> anyhow::Result<TotalStats> {
    let (keys, _corr) = store.total()?;
    let total = keys as u64;
    let since_ts = store.earliest_minute()?.map(|m| m * 60);
    Ok(TotalStats {
        total,
        since_ts,
        kcal: total as f64 * KCAL_PER_KEY,
    })
}

pub fn live(store: &Store) -> anyhow::Result<LiveStats> {
    let t = today(store)?;
    Ok(LiveStats {
        today: t.total,
        corrections: t.corrections,
        rework: t.rework,
        kcal: t.kcal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(rows: &[(i64, i64, i64)]) -> Store {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let i = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("keycount-stats-{ns}-{i}.sqlite"));
        let s = Store::open(&path).unwrap();
        for (m, c, r) in rows {
            s.add_minute(*m, *c, *r).unwrap();
        }
        s
    }

    #[test]
    fn total_roundtrip() {
        let s = store_with(&[(100, 5, 0), (200, 7, 1), (300, 3, 2)]);
        let t = super::total(&s).unwrap();
        assert_eq!(t.total, 15);
        assert!(t.kcal > 0.0);
        assert_eq!(t.since_ts, Some(100 * 60));
    }

    #[test]
    fn today_buckets_by_hour() {
        let d = Local::now().date_naive();
        let h0 = Local
            .from_local_datetime(&d.and_hms_opt(0, 30, 0).unwrap())
            .unwrap()
            .timestamp()
            / 60;
        let h5 = Local
            .from_local_datetime(&d.and_hms_opt(5, 15, 0).unwrap())
            .unwrap()
            .timestamp()
            / 60;
        let s = store_with(&[(h0, 10, 1), (h5, 20, 3)]);
        let t = super::today(&s).unwrap();
        assert_eq!(t.total, 30);
        assert_eq!(t.corrections, 4);
        assert_eq!(t.per_hour[0], 10);
        assert_eq!(t.per_hour[5], 20);
        // rework = 4 / (30 + 4) ≒ 0.1176
        assert!((t.rework - 4.0 / 34.0).abs() < 1e-9);
        // active_minutes = 2, peak = max(11, 23) = 23, avg = (11+23)/2 = 17
        assert_eq!(t.active_minutes, 2);
        assert_eq!(t.peak_kpm, 23);
        assert_eq!(t.avg_kpm, 17);
    }

    #[test]
    fn today_speed_with_no_data() {
        let s = store_with(&[]);
        let t = super::today(&s).unwrap();
        assert_eq!(t.active_minutes, 0);
        assert_eq!(t.avg_kpm, 0);
        assert_eq!(t.peak_kpm, 0);
    }

    #[test]
    fn rework_rate_basic() {
        assert_eq!(rework_rate(0, 0), 0.0);
        assert_eq!(rework_rate(10, 0), 0.0);
        assert!((rework_rate(10, 2) - 2.0 / 12.0).abs() < 1e-9);
        assert_eq!(rework_rate(0, 3), 1.0);
    }

    #[test]
    fn day_summary_basic() {
        let target = Local::now().date_naive() - CDuration::days(1);
        let m1 = Local
            .from_local_datetime(&target.and_hms_opt(10, 30, 0).unwrap())
            .unwrap()
            .timestamp()
            / 60;
        let m2 = Local
            .from_local_datetime(&target.and_hms_opt(11, 15, 0).unwrap())
            .unwrap()
            .timestamp()
            / 60;
        let s = store_with(&[(m1, 100, 5), (m2, 200, 10)]);
        let d = day_summary(&s, target).unwrap();
        assert_eq!(d.date, target.format("%Y-%m-%d").to_string());
        assert_eq!(d.keys, 300);
        assert_eq!(d.corrections, 15);
        assert_eq!(d.active_minutes, 2);
        assert_eq!(d.peak_kpm, 210); // 200 + 10
        assert_eq!(d.avg_kpm, (105 + 210) / 2);
    }
}
