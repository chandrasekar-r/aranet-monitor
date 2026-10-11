//! Small custom NSView subclasses. They draw with dynamic system colours in
//! `drawRect:`, so light/dark mode switches repaint correctly.

use super::UiAction;
use crate::viewmodel::Tone;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::sel;
use objc2::{
    AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
};
use objc2_app_kit::{
    NSAccessibility, NSAppearanceCustomization, NSAppearanceNameAccessibilityHighContrastDarkAqua,
    NSAppearanceNameDarkAqua, NSBezierPath, NSColor, NSEvent, NSGraphicsContext, NSImage,
    NSImageScaling, NSImageView, NSMenu, NSMenuItem, NSTrackingArea, NSTrackingAreaOptions, NSView,
    NSVisualEffectMaterial, NSVisualEffectView, NSWindow, NSWorkspace,
};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString, NSTimer};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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

/// Honor Reduced Motion: the air hero and gauge thumb stay still.
pub fn prefers_reduced_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

pub fn is_dark(view: &NSView) -> bool {
    let appearance = NSAppearanceCustomization::effectiveAppearance(view);
    let names = NSArray::from_slice(&[unsafe { NSAppearanceNameDarkAqua }, unsafe {
        NSAppearanceNameAccessibilityHighContrastDarkAqua
    }]);
    appearance
        .bestMatchFromAppearancesWithNames(&names)
        .is_some()
}

/// Dark mode uses the HUD material (dark vibrancy). Light mode keeps a normal popover.
pub fn sync_popover_material(content: &NSView) {
    let Some(parent) = (unsafe { content.superview() }) else {
        return;
    };
    let Some(effect) = parent.downcast_ref::<NSVisualEffectView>() else {
        return;
    };
    let material = if is_dark(content) {
        NSVisualEffectMaterial::HUDWindow
    } else {
        NSVisualEffectMaterial::Popover
    };
    if effect.material() != material {
        effect.setMaterial(material);
    }
}

fn plate_color(view: &NSView) -> Retained<NSColor> {
    if is_dark(view) {
        NSColor::whiteColor().colorWithAlphaComponent(if increase_contrast() { 0.16 } else { 0.10 })
    } else {
        NSColor::blackColor().colorWithAlphaComponent(if increase_contrast() { 0.10 } else { 0.06 })
    }
}

fn start_timer(
    slot: &RefCell<Option<Retained<NSTimer>>>,
    target: &AnyObject,
    selector: objc2::runtime::Sel,
    interval: f64,
) {
    if slot.borrow().is_some() {
        return;
    }
    let timer = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            interval, target, selector, None, true,
        )
    };
    timer.setTolerance(interval * 0.15);
    *slot.borrow_mut() = Some(timer);
}

fn stop_timer(slot: &RefCell<Option<Retained<NSTimer>>>) {
    if let Some(timer) = slot.borrow_mut().take() {
        timer.invalidate();
    }
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

        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn appearance_changed(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidChangeEffectiveAppearance] };
            sync_popover_material(self);
            self.setNeedsDisplay(true);
            for v in self.subviews().iter() {
                v.setNeedsDisplay(true);
            }
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
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        color: Retained<NSColor>,
        radius: f64,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ShapeIvars { color, radius });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    pub fn quaternary_fill(mtm: MainThreadMarker, frame: NSRect, radius: f64) -> Retained<Self> {
        let alpha = card_fill_alpha();
        Self::new(
            mtm,
            frame,
            NSColor::labelColor().colorWithAlphaComponent(alpha),
            radius,
        )
    }
}

// ---------------------------------------------------------------- Plate

pub struct PlateIvars {
    radius: f64,
}

define_class!(
    /// Translucent rounded chip / group background. Color resolves in `drawRect:`
    /// so light and dark mode stay in sync.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABPlateView"]
    #[ivars = PlateIvars]
    pub struct PlateView;

    impl PlateView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let r = self.ivars().radius;
            plate_color(self).setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(self.bounds(), r, r).fill();
        }
    }
);

