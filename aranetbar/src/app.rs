//! Application state and event handling. Lives on the main thread.

use crate::alerts::{Alert, Metric, SensorAlerts, Thresholds};
use crate::aranet::{DecodeError, Reading, short_name};
use crate::ble::{Advert, BleEvent, BleStatus};
use crate::config::{self, Config};
use crate::history::History;
use crate::ui::{Command, SettingsReply, Ui, UiAction};
use crate::viewmodel::{self, Inputs, Snapshot, thousands};
use crate::db::Db;
use crate::integrations;
use crate::shared_state::{self, FocusAlertsMode, GoldenGateState};
use crate::{csvlog, login, notify, telegram};
use chrono::{DateTime, Local, Utc};
use objc2::MainThreadMarker;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tao::event_loop::EventLoopProxy;

#[derive(Debug)]
pub enum UserEvent {
    Ble(BleEvent),
    Ui(UiAction),
    Tick,
    Notifications(bool),
    /// Outcome of a Telegram send; `test` is set for the settings' Send Test.
    Telegram { result: Result<(), String>, test: bool },
}

#[derive(Default)]
struct Sensor {
    reading: Option<Reading>,
    error: Option<DecodeError>,
    seen: Option<Instant>,
    measured_at: Option<DateTime<Local>>,
    logged_at: Option<DateTime<Local>>,
    alerts: SensorAlerts,
}

pub struct App {
    config: Config,
    config_error: Option<String>,
    sensors: BTreeMap<String, Sensor>,
    history: History,
    db: Option<Db>,
    ble: Option<BleStatus>,
    notifications_ok: bool,
    telegram_error: Option<String>,
    ui: Ui,
    proxy: EventLoopProxy<UserEvent>,
    golden_gate: GoldenGateState,
}

impl App {
    /// Must be called once the event loop has started (macOS requirement).
    pub fn new(mtm: MainThreadMarker, proxy: EventLoopProxy<UserEvent>) -> Self {
        let (config, config_error) = Config::load();
        let p = proxy.clone();
        let ui = Ui::new(mtm, Rc::new(move |a| {
            let _ = p.send_event(UserEvent::Ui(a));
        }));

        let p = proxy.clone();
        notify::init(move |ok| {
            let _ = p.send_event(UserEvent::Notifications(ok));
        });

        let db = Db::open(&config.db_path, &config.log_path).map_err(|e| eprintln!("database: {e}")).ok();
        let mut golden_gate = shared_state::load();
        golden_gate.pinned_sensor = config.pinned.clone();
        let mut app = Self {
            history: History::load(&config.log_path, Local::now()),
            db,
            config,
            config_error,
            sensors: BTreeMap::new(),
            ble: None,
            notifications_ok: true,
            telegram_error: None,
            ui,
            proxy,
            golden_gate,
        };
        app.sync_rooms();
        app.refresh();
        app
    }

    fn sync_rooms(&self) {
        if let Some(db) = &self.db
            && let Err(e) = db.set_rooms(&self.config.nicknames)
        {
            eprintln!("database: {e}");
        }
    }

    /// Returns `false` when the app should quit.
    pub fn handle(&mut self, event: UserEvent) -> bool {
        match event {
            UserEvent::Ble(BleEvent::Status(s)) => self.ble = Some(s),
            UserEvent::Ble(BleEvent::Advert(a)) => self.on_advert(a),
            UserEvent::Tick => self.on_tick(),
            UserEvent::Notifications(ok) => self.notifications_ok = ok,
            UserEvent::Telegram { result, test } => self.on_telegram(result, test),
            UserEvent::Ui(a) => return self.on_ui(a),
        }
        self.refresh();
        true
    }

