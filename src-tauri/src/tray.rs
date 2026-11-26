use crate::server;
use crate::windows::LOCAL;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSApplication, NSAutoresizingMaskOptions, NSBezierPath, NSButton, NSColor, NSEvent, NSFont, NSGridView, NSImage, NSImageSymbolConfiguration, NSImageView, NSLayoutAttribute, NSLayoutConstraintOrientation, NSMenu, NSMenuItem, NSStackView, NSStackViewGravity, NSStatusBar, NSStatusItem, NSTextField, NSUserInterfaceLayoutOrientation, NSVariableStatusItemLength, NSView};
use objc2_foundation::{NSArray, NSCopying, NSData, NSEdgeInsets, NSPoint, NSRect, NSSize, NSString};
use std::cell::OnceCell;
use std::sync::OnceLock;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

static APP: OnceLock<AppHandle> = OnceLock::new();

struct Ui {
    _item: Retained<NSStatusItem>,
    header: Retained<Header>,
    versions: Retained<NSStackView>,
    server: Retained<NSTextField>,
}

thread_local! {
    static UI: OnceCell<Ui> = const { OnceCell::new() };
}

struct Parts {
    title: Retained<NSTextField>,
    status: Retained<NSTextField>,
    dot: Retained<NSImageView>,
    chevron: Retained<NSImageView>,
}

define_class!(
    #[unsafe(super(NSStackView, NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "ColabTrayHeader"]
    #[ivars = Parts]
    struct Header;

    impl Header {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let lit = self.enclosingMenuItem().is_some_and(|item| item.isHighlighted());
            let (text, dim) = if lit { (NSColor::selectedMenuItemTextColor(), NSColor::selectedMenuItemTextColor()) } else { (NSColor::labelColor(), NSColor::secondaryLabelColor()) };
            self.ivars().title.setTextColor(Some(&text));
            self.ivars().status.setTextColor(Some(&dim));
            self.ivars().chevron.setContentTintColor(Some(&dim));
            if lit {
                let size = self.bounds().size;
                NSColor::controlAccentColor().setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(NSRect::new(NSPoint::new(5.0, 0.0), NSSize::new(size.width - 10.0, size.height)), 5.0, 5.0).fill();
            }
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            self.close();
            open("server");
        }

        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: &AnyObject) {
            open("");
        }

        #[unsafe(method(quit:))]
        fn quit(&self, _sender: &AnyObject) {
            APP.get().inspect(|app| app.exit(0));
        }

        #[unsafe(method(copyVersions:))]
        fn copy_versions(&self, _sender: &AnyObject) {
            self.close();
            let Some(app) = APP.get() else { return };
            let server = UI.with(|ui| ui.get().map(|ui| ui.server.stringValue().to_string())).unwrap_or_default();
            let text = format!("Colab Notebook {}\nColab Notebook Server {server}", app.package_info().version);
            let _ = app.clipboard().write_text(text).map_err(|e| log::error!("copy versions: {e}"));
        }
    }
);

impl Header {
    fn close(&self) {
        if let Some(menu) = self.enclosingMenuItem().and_then(|item| unsafe { item.menu() }) {
            menu.cancelTracking();
        }
    }
}