impl PlateView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect, radius: f64) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PlateIvars { radius });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

// ---------------------------------------------------------------- Hairline

pub struct HairlineIvars {
    radius: f64,
}

define_class!(
    /// 1px inner border. Clicks pass through to the controls underneath.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABHairlineView"]
    #[ivars = HairlineIvars]
    pub struct HairlineView;

    impl HairlineView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let b = self.bounds();
            let inset = rect(0.5, 0.5, (b.size.width - 1.0).max(0.0), (b.size.height - 1.0).max(0.0));
            let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(inset, self.ivars().radius, self.ivars().radius);
            path.setLineWidth(1.0);
            let alpha = if increase_contrast() { 0.28 } else { 0.10 };
            let color = if is_dark(self) {
                NSColor::whiteColor().colorWithAlphaComponent(alpha)
            } else {
                NSColor::blackColor().colorWithAlphaComponent(alpha)
            };
            color.setStroke();
            path.stroke();
        }

        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }
    }
);

impl HairlineView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect, radius: f64) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(HairlineIvars { radius });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

// ---------------------------------------------------------------- Wave

pub struct WaveIvars {
    phase: Rc<Cell<f64>>,
    tone: Tone,
    intensity: f64,
    summary: String,
    timer: RefCell<Option<Retained<NSTimer>>>,
}

define_class!(
    /// Soft drifting air. Speed and color follow the CO₂ level. Origin bottom-left.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABWaveView"]
    #[ivars = WaveIvars]
    pub struct WaveView;

    impl WaveView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let iv = self.ivars();
            let b = self.bounds();
            let phase = iv.phase.get();
            let tone_paint = match iv.tone {
                Tone::Muted => NSColor::secondaryLabelColor(),
                tone => tone_color(tone),
            };
            let boost = if increase_contrast() { 1.45 } else { 1.0 };
            let amp = (6.0 + iv.intensity * 10.0) * if prefers_reduced_motion() { 0.65 } else { 1.0 };
            fill_wave(b, phase, b.size.height * 0.42, amp * 1.15, 92.0, &tone_paint.colorWithAlphaComponent(0.10 * boost));
            stroke_wave(b, phase, b.size.height * 0.62, amp * 0.7, 70.0, 1.6, &tone_paint.colorWithAlphaComponent(0.28 * boost));
            stroke_wave(b, phase * 1.15 + 1.4, b.size.height * 0.46, amp, 108.0, 2.4, &tone_paint.colorWithAlphaComponent(0.55 * boost));
            stroke_wave(b, phase * 0.8 + 2.6, b.size.height * 0.30, amp * 0.55, 64.0, 1.4, &tone_paint.colorWithAlphaComponent(0.22 * boost));
        }

        #[unsafe(method(tick:))]
        fn tick(&self, _timer: Option<&NSTimer>) {
            let iv = self.ivars();
            let speed = 0.45 + iv.intensity * 1.15;
            let mut phase = iv.phase.get() + speed / 30.0;
            let wrap = std::f64::consts::TAU * 8.0;
            if phase > wrap {
                phase -= wrap;
            }
            iv.phase.set(phase);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(viewWillMoveToWindow:))]
        fn view_will_move(&self, new_window: Option<&NSWindow>) {
            if new_window.is_none() {
                stop_timer(&self.ivars().timer);
            }
            let _: () = unsafe { msg_send![super(self), viewWillMoveToWindow: new_window] };
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn view_did_move(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidMoveToWindow] };
            if self.window().is_some() && !prefers_reduced_motion() {
                start_timer(&self.ivars().timer, self, sel!(tick:), 1.0 / 30.0);
            }
        }
    }
);

fn stroke_wave(
    bounds: NSRect,
    phase: f64,
    y_center: f64,
    amp: f64,
    wavelength: f64,
    width: f64,
    color: &NSColor,
) {
    let path = wave_path(bounds.size.width, phase, y_center, amp, wavelength);
    path.setLineWidth(width);
    path.setLineCapStyle(objc2_app_kit::NSLineCapStyle::Round);
    color.setStroke();
    path.stroke();
}

