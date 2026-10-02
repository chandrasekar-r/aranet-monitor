#import "sparkle_updater.h"
#import <AppKit/AppKit.h>
#import <Sparkle/Sparkle.h>

static SPUStandardUpdaterController *gUpdaterController;

void aranetbar_sparkle_start(void) {
    if (gUpdaterController != nil) {
        return;
    }
    gUpdaterController =
        [[SPUStandardUpdaterController alloc] initWithStartingUpdater:YES
                                                       updaterDelegate:nil
                                                    userDriverDelegate:nil];
}

void aranetbar_sparkle_configure_menu_item(id item) {
    if (gUpdaterController == nil || item == nil) {
        return;
    }
    NSMenuItem *menuItem = (NSMenuItem *)item;
    menuItem.target = gUpdaterController;
    menuItem.action = @selector(checkForUpdates:);
}
