//! Appends one CSV row per new sensor measurement.

use crate::aranet::Reading;
use chrono::{DateTime, Local};
use std::io::Write;
use std::path::Path;

const HEADER: &str = "time,sensor,co2,temp_c,humidity,pressure,battery\n";

fn opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

pub fn append(path: &Path, measured_at: DateTime<Local>, sensor: &str, r: &Reading) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let new = !path.exists();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    if new {
        f.write_all(HEADER.as_bytes())?;
    }
    writeln!(
        f,
        "{},{},{},{},{},{},{}",
        measured_at.format("%Y-%m-%dT%H:%M:%S"),
        sensor,
        opt(r.co2),
        opt(r.temperature.map(|t| format!("{t:.1}"))),
        r.humidity,
        opt(r.pressure.map(|p| format!("{p:.1}"))),
        r.battery,
    )
}