fn fill_wave(
    bounds: NSRect,
    phase: f64,
    y_center: f64,
    amp: f64,
    wavelength: f64,
    color: &NSColor,
) {
    let path = wave_path(bounds.size.width, phase, y_center, amp, wavelength);
    path.lineToPoint(NSPoint::new(bounds.size.width, 0.0));
    path.lineToPoint(NSPoint::new(0.0, 0.0));
    path.closePath();
    color.setFill();
    path.fill();
}

fn wave_path(
    width: f64,
    phase: f64,
    y_center: f64,
    amp: f64,
    wavelength: f64,
) -> Retained<NSBezierPath> {
    let path = NSBezierPath::bezierPath();
    let steps = 48;
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = t * width;
        let y = y_center + (x / wavelength + phase).sin() * amp;
        let point = NSPoint::new(x, y);
        if i == 0 {
            path.moveToPoint(point);
        } else {
            path.lineToPoint(point);
        }
    }
    path
}

impl WaveView {
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        phase: Rc<Cell<f64>>,
        tone: Tone,
        intensity: f64,
        summary: String,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(WaveIvars {
            phase,
            tone,
            intensity,
            summary,
            timer: RefCell::new(None),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setAccessibilityElement(true);
        view.setAccessibilityLabel(Some(&NSString::from_str("Air quality")));
        unsafe {
            view.setAccessibilityValue(Some(&NSString::from_str(&view.ivars().summary)));
        }
        view
    }
}

// ---------------------------------------------------------------- Gauge

pub struct GaugeIvars {
    pos: Rc<Cell<f64>>,
    vel: Rc<Cell<f64>>,
    target: f64,
    tone: Tone,
    warn_frac: f64,
    timer: RefCell<Option<Retained<NSTimer>>>,
}

define_class!(
    /// Read-only ppm track. The frosted thumb eases with a spring. Origin bottom-left.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABGaugeView"]
    #[ivars = GaugeIvars]
    pub struct GaugeView;

    impl GaugeView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let iv = self.ivars();
            let b = self.bounds();
            let dark = is_dark(self);
            let track = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(b, b.size.height / 2.0, b.size.height / 2.0);
            let rest = if dark {
                NSColor::whiteColor().colorWithAlphaComponent(0.12)
            } else {
                NSColor::blackColor().colorWithAlphaComponent(0.08)
            };
            if let Some(ctx) = NSGraphicsContext::currentContext() {
                ctx.saveGraphicsState();
                track.addClip();
                rest.setFill();
                NSBezierPath::fillRect(b);
                let fill = match iv.tone {
                    Tone::Muted => NSColor::secondaryLabelColor().colorWithAlphaComponent(0.45),
                    tone => tone_color(tone),
                };
                let thumb_w = 18.0;
                let travel = (b.size.width - thumb_w - 4.0).max(1.0);
                let x = 2.0 + iv.pos.get().clamp(0.0, 1.0) * travel;
                fill.setFill();
                NSBezierPath::fillRect(rect(0.0, 0.0, x + thumb_w * 0.5, b.size.height));
                if (0.02..0.98).contains(&iv.warn_frac) {
                    let wx = 2.0 + iv.warn_frac * travel + thumb_w * 0.5;
                    NSColor::labelColor().colorWithAlphaComponent(if dark { 0.35 } else { 0.28 }).setFill();
                    NSBezierPath::fillRect(rect(wx, 3.0, 1.5, b.size.height - 6.0));
                }
                let thumb = rect(x, 3.0, thumb_w, b.size.height - 6.0);
                NSColor::blackColor().colorWithAlphaComponent(0.18).setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                    rect(thumb.origin.x, thumb.origin.y - 0.5, thumb.size.width, thumb.size.height),
                    6.0,
                    6.0,
                )
                .fill();
                NSColor::whiteColor().colorWithAlphaComponent(if dark { 0.94 } else { 0.98 }).setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(thumb, 6.0, 6.0).fill();
                NSColor::blackColor().colorWithAlphaComponent(0.16).setStroke();
                let outline = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(thumb, 6.0, 6.0);
                outline.setLineWidth(1.0);
                outline.stroke();
                ctx.restoreGraphicsState();
            }
        }

        #[unsafe(method(tick:))]
        fn tick(&self, _timer: Option<&NSTimer>) {
            let iv = self.ivars();
            let (pos, vel) = crate::viewmodel::spring_step(iv.pos.get(), iv.vel.get(), iv.target, 1.0 / 60.0);
            if crate::viewmodel::spring_settled(pos, vel, iv.target) {
                iv.pos.set(iv.target);
                iv.vel.set(0.0);
                stop_timer(&iv.timer);
            } else {
                iv.pos.set(pos);
                iv.vel.set(vel);
            }
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(viewWillMoveToWindow:))]
        fn view_will_move(&self, new_window: Option<&NSWindow>) {
            if new_window.is_none() {
                stop_timer(&self.ivars().timer);
            }
            let _: () = unsafe { msg_send![super(self), viewWillMoveToWindow: new_window] };
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn view_did_move(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidMoveToWindow] };
            if self.window().is_none() {
                return;
            }
            let iv = self.ivars();
            if prefers_reduced_motion() {
                iv.pos.set(iv.target);
                iv.vel.set(0.0);
                self.setNeedsDisplay(true);
                return;
            }
            if !crate::viewmodel::spring_settled(iv.pos.get(), iv.vel.get(), iv.target) {
                start_timer(&iv.timer, self, sel!(tick:), 1.0 / 60.0);
            }
        }
    }
);

