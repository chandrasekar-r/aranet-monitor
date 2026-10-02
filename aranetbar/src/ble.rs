//! Passive BLE scanning for Aranet4 advertisements on a background thread.
//! Never connects to a sensor; only listens to its broadcasts.

use crate::aranet::{self, DecodeError, Reading};
use btleplug::api::{Central, CentralEvent, CentralState, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, PeripheralId};
use futures::StreamExt;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Minimum gap between two forwarded updates of the same sensor. Sensors
/// re-broadcast every second or so but only measure every few minutes.
const THROTTLE: Duration = Duration::from_secs(15);
const RETRY_AFTER: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub enum BleStatus {
    Scanning,
    PoweredOff,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct Advert {
    pub name: String,
    pub data: Result<Reading, DecodeError>,
}

#[derive(Debug, Clone)]
pub enum BleEvent {
    Status(BleStatus),
    Advert(Advert),
}

pub fn spawn(send: impl Fn(BleEvent) + Send + 'static) {
    std::thread::Builder::new()
        .name("ble".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("tokio runtime");
            rt.block_on(async {
                loop {
                    if let Err(e) = scan(&send).await {
                        send(BleEvent::Status(BleStatus::Error(e.to_string())));
                    }
                    tokio::time::sleep(RETRY_AFTER).await;
                }
            });
        })
        .expect("spawn ble thread");
}

async fn scan(send: &impl Fn(BleEvent)) -> Result<(), Box<dyn std::error::Error>> {
    let manager = Manager::new().await?;
    let adapter = manager.adapters().await?.into_iter().next().ok_or("no Bluetooth adapter found")?;
    let mut events = adapter.events().await?;
    adapter.start_scan(ScanFilter::default()).await?;
    send(BleEvent::Status(BleStatus::Scanning));

    let mut names: HashMap<PeripheralId, String> = HashMap::new();
    let mut last_sent: HashMap<PeripheralId, Instant> = HashMap::new();

    while let Some(event) = events.next().await {
        match event {
            CentralEvent::StateUpdate(CentralState::PoweredOff) => send(BleEvent::Status(BleStatus::PoweredOff)),
            CentralEvent::StateUpdate(CentralState::PoweredOn) => {
                adapter.start_scan(ScanFilter::default()).await?;
                send(BleEvent::Status(BleStatus::Scanning));
            }
            CentralEvent::StateUpdate(_) => send(BleEvent::Status(BleStatus::Error(
                "Bluetooth unavailable — allow AranetBar in System Settings › Privacy & Security › Bluetooth".into(),
            ))),
            CentralEvent::ManufacturerDataAdvertisement { id, manufacturer_data } => {
                let Some(bytes) = manufacturer_data.get(&aranet::MANUFACTURER_ID) else { continue };
                if last_sent.get(&id).is_some_and(|t| t.elapsed() < THROTTLE) {
                    continue;
                }
                let name = identify(&adapter, &id, &mut names).await;
                let Some(name) = name.filter(|n| n.starts_with("Aranet4")) else { continue };
                last_sent.insert(id, Instant::now());
                send(BleEvent::Advert(Advert { name, data: aranet::decode(bytes) }));
            }
            _ => {}
        }
    }
    Err("Bluetooth event stream ended".into())
}

/// Resolves a peripheral's advertised name, cached because passive scans
/// can omit it.
async fn identify(
    adapter: &Adapter,
    id: &PeripheralId,
    names: &mut HashMap<PeripheralId, String>,
) -> Option<String> {
    if !names.contains_key(id)
        && let Ok(p) = adapter.peripheral(id).await
        && let Ok(Some(props)) = p.properties().await
        && let Some(n) = props.local_name
    {
        names.insert(id.clone(), n);
    }
    names.get(id).cloned()
}
