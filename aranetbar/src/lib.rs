//! Core logic for AranetBar. macOS UI and menu bar live behind the `macos-app` feature.

pub mod alerts;
pub mod aranet;
#[cfg(all(feature = "macos-app", target_os = "macos"))]
pub mod ble;
pub mod config;
pub mod csvlog;
pub mod db;
pub mod history;
pub mod telegram;
pub mod viewmodel;

#[cfg(all(feature = "macos-app", target_os = "macos"))]
pub mod app;
#[cfg(all(feature = "macos-app", target_os = "macos"))]
pub mod login;
#[cfg(all(feature = "macos-app", target_os = "macos"))]
pub mod notify;
#[cfg(all(feature = "macos-app", target_os = "macos"))]
pub mod ui;
#[cfg(all(feature = "macos-app", target_os = "macos"))]
pub mod updater;
