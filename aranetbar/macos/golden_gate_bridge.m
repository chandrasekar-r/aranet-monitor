#import "golden_gate_bridge.h"

#import <AppKit/AppKit.h>
#include <dlfcn.h>

void *aranetbar_popover_root_view(void) {
    @autoreleasepool {
        if (@available(macOS 27.0, *)) {
            Class glass = NSClassFromString(@"NSGlassEffectView");
            if (glass) {
                NSView *view = [[glass alloc] initWithFrame:NSMakeRect(0, 0, 340, 400)];
                if ([view respondsToSelector:@selector(setMaterial:)]) {
                    // NSGlassEffectView uses material-like API on newer SDKs.
                }
                return (__bridge_retained void *)view;
            }
        }
        NSVisualEffectView *effect = [[NSVisualEffectView alloc] initWithFrame:NSMakeRect(0, 0, 340, 400)];
        effect.material = NSVisualEffectMaterialPopover;
        effect.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        effect.state = NSVisualEffectStateActive;
        return (__bridge_retained void *)effect;
    }
}

static void reload_controls(BOOL all) {
    // ControlCenter has no Objective-C header on current SDKs; resolve at runtime.
    if (@available(macOS 26.0, *)) {
        Class cc = NSClassFromString(@"ControlCenter");
        if (cc == nil) {
            return;
        }
        id shared = [cc performSelector:@selector(shared)];
        if (shared == nil) {
            return;
        }
        if (all) {
            [shared performSelector:@selector(reloadAllControls)];
        } else {
            [shared performSelector:@selector(reloadControlsOfKind:)
                         withObject:@"com.20deg.aranetbar.mute-co2"];
        }
    }
}

static void reload_widgets(void) {
    // WidgetCenter likewise: resolve at runtime instead of relying on headers.
    if (@available(macOS 11.0, *)) {
        Class wc = NSClassFromString(@"WidgetCenter");
        if (wc == nil) {
            return;
        }
        id shared = [wc performSelector:@selector(shared)];
        if (shared == nil) {
            return;
        }
        [shared performSelector:@selector(reloadTimelinesOfKind:)
                     withObject:@"com.20deg.aranetbar.co2-widget"];
    }
}

void aranetbar_reload_control_center(void) {
    reload_controls(NO);
    reload_widgets();
}

void aranetbar_reload_all_control_center(void) {
    reload_controls(YES);
}

void aranetbar_refresh_app_intents(void) {
    // Implemented in Swift (AranetBarIntents); linked when extensions package is
    // built. Resolve at runtime so the main binary links without it — weak_import
    // references are promoted to strong by the linker on current toolchains.
    void (*refresh)(void) = (void (*)(void))dlsym(RTLD_DEFAULT, "AranetBarRefreshAppIntents");
    if (refresh != NULL) {
        refresh();
    }
}
