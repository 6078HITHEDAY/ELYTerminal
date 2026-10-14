//! `cargo run --example shot -- <png> <program> [args…]`: runs a program in the main window, then photographs the window.

#[cfg(target_os = "macos")]
fn main() {
    use std::time::Duration;

    use ely_gpui_component::Assets;
    use elyterminal::{app, term::Launch};

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("elyterminal=info"))
        .init();
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("a png path");
    let program = args.next().expect("a program");
    let launch = Launch {
        program: Some((program, args.collect())),
        cwd: std::env::current_dir().expect("a current folder"),
        env: Vec::new(),
    };
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut gpui::App| {
            ely_gpui_component::init(cx).expect("Ely starts");
            app::start(cx).expect("the app starts");
            let window = app::open(launch, cx).expect("the window opens");
            cx.activate(true);
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                let number = window
                    .update(cx, |_, window, _| capture::number(window))
                    .expect("the window lives")
                    .expect("a window number");
                capture::snapshot(number, std::path::Path::new(&out)).expect("a picture");
                log::info!("shot: wrote {out}");
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
}

#[cfg(target_os = "macos")]
mod capture {
    use std::path::Path;

    use anyhow::{Context as _, bail};
    use objc::{msg_send, runtime::Object, sel, sel_impl};
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

    /// Writes the window `number` as a PNG; the app's own window needs no permission.
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
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("shot photographs windows on macOS only");
    std::process::exit(1);
}
