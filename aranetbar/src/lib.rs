//! Library surface for unit tests and the `aranetbar` binary.

pub mod alerts;
pub mod aranet;
pub mod config;
pub mod csvlog;
pub mod db;
pub mod history;
pub mod viewmodel;

#[cfg(target_os = "macos")]
pub mod app;
#[cfg(target_os = "macos")]
pub mod ble;
#[cfg(target_os = "macos")]
pub mod login;
#[cfg(target_os = "macos")]
pub mod notify;
#[cfg(target_os = "macos")]
pub mod telegram;
#[cfg(target_os = "macos")]
pub mod ui;
