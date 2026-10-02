#import "golden_gate_bridge.h"

#import <AppKit/AppKit.h>

#if __has_include(<WidgetKit/WidgetKit.h>)
#import <WidgetKit/WidgetKit.h>
#define ARANETBAR_HAS_WIDGETKIT 1
#else
#define ARANETBAR_HAS_WIDGETKIT 0
#endif

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
#if ARANETBAR_HAS_WIDGETKIT
    if (@available(macOS 26.0, *)) {
        if (all) {
            [ControlCenter.shared reloadAllControls];
        } else {
            [ControlCenter.shared reloadControlsOfKind:@"com.20deg.aranetbar.mute-co2"];
        }
    }
#else
    (void)all;
#endif
}

static void reload_widgets(void) {
#if ARANETBAR_HAS_WIDGETKIT
    if (@available(macOS 11.0, *)) {
        [WidgetCenter.shared reloadTimelinesOfKind:@"com.20deg.aranetbar.co2-widget"];
    }
#endif
}

void aranetbar_reload_control_center(void) {
    reload_controls(NO);
    reload_widgets();
}

void aranetbar_reload_all_control_center(void) {
    reload_controls(YES);
}

void aranetbar_refresh_app_intents(void) {
    // Implemented in Swift (AranetBarIntents); linked when extensions package is built.
    extern void AranetBarRefreshAppIntents(void) __attribute__((weak_import));
    if (AranetBarRefreshAppIntents) {
        AranetBarRefreshAppIntents();
    }
}
