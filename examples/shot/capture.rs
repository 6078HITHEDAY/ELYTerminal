use std::path::Path;

use anyhow::{Context as _, bail};
use objc::{class, msg_send, runtime::Object, sel, sel_impl};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// A gpui window's number, as CoreGraphics knows it.
pub fn number(window: &gpui::Window) -> anyhow::Result<u32> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("no native handle: {error:?}"))?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        bail!("not an AppKit window");
    };
    let view = appkit.ns_view.as_ptr() as *mut Object;
    let native: *mut Object = unsafe { msg_send![view, window] };
    let number: i64 = unsafe { msg_send![native, windowNumber] };
    u32::try_from(number).context("window number out of range")
}

/// Shows the window as key, the app left inactive.
pub fn front(number: u32) -> anyhow::Result<()> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let window: *mut Object = msg_send![app, windowWithWindowNumber: i64::from(number)];
        anyhow::ensure!(!window.is_null(), "no window {number}");
        let _: () = msg_send![window, makeKeyWindow];
        let _: () = msg_send![window, orderFrontRegardless];
    }
    Ok(())
}

/// AppKit's class of each view under the content.
pub fn views(number: u32) -> anyhow::Result<Vec<String>> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let window: *mut Object = msg_send![app, windowWithWindowNumber: i64::from(number)];
        anyhow::ensure!(!window.is_null(), "no window {number}");
        let content: *mut Object = msg_send![window, contentView];
        let views: *mut Object = msg_send![content, subviews];
        let count: u64 = msg_send![views, count];
        let named = (0..count).map(|ix| {
            let view: *mut Object = msg_send![views, objectAtIndex: ix];
            let class: *mut Object = msg_send![view, className];
            let text: *const std::ffi::c_char = msg_send![class, UTF8String];
            std::ffi::CStr::from_ptr(text)
                .to_string_lossy()
                .into_owned()
        });
        Ok(named.collect())
    }
}

/// Each menu item with its key and modifier mask.
pub fn menus() -> Vec<String> {
    let text = |string: *mut Object| -> String {
        let bytes: *const std::ffi::c_char = unsafe { msg_send![string, UTF8String] };
        unsafe { std::ffi::CStr::from_ptr(bytes) }
            .to_string_lossy()
            .into_owned()
    };
    let mut lines = Vec::new();
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let bar: *mut Object = msg_send![app, mainMenu];
        let count: i64 = msg_send![bar, numberOfItems];
        for at in 0..count {
            let top: *mut Object = msg_send![bar, itemAtIndex: at];
            let menu: *mut Object = msg_send![top, submenu];
            let name = text(msg_send![menu, title]);
            let items: i64 = msg_send![menu, numberOfItems];
            for row in 0..items {
                let item: *mut Object = msg_send![menu, itemAtIndex: row];
                let key = text(msg_send![item, keyEquivalent]);
                let mask: u64 = msg_send![item, keyEquivalentModifierMask];
                let title = text(msg_send![item, title]);
                lines.push(format!("{name} > {title} [{key:?} {mask:#x}]"));
            }
        }
    }
    lines
}

/// Writes window `number` as a PNG.
pub fn snapshot(number: u32, path: &Path) -> anyhow::Result<()> {
    use core_graphics::{
        geometry::{CGPoint, CGRect, CGSize},
        window,
    };
    let image = window::create_image(
        CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(0.0, 0.0)),
        window::kCGWindowListOptionIncludingWindow,
        number,
        window::kCGWindowImageBoundsIgnoreFraming | window::kCGWindowImageBestResolution,
    )
    .context("window capture returned nothing")?;
    if image.bits_per_pixel() != 32 {
        bail!("{} bits per pixel", image.bits_per_pixel());
    }
    let (width, height, stride) = (image.width(), image.height(), image.bytes_per_row());
    let data = image.data();
    let bytes = data.bytes();
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let at = y * stride + x * 4;
            rgba.extend_from_slice(&[bytes[at + 2], bytes[at + 1], bytes[at], bytes[at + 3]]);
        }
    }
    image::save_buffer(
        path,
        &rgba,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    )
    .with_context(|| format!("cannot write {}", path.display()))
}
