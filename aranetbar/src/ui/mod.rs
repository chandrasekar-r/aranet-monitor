//! Menu bar item and popover, drawn from a `ViewModel`. Main thread only.

mod settings;
mod views;

pub use settings::Reply as SettingsReply;

use crate::aranet::short_name;
use crate::viewmodel::{Hero, Row, Tone, ViewModel, Title};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSApplication, NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSFont, NSFontWeight,
    NSFontWeightMedium, NSFontWeightRegular, NSFontWeightSemibold, NSForegroundColorAttributeName,
    NSFontAttributeName, NSImage, NSImageScaling, NSImageView, NSMenu, NSMenuItem, NSPopover, NSPopoverBehavior,
    NSStatusBar, NSStatusItem, NSTextField, NSVariableStatusItemLength, NSView, NSViewController,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSMutableAttributedString, NSPoint, NSRectEdge, NSSize, NSString};
use std::rc::Rc;
use views::{ChartView, FlippedView, RowView, ShapeView, Target, rect, tone_color};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    AlertSettings,
    ToggleLogin,
    OpenLog,
    ShowData,
    OpenConfig,
    ReloadConfig,
    TestAlert,
    Quit,
}

impl Command {
    const ALL: [Command; 8] = [
        Self::AlertSettings,
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
    /// Prompt for a new nickname for this sensor (full advertised name).
    Rename(String),
    Command(Command),
}

const W: f64 = 340.0;
const PAD: f64 = 14.0;
const INNER: f64 = W - 2.0 * PAD;

pub struct Ui {
    mtm: MainThreadMarker,
    status_item: Retained<NSStatusItem>,
    popover: Retained<NSPopover>,
    content: Retained<FlippedView>,
    gear: Retained<NSButton>,
    target: Retained<Target>,
    on_action: Rc<dyn Fn(UiAction)>,
    vm: Option<ViewModel>,
}

impl Ui {
    pub fn new(mtm: MainThreadMarker, on_action: Rc<dyn Fn(UiAction)>) -> Self {
        let cb = on_action.clone();
        let target = Target::new(mtm, Box::new(move |a| cb(a)));

        let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = status_item.button(mtm) {
            unsafe {
                button.setTarget(Some(&target));
                button.setAction(Some(sel!(statusClicked:)));
            }
        }

        let content = FlippedView::new(mtm, rect(0.0, 0.0, W, 400.0));
        let controller = NSViewController::new(mtm);
        controller.setView(&content);
        let popover = NSPopover::new(mtm);
        popover.setBehavior(NSPopoverBehavior::Transient);
        popover.setContentViewController(Some(&controller));

        let gear_image = symbol("gearshape", "Settings");
        let gear = unsafe {
            NSButton::buttonWithImage_target_action(&gear_image, Some(&target), Some(sel!(gearClicked:)), mtm)
        };
        gear.setBordered(false);
        gear.setContentTintColor(Some(&NSColor::secondaryLabelColor()));

        Self { mtm, status_item, popover, content, gear, target, on_action, vm: None }
    }

    pub fn render(&mut self, vm: ViewModel) {
        if self.vm.as_ref() == Some(&vm) {
            return;
        }
        self.set_title(&vm.title);
        self.vm = Some(vm);
        if self.popover.isShown() {
            self.layout();
        }
    }

    pub fn toggle(&self) {
        if self.popover.isShown() {
            self.popover.close();
            return;
        }
        let Some(button) = self.status_item.button(self.mtm) else { return };
        self.layout();
        NSApplication::sharedApplication(self.mtm).activate();
        self.popover.showRelativeToRect_ofView_preferredEdge(button.bounds(), &button, NSRectEdge::MinY);
    }

    pub fn close(&self) {
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

    /// Asks for a nickname. Returns `None` on cancel; an empty string clears it.
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
        settings::prompt(self.mtm, c, error)
    }

    /// A menu item acting on one sensor; the name rides along as the
    /// represented object so the long-lived `Target` can handle it.
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

