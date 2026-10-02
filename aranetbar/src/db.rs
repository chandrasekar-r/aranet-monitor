//! SQLite store of every measurement, split into year/month/day columns with
//! summary views for recaps. Open `readings.db` in any SQLite browser.

use crate::aranet::Reading;
use chrono::{DateTime, Datelike, Local, NaiveDateTime, Timelike};
use rusqlite::{Connection, params};
use std::collections::BTreeMap;
use std::path::Path;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS readings (
    time     TEXT    NOT NULL,  -- local time, YYYY-MM-DD HH:MM:SS
    year     INTEGER NOT NULL,
    month    INTEGER NOT NULL,
    day      INTEGER NOT NULL,
    hour     INTEGER NOT NULL,
    sensor   TEXT    NOT NULL,
    co2      INTEGER,
    temp_c   REAL,
    humidity INTEGER,
    pressure REAL,
    battery  INTEGER,
    UNIQUE (sensor, time)
);
CREATE INDEX IF NOT EXISTS readings_ymd ON readings (year, month, day);

-- Room names from the app's nicknames; kept in sync on rename.
CREATE TABLE IF NOT EXISTS sensors (
    sensor TEXT PRIMARY KEY,
    room   TEXT NOT NULL
);

CREATE VIEW IF NOT EXISTS readings_by_room AS
SELECT r.year, r.month, r.day, r.hour, r.time,
       COALESCE(s.room, r.sensor) AS room, r.sensor,
       r.co2, r.temp_c, r.humidity, r.pressure, r.battery
FROM readings r LEFT JOIN sensors s USING (sensor);

CREATE VIEW IF NOT EXISTS daily AS
SELECT year, month, day, room,
       ROUND(AVG(co2)) AS avg_co2, MIN(co2) AS min_co2, MAX(co2) AS max_co2,
       ROUND(AVG(temp_c), 1) AS avg_temp_c, ROUND(AVG(humidity)) AS avg_humidity,
       SUM(co2 >= 1000) AS readings_over_1000, COUNT(*) AS readings
FROM readings_by_room GROUP BY year, month, day, room;

CREATE VIEW IF NOT EXISTS monthly AS
SELECT year, month, room,
       ROUND(AVG(co2)) AS avg_co2, MIN(co2) AS min_co2, MAX(co2) AS max_co2,
       ROUND(AVG(temp_c), 1) AS avg_temp_c, ROUND(AVG(humidity)) AS avg_humidity,
       SUM(co2 >= 1000) AS readings_over_1000, COUNT(*) AS readings
FROM readings_by_room GROUP BY year, month, room;

CREATE VIEW IF NOT EXISTS yearly AS
SELECT year, room,
       ROUND(AVG(co2)) AS avg_co2, MIN(co2) AS min_co2, MAX(co2) AS max_co2,
       ROUND(AVG(temp_c), 1) AS avg_temp_c, ROUND(AVG(humidity)) AS avg_humidity,
       SUM(co2 >= 1000) AS readings_over_1000, COUNT(*) AS readings
FROM readings_by_room GROUP BY year, room;
";

pub struct Db {
    conn: Connection,
}

impl Db {
    /// Opens (creating if needed) the database. When it is empty, imports the
    /// existing CSV log so earlier readings aren't lost.
    pub fn open(path: &Path, csv: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let db = Self::from_conn(Connection::open(path)?)?;
        let empty: bool = db.conn.query_row("SELECT NOT EXISTS (SELECT 1 FROM readings)", [], |r| r.get(0))?;
        if empty && let Ok(text) = std::fs::read_to_string(csv) {
            db.import_csv(&text)?;
        }
        Ok(db)
    }

