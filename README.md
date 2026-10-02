# aranet-monitor

Watch [Aranet4](https://aranet.com) CO₂ sensors over Bluetooth Low Energy and get alerted before the air at your desk goes stale.

Two pieces, same idea:

- **`aranetbar/`** — a native macOS menu bar app written in Rust (the main project). It listens passively to Aranet4 BLE advertisements, shows live CO₂ / temperature / humidity / pressure in the menu bar, logs readings to a local SQLite database, charts history, and raises macOS notifications (and optional Telegram alerts) at configurable thresholds.
- **`monitor.py`** — the original 60-line prototype that does the same via passive BLE scanning and macOS notification banners.

## aranetbar features

- Passive listening to Aranet4 advertisements — no pairing, no cloud
- Live menu bar readout with sparkline history
- CO₂, temperature, humidity, pressure and battery
- Configurable warning / high / clear thresholds with hysteresis
- macOS notifications via `UNUserNotificationCenter`
- Optional Telegram bot alerts for when you're away from the desk
- Local SQLite history with a history view
- Launch-at-login via `SMAppService`
- CSV export
- In-app updates via [Sparkle 2](https://sparkle-project.org/) (automatic daily checks and **Check for Updates…** in the settings menu)

## Building aranetbar

Requires **macOS 14 (Sonoma)** or later, Xcode command line tools, and Rust.

Pre-built **DMG** and **ZIP** for each tagged release are on the [GitHub Releases](https://github.com/chandrasekar-r/aranet-monitor/releases) page (built by CI on macOS).

```sh
cd aranetbar
./bundle.sh          # fetches Sparkle, builds release, signs ad-hoc, installs to ~/Applications
./package-release.sh # builds target/AranetBar-v*.dmg and .zip (same as CI)
```

Or manually:

```sh
./scripts/fetch_sparkle.sh
cargo build --release --features macos-app
open target/release/aranetbar
```

Linux CI runs library tests only (`cargo test --lib`); the menu bar binary is macOS-only.

macOS will ask for Bluetooth permission the first time — grant it in System Settings → Privacy & Security → Bluetooth.

### In-app updates (maintainers)

Sparkle reads `SUFeedURL`, `SUPublicEDKey`, and version fields from `macos/Info.plist`. The feed URL is the **latest GitHub release** asset:

`https://github.com/chandrasekar-r/aranet-monitor/releases/latest/download/appcast.xml`

A copy is also kept in-repo at `aranetbar/macos/appcast.xml` for review; release workflow publishes the signed feed on each tag.

**One-time EdDSA keys** (on a Mac, after `./scripts/fetch_sparkle.sh`):

```sh
macos/Frameworks/sparkle-tools/generate_keys
```

Save the private key for CI and put the **public** key in `Info.plist` (`SUPublicEDKey`) and in the GitHub secret `SPARKLE_EDDSA_PUBLIC_KEY`.

**GitHub Actions secrets** for tagged releases (`v*`):

| Secret | Purpose |
|--------|---------|
| `SPARKLE_EDDSA_PRIVATE_KEY` | Signs update archives and appcast entries |
| `SPARKLE_EDDSA_PUBLIC_KEY` | Injected into `Info.plist` at release build time |

**Release checklist**

1. Bump `version` in `aranetbar/Cargo.toml`, `CFBundleShortVersionString`, and increment `CFBundleVersion` in `macos/Info.plist`; update `CHANGELOG.md`.
2. Merge to `main`, then tag `vX.Y.Z` (e.g. `v0.2.1`) and push the tag.
3. The [macOS release](.github/workflows/release-macos.yml) workflow builds versioned DMG/ZIP, `AranetBar.zip` (Sparkle update archive), and a signed `appcast.xml`, and attaches them to the GitHub release.
4. For distribution outside GitHub, optionally **notarize** the `.app` or `.zip` with your Apple Developer ID before archiving; Sparkle expects a zip of the `.app` bundle. Ad-hoc builds from `./bundle.sh` update-check in dev but production feeds should use release-signed assets.

For local dev without matching signatures, Sparkle still loads but will reject unsigned feeds until you use release-built artifacts or a test appcast.

## Running the Python monitor

```sh
python3 -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
python monitor.py
```

Edit `NAME` in `monitor.py` to match your sensor's BLE name, then leave it running. Readings append to `readings.csv`; notifications fire via `osascript`.

## Default thresholds

| Level | CO₂ | Behavior |
|-------|------|----------|
| 🟢 fresh | < 800 ppm | clears after a high/warn alert |
| 🟠 rising | 1000–1400 ppm | "time to ventilate" |
| 🔴 high | ≥ 1400 ppm | "open a window / take a break" |

## License

MIT — see [LICENSE](LICENSE).