    fn set_title(&self, title: &Title) {
        let Some(button) = self.status_item.button(self.mtm) else { return };
        let text = NSMutableAttributedString::new();
        let (dot, dot_color, value) = match title {
            Title::Reading { tone, text } => ("● ", tone_color(*tone), text.as_str()),
            Title::NoData => ("○ ", NSColor::secondaryLabelColor(), "—"),
            Title::Bluetooth => ("⚠︎ ", NSColor::systemOrangeColor(), "BT"),
        };
        text.appendAttributedString(&attributed(dot, &dot_color, &NSFont::systemFontOfSize(11.0)));
        text.appendAttributedString(&attributed(value, &NSColor::labelColor(), &mono(13.0, weight(Weight::Medium))));
        button.setAttributedTitle(&text);
    }

    /// Rebuilds the popover content from the current view model.
    fn layout(&self) {
        let Some(vm) = &self.vm else { return };
        let c = &*self.content;
        for v in c.subviews().iter() {
            v.removeFromSuperview();
        }
        let mut y = PAD;

        if let Some((lead, rest)) = &vm.banner {
            y = self.banner(c, y, lead, rest) + 12.0;
        }
        y = self.hero(c, y, &vm.hero);
        y = self.others(c, y, &vm.others);

        let size = NSSize::new(W, y + PAD - 4.0);
        c.setFrameSize(size);
        self.popover.setContentSize(size);
    }

    fn banner(&self, c: &NSView, y: f64, lead: &str, rest: &str) -> f64 {
        let text_w = INNER - 40.0;
        let label = NSTextField::wrappingLabelWithString(&NSString::new(), self.mtm);
        let s = NSMutableAttributedString::new();
        s.appendAttributedString(&attributed(lead, &NSColor::labelColor(), &font(12.0, weight(Weight::Semibold))));
        s.appendAttributedString(&attributed(&format!(" {rest}"), &NSColor::labelColor(), &font(12.0, weight(Weight::Regular))));
        label.setAttributedStringValue(&s);
        label.setPreferredMaxLayoutWidth(text_w);
        let text_h = label.sizeThatFits(NSSize::new(text_w, 1000.0)).height.ceil();
        let h = text_h + 18.0;
        let orange = NSColor::systemOrangeColor();
        add(c, &ShapeView::new(self.mtm, rect(PAD, y, INNER, h), orange.colorWithAlphaComponent(0.16), 8.0));
        let icon = image_view(self.mtm, "exclamationmark.triangle.fill", &orange);
        icon.setFrame(rect(PAD + 10.0, y + 10.0, 15.0, 14.0));
        add(c, &icon);
        label.setFrame(rect(PAD + 32.0, y + 9.0, text_w, text_h));
        add(c, &label);
        y + h
    }

