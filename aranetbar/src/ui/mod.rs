//! Menu bar item and popover, drawn from a `ViewModel`. Main thread only.

mod history_window;
mod menu;
mod settings;
mod views;

pub use settings::Reply as SettingsReply;

use crate::aranet::short_name;
use crate::viewmodel::{self, Hero, Row, Title, Tone, ViewModel, tone_state_label, tone_symbol};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityLayoutChangedNotification, NSAlert, NSAlertFirstButtonReturn, NSApplication,
    NSBackingStoreType, NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSFont, NSFontTextStyleBody,
    NSFontTextStyleCaption1, NSFontTextStyleFootnote, NSFontTextStyleLargeTitle, NSFontTextStyleSubheadline,
    NSFontWeight, NSFontWeightMedium, NSFontWeightRegular, NSFontWeightSemibold, NSForegroundColorAttributeName,
    NSFontAttributeName, NSImage, NSImageScaling, NSImageView, NSMenu, NSMenuItem, NSPopover, NSPopoverBehavior,
    NSStatusBar, NSStatusItem, NSTextField, NSVariableStatusItemLength, NSView, NSViewController, NSVisualEffectBlendingMode,
    NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindow, NSWindowStyleMask, NSEvent, NSEventMask,
    NSCellImagePosition,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSMutableAttributedString, NSPoint, NSRectEdge, NSSize, NSString};
use std::cell::RefCell;
use std::path::Path;
use std::ptr::NonNull;
use std::rc::Rc;
use views::{
    ChartView, FlippedView, RowView, ShapeView, Target, banner_fill_color, rect, tone_color,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    About,
    AlertSettings,
    ShowHistory,
    ToggleLogin,
    OpenLog,
    ShowData,
    OpenConfig,
    ReloadConfig,
    TestAlert,
    Quit,
}

impl Command {
    const ALL: [Command; 10] = [
        Self::About,
        Self::AlertSettings,
        Self::ShowHistory,
        Self::ToggleLogin,
        Self::OpenLog,
        Self::ShowData,
        Self::OpenConfig,
        Self::ReloadConfig,
        Self::TestAlert,
        Self::Quit,
    ];

    fn tag(self) -> isize {
        Self::ALL.iter().position(|c| *c == self).unwrap() as isize + 1
    }

    fn from_tag(tag: isize) -> Option<Self> {
        Self::ALL.get(usize::try_from(tag - 1).ok()?).copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    TogglePopover,
    Settings,
    Pin(String),
    Rename(String),
    Command(Command),
}

const W: f64 = 340.0;
const PAD: f64 = 16.0;
const INNER: f64 = W - 2.0 * PAD;
const GAP_S: f64 = 8.0;
const GAP_M: f64 = 16.0;
const ROW_H: f64 = 32.0;

pub struct Ui {
    mtm: MainThreadMarker,
    status_item: Retained<NSStatusItem>,
    popover: Retained<NSPopover>,
    effect_root: Retained<NSVisualEffectView>,
    content: Retained<FlippedView>,
    gear: Retained<NSButton>,
    target: Retained<Target>,
    sheet_parent: Retained<NSWindow>,
    escape_monitor: RefCell<Option<Retained<AnyObject>>>,
    history_windows: RefCell<Vec<Retained<NSWindow>>>,
    on_action: Rc<dyn Fn(UiAction)>,
    vm: Option<ViewModel>,
}

impl Ui {
    pub fn new(mtm: MainThreadMarker, on_action: Rc<dyn Fn(UiAction)>) -> Self {
        let cb = on_action.clone();
        let target = Target::new(mtm, Box::new(move |a| cb(a)));
        menu::install(mtm, &target);

        let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = status_item.button(mtm) {
            unsafe {
                button.setTarget(Some(&target));
                button.setAction(Some(sel!(statusClicked:)));
            }
            button.setAccessibilityLabel(Some(&NSString::from_str("AranetBar")));
        }

        let effect_root = NSVisualEffectView::new(mtm);
        effect_root.setMaterial(NSVisualEffectMaterial::Popover);
        effect_root.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect_root.setState(NSVisualEffectState::Active);

        let content = FlippedView::new(mtm, rect(0.0, 0.0, W, 400.0));
        content.setAutoresizingMask(
            objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
                | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        effect_root.addSubview(&content);

        let controller = NSViewController::new(mtm);
        controller.setView(&effect_root);
        let popover = NSPopover::new(mtm);
        popover.setBehavior(NSPopoverBehavior::Transient);
        popover.setContentViewController(Some(&controller));

        let gear_image = symbol("gearshape", "Settings");
        let gear = unsafe {
            NSButton::buttonWithImage_target_action(&gear_image, Some(&target), Some(sel!(gearClicked:)), mtm)
        };
        gear.setBordered(false);
        gear.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        gear.setAccessibilityLabel(Some(&NSString::from_str("Settings")));
        gear.setAccessibilityHelp(Some(&NSString::from_str("Opens the settings menu")));

        let sheet_parent = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(0.0, 0.0, 1.0, 1.0),
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };

        Self {
            mtm,
            status_item,
            popover,
            effect_root,
            content,
            gear,
            target,
            sheet_parent,
            escape_monitor: RefCell::new(None),
            history_windows: RefCell::new(Vec::new()),
            on_action,
            vm: None,
        }
    }