    fn on_advert(&mut self, a: Advert) {
        let t = self.config.thresholds();
        let s = self.sensors.entry(a.name.clone()).or_default();
        s.seen = Some(Instant::now());
        let r = match a.data {
            Ok(r) => r,
            Err(e) => {
                s.error = Some(e);
                return;
            }
        };
        s.error = None;
        s.reading = Some(r);
        let now = Local::now();
        let measured_at = now - chrono::Duration::seconds(r.ago.into());
        s.measured_at = Some(measured_at);

        // Log each measurement once; repeated broadcasts differ only by a
        // second or two of clock drift.
        if s.logged_at.is_none_or(|l| (measured_at - l).num_seconds().abs() > 30) {
            s.logged_at = Some(measured_at);
            if let Err(e) = csvlog::append(&self.config.log_path, measured_at, &a.name, &r) {
                eprintln!("csv log: {e}");
            }
            if let Some(db) = &self.db
                && let Err(e) = db.insert(measured_at, &a.name, &r)
            {
                eprintln!("database: {e}");
            }
            if let Some(co2) = r.co2 {
                self.history.push(&a.name, measured_at, co2, now);
            }
        }

        for alert in s.alerts.on_reading(&r, now.date_naive(), &t) {
            self.send_alert(&a.name, alert, &t);
        }
    }

    fn on_tick(&mut self) {
        integrations::on_tick_snooze(&mut self.golden_gate);
        self.golden_gate = integrations::sync_after_external_change(&self.golden_gate);
        let t = self.config.thresholds();
        let mut due = Vec::new();
        for (name, s) in &mut self.sensors {
            if let Some(seen) = s.seen
                && let Some(alert) = s.alerts.on_tick(seen.elapsed(), &t)
            {
                due.push((name.clone(), alert));
            }
        }
        for (name, alert) in due {
            self.send_alert(&name, alert, &t);
        }
        let p = self.proxy.clone();
        notify::check(move |ok| {
            let _ = p.send_event(UserEvent::Notifications(ok));
        });
    }

    fn on_ui(&mut self, action: UiAction) -> bool {
        match action {
            UiAction::TogglePopover => self.ui.toggle(),
            UiAction::Settings => self.ui.show_settings(login::is_enabled()),
            UiAction::Pin(name) => {
                self.config.pinned = name;
                self.config.save();
            }
            UiAction::Rename(name) => {
                self.ui.close();
                let current = self.config.nicknames.get(&name).map(String::as_str);
                if let Some(nick) = self.ui.prompt_rename(&name, current) {
                    let nick = nick.trim();
                    if nick.is_empty() {
                        self.config.nicknames.remove(&name);
                    } else {
                        self.config.nicknames.insert(name, nick.to_string());
                    }
                    self.config.save();
                    self.sync_rooms();
                }
            }
            UiAction::Command(cmd) => match cmd {
                Command::Quit => return false,
                Command::About => self.ui.show_about(),
                Command::ShowHistory => {
                    self.ui.close();
                    self.ui.show_history(&self.config.db_path, &self.config.log_path, &self.config.nicknames);
                }
                Command::AlertSettings => {
                    self.ui.close();
                    self.edit_alert_settings();
                }
                Command::ToggleLogin => {
                    if let Err(err) = login::set_enabled(!login::is_enabled()) {
                        notify::send("app", "Couldn't change Start at login", &err);
                    }
                }
                Command::OpenLog => {
                    self.ui.close();
                    let path = &self.config.log_path;
                    let target = if path.exists() { path.clone() } else { config::app_dir() };
                    let _ = std::process::Command::new("open").arg(target).spawn();
                }
                Command::OpenConfig => {
                    self.ui.close();
                    let _ = std::process::Command::new("open").arg("-t").arg(config::path()).spawn();
                }
                Command::ShowData => {
                    self.ui.close();
                    let db = &self.config.db_path;
                    let mut open = std::process::Command::new("open");
                    if db.exists() { open.arg("-R").arg(db) } else { open.arg(config::app_dir()) };
                    let _ = open.spawn();
                }
                Command::ReloadConfig => {
                    (self.config, self.config_error) = Config::load();
                    self.sync_rooms();
                }
                Command::TestAlert => {
                    let (title, body) = TEST_ALERT;
                    notify::send("test", title, body);
                    self.send_telegram(&self.config, &format!("{title}\n{body}"), true);
                }
            },
        }
        self.refresh();
        true
    }

