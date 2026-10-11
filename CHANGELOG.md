# Changelog

All notable changes to this repository are documented here.

## [0.4.0] - 2026-10-11

### Changed

- **AranetBar:** Menu bar popover redesigned in the same native AppKit UI. Dark vibrancy (HUD material) in dark mode and a standard popover material in light mode, with a 16px corner, 16px padding, and a 1px inner hairline. The pinned sensor shows a drifting air hero colored by CO₂ level, an SF Pro Rounded ppm readout with a Good / Fair / Poor pill, a read-only 400–2,000+ gauge, and chips for temperature, humidity, pressure, and battery. The footer shows Listening, Scanning…, or Not seen, and the settings menu. History stays on the header button and in the menu. The menu bar item is an air-quality symbol plus the live ppm, tinted when the level is Fair or Poor.
- Other sensors in range stay in the popover: click to pin, chevron to rename.

## [0.3.0] - 2026-10-02

### Added

- **AranetBar (Golden Gate):** Shared App Group state for WidgetKit / Control Center / App Intents; CO₂ alert snooze (1 h) with Control Center reload FFI; Focus filter mode (`pinned_only`) with `UNNotification` `filterCriteria`; compact desktop widget sources; `NSGlassEffectView` popover root on macOS 27+ with `NSVisualEffectView` fallback; Siri / Spotlight `AppEntity` bridge and `QueryCo2Intent`.
- **Docs:** `macos/GoldenGate/README.md` for Xcode extension targets, App Group IDs, and signing.

## [0.2.1] - 2026-10-02

### Added

- **AranetBar:** Sparkle 2 in-app updates — automatic daily update checks, “Check for Updates…” in the settings menu, signed appcast published on GitHub releases.
- **CI:** Linux `cargo test --lib` and clippy; macOS release workflow for `AranetBar.zip`, signed appcast, and GitHub release assets.

## [0.2.0] - (prior release)

- Native macOS menu bar app with BLE monitoring, alerts, SQLite history, and launch-at-login.

[0.4.0]: https://github.com/chandrasekar-r/aranet-monitor/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/chandrasekar-r/aranet-monitor/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/chandrasekar-r/aranet-monitor/compare/v0.2.0...v0.2.1