    pub fn render(&mut self, vm: ViewModel) {
        if self.vm.as_ref() == Some(&vm) {
            return;
        }
        self.set_title(&vm);
        self.vm = Some(vm);
        if self.popover.isShown() {
            self.layout();
        }
    }

    pub fn toggle(&self) {
        if self.popover.isShown() {
            self.close();
            return;
        }
        let Some(button) = self.status_item.button(self.mtm) else { return };
        self.layout();
        NSApplication::sharedApplication(self.mtm).activate();
        self.popover.showRelativeToRect_ofView_preferredEdge(button.bounds(), &button, NSRectEdge::MinY);
        self.install_escape_monitor();
        if self.vm.as_ref().is_some_and(|vm| vm.banner.is_some()) {
            unsafe {
                objc2_app_kit::NSAccessibilityPostNotification(
                    self.content.as_ref(),
                    NSAccessibilityLayoutChangedNotification,
                );
            }
        }
    }

    pub fn close(&self) {
        self.remove_escape_monitor();
        self.popover.close();
    }

    pub fn show_settings(&self, login_enabled: bool) {
        let menu = NSMenu::new(self.mtm);
        let add = |title: &str, cmd: Command, key: &str| {
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(self.mtm),
                    &NSString::from_str(title),
                    Some(sel!(menuPicked:)),
                    &NSString::from_str(key),
                )
            };
            unsafe { item.setTarget(Some(&self.target)) };
            item.setTag(cmd.tag());
            menu.addItem(&item);
            item
        };
        if let Some(hero) = self.vm.as_ref().map(|vm| &vm.hero) {
            let label = hero.nickname.as_deref().unwrap_or(&hero.id);
            menu.addItem(&self.sensor_item(&format!("Rename “{label}”…"), sel!(renameSensor:), &hero.name));
            menu.addItem(&NSMenuItem::separatorItem(self.mtm));
        }
        add("Alert settings…", Command::AlertSettings, ",");
        add("History…", Command::ShowHistory, "h");
        menu.addItem(&NSMenuItem::separatorItem(self.mtm));
        let login = add("Start at login", Command::ToggleLogin, "");
        login.setState(if login_enabled { NSControlStateValueOn } else { NSControlStateValueOff });
        add("Open readings log", Command::OpenLog, "");
        add("Show database in Finder", Command::ShowData, "");
        add("Open config file", Command::OpenConfig, "");
        add("Reload config", Command::ReloadConfig, "");
        add("Send test alert", Command::TestAlert, "");
        menu.addItem(&NSMenuItem::separatorItem(self.mtm));
        add("Quit AranetBar", Command::Quit, "q");
        let h = self.gear.bounds().size.height;
        menu.popUpMenuPositioningItem_atLocation_inView(None, NSPoint::new(0.0, h + 4.0), Some(&self.gear));
    }

    pub fn show_history(&self, db_path: &Path, log_path: &Path, nicknames: &std::collections::BTreeMap<String, String>) {
        let window = history_window::show(self.mtm, db_path, log_path, nicknames);
        self.history_windows.borrow_mut().push(window);
    }

    pub fn show_about(&self) {
        let alert = NSAlert::new(self.mtm);
        alert.setMessageText(&NSString::from_str("AranetBar"));
        alert.setInformativeText(&NSString::from_str(
            "Passive Aranet4 CO₂ monitor for the menu bar.\n\nVersion 0.2.0",
        ));
        alert.addButtonWithTitle(&NSString::from_str("OK"));
        NSApplication::sharedApplication(self.mtm).activate();
        alert.runModal();
    }

    pub fn prompt_rename(&self, name: &str, current: Option<&str>) -> Option<String> {
        let m = self.mtm;
        let alert = NSAlert::new(m);
        alert.setMessageText(&NSString::from_str("Rename sensor"));
        alert.setInformativeText(&NSString::from_str(&format!(
            "Give {name} a name, like the room it's in. Leave empty to show its ID."
        )));
        alert.addButtonWithTitle(&NSString::from_str("Save"));
        alert.addButtonWithTitle(&NSString::from_str("Cancel"));
        let field = NSTextField::textFieldWithString(&NSString::from_str(current.unwrap_or("")), m);
        field.setPlaceholderString(Some(&NSString::from_str(short_name(name))));
        field.setFrame(rect(0.0, 0.0, 240.0, 24.0));
        alert.setAccessoryView(Some(&field));
        alert.window().setInitialFirstResponder(Some(&field));
        NSApplication::sharedApplication(m).activate();
        (alert.runModal() == NSAlertFirstButtonReturn).then(|| field.stringValue().to_string())
    }

    pub fn prompt_alert_settings(&self, c: &crate::config::Config, error: Option<&str>) -> SettingsReply {
        settings::prompt(self.mtm, Some(&self.sheet_parent), c, error)
    }

    fn sensor_item(&self, title: &str, action: objc2::runtime::Sel, name: &str) -> Retained<NSMenuItem> {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(self.mtm),
                &NSString::from_str(title),
                Some(action),
                &NSString::new(),
            )
        };
        unsafe {
            item.setTarget(Some(&self.target));
            item.setRepresentedObject(Some(&NSString::from_str(name)));
        }
        item
    }

    fn row_menu(&self, name: &str) -> Retained<NSMenu> {
        let menu = NSMenu::new(self.mtm);
        menu.addItem(&self.sensor_item("Pin to menu bar", sel!(pinSensor:), name));
        menu.addItem(&self.sensor_item("Rename…", sel!(renameSensor:), name));
        menu
    }

    fn set_title(&self, vm: &ViewModel) {
        let Some(button) = self.status_item.button(self.mtm) else { return };
        let (symbol_name, value, tone) = match &vm.title {
            Title::Reading { tone, text } => (tone_symbol(*tone), text.as_str(), Some(*tone)),
            Title::NoData => ("circle.dashed", "—", None),
            Title::Bluetooth => ("antenna.radiowaves.left.and.right.slash", "BT", None),
        };
        let img = template_symbol(symbol_name);
        button.setImage(Some(&img));
        button.setImagePosition(NSCellImagePosition::ImageLeft);
        let text = NSMutableAttributedString::new();
        text.appendAttributedString(&attributed(value, &NSColor::labelColor(), &mono_style(TextStyle::Subheadline, weight(Weight::Medium))));
        button.setAttributedTitle(&text);

        let ax_value = match &vm.title {
            Title::Reading { tone, text } => format!("CO₂ {text} ppm, {}", tone_state_label(*tone)),
            Title::NoData => "No sensor data".into(),
            Title::Bluetooth => "Bluetooth unavailable".into(),
        };
        unsafe {
            button.setAccessibilityValue(Some(&NSString::from_str(&ax_value)));
        }
        let _ = tone;
    }

    fn layout(&self) {
        let Some(vm) = &self.vm else { return };
        let c = &*self.content;
        for v in c.subviews().iter() {
            v.removeFromSuperview();
        }
        let mut y = PAD;

        if let Some((lead, rest)) = &vm.banner {
            y = self.banner(c, y, lead, rest) + GAP_M;
        }
        y = self.hero(c, y, &vm.hero);
        y = self.others(c, y, &vm.others);

        let size = NSSize::new(W, y + PAD);
        c.setFrameSize(size);
        self.effect_root.setFrameSize(size);
        self.popover.setContentSize(size);
    }

    fn banner(&self, c: &NSView, y: f64, lead: &str, rest: &str) -> f64 {
        let text_w = INNER - 48.0;
        let label = NSTextField::wrappingLabelWithString(&NSString::new(), self.mtm);
        let s = NSMutableAttributedString::new();
        s.appendAttributedString(&attributed(lead, &NSColor::labelColor(), &styled(TextStyle::Subheadline)));
        s.appendAttributedString(&attributed(&format!(" {rest}"), &NSColor::labelColor(), &styled(TextStyle::Body)));
        label.setAttributedStringValue(&s);
        label.setPreferredMaxLayoutWidth(text_w);
        let text_h = label.sizeThatFits(NSSize::new(text_w, 1000.0)).height.ceil();
        let h = text_h + GAP_M;
        add(c, &ShapeView::new(self.mtm, rect(PAD, y, INNER, h), banner_fill_color(), 8.0));
        let icon = image_view(self.mtm, "exclamationmark.triangle.fill", &NSColor::systemOrangeColor());
        icon.setFrame(rect(PAD + GAP_S, y + GAP_S, 16.0, 16.0));
        add(c, &icon);
        label.setFrame(rect(PAD + 32.0, y + GAP_S - 1.0, text_w, text_h));
        add(c, &label);
        label.setAccessibilityLabel(Some(&NSString::from_str(&format!("{lead} {rest}"))));
        y + h
    }

    fn hero(&self, c: &NSView, mut y: f64, h: &Hero) -> f64 {
        let m = self.mtm;
        let id = label(m, &h.id, styled(TextStyle::Subheadline), &NSColor::labelColor());
        place(c, &id, PAD, y + 4.0);
        if let Some(nick) = &h.nickname {
            let n = label(m, &format!("· {nick}"), styled(TextStyle::Body), &NSColor::secondaryLabelColor());
            place(c, &n, PAD + width(&id) + 4.0, y + 4.0);
        }
        self.gear.setFrame(rect(W - PAD - 28.0, y, 28.0, 28.0));
        add(c, &self.gear);
        y += 32.0;

        let big = label(m, &h.co2, mono_style(TextStyle::LargeTitle, weight(Weight::Semibold)), &NSColor::labelColor());
        place(c, &big, PAD, y);
        big.setAccessibilityLabel(Some(&NSString::from_str("CO₂")));
        unsafe {
            big.setAccessibilityValue(Some(&NSString::from_str(&format!("{} ppm", h.co2))));
        }
        let big_h = big.frame().size.height;
        let unit = label(m, "ppm CO₂", styled(TextStyle::Footnote), &NSColor::secondaryLabelColor());
        place(c, &unit, PAD + width(&big) + 4.0, y + big_h - unit.frame().size.height - 8.0);
        if !h.pill.is_empty() {
            self.pill(c, y + (big_h - 24.0) / 2.0, h.pill, h.tone);
        }
        y += big_h;
        let updated = label(m, &h.updated, font(TextStyle::Caption1), &NSColor::tertiaryLabelColor());
        place(c, &updated, PAD, y);
        y += height(&updated) + GAP_M;

        let chart_h = 52.0;
        let bar_w = (INNER / 18.0 - 2.0).max(3.0);
        let summary = format!(
            "CO₂ over the last 3 hours. Warning threshold {} ppm.",
            viewmodel::thousands(h.warn_co2)
        );
        let chart = ChartView::new(m, rect(PAD, y, INNER, chart_h), h.chart.clone(), bar_w, h.warn_co2, summary);
        add(c, &chart);
        let (lo, hi) = views::chart_scale(&h.chart);
        let line_from_top = chart_h - ((h.warn_co2 as f64 - lo) / (hi - lo)).clamp(0.0, 1.0) * chart_h;
        let thr = label(
            m,
            &viewmodel::thousands(h.warn_co2),
            mono_style(TextStyle::Caption2, weight(Weight::Regular)),
            &NSColor::tertiaryLabelColor(),
        );
        place(c, &thr, W - PAD - width(&thr), y + line_from_top - height(&thr) - 1.0);
        if h.chart.is_empty() {
            let empty = label(m, "No readings in the last 3 hours", font(TextStyle::Caption1), &NSColor::tertiaryLabelColor());
            place(c, &empty, PAD + (INNER - width(&empty)) / 2.0, y + chart_h - height(&empty) - 4.0);
        }
        y += chart_h + GAP_S;
        let axis = ["3 h ago", "2 h", "1 h", "Now"];
        for (i, text) in axis.iter().enumerate() {
            let l = label(m, text, font(TextStyle::Caption2), &NSColor::tertiaryLabelColor());
            let x = match i {
                0 => PAD,
                3 => W - PAD - width(&l),
                _ => PAD + INNER * i as f64 / 3.0 - width(&l) / 2.0,
            };
            place(c, &l, x, y);
        }
        y += GAP_M + GAP_M;

        if !h.stats.is_empty() {
            let gap = GAP_S - 2.0;
            let cw = (INNER - 3.0 * gap) / 4.0;
            for (i, s) in h.stats.iter().enumerate() {
                let x = PAD + i as f64 * (cw + gap);
                add(c, &ShapeView::quaternary_fill(m, rect(x, y, cw, 48.0), 8.0));
                let icon = image_view(m, s.symbol, &NSColor::secondaryLabelColor());
                icon.setFrame(rect(x + GAP_S, y + GAP_S, 12.0, 12.0));
                add(c, &icon);
                let cap = label(m, s.caption, font(TextStyle::Caption2), &NSColor::secondaryLabelColor());
                place(c, &cap, x + 24.0, y + 6.0);
                let v = label(m, &s.value, mono_style(TextStyle::Subheadline, weight(Weight::Semibold)), &NSColor::labelColor());
                place(c, &v, x + GAP_S, y + 22.0);
            }
            y += 48.0 + GAP_M;
        }
        y
    }

    fn pill(&self, c: &NSView, y: f64, text: &str, tone: Tone) {
        let color = tone_color(tone);
        let l = label(self.mtm, text, styled(TextStyle::Footnote), &color);
        let sym = image_view(self.mtm, tone_symbol(tone), &color);
        sym.setFrame(rect(0.0, 0.0, 14.0, 14.0));
        let w = GAP_S + 14.0 + 6.0 + width(&l) + GAP_S;
        let x = W - PAD - w;
        add(c, &ShapeView::new(self.mtm, rect(x, y, w, 24.0), color.colorWithAlphaComponent(0.16), 12.0));
        sym.setFrame(rect(x + GAP_S, y + 5.0, 14.0, 14.0));
        add(c, &sym);
        place(c, &l, x + GAP_S + 18.0, y + (24.0 - height(&l)) / 2.0);
        l.setAccessibilityLabel(Some(&NSString::from_str(text)));
        unsafe {
            l.setAccessibilityValue(Some(&NSString::from_str(tone_state_label(tone))));
        }
    }

    fn others(&self, c: &NSView, mut y: f64, rows: &[Row]) -> f64 {
        let m = self.mtm;
        y += GAP_S;
        let head = label(m, "Other sensors", styled(TextStyle::Footnote), &NSColor::secondaryLabelColor());
        place(c, &head, PAD, y);
        if !rows.is_empty() {
            let hint = label(
                m,
                "Click to pin · Control-click for menu",
                font(TextStyle::Caption1),
                &NSColor::tertiaryLabelColor(),
            );
            place(c, &hint, W - PAD - width(&hint), y);
        }
        y += height(&head) + GAP_S;

        if rows.is_empty() {
            let none = label(m, "No other sensors in range", styled(TextStyle::Body), &NSColor::tertiaryLabelColor());
            place(c, &none, PAD, y + GAP_S);
            return y + 32.0;
        }

        let group_h = rows.len() as f64 * ROW_H + GAP_S;
        add(c, &ShapeView::quaternary_fill(m, rect(PAD, y, INNER, group_h), 10.0));
        let mut row_y = y + GAP_S / 2.0;
        for (i, r) in rows.iter().enumerate() {
            if i > 0 {
                add(
                    c,
                    &ShapeView::new(
                        m,
                        rect(PAD + GAP_S, row_y, INNER - GAP_M, 1.0),
                        NSColor::separatorColor().colorWithAlphaComponent(0.8),
                        0.0,
                    ),
                );
            }
            self.row(c, row_y, r);
            row_y += ROW_H;
        }
        y + group_h + GAP_S
    }

    fn row(&self, c: &NSView, y: f64, r: &Row) {
        let m = self.mtm;
        let rw = INNER;
        let cb = self.on_action.clone();
        let ax_label = r.label.clone();
        let ax_value = row_accessibility_value(r);
        let view = RowView::new(
            m,
            rect(PAD, y, rw, ROW_H),
            r.name.clone(),
            self.row_menu(&r.name),
            Box::new(move |a| cb(a)),
            &ax_label,
            &ax_value,
        );
        add(c, &view);
        let muted = r.co2.is_none();
        let icon_name = if muted { "circle.dashed" } else { tone_symbol(r.tone) };
        let icon_color = if muted { NSColor::tertiaryLabelColor() } else { tone_color(r.tone) };
        let sym = image_view(m, icon_name, &icon_color);
        sym.setFrame(rect(GAP_M, (ROW_H - 14.0) / 2.0, 14.0, 14.0));
        add(&view, &sym);
        let name_color = if muted { NSColor::secondaryLabelColor() } else { NSColor::labelColor() };
        let name = label(m, &r.label, styled(TextStyle::Body), &name_color);
        place(&view, &name, 36.0, (ROW_H - height(&name)) / 2.0);

        let right = rw - GAP_S;
        if let Some(note) = &r.note {
            let n = label(m, note, font(TextStyle::Callout), &NSColor::tertiaryLabelColor());
            place(&view, &n, right - width(&n), (ROW_H - height(&n)) / 2.0);
            return;
        }
        let age = label(m, &r.age, mono_style(TextStyle::Caption1, weight(Weight::Regular)), &NSColor::tertiaryLabelColor());
        place(&view, &age, right - width(&age), (ROW_H - height(&age)) / 2.0);
        let temp = label(m, &r.temp, mono_style(TextStyle::Callout, weight(Weight::Regular)), &NSColor::secondaryLabelColor());
        place(&view, &temp, right - 48.0 - width(&temp), (ROW_H - height(&temp)) / 2.0);
        if let Some(co2) = &r.co2 {
            let s = NSMutableAttributedString::new();
            s.appendAttributedString(&attributed(co2, &NSColor::labelColor(), &mono_style(TextStyle::Body, weight(Weight::Semibold))));
            s.appendAttributedString(&attributed(" ppm", &NSColor::secondaryLabelColor(), &font(TextStyle::Caption1)));
            let l = NSTextField::labelWithAttributedString(&s, m);
            l.sizeToFit();
            place(&view, &l, right - 48.0 - 64.0 - width(&l), (ROW_H - height(&l)) / 2.0);
        }
    }

    fn install_escape_monitor(&self) {
        self.remove_escape_monitor();
        let popover = self.popover.clone();
        let monitor_cell = self.escape_monitor.clone();
        let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            let e = unsafe { event.as_ref() };
            if e.keyCode() == 53 {
                popover.close();
                if let Some(m) = monitor_cell.borrow().as_ref() {
                    unsafe { NSEvent::removeMonitor(m) };
                }
                *monitor_cell.borrow_mut() = None;
                return std::ptr::null_mut();
            }
            event.as_ptr()
        });
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)
        };
        *self.escape_monitor.borrow_mut() = monitor;
    }

    fn remove_escape_monitor(&self) {
        if let Some(m) = self.escape_monitor.borrow_mut().take() {
            unsafe { NSEvent::removeMonitor(&m) };
        }
    }
}

