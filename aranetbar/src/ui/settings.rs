//! Alert settings in a standard sheet window (not an NSAlert accessory).

use super::views::{FlippedView, ShapeView, rect};
use super::{TextStyle, add, font, height, label, mono_style, place, styled, weight, width};
use crate::config::Config;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{AllocAnyThread, DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSAlertThirdButtonReturn, NSApplication, NSBackingStoreType,
    NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn,
    NSSecureTextField, NSTextField, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSSize, NSString};
use std::cell::RefCell;
use std::str::FromStr;

#[derive(Clone)]
pub enum Reply {
    Cancel,
    Save(Edited),
    Test(Edited),
}

/// The form's values. Fields that didn't parse keep their previous value and
/// `error` says what was wrong, so the form can be shown again as edited.
#[derive(Clone)]
pub struct Edited {
    pub config: Config,
    pub error: Option<String>,
}

const W: f64 = 480.0;
const ROW: f64 = 32.0;
const COL: f64 = 120.0;

struct Form {
    co2: Retained<NSButton>,
    warn_co2: Retained<NSTextField>,
    high_co2: Retained<NSTextField>,
    clear_co2: Retained<NSTextField>,
    temperature: Retained<NSButton>,
    temp_min: Retained<NSTextField>,
    temp_max: Retained<NSTextField>,
    humidity: Retained<NSButton>,
    humidity_min: Retained<NSTextField>,
    humidity_max: Retained<NSTextField>,
    battery: Retained<NSButton>,
    low_battery: Retained<NSTextField>,
    missing: Retained<NSTextField>,
    token: Retained<NSTextField>,
    chat: Retained<NSTextField>,
}

struct PanelIvars {
    mtm: MainThreadMarker,
    form: RefCell<Option<Form>>,
    base: RefCell<Config>,
    result: RefCell<Option<Reply>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABSettingsPanelTarget"]
    #[ivars = PanelIvars]
    pub struct PanelTarget;

    unsafe impl NSObjectProtocol for PanelTarget {}

    impl PanelTarget {
        #[unsafe(method(save:))]
        fn save(&self, _sender: Option<&AnyObject>) {
            if let Some(form) = self.ivars().form.borrow().as_ref() {
                let edited = form.read(&self.ivars().base.borrow());
                *self.ivars().result.borrow_mut() = Some(Reply::Save(edited));
            }
            let mtm = self.ivars().mtm;
            NSApplication::sharedApplication(mtm).stopModalWithCode(NSAlertFirstButtonReturn);
        }

        #[unsafe(method(cancel:))]
        fn cancel(&self, _sender: Option<&AnyObject>) {
            *self.ivars().result.borrow_mut() = Some(Reply::Cancel);
            let mtm = self.ivars().mtm;
            NSApplication::sharedApplication(mtm).stopModalWithCode(NSAlertSecondButtonReturn);
        }

        #[unsafe(method(test:))]
        fn test(&self, _sender: Option<&AnyObject>) {
            if let Some(form) = self.ivars().form.borrow().as_ref() {
                let edited = form.read(&self.ivars().base.borrow());
                *self.ivars().result.borrow_mut() = Some(Reply::Test(edited));
            }
            let mtm = self.ivars().mtm;
            NSApplication::sharedApplication(mtm).stopModalWithCode(NSAlertThirdButtonReturn);
        }
    }
);

struct Builder<'a> {
    mtm: MainThreadMarker,
    view: &'a FlippedView,
    x: f64,
    y: f64,
}

impl Builder<'_> {
    fn section(&mut self, title: &str) {
        self.y += 8.0;
        let head = label(self.mtm, title, styled(TextStyle::Headline), &NSColor::labelColor());
        place(self.view, &head, 0.0, self.y);
        self.y += height(&head) + 8.0;
        self.x = 0.0;
    }

    fn check(&mut self, title: &str, on: bool) -> Retained<NSButton> {
        let b = unsafe { NSButton::checkboxWithTitle_target_action(&NSString::from_str(title), None, None, self.mtm) };
        b.setState(if on { NSControlStateValueOn } else { NSControlStateValueOff });
        b.sizeToFit();
        place(self.view, &b, 0.0, self.y + (ROW - height(&b)) / 2.0);
        self.x = COL;
        b
    }

    fn caption(&mut self, text: &str) {
        let l = label(self.mtm, text, styled(TextStyle::Body), &NSColor::labelColor());
        place(self.view, &l, self.x, self.y + (ROW - height(&l)) / 2.0);
        self.x += width(&l) + 8.0;
    }

    fn field(&mut self, value: &str, w: f64) -> Retained<NSTextField> {
        let f = NSTextField::textFieldWithString(&NSString::from_str(value), self.mtm);
        self.put_field(&f, w);
        f
    }

    fn put_field(&mut self, f: &NSTextField, w: f64) {
        f.setFrame(rect(self.x, self.y + 4.0, w, 24.0));
        add(self.view, f);
        self.x += w + 8.0;
    }

    fn row(&mut self) {
        self.y += ROW;
        self.x = COL;
    }
}

