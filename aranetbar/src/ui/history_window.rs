//! SQLite / CSV history in a standard window with CSV export.

use super::views::{FlippedView, rect};
use crate::db::Db;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{AllocAnyThread, DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSButton, NSColor, NSFont, NSSavePanel, NSScrollView, NSTextView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSRect, NSSize, NSString};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct ExportIvars {
    mtm: MainThreadMarker,
    log_path: RefCell<PathBuf>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ABHistoryExportTarget"]
    #[ivars = ExportIvars]
    pub struct ExportTarget;

    unsafe impl NSObjectProtocol for ExportTarget {}

    impl ExportTarget {
        #[unsafe(method(exportCsv:))]
        fn export_csv(&self, _sender: Option<&AnyObject>) {
            let mtm = self.ivars().mtm;
            let path = self.ivars().log_path.borrow().clone();
            let panel = unsafe { NSSavePanel::savePanel(mtm) };
            panel.setTitle(Some(&NSString::from_str("Export readings")));
            panel.setNameFieldStringValue(&NSString::from_str("aranet-readings.csv"));
            if panel.runModal() != objc2_app_kit::NSModalResponseOK {
                return;
            }
            let Some(url) = panel.URL() else { return };
            let Some(dest) = url.path().map(|p| p.to_string()) else { return };
            if path.exists() {
                let _ = std::fs::copy(&path, dest);
            }
        }
    }
);

pub fn show(
    mtm: MainThreadMarker,
    db_path: &Path,
    log_path: &Path,
    nicknames: &BTreeMap<String, String>,
) -> Retained<NSWindow> {
    let body = format_history(db_path, nicknames);
    let text = unsafe { NSTextView::initWithFrame(NSTextView::alloc(mtm), NSRect::ZERO) };
    text.setEditable(false);
    text.setDrawsBackground(false);
    text.setString(&NSString::from_str(&body));
    text.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(11.0, 0.0)));
    text.setTextColor(Some(&NSColor::labelColor()));

    let scroll = unsafe { NSScrollView::initWithFrame(NSScrollView::alloc(mtm), NSRect::ZERO) };
    scroll.setHasVerticalScroller(true);
    scroll.setBorderType(objc2_app_kit::NSBorderType::NoBorder);
    scroll.setDocumentView(Some(&text));
    text.setMinSize(NSSize::new(0.0, 0.0));

    let export: Retained<ExportTarget> = {
        let this = ExportTarget::alloc(mtm).set_ivars(ExportIvars {
            mtm,
            log_path: RefCell::new(log_path.to_path_buf()),
        });
        unsafe { msg_send![super(this), init] }
    };
    let export_btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Export CSV…"),
            Some(&export),
            Some(objc2::sel!(exportCsv:)),
            mtm,
        )
    };
    export_btn.setBezelStyle(objc2_app_kit::NSBezelStyle::Rounded);

    let w = 580.0;
    let h = 440.0;
    let toolbar_h = 44.0;
    let content = FlippedView::new(mtm, rect(0.0, 0.0, w, h));
    export_btn.setFrame(rect(16.0, 8.0, 128.0, 28.0));
    scroll.setFrame(rect(0.0, toolbar_h, w, h - toolbar_h));
    content.addSubview(&export_btn);
    content.addSubview(&scroll);

    let style = NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Resizable;
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            rect(0.0, 0.0, w, h),
            style,
            objc2_app_kit::NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str("History"));
    window.setContentView(Some(&content));
    window.center();
    window.makeKeyAndOrderFront(None);
    let _export = export;
    window
}

fn format_history(db_path: &Path, nicknames: &BTreeMap<String, String>) -> String {
    let Ok(db) = Db::open(db_path, Path::new("/dev/null")) else {
        return "Could not open the readings database.".into();
    };
    let rows = db.recent_readings(None, 400).unwrap_or_default();
    if rows.is_empty() {
        return "No readings stored yet.\n\nMeasurements appear here after your sensors broadcast.".into();
    }
    let mut out = String::from("Time\t\tSensor / room\t\tCO₂\tTemp\n\n");
    for (sensor, time, co2, temp) in rows {
        let label = nicknames.get(&sensor).cloned().unwrap_or(sensor);
        let co2s = co2.map(|c| c.to_string()).unwrap_or_else(|| "—".into());
        let temps = temp.map(|t| format!("{t:.1}°C")).unwrap_or_else(|| "—".into());
        out.push_str(&format!("{time}\t{label}\t{co2s} ppm\t{temps}\n"));
    }
    out
}