    fn hero(&self, c: &NSView, mut y: f64, h: &Hero) -> f64 {
        let m = self.mtm;
        // Header: "0874F · Work" and the gear button.
        let id = label(m, &h.id, font(13.0, weight(Weight::Semibold)), &NSColor::labelColor());
        place(c, &id, PAD, y + 6.0);
        if let Some(nick) = &h.nickname {
            let n = label(m, &format!("· {nick}"), font(13.0, weight(Weight::Regular)), &NSColor::secondaryLabelColor());
            place(c, &n, PAD + width(&id) + 4.0, y + 6.0);
        }
        self.gear.setFrame(rect(W - PAD - 28.0, y, 28.0, 28.0));
        add(c, &self.gear);
        y += 32.0;

        // Big reading, unit, status pill.
        let big = label(m, &h.co2, mono(46.0, weight(Weight::Semibold)), &NSColor::labelColor());
        place(c, &big, PAD - 2.0, y);
        let big_h = big.frame().size.height;
        let unit = label(m, "ppm CO₂", font(13.0, weight(Weight::Regular)), &NSColor::secondaryLabelColor());
        place(c, &unit, PAD + width(&big) + 2.0, y + big_h - unit.frame().size.height - 7.0);
        if !h.pill.is_empty() {
            self.pill(c, y + (big_h - 24.0) / 2.0, h.pill, h.tone);
        }
        y += big_h;
        let updated = label(m, &h.updated, font(11.5, weight(Weight::Regular)), &NSColor::tertiaryLabelColor());
        place(c, &updated, PAD, y);
        y += height(&updated) + 12.0;

        // Chart with the threshold label and time axis.
        let chart_h = 52.0;
        let bar_w = (INNER / 18.0 - 2.0).max(3.0);
        let chart = ChartView::new(m, rect(PAD, y, INNER, chart_h), h.chart.clone(), bar_w, h.warn_co2);
        add(c, &chart);
        let (lo, hi) = views::chart_scale(&h.chart);
        let line_from_top = chart_h - ((h.warn_co2 as f64 - lo) / (hi - lo)).clamp(0.0, 1.0) * chart_h;
        let thr = label(m, &crate::viewmodel::thousands(h.warn_co2), mono(10.0, weight(Weight::Regular)), &NSColor::tertiaryLabelColor());
        place(c, &thr, W - PAD - width(&thr), y + line_from_top - height(&thr) - 1.0);
        if h.chart.is_empty() {
            let empty = label(m, "No readings in the last 3 hours", font(11.5, weight(Weight::Regular)), &NSColor::tertiaryLabelColor());
            place(c, &empty, PAD + (INNER - width(&empty)) / 2.0, y + chart_h - height(&empty) - 4.0);
        }
        y += chart_h + 5.0;
        let axis = ["3 h ago", "2 h", "1 h", "Now"];
        for (i, text) in axis.iter().enumerate() {
            let l = label(m, text, font(10.5, weight(Weight::Regular)), &NSColor::tertiaryLabelColor());
            let x = match i {
                0 => PAD,
                3 => W - PAD - width(&l),
                _ => PAD + INNER * i as f64 / 3.0 - width(&l) / 2.0,
            };
            place(c, &l, x, y);
        }
        y += 14.0 + 14.0;

        // Four stat cards.
        if !h.stats.is_empty() {
            let gap = 6.0;
            let cw = (INNER - 3.0 * gap) / 4.0;
            for (i, s) in h.stats.iter().enumerate() {
                let x = PAD + i as f64 * (cw + gap);
                add(c, &ShapeView::new(m, rect(x, y, cw, 48.0), NSColor::labelColor().colorWithAlphaComponent(0.05), 8.0));
                let icon = image_view(m, s.symbol, &NSColor::secondaryLabelColor());
                icon.setFrame(rect(x + 8.0, y + 8.0, 12.0, 12.0));
                add(c, &icon);
                let cap = label(m, s.caption, font(10.5, weight(Weight::Regular)), &NSColor::secondaryLabelColor());
                place(c, &cap, x + 23.0, y + 7.0);
                let v = label(m, &s.value, mono(14.0, weight(Weight::Semibold)), &NSColor::labelColor());
                place(c, &v, x + 8.0, y + 23.0);
            }
            y += 48.0 + 14.0;
        }
        y
    }

    fn pill(&self, c: &NSView, y: f64, text: &str, tone: Tone) {
        let color = tone_color(tone);
        let l = label(self.mtm, text, font(12.0, weight(Weight::Semibold)), &color);
        let w = 10.0 + 7.0 + 6.0 + width(&l) + 10.0;
        let x = W - PAD - w;
        add(c, &ShapeView::new(self.mtm, rect(x, y, w, 24.0), color.colorWithAlphaComponent(0.16), 12.0));
        add(c, &ShapeView::new(self.mtm, rect(x + 10.0, y + 8.5, 7.0, 7.0), color.clone(), 3.5));
        place(c, &l, x + 23.0, y + (24.0 - height(&l)) / 2.0);
    }

    fn others(&self, c: &NSView, mut y: f64, rows: &[Row]) -> f64 {
        let m = self.mtm;
        add(c, &ShapeView::new(m, rect(PAD, y, INNER, 1.0), NSColor::separatorColor(), 0.0));
        y += 11.0;
        let head = label(m, "OTHER SENSORS", font(11.0, weight(Weight::Semibold)), &NSColor::secondaryLabelColor());
        place(c, &head, PAD + 4.0, y);
        if !rows.is_empty() {
            let hint = label(m, "Click to pin · right-click to rename", font(11.0, weight(Weight::Regular)), &NSColor::tertiaryLabelColor());
            place(c, &hint, W - PAD - 4.0 - width(&hint), y);
        }
        y += height(&head) + 6.0;

        if rows.is_empty() {
            let none = label(m, "No other sensors in range", font(12.0, weight(Weight::Regular)), &NSColor::tertiaryLabelColor());
            place(c, &none, PAD + 4.0, y + 6.0);
            return y + 30.0;
        }
        for r in rows {
            self.row(c, y, r);
            y += 40.0;
        }
        y
    }