impl GaugeView {
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        pos: Rc<Cell<f64>>,
        vel: Rc<Cell<f64>>,
        target: f64,
        tone: Tone,
        warn_frac: f64,
        summary: String,
    ) -> Retained<Self> {
        if prefers_reduced_motion() {
            pos.set(target);
            vel.set(0.0);
        }
        let this = Self::alloc(mtm).set_ivars(GaugeIvars {
            pos,
            vel,
            target,
            tone,
            warn_frac,
            timer: RefCell::new(None),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setAccessibilityElement(true);
        view.setAccessibilityLabel(Some(&NSString::from_str("CO₂ level")));
        unsafe {
            view.setAccessibilityValue(Some(&NSString::from_str(&summary)));
        }
        view.setAccessibilityRoleDescription(Some(&NSString::from_str("level indicator")));
        view
    }
}

// ---------------------------------------------------------------- Spin

pub struct SpinIvars {
    angle: Cell<f64>,
    timer: RefCell<Option<Retained<NSTimer>>>,
}

define_class!(
    /// Small sync spinner used while Bluetooth is scanning and no reading has arrived.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABSpinView"]
    #[ivars = SpinIvars]
    pub struct SpinView;

    impl SpinView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let b = self.bounds();
            let radius = (b.size.width.min(b.size.height) / 2.0) - 1.5;
            let center = NSPoint::new(b.size.width / 2.0, b.size.height / 2.0);
            let track = NSBezierPath::bezierPath();
            track.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(center, radius, 0.0, 360.0);
            track.setLineWidth(1.5);
            NSColor::secondaryLabelColor().colorWithAlphaComponent(0.35).setStroke();
            track.stroke();
            let start = self.ivars().angle.get();
            let arc = NSBezierPath::bezierPath();
            arc.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(center, radius, start, start + 280.0, false);
            arc.setLineWidth(1.5);
            arc.setLineCapStyle(objc2_app_kit::NSLineCapStyle::Round);
            NSColor::labelColor().setStroke();
            arc.stroke();
        }

        #[unsafe(method(tick:))]
        fn tick(&self, _timer: Option<&NSTimer>) {
            let angle = (self.ivars().angle.get() + 8.0) % 360.0;
            self.ivars().angle.set(angle);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(viewWillMoveToWindow:))]
        fn view_will_move(&self, new_window: Option<&NSWindow>) {
            if new_window.is_none() {
                stop_timer(&self.ivars().timer);
            }
            let _: () = unsafe { msg_send![super(self), viewWillMoveToWindow: new_window] };
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn view_did_move(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidMoveToWindow] };
            if self.window().is_some() && !prefers_reduced_motion() {
                start_timer(&self.ivars().timer, self, sel!(tick:), 1.0 / 30.0);
            }
        }
    }
);

