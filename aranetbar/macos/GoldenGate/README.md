# Golden Gate extensions (macOS 26–27)

Swift sources for WidgetKit **Control Center**, **desktop widget**, and **App Intents** (Siri / Spotlight entities, Focus filters).

## App Group

| Item | Value |
|------|--------|
| App Group ID | `group.com.20deg.aranetbar` |
| Shared state file | `golden_gate_state.json` in the group container |
| Control kind | `com.20deg.aranetbar.mute-co2` |
| Widget kind | `com.20deg.aranetbar.co2-widget` |

The Rust app writes `golden_gate_state.json` and periodically copies `readings.db` to `readings_shared.db` in the group container. Extensions read those files; toggling snooze from Control Center writes back to JSON and the main app picks it up on the next tick.

## Xcode targets (one-time)

1. Open `aranetbar/macos/GoldenGate/AranetBarGoldenGate.xcodeproj` (or add targets manually to your workspace).
2. Create three extension targets:
   - **AranetBarControlExtension** — platform macOS, type *Control Center*; sources: `Sources/AranetBarControlExtension/*.swift` + `AranetBarShared`.
   - **AranetBarWidgetExtension** — platform macOS, type *Widget Extension*; sources: `Sources/AranetBarWidgetExtension/*.swift` + `AranetBarShared`.
   - Embed both in **AranetBar.app** under `Contents/PlugIns/`.
3. Add **App Groups** capability to the main app and each extension (`group.com.20deg.aranetbar`).
4. Link the main app with the static library from `swift build` (see below).

Bundle identifiers (suggested):

- `com.20deg.aranetbar`
- `com.20deg.aranetbar.control`
- `com.20deg.aranetbar.widget`

## Build intents bridge (main app)

```sh
cd aranetbar/macos/GoldenGate
swift build -c release
# Link libAranetBarIntentsBridge.a + Swift runtime in build-app.sh (optional; weak symbol if omitted)
```

When the bridge is linked, `AranetBarRefreshAppIntents()` donates `AranetSensorEntity` values after each UI refresh.

## FFI: reload Control Center from Rust

When snooze changes in the Rust core, `integrations::set_snooze_until` / `on_tick_snooze` call:

- `aranetbar_reload_control_center()` — `ControlCenter.shared.reloadControls(ofKind:)`
- `aranetbar_reload_all_control_center()` — when snooze expires

Implemented in `macos/golden_gate_bridge.m`.

## Signing

Use the same Team ID for the app and extensions. For local dev, ad-hoc sign with entitlements:

```sh
codesign --force --sign - --entitlements macos/AranetBar.entitlements target/AranetBar.app
```

Production releases should use a Developer ID and enable App Groups on the App ID in the Apple Developer portal.
