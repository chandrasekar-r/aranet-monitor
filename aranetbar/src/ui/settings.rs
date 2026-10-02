//! The "Alert settings" form: a modal alert with limits per metric and the
//! Telegram bot details.

use super::views::{FlippedView, ShapeView, rect};
use super::{Weight, add, font, height, label, place, weight, width};
use crate::config::Config;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertThirdButtonReturn, NSApplication, NSButton, NSColor,
    NSControlStateValueOff, NSControlStateValueOn, NSSecureTextField, NSTextField,
};
use objc2_foundation::{NSSize, NSString};
use std::str::FromStr;

pub enum Reply {
    Cancel,
    Save(Edited),
    Test(Edited),
}

/// The form's values. Fields that didn't parse keep their previous value and
/// `error` says what was wrong, so the form can be shown again as edited.
pub struct Edited {
    pub config: Config,
    pub error: Option<String>,
}

const W: f64 = 460.0;
const ROW: f64 = 30.0;
/// Where the limit fields start, right of the metric checkboxes.
const COL: f64 = 118.0;

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

/// Lays controls out left to right, one row at a time.
struct Builder<'a> {
    mtm: MainThreadMarker,
    view: &'a FlippedView,
    x: f64,
    y: f64,
}

impl Builder<'_> {
    fn check(&mut self, title: &str, on: bool) -> Retained<NSButton> {
        let b = unsafe { NSButton::checkboxWithTitle_target_action(&NSString::from_str(title), None, None, self.mtm) };
        b.setState(if on { NSControlStateValueOn } else { NSControlStateValueOff });
        b.sizeToFit();
        place(self.view, &b, 0.0, self.y + (ROW - height(&b)) / 2.0);
        self.x = COL;
        b
    }

    fn caption(&mut self, text: &str) {
        let l = label(self.mtm, text, font(13.0, weight(Weight::Regular)), &NSColor::labelColor());
        place(self.view, &l, self.x, self.y + (ROW - height(&l)) / 2.0);
        self.x += width(&l) + 6.0;
    }

    fn field(&mut self, value: &str, w: f64) -> Retained<NSTextField> {
        let f = NSTextField::textFieldWithString(&NSString::from_str(value), self.mtm);
        self.put_field(&f, w);
        f
    }

    fn put_field(&mut self, f: &NSTextField, w: f64) {
        f.setFrame(rect(self.x, self.y + 4.0, w, 22.0));
        add(self.view, f);
        self.x += w + 12.0;
    }

    fn row(&mut self) {
        self.y += ROW;
        self.x = COL;
    }
}

fn build(mtm: MainThreadMarker, c: &Config) -> (Retained<FlippedView>, Form) {
    let view = FlippedView::new(mtm, rect(0.0, 0.0, W, 400.0));
    let mut b = Builder { mtm, view: &view, x: 0.0, y: 0.0 };
    let n = |v: &dyn ToString| v.to_string();

    let co2 = b.check("CO₂", c.co2_alerts);
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
    b.caption("Sensor missing");
    b.x = COL;
    b.caption("After");
    let missing = b.field(&n(&c.missing_minutes), 48.0);
    b.caption("min without a reading");
    b.row();

    b.y += 8.0;
    add(&view, &ShapeView::new(mtm, rect(0.0, b.y, W, 1.0), NSColor::separatorColor(), 0.0));
    b.y += 10.0;
    let head = label(mtm, "Telegram", font(13.0, weight(Weight::Semibold)), &NSColor::labelColor());
    place(&view, &head, 0.0, b.y);
    let opt = label(mtm, "optional · also sends every alert to your phone", font(12.0, weight(Weight::Regular)), &NSColor::secondaryLabelColor());
    place(&view, &opt, width(&head) + 6.0, b.y + 1.0);
    b.y += height(&head) + 4.0;

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
            "In Telegram, message @BotFather to create a bot and paste its token here. \
             Send your new bot any message, then get your chat ID from @userinfobot. \
             Leave both empty to turn Telegram off.",
        ),
        mtm,
    );
    hint.setFont(Some(&font(11.5, weight(Weight::Regular))));
    hint.setTextColor(Some(&NSColor::secondaryLabelColor()));
    let hint_h = hint.sizeThatFits(NSSize::new(W, 1000.0)).height.ceil();
    hint.setFrame(rect(0.0, b.y + 2.0, W, hint_h));
    add(&view, &hint);
    view.setFrameSize(NSSize::new(W, b.y + 2.0 + hint_h));

    let form = Form {
        co2, warn_co2, high_co2, clear_co2, temperature, temp_min, temp_max, humidity, humidity_min, humidity_max,
        battery, low_battery, missing, token, chat,
    };
    (view, form)
}

fn on(b: &NSButton) -> bool {
    b.state() == NSControlStateValueOn
}

/// Parses number fields, remembering the first one that's invalid.
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

/// Shows the form filled from `c`, with `error` (from a previous attempt)
/// above it.
pub fn prompt(mtm: MainThreadMarker, c: &Config, error: Option<&str>) -> Reply {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str("Alert settings"));
    let info = match error {
        Some(e) => format!("⚠︎ {e}"),
        None => "Get an alert when a reading leaves its range. Defaults follow healthy indoor levels.".into(),
    };
    alert.setInformativeText(&NSString::from_str(&info));
    alert.addButtonWithTitle(&NSString::from_str("Save"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    alert.addButtonWithTitle(&NSString::from_str("Send Test"));
    let (view, form) = build(mtm, c);
    alert.setAccessoryView(Some(&view));
    NSApplication::sharedApplication(mtm).activate();
    match alert.runModal() {
        r if r == NSAlertFirstButtonReturn => Reply::Save(form.read(c)),
        r if r == NSAlertThirdButtonReturn => Reply::Test(form.read(c)),
        _ => Reply::Cancel,
    }
}
