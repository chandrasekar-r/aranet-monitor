//! "Start at login" via SMAppService (macOS 13+). Requires the .app bundle.

use objc2_service_management::{SMAppService, SMAppServiceStatus};

pub fn is_enabled() -> bool {
    unsafe { SMAppService::mainAppService().status() == SMAppServiceStatus::Enabled }
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    let service = unsafe { SMAppService::mainAppService() };
    let result = unsafe {
        if on { service.registerAndReturnError() } else { service.unregisterAndReturnError() }
    };
    result.map_err(|e| e.localizedDescription().to_string())
}