fn build_form(mtm: MainThreadMarker, c: &Config, error: Option<&str>) -> (Retained<FlippedView>, Form, f64) {
    let view = FlippedView::new(mtm, rect(0.0, 0.0, W, 400.0));
    let mut b = Builder { mtm, view: &view, x: 0.0, y: 0.0 };
    let n = |v: &dyn ToString| v.to_string();

    if let Some(e) = error {
        let err = label(mtm, e, styled(TextStyle::Callout), &NSColor::systemOrangeColor());
        place(&view, &err, 0.0, b.y);
        b.y += height(&err) + 8.0;
    }

    b.section("CO₂");
    let co2 = b.check("Enable CO₂ alerts", c.co2_alerts);
    b.caption("Warn above");
    let warn_co2 = b.field(&n(&c.warn_co2), 56.0);
    b.caption("Urgent above");
    let high_co2 = b.field(&n(&c.high_co2), 56.0);
    b.caption("ppm");
    b.row();
    b.caption("Back to normal below");
    let clear_co2 = b.field(&n(&c.clear_co2), 56.0);
    b.caption("ppm");
    b.row();

    b.section("Environment");
    let temperature = b.check("Temperature", c.temperature_alerts);
    b.caption("Below");
    let temp_min = b.field(&n(&c.temp_min), 48.0);
    b.caption("Above");
    let temp_max = b.field(&n(&c.temp_max), 48.0);
    b.caption("°C");
    b.row();

    let humidity = b.check("Humidity", c.humidity_alerts);
    b.caption("Below");
    let humidity_min = b.field(&n(&c.humidity_min), 48.0);
    b.caption("Above");
    let humidity_max = b.field(&n(&c.humidity_max), 48.0);
    b.caption("%");
    b.row();

    let battery = b.check("Battery", c.battery_alerts);
    b.caption("Below");
    let low_battery = b.field(&n(&c.low_battery), 48.0);
    b.caption("%");
    b.row();

    b.x = 0.0;
    b.caption("Sensor missing after");
    b.x = COL;
    let missing = b.field(&n(&c.missing_minutes), 48.0);
    b.caption("min without a reading");
    b.row();

    b.y += 8.0;
    add(&view, &ShapeView::new(mtm, rect(0.0, b.y, W, 1.0), NSColor::separatorColor(), 0.0));
    b.y += 16.0;
    b.section("Telegram");
    let opt = label(
        mtm,
        "Optional · also sends every alert to your phone",
        font(TextStyle::Caption1),
        &NSColor::secondaryLabelColor(),
    );
    place(&view, &opt, 0.0, b.y);
    b.y += height(&opt) + 8.0;

    b.x = 0.0;
    b.caption("Bot token");
    b.x = COL;
    let token: Retained<NSTextField> = NSSecureTextField::new(mtm).into_super();
    token.setStringValue(&NSString::from_str(&c.telegram_token));
    token.setPlaceholderString(Some(&NSString::from_str("123456789:AA…")));
    b.put_field(&token, W - COL);
    b.row();
    b.x = 0.0;
    b.caption("Chat ID");
    b.x = COL;
    let chat = b.field(&c.telegram_chat_id, 150.0);
    chat.setPlaceholderString(Some(&NSString::from_str("e.g. 123456789")));
    b.row();

    let hint = NSTextField::wrappingLabelWithString(
        &NSString::from_str(
            "Message @BotFather to create a bot, paste its token here, then send your bot a message and look up your chat ID with @userinfobot. Leave both empty to turn Telegram off.",
        ),
        mtm,
    );
    hint.setFont(Some(&font(TextStyle::Caption1)));
    hint.setTextColor(Some(&NSColor::secondaryLabelColor()));
    let hint_h = hint.sizeThatFits(NSSize::new(W, 1000.0)).height.ceil();
    hint.setFrame(rect(0.0, b.y + 2.0, W, hint_h));
    add(&view, &hint);
    let total_h = b.y + 2.0 + hint_h;
    view.setFrameSize(NSSize::new(W, total_h));

    let form = Form {
        co2, warn_co2, high_co2, clear_co2, temperature, temp_min, temp_max, humidity, humidity_min, humidity_max,
        battery, low_battery, missing, token, chat,
    };
    (view, form, total_h)
}

