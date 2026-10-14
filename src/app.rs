use std::borrow::Cow;

use anyhow::Context as _;
use ely_gpui_component::{primitives::FocusScope, theme::ActiveTheme};
use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, Styled, TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions, px, size,
};

#[cfg(target_os = "macos")]
use crate::term::mac;
use crate::{
    settings::SettingsStore,
    term::{ICONS, Launch, MONO, TerminalView, config, input, shell::ShellIntegration, spawn},
    themes,
};

/// The main window's content.
pub struct Workspace {
    focus: FocusHandle,
    terminal: Entity<TerminalView>,
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        FocusScope::new(&self.focus)
            .root()
            .size_full()
            .bg(cx.theme().colors.bg)
            .child(self.terminal.clone())
    }
}

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
    #[cfg(target_os = "macos")]
    {
        mac::repeat_held_keys();
        mac::deliver_command_key_ups()?;
    }
    let family = &SettingsStore::get(cx).font_family;
    anyhow::ensure!(
        cx.text_system().all_font_names().contains(family),
        "settings: font_family is {family:?}, which is not installed"
    );
    themes::apply(cx);
    Ok(())
}

/// Opens the main window running `launch`.
pub fn open(launch: Launch, cx: &mut App) -> anyhow::Result<WindowHandle<Workspace>> {
    let bounds = Bounds::centered(None, size(px(1200.0), px(760.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("ELYTerminal".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let (pty, events) = spawn(&launch, (80, 24), (8, 16), config(SettingsStore::get(cx)))?;
    let window = cx.open_window(options, |window, cx| {
        window
            .observe_window_appearance(|_, cx| themes::apply(cx))
            .detach();
        let terminal = cx.new(|cx| TerminalView::new(pty, events, window, cx));
        window.focus(&terminal.focus_handle(cx), cx);
        cx.new(|cx| Workspace {
            focus: cx.focus_handle(),
            terminal,
        })
    })?;
    log::info!("app: main window open");
    Ok(window)
}
