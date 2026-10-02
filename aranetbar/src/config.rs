//! User settings, stored as TOML in ~/Library/Application Support/AranetBar/.

use crate::alerts::Thresholds;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Full advertised name of the sensor shown in the menu bar.
    pub pinned: String,
    pub co2_alerts: bool,
    pub warn_co2: u16,
    pub high_co2: u16,
    pub clear_co2: u16,
    pub temperature_alerts: bool,
    pub temp_min: f32,
    pub temp_max: f32,
    pub humidity_alerts: bool,
    pub humidity_min: u8,
    pub humidity_max: u8,
    pub battery_alerts: bool,
    pub low_battery: u8,
    pub missing_minutes: u64,
    /// Telegram bot token from @BotFather; empty turns Telegram alerts off.
    pub telegram_token: String,
    pub telegram_chat_id: String,
    /// CSV file readings are appended to.
    pub log_path: PathBuf,
    /// SQLite database with the same readings plus daily/monthly/yearly views.
    pub db_path: PathBuf,
    /// Friendly names by advertised sensor name, e.g. "Aranet4 0874F" = "Work".
    pub nicknames: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pinned: "Aranet4 0874F".into(),
            // Healthy indoor ranges: CO₂ under 1,000 ppm, 18–24 °C, 40–60 % RH.
            co2_alerts: true,
            warn_co2: 1000,
            high_co2: 1400,
            clear_co2: 800,
            temperature_alerts: true,
            temp_min: 18.0,
            temp_max: 24.0,
            humidity_alerts: true,
            humidity_min: 40,
            humidity_max: 60,
            battery_alerts: true,
            low_battery: 10,
            missing_minutes: 30,
            telegram_token: String::new(),
            telegram_chat_id: String::new(),
            log_path: app_dir().join("readings.csv"),
            db_path: app_dir().join("readings.db"),
            nicknames: BTreeMap::from([("Aranet4 0874F".into(), "Work".into())]),
        }
    }
}

pub fn app_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("AranetBar")
}

pub fn path() -> PathBuf {
    app_dir().join("config.toml")
}

impl Config {
    /// Loads the config, writing defaults on first run. On a parse error the
    /// defaults are used and the error is returned alongside for display.
    pub fn load() -> (Self, Option<String>) {
        match std::fs::read_to_string(path()) {
            Ok(s) => match toml::from_str(&s) {
                Ok(c) => (c, None),
                Err(e) => (Self::default(), Some(format!("config.toml invalid: {}", e.message()))),
            },
            Err(_) => {
                let c = Self::default();
                c.save();
                (c, None)
            }
        }
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(app_dir());
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = std::fs::write(path(), s);
        }
    }

    pub fn thresholds(&self) -> Thresholds {
        Thresholds {
            co2_alerts: self.co2_alerts,
            warn_co2: self.warn_co2,
            high_co2: self.high_co2,
            clear_co2: self.clear_co2,
            temperature: self.temperature_alerts.then_some((self.temp_min, self.temp_max)),
            humidity: self.humidity_alerts.then_some((self.humidity_min.into(), self.humidity_max.into())),
            low_battery: self.battery_alerts.then_some(self.low_battery),
            missing_after: Duration::from_secs(self.missing_minutes * 60),
        }
    }

    pub fn telegram(&self) -> Option<(&str, &str)> {
        let (token, chat) = (self.telegram_token.trim(), self.telegram_chat_id.trim());
        (!token.is_empty() && !chat.is_empty()).then_some((token, chat))
    }

    /// Checks the limits make sense together; the message is shown to the user.
    pub fn validate(&self) -> Result<(), String> {
        if !(self.clear_co2 < self.warn_co2 && self.warn_co2 < self.high_co2) {
            return Err("CO₂ levels must go back-to-normal < warn < urgent.".into());
        }
        if self.temp_min >= self.temp_max {
            return Err("Temperature minimum must be below the maximum.".into());
        }
        if self.humidity_min >= self.humidity_max || self.humidity_max > 100 {
            return Err("Humidity minimum must be below the maximum (0–100 %).".into());
        }
        if self.low_battery > 100 {
            return Err("Battery level must be 0–100 %.".into());
        }
        if self.missing_minutes == 0 {
            return Err("Sensor-missing time must be at least 1 minute.".into());
        }
        if self.telegram_token.trim().is_empty() != self.telegram_chat_id.trim().is_empty() {
            return Err("Telegram needs both a bot token and a chat ID (or leave both empty).".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_old_configs_gain_new_fields() {
        assert_eq!(Config::default().validate(), Ok(()));
        let old: Config = toml::from_str("pinned = \"Aranet4 0874F\"\nwarn_co2 = 900").unwrap();
        assert_eq!((old.warn_co2, old.humidity_min, old.humidity_max), (900, 40, 60));
        assert_eq!(old.telegram(), None);
    }

    #[test]
    fn validation_catches_inverted_ranges_and_half_telegram() {
        let bad = |f: fn(&mut Config)| {
            let mut c = Config::default();
            f(&mut c);
            c.validate().is_err()
        };
        assert!(bad(|c| c.humidity_min = 70));
        assert!(bad(|c| c.temp_max = 10.0));
        assert!(bad(|c| c.clear_co2 = 1200));
        assert!(bad(|c| c.telegram_token = "123:abc".into()));
    }
}
