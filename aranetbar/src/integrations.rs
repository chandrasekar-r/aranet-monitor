//! macOS Golden Gate hooks (Control Center, App Intents donation, popover glass).

use crate::shared_state::GoldenGateState;
use crate::viewmodel::ViewModel;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use std::path::Path;

#[cfg(all(feature = "macos-app", target_os = "macos"))]
use crate::viewmodel::Tone;

#[cfg(all(feature = "macos-app", target_os = "macos"))]
use std::sync::atomic::{AtomicI64, Ordering};

#[cfg(all(feature = "macos-app", target_os = "macos"))]
static LAST_DB_SYNC: AtomicI64 = AtomicI64::new(0);

#[cfg(all(feature = "macos-app", target_os = "macos"))]
extern "C" {
    fn aranetbar_popover_root_view() -> *mut std::ffi::c_void;
    fn aranetbar_reload_control_center();
    fn aranetbar_reload_all_control_center();
    fn aranetbar_refresh_app_intents();
}

pub fn popover_root_view() -> *mut std::ffi::c_void {
    #[cfg(all(feature = "macos-app", target_os = "macos"))]
    {
        return unsafe { aranetbar_popover_root_view() };
    }
    #[cfg(not(all(feature = "macos-app", target_os = "macos")))]
    {
        std::ptr::null_mut()
    }
}

pub fn reload_controls() {
    #[cfg(all(feature = "macos-app", target_os = "macos"))]
    unsafe {
        aranetbar_reload_control_center();
    }
}

pub fn reload_all_controls() {
    #[cfg(all(feature = "macos-app", target_os = "macos"))]
    unsafe {
        aranetbar_reload_all_control_center();
    }
}

pub fn refresh_app_intents() {
    #[cfg(all(feature = "macos-app", target_os = "macos"))]
    unsafe {
        aranetbar_refresh_app_intents();
    }
}

pub fn sync_after_external_change(prev: &GoldenGateState) -> GoldenGateState {
    let mut state = crate::shared_state::merge_from_disk(prev.clone());
    #[cfg(all(feature = "macos-app", target_os = "macos"))]
    if state.co2_snooze_until != prev.co2_snooze_until || state.focus_alerts_mode != prev.focus_alerts_mode {
        reload_controls();
    }
    state
}

#[cfg(all(feature = "macos-app", target_os = "macos"))]
fn tone_name(t: Tone) -> &'static str {
    match t {
        Tone::Good => "fresh",
        Tone::Warn => "rising",
        Tone::High => "high",
        Tone::Muted => "unknown",
    }
}

pub fn publish(
    state: &mut GoldenGateState,
    vm: &ViewModel,
    pinned: &str,
    nicknames: &BTreeMap<String, String>,
    db_path: &Path,
    interval_secs: u32,
    latest_co2: Option<u16>,
) {
    #[cfg(all(feature = "macos-app", target_os = "macos"))]
    {
        let now = Utc::now();
        *state = crate::shared_state::merge_from_disk(state.clone());

        state.pinned_sensor = pinned.to_string();
        state.pinned_nickname = nicknames.get(pinned).cloned().unwrap_or_else(|| pinned.to_string());
        state.tone = tone_name(vm.hero.tone).to_string();
        state.latest_co2 = latest_co2;
        state.measurement_interval_secs = interval_secs.max(60);
        state.db_path = db_path.display().to_string();
        state.shared_db_path = crate::shared_state::shared_db_path().display().to_string();

        let snooze_before = state.co2_snooze_until;
        if let Err(e) = crate::shared_state::save(state) {
            eprintln!("golden gate state: {e}");
        }

        if state.co2_snooze_until != snooze_before {
            reload_controls();
        }

        let last = LAST_DB_SYNC.load(Ordering::Relaxed);
        let now_secs = now.timestamp();
        if now_secs - last > 60 {
            if let Err(e) = crate::shared_state::sync_db_snapshot(db_path) {
                eprintln!("golden gate db snapshot: {e}");
            }
            LAST_DB_SYNC.store(now_secs, Ordering::Relaxed);
        }

        refresh_app_intents();
    }
    let _ = (state, vm, pinned, nicknames, db_path, interval_secs, latest_co2);
}

pub fn set_snooze_until(state: &mut GoldenGateState, until: Option<DateTime<Utc>>) {
    state.set_snooze_until(until);
    let _ = crate::shared_state::save(state);
    reload_controls();
    reload_all_controls();
}

pub fn on_tick_snooze(state: &mut GoldenGateState) {
    let now = Utc::now();
    if state.co2_snooze_until.is_some_and(|u| u <= now) {
        state.set_snooze_until(None);
        let _ = crate::shared_state::save(state);
        reload_all_controls();
    }
}