fn row_accessibility_value(r: &Row) -> String {
    if let Some(note) = &r.note {
        return note.clone();
    }
    let co2 = r.co2.as_deref().unwrap_or("—");
    format!("{co2} ppm, {}, updated {}", r.temp, r.age)
}

// ---------------------------------------------------------------- helpers

pub(crate) enum TextStyle {
    LargeTitle,
    Headline,
    Subheadline,
    Body,
    Callout,
    Footnote,
    Caption1,
    Caption2,
}

enum Weight {
    Regular,
    Medium,
    Semibold,
}

fn weight(w: Weight) -> NSFontWeight {
    unsafe {
        match w {
            Weight::Regular => NSFontWeightRegular,
            Weight::Medium => NSFontWeightMedium,
            Weight::Semibold => NSFontWeightSemibold,
        }
    }
}

fn style_ref(style: TextStyle) -> &'static objc2_app_kit::NSFontTextStyle {
    unsafe {
        match style {
            TextStyle::LargeTitle => NSFontTextStyleLargeTitle,
            TextStyle::Headline => objc2_app_kit::NSFontTextStyleHeadline,
            TextStyle::Subheadline => NSFontTextStyleSubheadline,
            TextStyle::Body => NSFontTextStyleBody,
            TextStyle::Callout => objc2_app_kit::NSFontTextStyleCallout,
            TextStyle::Footnote => NSFontTextStyleFootnote,
            TextStyle::Caption1 => NSFontTextStyleCaption1,
            TextStyle::Caption2 => objc2_app_kit::NSFontTextStyleCaption2,
        }
    }
}

