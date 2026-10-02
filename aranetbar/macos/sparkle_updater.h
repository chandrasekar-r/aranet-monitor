#pragma once

#include <objc/objc.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Starts Sparkle’s standard updater (automatic background checks per Info.plist).
void aranetbar_sparkle_start(void);

/// Wires a menu item to Sparkle’s “Check for Updates…” action and KVO enablement.
void aranetbar_sparkle_configure_menu_item(id item);

#ifdef __cplusplus
}
#endif
