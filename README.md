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

## Building aranetbar

Requires a Mac with Xcode command line tools and Rust.

```sh
cd aranetbar
./bundle.sh          # builds release, signs ad-hoc, installs to ~/Applications
```

Or manually:

```sh
cargo build --release
open target/release/aranetbar
```

macOS will ask for Bluetooth permission the first time — grant it in System Settings → Privacy & Security → Bluetooth.

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
