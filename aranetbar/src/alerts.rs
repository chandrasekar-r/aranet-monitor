//! Per-sensor alert state machine. Pure logic: callers feed readings and ticks,
//! and get back the alerts that should be shown.

use crate::aranet::Reading;
use chrono::NaiveDate;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    /// CO₂ levels are also used for colours, so they exist even with alerts off.
    pub co2_alerts: bool,
    pub warn_co2: u16,
    pub high_co2: u16,
    pub clear_co2: u16,
    /// (min, max) °C; `None` when temperature alerts are off.
    pub temperature: Option<(f32, f32)>,
    /// (min, max) %; `None` when humidity alerts are off.
    pub humidity: Option<(f32, f32)>,
    pub low_battery: Option<u8>,
    pub missing_after: Duration,
}

/// How far back inside the range a value must come before "back to normal",
/// so a reading hovering at the limit doesn't flap.
const TEMP_HYSTERESIS: f32 = 0.5;
const HUMIDITY_HYSTERESIS: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Temperature,
    Humidity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Alert {
    Warn { co2: u16 },
    High { co2: u16 },
    Clear { co2: u16 },
    TooLow { metric: Metric, value: f32, limit: f32 },
    TooHigh { metric: Metric, value: f32, limit: f32 },
    InRange { metric: Metric, value: f32, min: f32, max: f32 },
    LowBattery { pct: u8 },
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Band {
    Low,
    Ok,
    High,
}

/// Low/ok/high tracking for one metric with a min/max range.
#[derive(Debug, Default)]
struct Range {
    /// `None` until the first reading (or while alerts are off).
    band: Option<Band>,
}

impl Range {
    fn update(&mut self, metric: Metric, value: Option<f32>, limits: Option<(f32, f32)>, hyst: f32) -> Option<Alert> {
        let Some((min, max)) = limits else {
            self.band = None;
            return None;
        };
        let value = value?;
        let prev = self.band;
        let next = if value < min {
            Band::Low
        } else if value > max {
            Band::High
        } else if prev == Some(Band::Low) && value < min + hyst {
            Band::Low
        } else if prev == Some(Band::High) && value > max - hyst {
            Band::High
        } else {
            Band::Ok
        };
        self.band = Some(next);
        match (prev, next) {
            (p, Band::Low) if p != Some(Band::Low) => Some(Alert::TooLow { metric, value, limit: min }),
            (p, Band::High) if p != Some(Band::High) => Some(Alert::TooHigh { metric, value, limit: max }),
            (Some(Band::Low | Band::High), Band::Ok) => Some(Alert::InRange { metric, value, min, max }),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct SensorAlerts {
    /// `None` until the first valid CO₂ reading.
    level: Option<Level>,
    temperature: Range,
    humidity: Range,
    missing_alerted: bool,
    battery_alerted_on: Option<NaiveDate>,
}

impl SensorAlerts {
    #[cfg(test)]
    pub fn level(&self) -> Option<Level> {
        self.level
    }

    pub fn on_reading(&mut self, r: &Reading, today: NaiveDate, t: &Thresholds) -> Vec<Alert> {
        let mut out = Vec::new();
        self.missing_alerted = false;

        if !t.co2_alerts {
            self.level = None;
        } else if let Some(co2) = r.co2 {
            let prev = self.level;
            let next = if co2 >= t.high_co2 {
                Level::High
            } else if co2 >= t.warn_co2 {
                // Coming down from High stays High until it clears.
                if prev == Some(Level::High) { Level::High } else { Level::Warn }
            } else if co2 < t.clear_co2 {
                Level::Ok
            } else {
                // Hysteresis band: keep the current level.
                prev.unwrap_or(Level::Ok)
            };

            match (prev, next) {
                (p, Level::High) if p != Some(Level::High) => out.push(Alert::High { co2 }),
                (None | Some(Level::Ok), Level::Warn) => out.push(Alert::Warn { co2 }),
                (Some(Level::Warn | Level::High), Level::Ok) => out.push(Alert::Clear { co2 }),
                _ => {}
            }
            self.level = Some(next);
        }

        out.extend(self.temperature.update(Metric::Temperature, r.temperature, t.temperature, TEMP_HYSTERESIS));
        out.extend(self.humidity.update(Metric::Humidity, Some(r.humidity.into()), t.humidity, HUMIDITY_HYSTERESIS));

        if t.low_battery.is_some_and(|low| r.battery <= low) && self.battery_alerted_on != Some(today) {
            self.battery_alerted_on = Some(today);
            out.push(Alert::LowBattery { pct: r.battery });
        }
        out
    }

    /// Call periodically with the time since this sensor was last heard.
    pub fn on_tick(&mut self, since_seen: Duration, t: &Thresholds) -> Option<Alert> {
        if since_seen >= t.missing_after && !self.missing_alerted {
            self.missing_alerted = true;
            return Some(Alert::Missing);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Thresholds = Thresholds {
        co2_alerts: true,
        warn_co2: 1000,
        high_co2: 1400,
        clear_co2: 800,
        temperature: Some((18.0, 24.0)),
        humidity: Some((40.0, 60.0)),
        low_battery: Some(10),
        missing_after: Duration::from_secs(30 * 60),
    };

    fn reading(co2: Option<u16>, temp: f32, humidity: u8, battery: u8) -> Reading {
        Reading { co2, temperature: Some(temp), pressure: Some(1017.0), humidity, battery, interval: 300, ago: 0 }
    }

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, d).unwrap()
    }

    fn feed(s: &mut SensorAlerts, co2: u16) -> Vec<Alert> {
        s.on_reading(&reading(Some(co2), 21.0, 50, 90), day(1), &T)
    }

    #[test]
    fn first_normal_reading_is_silent() {
        let mut s = SensorAlerts::default();
        assert!(feed(&mut s, 600).is_empty());
        assert_eq!(s.level(), Some(Level::Ok));
    }

    #[test]
    fn first_reading_in_hysteresis_band_is_ok_and_silent() {
        let mut s = SensorAlerts::default();
        assert!(feed(&mut s, 900).is_empty());
        assert_eq!(s.level(), Some(Level::Ok));
    }

    #[test]
    fn first_reading_already_high_alerts() {
        let mut s = SensorAlerts::default();
        assert_eq!(feed(&mut s, 1100), vec![Alert::Warn { co2: 1100 }]);
        let mut s = SensorAlerts::default();
        assert_eq!(feed(&mut s, 1500), vec![Alert::High { co2: 1500 }]);
    }

    #[test]
    fn full_cycle_with_hysteresis() {
        let mut s = SensorAlerts::default();
        feed(&mut s, 700);
        assert_eq!(feed(&mut s, 1000), vec![Alert::Warn { co2: 1000 }]);
        assert!(feed(&mut s, 1050).is_empty(), "no repeat while warn");
        assert!(feed(&mut s, 950).is_empty(), "band keeps warn");
        assert!(feed(&mut s, 1010).is_empty(), "no re-alert after dipping into band");
        assert_eq!(feed(&mut s, 1400), vec![Alert::High { co2: 1400 }]);
        assert!(feed(&mut s, 1200).is_empty(), "high stays high above clear");
        assert!(feed(&mut s, 850).is_empty());
        assert_eq!(s.level(), Some(Level::High));
        assert_eq!(feed(&mut s, 799), vec![Alert::Clear { co2: 799 }]);
        assert!(feed(&mut s, 700).is_empty());
        assert_eq!(feed(&mut s, 1001), vec![Alert::Warn { co2: 1001 }]);
    }

    #[test]
    fn invalid_co2_keeps_state() {
        let mut s = SensorAlerts::default();
        feed(&mut s, 1100);
        assert!(s.on_reading(&reading(None, 21.0, 50, 90), day(1), &T).is_empty());
        assert_eq!(s.level(), Some(Level::Warn));
    }

    #[test]
    fn low_battery_once_per_day() {
        let mut s = SensorAlerts::default();
        let b = |pct| reading(Some(600), 21.0, 50, pct);
        assert_eq!(s.on_reading(&b(10), day(1), &T), vec![Alert::LowBattery { pct: 10 }]);
        assert!(s.on_reading(&b(9), day(1), &T).is_empty());
        assert_eq!(s.on_reading(&b(9), day(2), &T), vec![Alert::LowBattery { pct: 9 }]);
        assert!(s.on_reading(&b(11), day(3), &T).is_empty());
    }

    #[test]
    fn missing_alerts_once_and_resets_on_reading() {
        let mut s = SensorAlerts::default();
        feed(&mut s, 600);
        assert_eq!(s.on_tick(Duration::from_secs(29 * 60), &T), None);
        assert_eq!(s.on_tick(Duration::from_secs(30 * 60), &T), Some(Alert::Missing));
        assert_eq!(s.on_tick(Duration::from_secs(60 * 60), &T), None);
        feed(&mut s, 600);
        assert_eq!(s.on_tick(Duration::from_secs(31 * 60), &T), Some(Alert::Missing));
    }

    fn humid(s: &mut SensorAlerts, h: u8) -> Vec<Alert> {
        s.on_reading(&reading(Some(600), 21.0, h, 90), day(1), &T)
    }

    #[test]
    fn humidity_range_with_hysteresis() {
        let mut s = SensorAlerts::default();
        assert!(humid(&mut s, 45).is_empty());
        assert!(humid(&mut s, 40).is_empty(), "the limit itself is fine");
        assert_eq!(humid(&mut s, 39), vec![Alert::TooLow { metric: Metric::Humidity, value: 39.0, limit: 40.0 }]);
        assert!(humid(&mut s, 38).is_empty(), "no repeat");
        assert!(humid(&mut s, 41).is_empty(), "still within hysteresis");
        assert_eq!(humid(&mut s, 42), vec![Alert::InRange { metric: Metric::Humidity, value: 42.0, min: 40.0, max: 60.0 }]);
        assert_eq!(humid(&mut s, 61), vec![Alert::TooHigh { metric: Metric::Humidity, value: 61.0, limit: 60.0 }]);
        assert_eq!(humid(&mut s, 30), vec![Alert::TooLow { metric: Metric::Humidity, value: 30.0, limit: 40.0 }]);
    }

    #[test]
    fn temperature_out_of_range_on_first_reading_alerts() {
        let mut s = SensorAlerts::default();
        let r = reading(Some(600), 16.5, 50, 90);
        assert_eq!(s.on_reading(&r, day(1), &T), vec![Alert::TooLow { metric: Metric::Temperature, value: 16.5, limit: 18.0 }]);
    }

    #[test]
    fn disabled_metrics_stay_silent() {
        let t = Thresholds { co2_alerts: false, temperature: None, humidity: None, low_battery: None, ..T };
        let mut s = SensorAlerts::default();
        assert!(s.on_reading(&reading(Some(2000), 30.0, 20, 5), day(1), &t).is_empty());
        // Re-enabling starts fresh rather than reporting "back to normal".
        assert!(s.on_reading(&reading(Some(600), 21.0, 50, 90), day(1), &T).is_empty());
    }
}
