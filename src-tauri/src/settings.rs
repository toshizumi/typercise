use std::path::{Path, PathBuf};

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub client_id: String,
    #[serde(default = "default_telemetry_enabled")]
    pub telemetry_enabled: bool,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub first_run_consent_shown: bool,
    /// 最後にアップロードした日付 (YYYY-MM-DD, ローカル日)。未送信なら None。
    #[serde(default)]
    pub last_uploaded_date: Option<String>,
}

fn default_telemetry_enabled() -> bool {
    true
}

impl Settings {
    fn fresh() -> Self {
        Self {
            client_id: uuid::Uuid::new_v4().to_string(),
            telemetry_enabled: true,
            nickname: None,
            first_run_consent_shown: false,
            last_uploaded_date: None,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    inner: RwLock<Settings>,
}

impl SettingsStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let settings = match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str::<Settings>(&s).unwrap_or_else(|e| {
                tracing::warn!(?e, "settings parse failed; using fresh");
                Settings::fresh()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::fresh(),
            Err(e) => return Err(e.into()),
        };
        let store = Self {
            path: path.to_path_buf(),
            inner: RwLock::new(settings),
        };
        // 新規生成された UUID を即永続化（次回起動で同じ ID を使うため）
        store.persist()?;
        Ok(store)
    }

    pub fn get(&self) -> Settings {
        self.inner.read().clone()
    }

    pub fn update<F: FnOnce(&mut Settings)>(&self, f: F) -> Result<Settings> {
        {
            let mut s = self.inner.write();
            f(&mut s);
        }
        self.persist()?;
        Ok(self.get())
    }

    fn persist(&self) -> Result<()> {
        let s = self.inner.read();
        let json = serde_json::to_string_pretty(&*s)?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

/// nickname を 32 文字以内、制御文字除去でサニタイズ。空なら None。
pub fn sanitize_nickname(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string();
    if cleaned.is_empty() {
        None
    } else if cleaned.chars().count() > 32 {
        Some(cleaned.chars().take(32).collect())
    } else {
        Some(cleaned)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_path() -> PathBuf {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let i = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("typercise-settings-{ns}-{i}.json"))
    }

    #[test]
    fn fresh_creates_uuid() {
        let path = temp_path();
        let store = SettingsStore::open(&path).unwrap();
        let s = store.get();
        assert_eq!(s.client_id.len(), 36);
        assert!(s.telemetry_enabled);
        assert!(!s.first_run_consent_shown);
        assert!(s.nickname.is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn round_trip() {
        let path = temp_path();
        let store = SettingsStore::open(&path).unwrap();
        let s1 = store.update(|s| {
            s.nickname = Some("alice".into());
            s.telemetry_enabled = false;
            s.first_run_consent_shown = true;
        }).unwrap();
        // 新インスタンスで読み直す
        let store2 = SettingsStore::open(&path).unwrap();
        let s2 = store2.get();
        assert_eq!(s1.client_id, s2.client_id);
        assert_eq!(s2.nickname.as_deref(), Some("alice"));
        assert!(!s2.telemetry_enabled);
        assert!(s2.first_run_consent_shown);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sanitize_nickname_basics() {
        assert_eq!(sanitize_nickname(None), None);
        assert_eq!(sanitize_nickname(Some("".into())), None);
        assert_eq!(sanitize_nickname(Some("  ".into())), None);
        assert_eq!(sanitize_nickname(Some("alice".into())).as_deref(), Some("alice"));
        assert_eq!(sanitize_nickname(Some("  bob  ".into())).as_deref(), Some("bob"));
        // 制御文字除去
        assert_eq!(sanitize_nickname(Some("a\nb".into())).as_deref(), Some("ab"));
        // 32 文字制限
        let long = "a".repeat(40);
        assert_eq!(sanitize_nickname(Some(long)).map(|s| s.len()), Some(32));
    }
}
