//! Small custom NSView subclasses. They draw with dynamic system colours in
//! `drawRect:`, so light/dark mode switches repaint correctly.

use super::UiAction;
use crate::viewmodel::Tone;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSBezierPath, NSColor, NSEvent, NSMenu, NSMenuItem, NSTrackingArea, NSTrackingAreaOptions, NSView,
    NSWorkspace,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::cell::Cell;

pub fn tone_color(tone: Tone) -> Retained<NSColor> {
    match tone {
        Tone::Good => NSColor::systemGreenColor(),
        Tone::Warn => NSColor::systemOrangeColor(),
        Tone::High => NSColor::systemRedColor(),
        Tone::Muted => NSColor::tertiaryLabelColor(),
    }
}

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

pub fn increase_contrast() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldIncreaseContrast()
        || NSWorkspace::sharedWorkspace().accessibilityDisplayShouldDifferentiateWithoutColor()
}

/// Honor Reduced Motion before any future chart animation.
pub fn prefers_reduced_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

fn card_fill_alpha() -> f64 {
    if increase_contrast() { 0.12 } else { 0.05 }
}

fn banner_fill_alpha() -> f64 {
    if increase_contrast() { 0.28 } else { 0.16 }
}

fn hover_fill_alpha() -> f64 {
    if increase_contrast() { 0.14 } else { 0.07 }
}

// ---------------------------------------------------------------- Flipped

define_class!(
    /// Container with a top-left origin, so layout reads top to bottom.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABFlippedView"]
    pub struct FlippedView;

    impl FlippedView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl FlippedView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

// ---------------------------------------------------------------- Shape

pub struct ShapeIvars {
    color: Retained<NSColor>,
    radius: f64,
}

define_class!(
    /// A filled rounded rectangle (dots, pills, cards, dividers).
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABShapeView"]
    #[ivars = ShapeIvars]
    pub struct ShapeView;

    impl ShapeView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let r = self.ivars().radius;
            self.ivars().color.setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(self.bounds(), r, r).fill();
        }
    }
);

impl ShapeView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect, color: Retained<NSColor>, radius: f64) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ShapeIvars { color, radius });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    pub fn quaternary_fill(mtm: MainThreadMarker, frame: NSRect, radius: f64) -> Retained<Self> {
        let alpha = card_fill_alpha();
        Self::new(mtm, frame, NSColor::labelColor().colorWithAlphaComponent(alpha), radius)
    }
}

// ---------------------------------------------------------------- Chart

pub struct ChartIvars {
    /// (x 0..1, co2, tone)
    points: Vec<(f64, u16, Tone)>,
    bar_width: f64,
    warn_co2: u16,
    accessibility_summary: String,
}

/// Maps CO₂ to a height fraction; shared with the threshold label placement.
pub fn chart_scale(points: &[(f64, u16, Tone)]) -> (f64, f64) {
    let max = points.iter().map(|p| p.1).max().unwrap_or(0) as f64;
    (400.0, (max + 100.0).max(1500.0))
}

define_class!(
    /// Bar chart of the last few hours, bars coloured by threshold, with a
    /// dashed line at the warning level. Origin bottom-left.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABChartView"]
    #[ivars = ChartIvars]
    pub struct ChartView;

    impl ChartView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let iv = self.ivars();
            let b = self.bounds();
            let (lo, hi) = chart_scale(&iv.points);
            let frac = |c: f64| ((c - lo) / (hi - lo)).clamp(0.0, 1.0);
            let n = iv.points.len();
            let high_contrast = increase_contrast();
            for (i, &(x, co2, tone)) in iv.points.iter().enumerate() {
                let h = (frac(co2 as f64) * b.size.height).max(3.0);
                let left = (x * (b.size.width - iv.bar_width)).max(0.0);
                let alpha = if i + 1 == n { 1.0 } else { 0.78 };
                let bar_rect = rect(left, 0.0, iv.bar_width, h);
                tone_color(tone).colorWithAlphaComponent(alpha).setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bar_rect, 2.0, 2.0).fill();
                if high_contrast {
                    NSColor::labelColor().colorWithAlphaComponent(0.35).setStroke();
                    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bar_rect, 2.0, 2.0).stroke();
                }
            }
            let y = (frac(iv.warn_co2 as f64) * b.size.height).round() + 0.5;
            let line = NSBezierPath::bezierPath();
            line.moveToPoint(NSPoint::new(0.0, y));
            line.lineToPoint(NSPoint::new(b.size.width, y));
            line.setLineWidth(if high_contrast { 1.5 } else { 1.0 });
            let mut dash = [3.0, 3.0];
            unsafe { line.setLineDash_count_phase(dash.as_mut_ptr(), 2, 0.0) };
            NSColor::secondaryLabelColor().setStroke();
            line.stroke();
        }
    }
);

