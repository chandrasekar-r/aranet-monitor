"""Watch one Aranet4 sensor over BLE advertisements and raise macOS notifications."""
import asyncio, csv, datetime, os, subprocess
import aranet4
from bleak import BleakScanner

NAME = "Aranet4 0874F"
POLL_SECONDS = 300
SCAN_SECONDS = 25
WARN_CO2, HIGH_CO2, CLEAR_CO2 = 1000, 1400, 800   # ppm; clear threshold gives hysteresis
MISSING_ALERT_MIN = 30
DIR = os.path.dirname(os.path.abspath(__file__))
LOG = os.path.join(DIR, "readings.csv")

def notify(title, msg, sound="Glass"):
    subprocess.run(["osascript", "-e",
        f'display notification "{msg}" with title "{title}" sound name "{sound}"'])
    print(f"{datetime.datetime.now():%F %T} ALERT {title}: {msg}", flush=True)

async def read_once():
    result = None
    def cb(dev, adv):
        nonlocal result
        if dev.name == NAME:
            a = aranet4.client.Aranet4Advertisement(dev, adv)
            if a.readings: result = a.readings
    s = BleakScanner(detection_callback=cb)
    await s.start()
    for _ in range(SCAN_SECONDS * 2):
        if result: break
        await asyncio.sleep(0.5)
    await s.stop()
    return result

async def main():
    level = "ok"          # ok | warn | high
    last_seen = datetime.datetime.now()
    missing_alerted = False
    new_file = not os.path.exists(LOG)
    print(f"{datetime.datetime.now():%F %T} monitoring {NAME}", flush=True)
    while True:
        now = datetime.datetime.now()
        try:
            r = await read_once()
        except Exception as e:
            r = None
            print(f"{now:%F %T} scan error: {e}", flush=True)
        if r:
            last_seen, missing_alerted = now, False
            with open(LOG, "a", newline="") as f:
                w = csv.writer(f)
                if new_file:
                    w.writerow(["time", "co2", "temp_c", "humidity", "pressure", "battery"]); new_file = False
                w.writerow([now.isoformat(timespec="seconds"), r.co2, r.temperature, r.humidity, r.pressure, r.battery])
            print(f"{now:%F %T} CO2={r.co2} T={r.temperature} RH={r.humidity}", flush=True)
            if r.co2 >= HIGH_CO2 and level != "high":
                level = "high"; notify("🔴 CO₂ very high", f"{r.co2} ppm at your desk — open a window / take a break", "Sosumi")
            elif WARN_CO2 <= r.co2 < HIGH_CO2 and level == "ok":
                level = "warn"; notify("🟠 CO₂ rising", f"{r.co2} ppm — time to ventilate")
            elif r.co2 < CLEAR_CO2 and level != "ok":
                level = "ok"; notify("🟢 Air is fresh again", f"CO₂ back to {r.co2} ppm")
            if r.battery <= 10 and now.hour == 9 and now.minute < POLL_SECONDS // 60:
                notify("🔋 Aranet battery low", f"{r.battery}% left")
        elif not missing_alerted and (now - last_seen).total_seconds() > MISSING_ALERT_MIN * 60:
            missing_alerted = True
            notify("Aranet sensor not found", f"No reading from {NAME} for {MISSING_ALERT_MIN}+ min")
        await asyncio.sleep(POLL_SECONDS)

asyncio.run(main())