fn open(id: &str) {
    if let Some(app) = APP.get() {
        crate::windows::settings(app, id);
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let _ = APP.set(app.clone());
    let mtm = MainThreadMarker::new().expect("the tray is created on the main thread");
    let included = app.package_info().version.to_string();
    let label = |text: &str, font: Retained<NSFont>, color: Retained<NSColor>| {
        let field = NSTextField::labelWithString(&NSString::from_str(text), mtm);
        field.setFont(Some(&font));
        field.setTextColor(Some(&color));
        field
    };
    let small = || NSFont::systemFontOfSize(NSFont::smallSystemFontSize());
    let symbol = |name: &str| NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(name), None).expect("SF Symbol");
    let stack = |views: &[&NSView], vertical: bool, spacing: f64| {
        let stack = NSStackView::stackViewWithViews(&NSArray::from_slice(views), mtm);
        stack.setSpacing(spacing);
        if vertical {
            stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            stack.setAlignment(NSLayoutAttribute::Leading);
        }
        stack
    };
    let fit = |view: &NSView, height: f64| {
        view.heightAnchor().constraintEqualToConstant(height).setActive(true);
        view.setFrameSize(view.fittingSize());
        view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    };

    let icon = NSApplication::sharedApplication(mtm).applicationIconImage().expect("app icon").copy();
    icon.setSize(NSSize::new(40.0, 40.0));
    let parts = Parts {
        title: label("Colab Notebook Server", NSFont::boldSystemFontOfSize(13.0), NSColor::labelColor()),
        status: label("Stopped", small(), NSColor::secondaryLabelColor()),
        dot: NSImageView::imageViewWithImage(&symbol("circle.fill"), mtm),
        chevron: NSImageView::imageViewWithImage(&symbol("chevron.right"), mtm),
    };
    parts.dot.setSymbolConfiguration(Some(&NSImageSymbolConfiguration::configurationWithPointSize_weight(6.0, 0.0)));
    let text = stack(&[&parts.title, &stack(&[&parts.dot, &parts.status], false, 5.0)], true, 2.0);
    let icon = NSImageView::imageViewWithImage(&icon, mtm);
    let header: Retained<Header> = unsafe { msg_send![super(Header::alloc(mtm).set_ivars(parts)), initWithFrame: NSRect::ZERO] };
    header.addView_inGravity(&icon, NSStackViewGravity::Leading);
    header.addView_inGravity(&text, NSStackViewGravity::Leading);
    header.addView_inGravity(&header.ivars().chevron, NSStackViewGravity::Trailing);
    header.setSpacing(6.0);
    header.setEdgeInsets(NSEdgeInsets { top: 0.0, left: 13.0, bottom: 0.0, right: 14.0 });
    fit(&header, 48.0);

    let name = |text: &str| label(text, small(), NSColor::secondaryLabelColor());
    let row = |name: &NSView, version: &NSView| NSArray::from_slice(&[name, version]);
    let server = label(&server_line(None, &included), small(), NSColor::labelColor());
    let grid = NSGridView::gridViewWithViews(&NSArray::from_retained_slice(&[row(&name("Colab Notebook"), &label(&included, small(), NSColor::labelColor())), row(&name("Colab Notebook Server"), &server)]), mtm);
    grid.setColumnSpacing(9.0);
    grid.setRowSpacing(2.0);
    grid.setContentHuggingPriority_forOrientation(999.0, NSLayoutConstraintOrientation::Vertical);
    let copy_image = symbol("doc.on.doc").imageWithSymbolConfiguration(&NSImageSymbolConfiguration::configurationWithHierarchicalColor(&NSColor::secondaryLabelColor())).expect("copy symbol");
    let copy = unsafe { NSButton::buttonWithImage_target_action(&copy_image, Some(&header), Some(sel!(copyVersions:)), mtm) };
    copy.setBordered(false);
    let versions = stack(&[&grid], false, 12.0);
    versions.addView_inGravity(&copy, NSStackViewGravity::Trailing);
    versions.setEdgeInsets(NSEdgeInsets { top: 0.0, left: 15.0, bottom: 0.0, right: 12.0 });
    fit(&versions, 39.0);

    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    let view_item = |view: &NSView| {
        let item = NSMenuItem::new(mtm);
        item.setView(Some(view));
        item
    };
    let action_item = |title: &str, action: Sel, key: &str| {
        let item = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(mtm), &NSString::from_str(title), Some(action), &NSString::from_str(key)) };
        unsafe { item.setTarget(Some(&header)) };
        item
    };
    for item in [view_item(&header), NSMenuItem::separatorItem(mtm), view_item(&versions), NSMenuItem::separatorItem(mtm), action_item("Settings...", sel!(openSettings:), ","), NSMenuItem::separatorItem(mtm), action_item("Quit Colab Notebook", sel!(quit:), "q")] {
        menu.addItem(&item);
    }

    let tray = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(include_bytes!("../icons/tray.png"))).expect("tray icon");
    tray.setSize(NSSize::new(89.0 * 18.0 / 64.0, 18.0));
    tray.setTemplate(true);
    let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(button) = item.button(mtm) {
        button.setImage(Some(&tray));
        button.setToolTip(Some(&NSString::from_str("Colab Notebook")));
    }
    item.setMenu(Some(&menu));
    UI.with(|ui| ui.set(Ui { _item: item, header, versions, server }).ok());
    tauri::async_runtime::spawn(watch(app.clone()));
    Ok(())
}

fn show(status: &str, running: Option<&str>, included: &str) {
    UI.with(|ui| {
        let Some(ui) = ui.get() else { return };
        let parts = ui.header.ivars();
        parts.status.setStringValue(&NSString::from_str(status));
        let dot = match status {
            "Running" => NSColor::systemGreenColor(),
            "Reconnecting" => NSColor::systemOrangeColor(),
            _ => NSColor::systemGrayColor(),
        };
        parts.dot.setContentTintColor(Some(&dot));
        ui.server.setStringValue(&NSString::from_str(&server_line(running, included)));
        ui.versions.setFrameSize(ui.versions.fittingSize());
    });
}

async fn watch(app: AppHandle) {
    let server = server::of(&app, LOCAL);
    let mut connection = server.connection();
    let included = app.package_info().version.to_string();
    let mut shown = None;
    loop {
        let connected = connection.borrow_and_update().is_some();
        let running = if connected { tokio::time::timeout(Duration::from_secs(3), server.call("version", serde_json::Value::Null)).await.ok().and_then(Result::ok).and_then(|v| v.as_str().map(String::from)) } else { None };
        let now = (server.status(), running);
        if shown.as_ref() != Some(&now) {
            let (status, running, included) = (now.0, now.1.clone(), included.clone());
            let _ = app.run_on_main_thread(move || show(status, running.as_deref(), &included)).map_err(|e| log::error!("tray: {e}"));
            shown = Some(now);
        }
        tokio::select! {
            _ = connection.changed() => {}
            () = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
    }
}

fn server_line(running: Option<&str>, included: &str) -> String {
    match running {
        Some(running) if server::older(running, included) => format!("{running} running, {included} included"),
        Some(running) => format!("{running} running"),
        None => format!("{included} included"),
    }
}