impl ChartView {
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        points: Vec<(f64, u16, Tone)>,
        bar_width: f64,
        warn_co2: u16,
        accessibility_summary: String,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ChartIvars { points, bar_width, warn_co2, accessibility_summary });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setAccessibilityElement(true);
        view.setAccessibilityLabel(Some(&NSString::from_str("CO₂ history chart")));
        view.setAccessibilityValue(Some(&NSString::from_str(&view.ivars().accessibility_summary)));
        view.setAccessibilityRoleDescription(Some(&NSString::from_str("chart")));
        view
    }
}

// ---------------------------------------------------------------- Row

pub struct RowIvars {
    name: String,
    menu: Retained<NSMenu>,
    hover: Cell<bool>,
    on_action: Box<dyn Fn(UiAction)>,
}

define_class!(
    /// A clickable sensor row with a hover highlight. Clicking pins the sensor;
    /// right-click shows a Pin / Rename menu.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABRowView"]
    #[ivars = RowIvars]
    pub struct RowView;

    impl RowView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            if self.ivars().hover.get() {
                NSColor::labelColor().colorWithAlphaComponent(hover_fill_alpha()).setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(self.bounds(), 6.0, 6.0).fill();
            }
        }

        // Swallow clicks on the labels inside the row.
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> *mut NSView {
            let f = self.frame();
            let inside = point.x >= f.origin.x && point.x <= f.origin.x + f.size.width
                && point.y >= f.origin.y && point.y <= f.origin.y + f.size.height;
            if inside { self as *const Self as *mut NSView } else { std::ptr::null_mut() }
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _e: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _e: &NSEvent) {
            self.ivars().hover.set(true);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _e: &NSEvent) {
            self.ivars().hover.set(false);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _e: &NSEvent) {
            (self.ivars().on_action)(UiAction::Pin(self.ivars().name.clone()));
        }

        #[unsafe(method_id(menuForEvent:))]
        fn menu_for_event(&self, _e: &NSEvent) -> Option<Retained<NSMenu>> {
            Some(self.ivars().menu.clone())
        }
    }
);

impl RowView {
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        name: String,
        menu: Retained<NSMenu>,
        on_action: Box<dyn Fn(UiAction)>,
        accessibility_label: &str,
        accessibility_value: &str,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(RowIvars { name, menu, hover: Cell::new(false), on_action });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setAccessibilityElement(true);
        view.setAccessibilityLabel(Some(&NSString::from_str(accessibility_label)));
        view.setAccessibilityValue(Some(&NSString::from_str(accessibility_value)));
        view.setAccessibilityHelp(Some(&NSString::from_str("Click to pin to menu bar. Control-click for more actions.")));
        view.setAccessibilityRoleDescription(Some(&NSString::from_str("button")));
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&view),
                None,
            )
        };
        view.addTrackingArea(&area);
        view
    }
}

pub fn banner_fill_color() -> Retained<NSColor> {
    NSColor::systemOrangeColor().colorWithAlphaComponent(banner_fill_alpha())
}

// ---------------------------------------------------------------- Target

pub struct TargetIvars {
    on_action: Box<dyn Fn(UiAction)>,
}

define_class!(
    /// Receives AppKit target/action messages and forwards them as `UiAction`s.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABActionTarget"]
    #[ivars = TargetIvars]
    pub struct Target;

    unsafe impl NSObjectProtocol for Target {}

    impl Target {
        #[unsafe(method(statusClicked:))]
        fn status_clicked(&self, _sender: Option<&AnyObject>) {
            (self.ivars().on_action)(UiAction::TogglePopover);
        }

        #[unsafe(method(gearClicked:))]
        fn gear_clicked(&self, _sender: Option<&AnyObject>) {
            (self.ivars().on_action)(UiAction::Settings);
        }

        #[unsafe(method(pinSensor:))]
        fn pin_sensor(&self, sender: &NSMenuItem) {
            if let Some(name) = sensor_name(sender) {
                (self.ivars().on_action)(UiAction::Pin(name));
            }
        }

        #[unsafe(method(renameSensor:))]
        fn rename_sensor(&self, sender: &NSMenuItem) {
            if let Some(name) = sensor_name(sender) {
                (self.ivars().on_action)(UiAction::Rename(name));
            }
        }

        #[unsafe(method(menuPicked:))]
        fn menu_picked(&self, sender: &NSMenuItem) {
            if let Some(cmd) = super::Command::from_tag(sender.tag()) {
                (self.ivars().on_action)(UiAction::Command(cmd));
            }
        }
    }
);

fn sensor_name(item: &NSMenuItem) -> Option<String> {
    let obj = item.representedObject()?;
    Some(obj.downcast::<NSString>().ok()?.to_string())
}

impl Target {
    pub fn new(mtm: MainThreadMarker, on_action: Box<dyn Fn(UiAction)>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TargetIvars { on_action });
        unsafe { msg_send![super(this), init] }
    }
}