    /// Shows the settings form until the user saves valid values or cancels.
    /// Send Test keeps the form open with the edited values.
    fn edit_alert_settings(&mut self) {
        let mut draft = self.config.clone();
        let mut error: Option<String> = None;
        loop {
            match self.ui.prompt_alert_settings(&draft, error.as_deref()) {
                SettingsReply::Cancel => return,
                SettingsReply::Save(e) => {
                    draft = e.config;
                    error = e.error;
                    if error.is_none() {
                        self.config = draft;
                        self.config.save();
                        return;
                    }
                }
                SettingsReply::Test(e) => {
                    draft = e.config;
                    error = e.error;
                    if error.is_none() {
                        let (title, body) = TEST_ALERT;
                        notify::send("test", title, body);
                        if draft.telegram().is_some() {
                            self.send_telegram(&draft, &format!("{title}\n{body}"), true);
                        } else {
                            error = Some("Sent a Mac notification. Add a Telegram bot token and chat ID to test Telegram too.".into());
                        }
                    }
                }
            }
        }
    }

    fn send_telegram(&self, config: &Config, text: &str, test: bool) {
        let Some((token, chat)) = config.telegram() else { return };
        let p = self.proxy.clone();
        telegram::send(token, chat, text, move |result| {
            let _ = p.send_event(UserEvent::Telegram { result, test });
        });
    }

    fn on_telegram(&mut self, result: Result<(), String>, test: bool) {
        match (&result, test) {
            (Ok(()), true) => notify::send("test", "Telegram is working", "The test message was sent to your chat."),
            (Err(e), true) => notify::send("test", "Telegram test failed", e),
            // Only announce a failing bot once, not on every alert.
            (Err(e), false) if self.telegram_error.is_none() => notify::send("app", "Telegram alerts failing", e),
            _ => {}
        }
        self.telegram_error = result.err();
    }

    fn send_alert(&self, name: &str, alert: Alert, t: &Thresholds) {
        if self.should_suppress_notification(name, &alert) {
            return;
        }
        let (title, body) = alert_text(name, self.config.nicknames.get(name), alert, t);
        let filter = self.notification_filter_criteria(name);
        notify::send_filtered(name, &title, &body, filter.as_deref());
        self.send_telegram(&self.config, &format!("{title}\n{body}"), false);
    }

    fn should_suppress_notification(&self, name: &str, alert: &Alert) -> bool {
        if matches!(
            alert,
            Alert::Warn { .. } | Alert::High { .. } | Alert::Clear { .. }
        ) && self.golden_gate.co2_alerts_snoozed(Utc::now())
        {
            return true;
        }
        if self.golden_gate.focus_alerts_mode == FocusAlertsMode::PinnedOnly && name != self.config.pinned {
            return true;
        }
        false
    }

    fn notification_filter_criteria(&self, sensor: &str) -> Option<String> {
        if self.golden_gate.focus_alerts_mode != FocusAlertsMode::PinnedOnly {
            return None;
        }
        Some(format!("aranetbar:sensor:{sensor}"))
    }

