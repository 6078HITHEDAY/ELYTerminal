use std::{borrow::Cow, time::Duration};

use anyhow::Context as _;
use ely_gpui_component::theme::ActiveTheme;
use gpui::{
    App, AppContext, Bounds, Context, Pixels, Size, Task, TitlebarOptions, Window, WindowBounds,
    WindowDecorations, WindowHandle, WindowOptions, px, size,
};

#[cfg(target_os = "macos")]
use crate::term::mac;
use crate::{
    blur, menus,
    settings::{self, SettingsStore},
    term::{ICONS, Launch, MONO, config, input, shell::ShellIntegration, spawn},
    themes,
    workspace::{self, TITLE, Workspace},
};

/// How long the window's size holds before it is kept, so a drag saves once.
const SETTLE: Duration = Duration::from_millis(500);

/// The id the macOS bundle and Linux desktop entry share.
pub const APP_ID: &str = "com.zacharyzhang.elyterminal";

/// Fonts, settings, integration scripts and the chosen font, each checked before a window opens.
pub fn start(cx: &mut App) -> anyhow::Result<()> {
    cx.text_system()
        .add_fonts(vec![
            Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf")),
            Cow::Borrowed(include_bytes!(
                "../assets/fonts/JetBrainsMono-BoldItalic.ttf"
            )),
            Cow::Borrowed(include_bytes!(
                "../assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"
            )),
        ])
        .context("app: the bundled fonts failed to register")?;
    let names = cx.text_system().all_font_names();
    for family in [MONO, ICONS] {
        anyhow::ensure!(
            names.iter().any(|name| name == family),
            "app: {family} is not among the registered fonts"
        );
    }
    SettingsStore::init(cx)?;
    ShellIntegration::init(cx)?;
    input::bind_keys(cx);
    workspace::bind_keys(cx);
    settings::window::bind_keys(cx);
    menus::init(cx);
    // Settings alone hold no terminal, so they go with the last one.
    cx.on_window_closed(|cx, _| {
        let windows = cx.windows();
        if !windows
            .iter()
            .any(|window| window.downcast::<Workspace>().is_some())
        {
            log::info!("app: no terminal window left; quitting");
            cx.quit();
        }
    })
    .detach();
    #[cfg(target_os = "macos")]
    {
        mac::repeat_held_keys();
        mac::deliver_command_key_ups()?;
    }
    #[cfg(target_os = "linux")]
    themes::omarchy::start(cx)?;
    themes::apply(cx);
    let chosen = |settings: &settings::Settings| {
        let theme = (settings.theme.clone(), settings.dark_theme.clone());
        (theme, settings.background_opacity)
    };
    let mut worn = chosen(SettingsStore::get(cx));
    cx.observe_global::<SettingsStore>(move |cx| {
        let now = chosen(SettingsStore::get(cx));
        if now != worn {
            worn = now;
            themes::apply(cx);
        }
    })
    .detach();
    Ok(())
}

/// Opens the main window running `launch`.
pub fn open(launch: Launch, cx: &mut App) -> anyhow::Result<WindowHandle<Workspace>> {
    let kept = opening_size(cx);
    let bounds = Bounds::centered(None, kept, cx);
    let background = blur::appearance(SettingsStore::get(cx));
    let least = px(*settings::WINDOW.start());
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // No smaller than the settings keep.
        window_min_size: Some(size(least, least)),
        titlebar: Some(TitlebarOptions {
            title: Some(TITLE.into()),
            appears_transparent: true,
            traffic_light_position: Some(cx.theme().traffic_light_origin()),
        }),
        // Ely's columns draw the controls; only Linux reads this, and must not draw its own.
        window_decorations: Some(WindowDecorations::Client),
        window_background: background,
        ..Default::default()
    };
    let (pty, events) = spawn(&launch, (80, 24), (8, 16), config(SettingsStore::get(cx)))?;
    let window = cx.open_window(options, |window, cx| {
        window.set_app_id(APP_ID);
        window
            .observe_window_appearance(|_, cx| themes::apply(cx))
            .detach();
        cx.new(|cx| {
            keep_size(window, cx);
            Workspace::new(pty, events, launch, window, cx)
        })
    })?;
    log::info!(
        "app: main window open at {} by {}, blur: {background:?} on {}",
        f32::from(kept.width),
        f32::from(kept.height),
        std::env::consts::OS
    );
    Ok(window)
}

/// The kept size, within the primary display's visible area where that is known.
fn opening_size(cx: &App) -> Size<Pixels> {
    let settings = SettingsStore::get(cx);
    let room = cx
        .primary_display()
        .map(|display| display.visible_bounds().size);
    let fit = |want: f32, side: Option<Pixels>| match side.map(f32::from) {
        Some(side) if side > 0.0 && want > side => {
            log::info!(
                "app: a kept side of {want} px passes the screen's {side}; the window takes {side}"
            );
            px(side)
        }
        _ => px(want),
    };
    size(
        fit(settings.window_width, room.map(|area| area.width)),
        fit(settings.window_height, room.map(|area| area.height)),
    )
}

/// Keeps the window's size in the settings once it settles; maximized, full screen or hidden, the size before stays.
fn keep_size(window: &mut Window, cx: &mut Context<Workspace>) {
    let mut _settling: Option<Task<()>> = None;
    cx.observe_window_bounds(window, move |_, window, cx| {
        _settling = Some(cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(SETTLE).await;
            // Read once settled: X11 marks a window maximized late, and unmaximized while hidden.
            let kept = cx.update(|window, cx| {
                if !window.is_visible() || window.is_maximized() || window.is_fullscreen() {
                    log::debug!(
                        "app: the window is hidden, maximized or full screen; its kept size stays"
                    );
                    return Ok(());
                }
                let area = window.window_bounds().get_bounds().size;
                let (width, height) = (
                    f32::from(area.width).round(),
                    f32::from(area.height).round(),
                );
                SettingsStore::update(cx, |settings| {
                    settings.window_width = width;
                    settings.window_height = height;
                })
                .with_context(|| {
                    format!("app: the window's size, {width} by {height}, is not kept")
                })
            });
            match kept {
                Ok(Ok(())) => {}
                Ok(Err(error)) => log::error!("{error:#}"),
                Err(_) => log::debug!("app: the window closed before its size settled"),
            }
        }));
    })
    .detach();
}
