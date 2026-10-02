//! In-app updates via [Sparkle](https://sparkle-project.org/) 2.

use objc2::rc::Retained;
use objc2_app_kit::NSMenuItem;

unsafe extern "C" {
    fn aranetbar_sparkle_start();
    fn aranetbar_sparkle_configure_menu_item(item: *mut objc2::runtime::AnyObject);
}

/// Must run on the main thread before the event loop handles user input.
pub fn start() {
    unsafe { aranetbar_sparkle_start() };
}

/// Connects a settings-menu item to Sparkle’s standard “Check for Updates…” behavior.
pub fn configure_menu_item(item: &Retained<NSMenuItem>) {
    let ptr = Retained::as_ptr(item) as *mut objc2::runtime::AnyObject;
    unsafe { aranetbar_sparkle_configure_menu_item(ptr) };
}
