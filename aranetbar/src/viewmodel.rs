//! Everything the UI shows, computed from app state as plain data. Keeps text,
//! colours and stale/pinned logic testable without AppKit.

use crate::alerts::Thresholds;
use crate::aranet::{DecodeError, Reading, short_name};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Good,
    Warn,
    High,
    /// Not seen recently, or no data yet.
    Muted,
}

pub fn tone(co2: u16, t: &Thresholds) -> Tone {
    if co2 >= t.high_co2 {
        Tone::High
    } else if co2 >= t.warn_co2 {
        Tone::Warn
    } else {
        Tone::Good
    }
}

/// What the app knows about one sensor, passed in by `App`.
pub struct Snapshot<'a> {
    pub name: &'a str,
    pub reading: Option<Reading>,
    pub error: Option<DecodeError>,
    pub since_seen: Option<Duration>,
    pub measured_ago: Option<Duration>,
    /// Chart points: (0.0 oldest … 1.0 now, co2).
    pub history: Vec<(f64, u16)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Title {
    Reading { tone: Tone, text: String },
    NoData,
    Bluetooth,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stat {
    pub symbol: &'static str,
    pub caption: &'static str,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hero {
    /// Full advertised name, used to rename the sensor.
    pub name: String,
    pub id: String,
    pub nickname: Option<String>,
    pub co2: String,
    pub tone: Tone,
    pub pill: &'static str,
    pub updated: String,
    pub stats: Vec<Stat>,
    pub chart: Vec<(f64, u16, Tone)>,
    pub warn_co2: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub name: String,
    pub label: String,
    pub tone: Tone,
    /// `None` for rows showing only `note` (stale, no data).
    pub co2: Option<String>,
    pub temp: String,
    pub age: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewModel {
    pub title: Title,
    /// (bold lead, rest) for the amber banner.
    pub banner: Option<(String, String)>,
    pub hero: Hero,
    pub others: Vec<Row>,
}

pub struct Inputs<'a> {
    pub sensors: Vec<Snapshot<'a>>,
    pub pinned: &'a str,
    pub nicknames: &'a BTreeMap<String, String>,
    pub thresholds: Thresholds,
    pub bluetooth_problem: Option<String>,
    pub config_error: Option<String>,
    pub telegram_error: Option<String>,
    pub notifications_ok: bool,
}

pub fn thousands(n: u16) -> String {
    if n >= 1000 { format!("{},{:03}", n / 1000, n % 1000) } else { n.to_string() }
}

fn minutes(d: Duration) -> u64 {
    d.as_secs() / 60
}

fn is_stale(s: &Snapshot, t: &Thresholds) -> bool {
    s.since_seen.is_none_or(|d| d >= t.missing_after)
}

pub fn build(i: &Inputs) -> ViewModel {
    let t = &i.thresholds;
    let pinned = i.sensors.iter().find(|s| s.name == i.pinned);

    let title = match (&i.bluetooth_problem, pinned) {
        (Some(_), _) => Title::Bluetooth,
        (None, Some(s)) if !is_stale(s, t) => match s.reading.and_then(|r| r.co2) {
            Some(c) => Title::Reading { tone: tone(c, t), text: thousands(c) },
            None => Title::NoData,
        },
        _ => Title::NoData,
    };

    let banner = if let Some(p) = &i.bluetooth_problem {
        Some(("Bluetooth unavailable.".into(), p.clone()))
    } else if let Some(e) = &i.config_error {
        Some(("Config problem.".into(), format!("{e}. Using defaults.")))
    } else if let Some(e) = &i.telegram_error {
        Some(("Telegram alerts failing.".into(), format!("{e}. Check Alert settings.")))
    } else if !i.notifications_ok {
        Some((
            "Notifications are off.".into(),
            "Turn them on in System Settings › Notifications to get CO₂ alerts.".into(),
        ))
    } else {
        None
    };

    ViewModel {
        title,
        banner,
        hero: hero(i.pinned, pinned, i.nicknames.get(i.pinned).cloned(), t),
        others: i.sensors.iter().filter(|s| s.name != i.pinned).map(|s| row(s, i.nicknames, t)).collect(),
    }
}

fn hero(name: &str, s: Option<&Snapshot>, nickname: Option<String>, t: &Thresholds) -> Hero {
    let mut h = Hero {
        name: name.to_string(),
        id: short_name(name).to_string(),
        nickname,
        co2: "—".into(),
        tone: Tone::Muted,
        pill: "Waiting",
        updated: "Waiting for the sensor…".into(),
        stats: Vec::new(),
        chart: Vec::new(),
        warn_co2: t.warn_co2,
    };
    let Some(s) = s else { return h };
    h.chart = s.history.iter().map(|&(x, c)| (x, c, tone(c, t))).collect();

    if s.error == Some(DecodeError::IntegrationsOff) {
        h.pill = "No data";
        h.updated = "Turn on Smart Home integrations in the Aranet app".into();
        return h;
    }
    let Some(r) = s.reading else { return h };
    h.stats = vec![
        Stat { symbol: "thermometer.medium", caption: "Temp", value: r.temperature.map(|v| format!("{v:.1}°C")).unwrap_or("–".into()) },
        Stat { symbol: "humidity", caption: "Humidity", value: format!("{}%", r.humidity) },
        Stat { symbol: "gauge.with.dots.needle.33percent", caption: "hPa", value: r.pressure.map(|v| format!("{v:.0}")).unwrap_or("–".into()) },
        Stat { symbol: "battery.75percent", caption: "Battery", value: format!("{}%", r.battery) },
    ];
    if is_stale(s, t) {
        h.pill = "Not seen";
        h.updated = format!("Last seen {} min ago", s.since_seen.map(minutes).unwrap_or(0));
        return h;
    }
    match r.co2 {
        Some(c) => {
            h.co2 = thousands(c);
            h.tone = tone(c, t);
            h.pill = match h.tone {
                Tone::Good => "Good",
                Tone::Warn => "Ventilate soon",
                Tone::High => "Ventilate now",
                Tone::Muted => "",
            };
        }
        None => h.pill = "Calibrating",
    }
    h.updated = match s.measured_ago.map(minutes) {
        Some(0) | None => "Updated just now".into(),
        Some(m) => format!("Updated {m} min ago"),
    };
    h
}

fn row(s: &Snapshot, nicknames: &BTreeMap<String, String>, t: &Thresholds) -> Row {
    let label = nicknames.get(s.name).cloned().unwrap_or_else(|| short_name(s.name).to_string());
    let mut r = Row { name: s.name.to_string(), label, tone: Tone::Muted, co2: None, temp: String::new(), age: String::new(), note: None };
    if s.error == Some(DecodeError::IntegrationsOff) {
        r.note = Some("No data · enable Smart Home".into());
        return r;
    }
    if is_stale(s, t) {
        r.note = Some(format!("Not seen · {} min", s.since_seen.map(minutes).unwrap_or(0)));
        return r;
    }
    let Some(reading) = s.reading else {
        r.note = Some("Waiting for data…".into());
        return r;
    };
    match reading.co2 {
        Some(c) => {
            r.tone = tone(c, t);
            r.co2 = Some(thousands(c));
        }
        None => r.note = Some("Calibrating".into()),
    }
    r.temp = reading.temperature.map(|v| format!("{v:.1}°C")).unwrap_or_default();
    r.age = match s.measured_ago.map(minutes) {
        Some(0) | None => "now".into(),
        Some(m) => format!("{m} m"),
    };
    r
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

    fn reading(co2: u16) -> Reading {
        Reading { co2: Some(co2), temperature: Some(20.3), pressure: Some(1017.2), humidity: 44, battery: 92, interval: 600, ago: 0 }
    }

    fn snap(name: &str, co2: u16, seen_min: u64) -> Snapshot<'_> {
        Snapshot {
            name,
            reading: Some(reading(co2)),
            error: None,
            since_seen: Some(Duration::from_secs(seen_min * 60)),
            measured_ago: Some(Duration::from_secs(120)),
            history: vec![(0.5, 900), (1.0, co2)],
        }
    }

    fn inputs<'a>(sensors: Vec<Snapshot<'a>>, nick: &'a BTreeMap<String, String>) -> Inputs<'a> {
        Inputs { sensors, pinned: "Aranet4 0874F", nicknames: nick, thresholds: T, bluetooth_problem: None, config_error: None, telegram_error: None, notifications_ok: true }
    }

    #[test]
    fn pinned_sensor_drives_title_and_hero() {
        let nick = BTreeMap::from([("Aranet4 0874F".to_string(), "Work".to_string())]);
        let vm = build(&inputs(vec![snap("Aranet4 08746", 528, 0), snap("Aranet4 0874F", 1180, 0)], &nick));
        assert_eq!(vm.title, Title::Reading { tone: Tone::Warn, text: "1,180".into() });
        assert_eq!(vm.hero.id, "0874F");
        assert_eq!(vm.hero.nickname.as_deref(), Some("Work"));
        assert_eq!(vm.hero.pill, "Ventilate soon");
        assert_eq!(vm.hero.updated, "Updated 2 min ago");
        assert_eq!(vm.hero.chart, vec![(0.5, 900, Tone::Good), (1.0, 1180, Tone::Warn)]);
        assert_eq!(vm.others.len(), 1);
        assert_eq!(vm.others[0].label, "08746");
        assert_eq!(vm.others[0].co2.as_deref(), Some("528"));
        assert_eq!(vm.others[0].age, "2 m");
        assert!(vm.banner.is_none());
    }

    #[test]
    fn stale_sensors_are_muted() {
        let nick = BTreeMap::new();
        let vm = build(&inputs(vec![snap("Aranet4 0874F", 600, 45), snap("Aranet4 088CB", 700, 42)], &nick));
        assert_eq!(vm.title, Title::NoData);
        assert_eq!(vm.hero.pill, "Not seen");
        assert_eq!(vm.hero.co2, "—");
        assert_eq!(vm.others[0].tone, Tone::Muted);
        assert_eq!(vm.others[0].note.as_deref(), Some("Not seen · 42 min"));
    }

    #[test]
    fn missing_pinned_sensor_shows_waiting() {
        let nick = BTreeMap::new();
        let vm = build(&inputs(vec![snap("Aranet4 088CB", 700, 0)], &nick));
        assert_eq!(vm.title, Title::NoData);
        assert_eq!(vm.hero.id, "0874F");
        assert_eq!(vm.hero.pill, "Waiting");
    }

    #[test]
    fn banner_priority_bluetooth_then_config_then_telegram_then_notifications() {
        let nick = BTreeMap::new();
        let mut i = inputs(vec![], &nick);
        i.notifications_ok = false;
        assert_eq!(build(&i).banner.unwrap().0, "Notifications are off.");
        i.telegram_error = Some("chat not found".into());
        assert_eq!(build(&i).banner.unwrap().0, "Telegram alerts failing.");
        i.config_error = Some("bad".into());
        assert_eq!(build(&i).banner.unwrap().0, "Config problem.");
        i.bluetooth_problem = Some("off".into());
        let vm = build(&i);
        assert_eq!(vm.banner.unwrap().0, "Bluetooth unavailable.");
        assert_eq!(vm.title, Title::Bluetooth);
    }

    #[test]
    fn integrations_off_row_explains_fix() {
        let nick = BTreeMap::new();
        let mut s = snap("Aranet4 088CB", 700, 0);
        s.error = Some(DecodeError::IntegrationsOff);
        let vm = build(&inputs(vec![s], &nick));
        assert_eq!(vm.others[0].note.as_deref(), Some("No data · enable Smart Home"));
    }

    #[test]
    fn thousands_separator() {
        assert_eq!(thousands(587), "587");
        assert_eq!(thousands(1040), "1,040");
    }
}