    fn from_conn(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn insert(&self, at: DateTime<Local>, sensor: &str, r: &Reading) -> rusqlite::Result<()> {
        self.insert_row(
            at.naive_local(),
            sensor,
            r.co2.map(i64::from),
            r.temperature.map(|t| round1(t.into())),
            Some(r.humidity.into()),
            r.pressure.map(|p| round1(p.into())),
            Some(r.battery.into()),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_row(
        &self,
        at: NaiveDateTime,
        sensor: &str,
        co2: Option<i64>,
        temp: Option<f64>,
        humidity: Option<i64>,
        pressure: Option<f64>,
        battery: Option<i64>,
    ) -> rusqlite::Result<()> {
        self.conn
            .prepare_cached(
                "INSERT OR IGNORE INTO readings
                 (time, year, month, day, hour, sensor, co2, temp_c, humidity, pressure, battery)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?
            .execute(params![
                at.format("%Y-%m-%d %H:%M:%S").to_string(),
                at.year(),
                at.month(),
                at.day(),
                at.hour(),
                sensor,
                co2,
                temp,
                humidity,
                pressure,
                battery,
            ])?;
        Ok(())
    }

    /// Imports rows written by `csvlog`, skipping any it can't parse.
    fn import_csv(&self, text: &str) -> rusqlite::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for line in text.lines().skip(1) {
            let c: Vec<&str> = line.split(',').collect();
            let [time, sensor, co2, temp, humidity, pressure, battery] = c[..] else { continue };
            let Ok(at) = NaiveDateTime::parse_from_str(time, "%Y-%m-%dT%H:%M:%S") else { continue };
            self.insert_row(at, sensor, co2.parse().ok(), temp.parse().ok(), humidity.parse().ok(), pressure.parse().ok(), battery.parse().ok())?;
        }
        tx.commit()
    }

    /// Mirrors the app's nicknames into the `sensors` table.
    pub fn set_rooms(&self, nicknames: &BTreeMap<String, String>) -> rusqlite::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        self.conn.execute("DELETE FROM sensors", [])?;
        for (sensor, room) in nicknames {
            self.conn.execute("INSERT INTO sensors (sensor, room) VALUES (?1, ?2)", params![sensor, room])?;
        }
        tx.commit()
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn mem() -> Db {
        Db::from_conn(Connection::open_in_memory().unwrap()).unwrap()
    }

    #[test]
    fn imports_csv_and_summarises_by_room_and_day() {
        let db = mem();
        db.import_csv(
            "time,sensor,co2,temp_c,humidity,pressure,battery\n\
             2026-09-23T15:57:16,Aranet4 0874F,600,20.2,44,1017.2,92\n\
             2026-09-23T16:02:25,Aranet4 0874F,1200,20.4,44,1017.2,92\n\
             2026-09-24T09:00:00,Aranet4 0874F,,20.0,40,1017.0,92\n\
             garbage\n",
        )
        .unwrap();
        db.set_rooms(&BTreeMap::from([("Aranet4 0874F".into(), "Office".into())])).unwrap();

        let (room, avg, over, n): (String, f64, i64, i64) = db
            .conn
            .query_row(
                "SELECT room, avg_co2, readings_over_1000, readings FROM daily WHERE year=2026 AND month=9 AND day=23",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!((room.as_str(), avg, over, n), ("Office", 900.0, 1, 2));
        let months: i64 = db.conn.query_row("SELECT readings FROM monthly", [], |r| r.get(0)).unwrap();
        assert_eq!(months, 3);
    }

    #[test]
    fn duplicate_measurements_are_ignored() {
        let db = mem();
        let at = Local.with_ymd_and_hms(2026, 9, 23, 12, 0, 0).unwrap();
        let r = Reading { co2: Some(700), temperature: Some(20.35), pressure: Some(1017.25), humidity: 44, battery: 92, interval: 300, ago: 0 };
        db.insert(at, "Aranet4 0874F", &r).unwrap();
        db.insert(at, "Aranet4 0874F", &r).unwrap();
        let (n, hour, temp): (i64, i64, f64) =
            db.conn.query_row("SELECT COUNT(*), MAX(hour), MAX(temp_c) FROM readings", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!((n, hour, temp), (1, 12, 20.4));
    }
}