    fn refresh(&mut self) {
        let now = Local::now();
        let bluetooth_problem = match &self.ble {
            Some(BleStatus::PoweredOff) => Some("Bluetooth is turned off.".to_string()),
            Some(BleStatus::Error(e)) => Some(e.clone()),
            _ => None,
        };
        let sensors = self
            .sensors
            .iter()
            .map(|(name, s)| Snapshot {
                name,
                reading: s.reading,
                error: s.error,
                since_seen: s.seen.map(|t| t.elapsed()),
                measured_ago: s.measured_at.map(|m| (now - m).to_std().unwrap_or_default()),
                history: self.history.recent(name, now),
            })
            .collect();
        let vm = viewmodel::build(&Inputs {
            sensors,
            pinned: &self.config.pinned,
            nicknames: &self.config.nicknames,
            thresholds: self.config.thresholds(),
            bluetooth_problem,
            config_error: self.config_error.clone(),
            telegram_error: self.telegram_error.clone(),
            notifications_ok: self.notifications_ok,
        });
        let latest_co2 = self
            .sensors
            .get(&self.config.pinned)
            .and_then(|s| s.reading)
            .and_then(|r| r.co2);
        let interval = self
            .sensors
            .get(&self.config.pinned)
            .and_then(|s| s.reading)
            .map(|r| r.interval)
            .unwrap_or(300);
        integrations::publish(
            &mut self.golden_gate,
            &vm,
            &self.config.pinned,
            &self.config.nicknames,
            &self.config.db_path,
            interval,
            latest_co2,
        );
        self.ui.render(vm);
    }
}

const TEST_ALERT: (&str, &str) = ("Test alert", "Alerts from AranetBar are working.");

fn alert_text(name: &str, nickname: Option<&String>, alert: Alert, t: &Thresholds) -> (String, String) {
    let who = match nickname {
        Some(n) => format!("{n} ({})", short_name(name)),
        None => short_name(name).to_string(),
    };
    let fmt = |metric: Metric, v: f32| match metric {
        Metric::Temperature => format!("{v:.1} °C"),
        Metric::Humidity => format!("{v:.0}%"),
    };
    let what = |metric: Metric| match metric {
        Metric::Temperature => "temperature",
        Metric::Humidity => "humidity",
    };
    match alert {
        Alert::Warn { co2 } => (format!("{who} · CO₂ {} ppm", thousands(co2)), "CO₂ is rising. Time to ventilate.".into()),
        Alert::High { co2 } => (
            format!("{who} · CO₂ {} ppm", thousands(co2)),
            "CO₂ is very high. Open a window or take a break.".into(),
        ),
        Alert::Clear { co2 } => (format!("{who} · CO₂ {} ppm", thousands(co2)), "Air is fresh again.".into()),
        Alert::TooLow { metric, value, limit } => (
            format!("{who} · {} {}", what(metric), fmt(metric, value)),
            match metric {
                Metric::Temperature => format!("Below {}. It's getting cold.", fmt(metric, limit)),
                Metric::Humidity => format!("Below {}. The air is dry; a humidifier helps.", fmt(metric, limit)),
            },
        ),
        Alert::TooHigh { metric, value, limit } => (
            format!("{who} · {} {}", what(metric), fmt(metric, value)),
            match metric {
                Metric::Temperature => format!("Above {}. It's getting warm.", fmt(metric, limit)),
                Metric::Humidity => format!("Above {}. The air is humid; ventilate or dehumidify.", fmt(metric, limit)),
            },
        ),
        Alert::InRange { metric, value, min, max } => (
            format!("{who} · {} {}", what(metric), fmt(metric, value)),
            format!("Back in range ({} – {}).", fmt(metric, min), fmt(metric, max)),
        ),
        Alert::LowBattery { pct } => (format!("{who} · battery {pct}%"), "Replace the batteries soon.".into()),
        Alert::Missing => (
            format!("{who} not seen"),
            format!("No broadcast from {name} for {} minutes.", t.missing_after.as_secs() / 60),
        ),
    }
}

/// Sends a tick every minute to refresh ages and run the missing-sensor check.
pub fn spawn_ticker(proxy: EventLoopProxy<UserEvent>) {
    std::thread::Builder::new()
        .name("tick".into())
        .spawn(move || {
            while proxy.send_event(UserEvent::Tick).is_ok() {
                std::thread::sleep(Duration::from_secs(60));
            }
        })
        .expect("spawn ticker");
}
