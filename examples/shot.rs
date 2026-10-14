//! Runs a program in the main window, plays keys and mouse gestures on it, and photographs it.

#[cfg(target_os = "macos")]
fn main() {
    use std::time::Duration;

    use ely_gpui_component::Assets;
    use elyterminal::{app, term::Launch};

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("elyterminal=info"))
        .init();
    let mut args = std::env::args().skip(1).peekable();
    let keys: Vec<String> = if args.peek().map(String::as_str) == Some("--keys") {
        args.next();
        let keys = args.next().expect("keys after --keys");
        keys.split_whitespace().map(String::from).collect()
    } else {
        Vec::new()
    };
    let out = args.next().expect("a png path");
    let program = args.next().expect("a program");
    let launch = Launch {
        program: Some((program, args.collect())),
        cwd: std::env::current_dir().expect("a current folder"),
        env: Vec::new(),
        reports: false,
    };
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut gpui::App| {
            ely_gpui_component::init(cx).expect("Ely starts");
            app::start(cx).expect("the app starts");
            let window = app::open(launch, cx).expect("the window opens");
            cx.spawn(async move |cx| {
                let number = window
                    .update(cx, |_, window, _| capture::number(window))
                    .expect("the window lives")
                    .expect("a window number");
                capture::front(number).expect("the window on screen");
                cx.background_executor().timer(Duration::from_secs(1)).await;
                for spec in &keys {
                    typing::post(spec, number).expect("a key event");
                    cx.background_executor()
                        .timer(Duration::from_millis(120))
                        .await;
                }
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let (number, title) = window
                    .update(cx, |_, window, _| {
                        (capture::number(window), window.window_title())
                    })
                    .expect("the window lives");
                let number = number.expect("a window number");
                capture::snapshot(number, std::path::Path::new(&out)).expect("a picture");
                log::info!("shot: wrote {out}, titled {title:?}");
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
}

#[cfg(target_os = "macos")]
mod capture {
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

    /// Shows the window without activating the app: gpui draws it, real input stays put.
    pub fn front(number: u32) -> anyhow::Result<()> {
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let window: *mut Object = msg_send![app, windowWithWindowNumber: i64::from(number)];
            anyhow::ensure!(!window.is_null(), "no window {number}");
            let _: () = msg_send![window, orderFrontRegardless];
        }
        Ok(())
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

#[cfg(target_os = "macos")]
mod typing {
    use anyhow::Context as _;
    use core_graphics::{
        event::{CGEvent, CGEventFlags, CGEventType, CGMouseButton, EventField, ScrollEventUnit},
        event_source::{CGEventSource, CGEventSourceStateID},
        geometry::{CGPoint, CGRect},
    };
    use foreign_types::ForeignType;
    use objc::{
        class, msg_send,
        runtime::{NO, Object, Sel, YES},
        sel, sel_impl,
    };

    /// US layout key codes and what each key types.
    const KEYS: [(&str, u16, &str); 26] = [
        ("a", 0, "a"),
        ("b", 11, "b"),
        ("c", 8, "c"),
        ("e", 14, "e"),
        ("f", 3, "f"),
        ("i", 34, "i"),
        ("s", 1, "s"),
        ("v", 9, "v"),
        ("t", 17, "t"),
        ("w", 13, "w"),
        ("1", 18, "1"),
        ("2", 19, "2"),
        ("5", 23, "5"),
        ("9", 25, "9"),
        ("[", 33, "["),
        ("space", 49, " "),
        ("tab", 48, "\t"),
        ("enter", 36, "\r"),
        ("backspace", 51, "\u{7f}"),
        ("escape", 53, "\u{1b}"),
        ("up", 126, "\u{F700}"),
        ("down", 125, "\u{F701}"),
        ("left", 123, "\u{F702}"),
        ("f1", 122, "\u{F704}"),
        ("f5", 96, "\u{F708}"),
        ("f6", 97, "\u{F709}"),
    ];

    /// AppKit's `NSRange`.
    #[repr(C)]
    struct Range {
        location: u64,
        length: u64,
    }

    fn string(text: &str) -> *mut Object {
        let text = std::ffi::CString::new(text).expect("no NUL in key text");
        unsafe { msg_send![class!(NSString), stringWithUTF8String: text.as_ptr()] }
    }

    /// `mark` composes `text` as an input method does; `commit` types it.
    fn compose(call: &str, text: &str, number: u32) -> anyhow::Result<()> {
        let none = Range {
            location: i64::MAX as u64,
            length: 0,
        };
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let window: *mut Object = msg_send![app, windowWithWindowNumber: i64::from(number)];
            anyhow::ensure!(!window.is_null(), "no window {number}");
            let client: *mut Object = msg_send![window, firstResponder];
            match call {
                "mark" => {
                    let end = Range {
                        location: text.encode_utf16().count() as u64,
                        length: 0,
                    };
                    let _: () = msg_send![client, setMarkedText: string(text) selectedRange: end replacementRange: none];
                }
                "commit" => {
                    let _: () = msg_send![client, insertText: string(text) replacementRange: none];
                }
                other => anyhow::bail!("no input method call {other}"),
            }
        }
        log::info!("shot: {call} {text:?}");
        Ok(())
    }

    unsafe extern "C" {
        fn CGEventSetLocation(event: *mut core_graphics::sys::CGEvent, location: CGPoint);
    }

    /// A gesture's steps: each one's name, point and click count.
    fn steps(gesture: &str, points: &[CGPoint]) -> anyhow::Result<Vec<(String, CGPoint, i64)>> {
        let step = |name: &str, at: CGPoint, clicks| (name.to_string(), at, clicks);
        let (at, to) = (points[0], points.get(1).copied());
        Ok(match gesture {
            "click" | "click2" | "click3" => (1..=gesture[5..].parse().unwrap_or(1))
                .flat_map(|clicks| [step("ldown", at, clicks), step("lup", at, clicks)])
                .collect(),
            "rclick" => vec![step("rdown", at, 1), step("rup", at, 1)],
            "mclick" => vec![step("mdown", at, 1), step("mup", at, 1)],
            "drag" => {
                let to = to.context("drag needs a second point")?;
                vec![
                    step("ldown", at, 1),
                    step("ldrag", to, 1),
                    step("lup", to, 1),
                ]
            }
            name => vec![step(name, at, 1)],
        })
    }

    /// A step's event type, button and the selector gpui's view takes it by.
    fn kind(step: &str) -> anyhow::Result<(CGEventType, CGMouseButton, &'static str)> {
        Ok(match step {
            "ldown" => (
                CGEventType::LeftMouseDown,
                CGMouseButton::Left,
                "mouseDown:",
            ),
            "lup" => (CGEventType::LeftMouseUp, CGMouseButton::Left, "mouseUp:"),
            "ldrag" => (
                CGEventType::LeftMouseDragged,
                CGMouseButton::Left,
                "mouseDragged:",
            ),
            "rdown" => (
                CGEventType::RightMouseDown,
                CGMouseButton::Right,
                "rightMouseDown:",
            ),
            "rup" => (
                CGEventType::RightMouseUp,
                CGMouseButton::Right,
                "rightMouseUp:",
            ),
            "mdown" => (
                CGEventType::OtherMouseDown,
                CGMouseButton::Center,
                "otherMouseDown:",
            ),
            "mup" => (
                CGEventType::OtherMouseUp,
                CGMouseButton::Center,
                "otherMouseUp:",
            ),
            "move" => (CGEventType::MouseMoved, CGMouseButton::Left, "mouseMoved:"),
            other => anyhow::bail!("no mouse gesture {other}"),
        })
    }

    /// Hands gpui's view a mouse gesture as AppKit events, even while the window is inactive.
    fn mouse(name: &str, flags: u64, number: u32) -> anyhow::Result<()> {
        let mut fields = name.split('@');
        let gesture = fields.next().context("a gesture")?;
        let numbers: Vec<f64> = fields
            .flat_map(|field| field.split(','))
            .map(str::parse)
            .collect::<Result<_, _>>()
            .with_context(|| format!("numbers in {name}"))?;
        anyhow::ensure!(numbers.len() >= 2, "{name} needs a point");
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let window: *mut Object = msg_send![app, windowWithWindowNumber: i64::from(number)];
            anyhow::ensure!(!window.is_null(), "no window {number}");
            let view: *mut Object = msg_send![window, firstResponder];
            let content: *mut Object = msg_send![window, contentView];
            let frame: CGRect = msg_send![content, frame];
            let height = frame.size.height;
            let points: Vec<CGPoint> = numbers
                .chunks_exact(2)
                .map(|pair| CGPoint::new(pair[0], height - pair[1]))
                .collect();
            let screens: *mut Object = msg_send![class!(NSScreen), screens];
            let main: *mut Object = msg_send![screens, objectAtIndex: 0u64];
            let screen: CGRect = msg_send![main, frame];
            let source = || {
                CGEventSource::new(CGEventSourceStateID::Private)
                    .map_err(|_| anyhow::anyhow!("no event source"))
            };
            let deliver = |event: CGEvent, at: CGPoint, selector: &str| -> anyhow::Result<()> {
                CGEventSetLocation(
                    event.as_ptr(),
                    CGPoint::new(at.x, screen.size.height - at.y),
                );
                event.set_flags(CGEventFlags::from_bits_truncate(flags));
                let native: *mut Object =
                    msg_send![class!(NSEvent), eventWithCGEvent: event.as_ptr()];
                anyhow::ensure!(!native.is_null(), "AppKit refused {name}");
                let selector = Sel::register(selector);
                let _: *mut Object = msg_send![view, performSelector: selector withObject: native];
                Ok(())
            };
            if let Some(up) = gesture.strip_prefix("scroll") {
                let lines = *numbers.get(2).context("a scroll needs lines")? as i32;
                let lines = if up == "up" { lines } else { -lines };
                let event =
                    CGEvent::new_scroll_event(source()?, ScrollEventUnit::LINE, 1, lines, 0, 0)
                        .map_err(|_| anyhow::anyhow!("no scroll event"))?;
                deliver(event, points[0], "scrollWheel:")?;
            } else {
                for (step, at, clicks) in steps(gesture, &points)? {
                    let (kind, button, selector) = kind(&step)?;
                    let event = CGEvent::new_mouse_event(source()?, kind, at, button)
                        .map_err(|_| anyhow::anyhow!("no mouse event"))?;
                    event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, clicks);
                    deliver(event, at, selector)?;
                }
            }
        }
        log::info!("shot: mouse {name}");
        Ok(())
    }

    /// Posts `spec`, a key, an input method call or a mouse gesture, on window `number`.
    pub fn post(spec: &str, number: u32) -> anyhow::Result<()> {
        if let Some((call, text)) = spec.split_once(':') {
            return compose(call, text, number);
        }
        if let "activate" | "deactivate" = spec {
            unsafe {
                let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                if spec == "activate" {
                    let _: () = msg_send![app, activateIgnoringOtherApps: YES];
                } else {
                    let _: () = msg_send![app, deactivate];
                }
            }
            log::info!("shot: {spec}");
            return Ok(());
        }
        let mut parts: Vec<&str> = spec.split('-').collect();
        let name = parts.pop().context("an empty key")?;
        let mut flags: u64 = 0;
        let mut repeat = NO;
        for modifier in &parts {
            flags |= match *modifier {
                "repeat" => {
                    repeat = YES;
                    0
                }
                "shift" => 0x20000 | 0x02,
                "ctrl" => 0x40000 | 0x01,
                "lopt" => 0x80000 | 0x20,
                "ropt" => 0x80000 | 0x40,
                "cmd" => 0x100000 | 0x08,
                other => anyhow::bail!("no modifier {other}"),
            };
        }
        if name.contains('@') {
            return mouse(name, flags, number);
        }
        let (_, code, plain) = KEYS
            .iter()
            .find(|(key, ..)| *key == name)
            .with_context(|| format!("no key code for {name}"))?;
        if plain
            .chars()
            .next()
            .is_some_and(|ch| ('\u{F700}'..='\u{F8FF}').contains(&ch))
        {
            flags |= 0x800000 | 0x200000;
        }
        let ignoring = if flags & 0x20000 != 0 {
            plain.to_uppercase()
        } else {
            plain.to_string()
        };
        let typed = match plain.chars().next() {
            _ if flags & 0x80000 != 0 => String::new(),
            Some(letter @ 'a'..='z') if flags & 0x40000 != 0 => {
                char::from(letter as u8 - b'a' + 1).to_string()
            }
            _ => ignoring.clone(),
        };
        for kind in [10u64, 11] {
            unsafe {
                let event: *mut Object = msg_send![
                    class!(NSEvent),
                    keyEventWithType: kind
                    location: CGPoint::new(0.0, 0.0)
                    modifierFlags: flags
                    timestamp: 0.0f64
                    windowNumber: i64::from(number)
                    context: std::ptr::null_mut::<Object>()
                    characters: string(&typed)
                    charactersIgnoringModifiers: string(&ignoring)
                    isARepeat: if kind == 10 { repeat } else { NO }
                    keyCode: *code
                ];
                anyhow::ensure!(!event.is_null(), "AppKit refused the key {spec}");
                let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                let _: () = msg_send![app, postEvent: event atStart: NO];
            }
        }
        log::info!("shot: typed {spec}");
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("shot photographs windows on macOS only");
    std::process::exit(1);
}