pub(crate) fn styled(style: TextStyle) -> Retained<NSFont> {
    unsafe { NSFont::preferredFontForTextStyle_options(style_ref(style), &NSDictionary::new()) }
}

pub(crate) fn font(style: TextStyle) -> Retained<NSFont> {
    styled(style)
}

pub(crate) fn mono_style(style: TextStyle, w: NSFontWeight) -> Retained<NSFont> {
    let size = styled(style).pointSize();
    NSFont::monospacedDigitSystemFontOfSize_weight(size, w)
}

fn attributed(text: &str, color: &NSColor, font: &NSFont) -> Retained<NSAttributedString> {
    let keys = unsafe { [NSForegroundColorAttributeName, NSFontAttributeName] };
    let values: [&AnyObject; 2] = [color.as_ref(), font.as_ref()];
    let attrs = NSDictionary::from_slices(&keys, &values);
    unsafe { NSAttributedString::new_with_attributes(&NSString::from_str(text), &attrs) }
}

pub(crate) fn label(mtm: MainThreadMarker, text: &str, font: Retained<NSFont>, color: &NSColor) -> Retained<NSTextField> {
    let l = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    l.setFont(Some(&font));
    l.setTextColor(Some(color));
    l.sizeToFit();
    l
}

fn symbol(name: &str, description: &str) -> Retained<NSImage> {
    NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(name), Some(&NSString::from_str(description)))
        .unwrap_or_else(NSImage::new)
}

fn template_symbol(name: &str) -> Retained<NSImage> {
    let img = symbol(name, "");
    img.setTemplate(true);
    img
}

fn image_view(mtm: MainThreadMarker, name: &str, tint: &NSColor) -> Retained<NSImageView> {
    let v = NSImageView::imageViewWithImage(&symbol(name, ""), mtm);
    v.setContentTintColor(Some(tint));
    v.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    v
}

pub(crate) fn add(parent: &NSView, child: &NSView) {
    parent.addSubview(child);
}

pub(crate) fn place(parent: &NSView, child: &NSView, x: f64, y: f64) {
    child.setFrameOrigin(NSPoint::new(x.round(), y.round()));
    parent.addSubview(child);
}

pub(crate) fn width(v: &NSView) -> f64 {
    v.frame().size.width
}

pub(crate) fn height(v: &NSView) -> f64 {
    v.frame().size.height
}