    fn row(&self, c: &NSView, y: f64, r: &Row) {
        let m = self.mtm;
        let rw = INNER + 12.0;
        let cb = self.on_action.clone();
        let view =
            RowView::new(m, rect(PAD - 6.0, y, rw, 40.0), r.name.clone(), self.row_menu(&r.name), Box::new(move |a| cb(a)));
        add(c, &view);
        let muted = r.co2.is_none();
        let dot_color = if muted { NSColor::tertiaryLabelColor() } else { tone_color(r.tone) };
        add(&view, &ShapeView::new(m, rect(10.0, 16.0, 8.0, 8.0), dot_color, 4.0));
        let name_color = if muted { NSColor::secondaryLabelColor() } else { NSColor::labelColor() };
        let name = label(m, &r.label, font(13.0, weight(Weight::Medium)), &name_color);
        place(&view, &name, 26.0, (40.0 - height(&name)) / 2.0);

        let right = rw - 10.0;
        if let Some(note) = &r.note {
            let n = label(m, note, font(12.0, weight(Weight::Regular)), &NSColor::tertiaryLabelColor());
            place(&view, &n, right - width(&n), (40.0 - height(&n)) / 2.0);
            return;
        }
        let age = label(m, &r.age, mono(11.5, weight(Weight::Regular)), &NSColor::tertiaryLabelColor());
        place(&view, &age, right - width(&age), (40.0 - height(&age)) / 2.0);
        let temp = label(m, &r.temp, mono(12.0, weight(Weight::Regular)), &NSColor::secondaryLabelColor());
        place(&view, &temp, right - 44.0 - width(&temp), (40.0 - height(&temp)) / 2.0);
        if let Some(co2) = &r.co2 {
            let s = NSMutableAttributedString::new();
            s.appendAttributedString(&attributed(co2, &NSColor::labelColor(), &mono(13.0, weight(Weight::Semibold))));
            s.appendAttributedString(&attributed(" ppm", &NSColor::secondaryLabelColor(), &font(11.0, weight(Weight::Regular))));
            let l = NSTextField::labelWithAttributedString(&s, m);
            l.sizeToFit();
            place(&view, &l, right - 44.0 - 58.0 - width(&l), (40.0 - height(&l)) / 2.0);
        }
    }
}

// ---------------------------------------------------------------- helpers

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

fn font(size: f64, w: NSFontWeight) -> Retained<NSFont> {
    NSFont::systemFontOfSize_weight(size, w)
}

fn mono(size: f64, w: NSFontWeight) -> Retained<NSFont> {
    NSFont::monospacedDigitSystemFontOfSize_weight(size, w)
}

fn attributed(text: &str, color: &NSColor, font: &NSFont) -> Retained<NSAttributedString> {
    let keys = unsafe { [NSForegroundColorAttributeName, NSFontAttributeName] };
    let values: [&AnyObject; 2] = [color.as_ref(), font.as_ref()];
    let attrs = NSDictionary::from_slices(&keys, &values);
    unsafe { NSAttributedString::new_with_attributes(&NSString::from_str(text), &attrs) }
}

fn label(mtm: MainThreadMarker, text: &str, font: Retained<NSFont>, color: &NSColor) -> Retained<NSTextField> {
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

fn image_view(mtm: MainThreadMarker, name: &str, tint: &NSColor) -> Retained<NSImageView> {
    let v = NSImageView::imageViewWithImage(&symbol(name, ""), mtm);
    v.setContentTintColor(Some(tint));
    v.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
    v
}

fn add(parent: &NSView, child: &NSView) {
    parent.addSubview(child);
}

fn place(parent: &NSView, child: &NSView, x: f64, y: f64) {
    child.setFrameOrigin(NSPoint::new(x.round(), y.round()));
    parent.addSubview(child);
}

fn width(v: &NSView) -> f64 {
    v.frame().size.width
}

fn height(v: &NSView) -> f64 {
    v.frame().size.height
}
