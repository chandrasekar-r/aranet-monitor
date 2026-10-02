//! Cross-process state for WidgetKit, Control Center, and App Intents.
//! On macOS the file lives in the App Group container; elsewhere it uses the
//! normal Application Support folder (Linux unit tests).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const APP_GROUP_ID: &str = "group.com.20deg.aranetbar";
const FILE_NAME: &str = "golden_gate_state.json";

/// Default CO₂ alert snooze when toggled from Control Center (no duration picker).
pub const DEFAULT_SNOOZE: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FocusAlertsMode {
    #[default]
    AllSensors,
    PinnedOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GoldenGateState {
    /// When set and in the future, CO₂ threshold notifications are suppressed.
    pub co2_snooze_until: Option<DateTime<Utc>>,
    pub focus_alerts_mode: FocusAlertsMode,
    pub pinned_sensor: String,
    pub pinned_nickname: String,
    pub latest_co2: Option<u16>,
    pub tone: String,
    pub measurement_interval_secs: u32,
    /// Absolute path to SQLite (main app); extensions use `shared_db_path` when present.
    pub db_path: String,
    pub shared_db_path: String,
    pub updated_at: Option<DateTime<Utc>>,
}

impl Default for GoldenGateState {
    fn default() -> Self {
        Self {
            co2_snooze_until: None,
            focus_alerts_mode: FocusAlertsMode::AllSensors,
            pinned_sensor: String::new(),
            pinned_nickname: String::new(),
            latest_co2: None,
            tone: "unknown".into(),
            measurement_interval_secs: 300,
            db_path: String::new(),
            shared_db_path: String::new(),
            updated_at: None,
        }
    }
}

impl GoldenGateState {
    pub fn co2_alerts_snoozed(&self, now: DateTime<Utc>) -> bool {
        self.co2_snooze_until.is_some_and(|until| until > now)
    }

    pub fn set_snooze_until(&mut self, until: Option<DateTime<Utc>>) {
        self.co2_snooze_until = until;
        self.updated_at = Some(Utc::now());
    }

    pub fn toggle_snooze(&mut self, now: DateTime<Utc>) -> bool {
        if self.co2_alerts_snoozed(now) {
            self.set_snooze_until(None);
            false
        } else {
            self.set_snooze_until(Some(now + DEFAULT_SNOOZE));
            true
        }
    }
}

pub fn container_dir() -> PathBuf {
    #[cfg(all(feature = "macos-app", target_os = "macos"))]
    {
        if let Some(home) = dirs::home_dir() {
            let group = home
                .join("Library")
                .join("Group Containers")
                .join(APP_GROUP_ID);
            if group.parent().is_some() {
                return group;
            }
        }
    }
    crate::config::app_dir()
}

pub fn state_path() -> PathBuf {
    container_dir().join(FILE_NAME)
}

pub fn shared_db_path() -> PathBuf {
    container_dir().join("readings_shared.db")
}

pub fn load() -> GoldenGateState {
    let path = state_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => GoldenGateState::default(),
    }
}

pub fn save(state: &GoldenGateState) -> std::io::Result<()> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut s = state.clone();
    s.updated_at = Some(Utc::now());
    let json = serde_json::to_string_pretty(&s)?;
    std::fs::write(path, json)
}

/// Copies the main database into the App Group for extension read access (best-effort).
pub fn sync_db_snapshot(source: &Path) -> std::io::Result<()> {
    if !source.exists() {
        return Ok(());
    }
    let dest = shared_db_path();
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, dest)?;
    Ok(())
}

pub fn merge_from_disk(mut state: GoldenGateState) -> GoldenGateState {
    let disk = load();
    // Extension / Control Center may have updated snooze or focus mode.
    state.co2_snooze_until = disk.co2_snooze_until;
    state.focus_alerts_mode = disk.focus_alerts_mode;
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snooze_toggle_and_expiry() {
        let now = Utc::now();
        let mut s = GoldenGateState::default();
        assert!(!s.co2_alerts_snoozed(now));
        assert!(s.toggle_snooze(now));
        assert!(s.co2_alerts_snoozed(now));
        assert!(!s.toggle_snooze(now));
        s.set_snooze_until(Some(now - chrono::Duration::seconds(1)));
        assert!(!s.co2_alerts_snoozed(now));
    }

    #[test]
    fn round_trip_json() {
        let mut s = GoldenGateState::default();
        s.pinned_sensor = "Aranet4 0874F".into();
        s.latest_co2 = Some(900);
        let json = serde_json::to_string(&s).unwrap();
        let back: GoldenGateState = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