impl SpinView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(SpinIvars {
            angle: Cell::new(0.0),
            timer: RefCell::new(None),
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setAccessibilityElement(false);
        view
    }
}

// ---------------------------------------------------------------- Icon button

pub struct IconButtonIvars {
    hover: Cell<bool>,
    pressed: Cell<bool>,
    on_click: Box<dyn Fn()>,
}

define_class!(
    /// Circular icon button. The translucent disc appears on hover and press.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABIconButton"]
    #[ivars = IconButtonIvars]
    pub struct IconButton;

    impl IconButton {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            if self.ivars().hover.get() || self.ivars().pressed.get() {
                let alpha = if self.ivars().pressed.get() { 0.18 } else { 0.12 };
                NSColor::labelColor().colorWithAlphaComponent(if increase_contrast() { alpha + 0.06 } else { alpha }).setFill();
                NSBezierPath::bezierPathWithOvalInRect(self.bounds()).fill();
            }
        }

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
            self.ivars().pressed.set(false);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _e: &NSEvent) {
            self.ivars().pressed.set(true);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, e: &NSEvent) {
            self.ivars().pressed.set(false);
            self.setNeedsDisplay(true);
            let p = self.convertPoint_fromView(e.locationInWindow(), None);
            let b = self.bounds();
            let inside = p.x >= 0.0 && p.y >= 0.0 && p.x <= b.size.width && p.y <= b.size.height;
            if inside {
                (self.ivars().on_click)();
            }
        }
    }
);

impl IconButton {
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        symbol: &str,
        accessibility_label: &str,
        accessibility_help: &str,
        on_click: Box<dyn Fn()>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(IconButtonIvars {
            hover: Cell::new(false),
            pressed: Cell::new(false),
            on_click,
        });
        let side = 16.0;
        let icon_origin = (
            (frame.size.width - side) / 2.0,
            (frame.size.height - side) / 2.0,
        );
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        let icon = NSImageView::imageViewWithImage(&symbol_image(symbol), mtm);
        icon.setContentTintColor(Some(&NSColor::labelColor()));
        icon.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
        icon.setFrame(rect(icon_origin.0, icon_origin.1, side, side));
        icon.setAccessibilityElement(false);
        view.addSubview(&icon);
        view.setAccessibilityElement(true);
        view.setAccessibilityLabel(Some(&NSString::from_str(accessibility_label)));
        view.setAccessibilityHelp(Some(&NSString::from_str(accessibility_help)));
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

fn symbol_image(name: &str) -> Retained<NSImage> {
    NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(name), None)
        .unwrap_or_else(NSImage::new)
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
        fn mouse_up(&self, e: &NSEvent) {
            let p = self.convertPoint_fromView(e.locationInWindow(), None);
            // Chevron opens Pin / Rename. The rest of the row pins the sensor.
            if p.x >= self.bounds().size.width - 28.0 {
                let menu = &self.ivars().menu;
                menu.popUpMenuPositioningItem_atLocation_inView(None, p, Some(self));
            } else {
                (self.ivars().on_action)(UiAction::Pin(self.ivars().name.clone()));
            }
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
        let this = Self::alloc(mtm).set_ivars(RowIvars {
            name,
            menu,
            hover: Cell::new(false),
            on_action,
        });
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setAccessibilityElement(true);
        view.setAccessibilityLabel(Some(&NSString::from_str(accessibility_label)));
        unsafe {
            view.setAccessibilityValue(Some(&NSString::from_str(accessibility_value)));
        }
        view.setAccessibilityHelp(Some(&NSString::from_str(
            "Click to pin to the menu bar. Click the chevron for more actions.",
        )));
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
