#pragma once

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Creates an popover root view: NSGlassEffectView on macOS 27+, else NSVisualEffectView.
void *aranetbar_popover_root_view(void);

/// Tell WidgetKit to reload Control Center controls for AranetBar (no-op when unavailable).
void aranetbar_reload_control_center(void);

/// Reload all Control Center controls (e.g. when snooze ends).
void aranetbar_reload_all_control_center(void);

/// Donate / refresh App Intents entities from current golden_gate_state.json (Swift helper).
void aranetbar_refresh_app_intents(void);

#ifdef __cplusplus
}
#endif
