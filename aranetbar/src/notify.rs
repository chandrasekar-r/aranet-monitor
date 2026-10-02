//! macOS notifications via UNUserNotificationCenter. Only works when running
//! from a signed .app bundle; otherwise falls back to `osascript` so
//! `cargo run` still shows alerts during development.

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread};
use objc2_foundation::{NSBundle, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationSettings, UNNotificationSound,
    UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};

define_class!(
    // Lets banners show even while our menu is open (app is "frontmost").
    #[unsafe(super(NSObject))]
    #[name = "AranetBarNotificationDelegate"]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            handler.call((UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::List
                | UNNotificationPresentationOptions::Sound,));
        }
    }
);

impl Delegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

fn in_bundle() -> bool {
    NSBundle::mainBundle().bundleIdentifier().is_some()
}

/// Requests permission and installs the delegate. `on_status(true)` means
/// notifications are allowed. Must be called on the main thread.
pub fn init(on_status: impl Fn(bool) + Clone + 'static) {
    if !in_bundle() {
        on_status(true);
        return;
    }
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let delegate = Delegate::new();
    center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    // The center only holds a weak reference; keep the delegate for the app's lifetime.
    std::mem::forget(delegate);

    let block = RcBlock::new(move |granted: Bool, _err: *mut NSError| on_status(granted.as_bool()));
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &block,
    );
}

/// Re-checks whether notifications are currently allowed.
pub fn check(on_status: impl Fn(bool) + 'static) {
    if !in_bundle() {
        return;
    }
    let block = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
        let status = unsafe { settings.as_ref() }.authorizationStatus();
        on_status(status == UNAuthorizationStatus::Authorized || status == UNAuthorizationStatus::Provisional);
    });
    UNUserNotificationCenter::currentNotificationCenter().getNotificationSettingsWithCompletionHandler(&block);
}

/// Shows a notification. `thread` groups notifications (one per sensor).
pub fn send(thread: &str, title: &str, body: &str) {
    if !in_bundle() {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("display notification \"{}\" with title \"{}\" sound name \"Glass\"", esc(body), esc(title));
        let _ = std::process::Command::new("osascript").args(["-e", &script]).spawn();
        return;
    }
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setThreadIdentifier(&NSString::from_str(thread));
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    let id = NSString::from_str(&format!("{thread}-{}", COUNTER.fetch_add(1, Ordering::Relaxed)));
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);
    UNUserNotificationCenter::currentNotificationCenter().addNotificationRequest_withCompletionHandler(&request, None);
}