fn on(b: &NSButton) -> bool {
    b.state() == NSControlStateValueOn
}

#[derive(Default)]
struct Reader {
    error: Option<String>,
}

impl Reader {
    fn num<T: FromStr>(&mut self, f: &NSTextField, what: &str, previous: T) -> T {
        let text = f.stringValue().to_string();
        text.trim().parse().unwrap_or_else(|_| {
            self.error.get_or_insert_with(|| format!("“{}” isn't a valid {what}.", text.trim()));
            previous
        })
    }
}

impl Form {
    fn read(&self, base: &Config) -> Edited {
        let mut r = Reader::default();
        let config = Config {
            co2_alerts: on(&self.co2),
            warn_co2: r.num(&self.warn_co2, "CO₂ warning level", base.warn_co2),
            high_co2: r.num(&self.high_co2, "CO₂ urgent level", base.high_co2),
            clear_co2: r.num(&self.clear_co2, "CO₂ back-to-normal level", base.clear_co2),
            temperature_alerts: on(&self.temperature),
            temp_min: r.num(&self.temp_min, "minimum temperature", base.temp_min),
            temp_max: r.num(&self.temp_max, "maximum temperature", base.temp_max),
            humidity_alerts: on(&self.humidity),
            humidity_min: r.num(&self.humidity_min, "minimum humidity", base.humidity_min),
            humidity_max: r.num(&self.humidity_max, "maximum humidity", base.humidity_max),
            battery_alerts: on(&self.battery),
            low_battery: r.num(&self.low_battery, "battery level", base.low_battery),
            missing_minutes: r.num(&self.missing, "number of minutes", base.missing_minutes),
            telegram_token: self.token.stringValue().to_string().trim().to_string(),
            telegram_chat_id: self.chat.stringValue().to_string().trim().to_string(),
            ..base.clone()
        };
        let error = r.error.or_else(|| config.validate().err());
        Edited { config, error }
    }
}

/// Presents settings in a dedicated window (sheet when `parent` is set).
pub fn prompt(mtm: MainThreadMarker, parent: Option<&NSWindow>, c: &Config, error: Option<&str>) -> Reply {
    let (form_view, form, form_h) = build_form(mtm, c, error);
    let target: Retained<PanelTarget> = {
        let this = PanelTarget::alloc(mtm).set_ivars(PanelIvars {
            mtm,
            form: RefCell::new(Some(form)),
            base: RefCell::new(c.clone()),
            result: RefCell::new(None),
        });
        unsafe { msg_send![super(this), init] }
    };

    let btn_h = 32.0;
    let chrome = FlippedView::new(mtm, rect(0.0, 0.0, W, form_h + btn_h + 16.0));
    form_view.setFrameOrigin(objc2_foundation::NSPoint::new(0.0, btn_h + 16.0));
    chrome.addSubview(&form_view);

    let save = unsafe {
        NSButton::buttonWithTitle_target_action(&NSString::from_str("Save"), Some(&target), Some(objc2::sel!(save:)), mtm)
    };
    save.setKeyEquivalent(&NSString::from_str("s"));
    save.setFrame(rect(W - 240.0, 8.0, 72.0, btn_h));
    let test = unsafe {
        NSButton::buttonWithTitle_target_action(&NSString::from_str("Send test"), Some(&target), Some(objc2::sel!(test:)), mtm)
    };
    test.setFrame(rect(W - 160.0, 8.0, 88.0, btn_h));
    let cancel = unsafe {
        NSButton::buttonWithTitle_target_action(&NSString::from_str("Cancel"), Some(&target), Some(objc2::sel!(cancel:)), mtm)
    };
    cancel.setKeyEquivalent(&NSString::from_str("\u{1b}"));
    cancel.setFrame(rect(W - 64.0, 8.0, 64.0, btn_h));
    chrome.addSubview(&save);
    chrome.addSubview(&test);
    chrome.addSubview(&cancel);

    let style = NSWindowStyleMask::Titled | NSWindowStyleMask::Closable;
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            rect(0.0, 0.0, W, form_h + btn_h + 16.0),
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str("Alert settings"));
    window.setContentView(Some(&chrome));

    NSApplication::sharedApplication(mtm).activate();
    if let Some(parent) = parent {
        parent.beginSheet_completionHandler(&window, None);
    } else {
        window.makeKeyAndOrderFront(None);
    }
    NSApplication::sharedApplication(mtm).runModalForWindow(&window);
    if parent.is_some() {
        parent.unwrap().endSheet(&window);
    }
    window.close();
    target.ivars().result.borrow().clone().unwrap_or(Reply::Cancel)
}
