//! Recent CO₂ measurements per sensor, for the popover's chart. Seeded from
//! the CSV log so the chart survives a relaunch.

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

pub const WINDOW: chrono::Duration = chrono::Duration::hours(3);

#[derive(Default)]
pub struct History {
    points: BTreeMap<String, VecDeque<(DateTime<Local>, u16)>>,
}

impl History {
    /// Loads the last `WINDOW` of readings from a CSV written by `csvlog`.
    pub fn load(path: &Path, now: DateTime<Local>) -> Self {
        let mut h = Self::default();
        if let Ok(text) = std::fs::read_to_string(path) {
            h.load_str(&text, now);
        }
        h
    }

    fn load_str(&mut self, text: &str, now: DateTime<Local>) {
        for line in text.lines().skip(1) {
            let mut cols = line.split(',');
            let (Some(time), Some(sensor), Some(co2)) = (cols.next(), cols.next(), cols.next()) else { continue };
            let Ok(naive) = NaiveDateTime::parse_from_str(time, "%Y-%m-%dT%H:%M:%S") else { continue };
            let Some(at) = Local.from_local_datetime(&naive).earliest() else { continue };
            let Ok(co2) = co2.parse() else { continue };
            if now - at <= WINDOW {
                self.push(sensor, at, co2, now);
            }
        }
    }

    pub fn push(&mut self, sensor: &str, at: DateTime<Local>, co2: u16, now: DateTime<Local>) {
        let q = self.points.entry(sensor.to_string()).or_default();
        q.push_back((at, co2));
        while q.front().is_some_and(|(t, _)| now - *t > WINDOW) {
            q.pop_front();
        }
    }

    /// Points within the window as (position 0.0 = oldest edge … 1.0 = now, co2).
    pub fn recent(&self, sensor: &str, now: DateTime<Local>) -> Vec<(f64, u16)> {
        let window = WINDOW.num_seconds() as f64;
        self.points
            .get(sensor)
            .into_iter()
            .flatten()
            .filter_map(|(t, c)| {
                let age = (now - *t).num_seconds() as f64;
                (0.0..=window).contains(&age).then(|| (1.0 - age / window, *c))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(h: u32, m: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 9, 23, h, m, 0).unwrap()
    }

    #[test]
    fn loads_recent_rows_per_sensor_from_csv() {
        let csv = "time,sensor,co2,temp_c,humidity,pressure,battery\n\
                   2026-09-23T11:00:00,Aranet4 0874F,500,20.0,40,1017.0,92\n\
                   2026-09-23T14:00:00,Aranet4 0874F,600,20.0,40,1017.0,92\n\
                   2026-09-23T15:30:00,Aranet4 088CB,700,20.0,40,1017.0,92\n\
                   garbage line\n\
                   2026-09-23T16:00:00,Aranet4 0874F,,20.0,40,1017.0,92\n";
        let mut h = History::default();
        h.load_str(csv, at(16, 0));
        assert_eq!(h.recent("Aranet4 0874F", at(16, 0)), vec![(1.0 - 2.0 / 3.0, 600)]);
        assert_eq!(h.recent("Aranet4 088CB", at(16, 0)), vec![(1.0 - 0.5 / 3.0, 700)]);
        assert!(h.recent("Aranet4 08746", at(16, 0)).is_empty());
    }

    #[test]
    fn push_prunes_points_older_than_window() {
        let mut h = History::default();
        h.push("s", at(12, 0), 500, at(12, 0));
        h.push("s", at(14, 0), 600, at(14, 0));
        h.push("s", at(15, 30), 700, at(15, 30));
        assert_eq!(h.points["s"].len(), 2, "12:00 is >3h before 15:30");
        assert_eq!(h.recent("s", at(15, 30)).last(), Some(&(1.0, 700)));
    }
}
