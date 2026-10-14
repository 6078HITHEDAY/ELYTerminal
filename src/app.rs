use std::borrow::Cow;

use anyhow::Context as _;
use ely_gpui_component::{primitives::FocusScope, theme::ActiveTheme};
use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, Styled, Subscription, TitlebarOptions, Window, WindowBounds, WindowHandle,
    WindowOptions, px, size,
};

#[cfg(target_os = "macos")]
use crate::term::mac;
use crate::{
    settings::SettingsStore,
    term::{
        ICONS, Launch, MONO, TerminalEvent, TerminalView, config, input, shell::ShellIntegration,
        spawn,
    },
    themes,
};

/// The window title while the program names none.
const TITLE: &str = "ELYTerminal";

/// The main window's content.
pub struct Workspace {
    focus: FocusHandle,
    terminal: Entity<TerminalView>,
    _events: Subscription,
}

impl Workspace {
    /// The window wears the terminal's title.
    fn terminal_event(
        &mut self,
        _: &Entity<TerminalView>,
        event: &TerminalEvent,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        if let TerminalEvent::Title(title) = event {
            let title = if title.is_empty() { TITLE } else { title };
            if window.window_title() != title {
                log::debug!("app: the window title is {title:?}");
                window.set_window_title(title);
            }
        }
    }
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
            title: Some(TITLE.into()),
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
            _events: cx.subscribe_in(&terminal, window, Workspace::terminal_event),
            terminal,
        })
    })?;
    log::info!("app: main window open");
    Ok(window)
}
