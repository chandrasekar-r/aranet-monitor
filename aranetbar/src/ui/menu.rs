//! Standard application menu for an accessory (menu bar) app.

use super::{Command, views::Target};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::sel;
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
use objc2_foundation::NSString;

pub fn install(mtm: MainThreadMarker, target: &Target) {
    let app_menu = NSMenu::new(mtm);
    app_menu.addItem(&item(mtm, target, "About AranetBar", sel!(menuPicked:), Command::About, ""));
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    app_menu.addItem(&item(mtm, target, "Alert settings…", sel!(menuPicked:), Command::AlertSettings, ","));
    app_menu.addItem(&item(mtm, target, "History…", sel!(menuPicked:), Command::ShowHistory, "h"));
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    app_menu.addItem(&item(mtm, target, "Quit AranetBar", sel!(menuPicked:), Command::Quit, "q"));

    let app_item = NSMenuItem::new(mtm);
    app_item.setSubmenu(&app_menu);
    let main = NSMenu::new(mtm);
    main.addItem(&app_item);
    NSApplication::sharedApplication(mtm).setMainMenu(Some(&main));
}

fn item(
    mtm: MainThreadMarker,
    target: &Target,
    title: &str,
    action: Sel,
    cmd: Command,
    key: &str,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(action),
            &NSString::from_str(key),
        )
    };
    unsafe { item.setTarget(Some(target)) };
    item.setTag(cmd.tag());
    item
}
