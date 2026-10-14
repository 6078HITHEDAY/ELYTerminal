use ely_gpui_component::{
    primitives::FocusScope,
    shell::{ColumnShell, ResizeBorder, ShellColumn},
    theme::ActiveTheme,
};
use gpui::{
    App, Context, Entity, FocusHandle, InteractiveElement, IntoElement, KeyBinding, ParentElement,
    Render, SharedString, Styled, Subscription, Window, actions, div,
};

use crate::{
    settings::{Settings, SettingsStore},
    term::{TerminalEvent, TerminalView},
};

actions!(elyterminal, [ToggleSessions, TogglePanel]);

/// The key context of the main window's content.
const CONTEXT: &str = "Workspace";

/// The window's title while the program names none.
pub const TITLE: &str = "ELYTerminal";

/// Cmd-B and Cmd-Alt-B toggle the side columns; Ctrl-Shift-B and Ctrl-Shift-Alt-B off macOS.
pub fn bind_keys(cx: &mut App) {
    let (sessions, panel) = if cfg!(target_os = "macos") {
        ("cmd-b", "cmd-alt-b")
    } else {
        ("ctrl-shift-b", "ctrl-shift-alt-b")
    };
    cx.bind_keys([
        KeyBinding::new(sessions, ToggleSessions, Some(CONTEXT)),
        KeyBinding::new(panel, TogglePanel, Some(CONTEXT)),
    ]);
}

/// The main window's content: sessions, the terminal, and Git or Files, in Ely's columns.
pub struct Workspace {
    focus: FocusHandle,
    terminal: Entity<TerminalView>,
    /// The program's title, or the app's name while it names none.
    title: SharedString,
    _events: Subscription,
    _settings: Subscription,
}

impl Workspace {
    pub fn new(
        terminal: Entity<TerminalView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus: cx.focus_handle(),
            title: TITLE.into(),
            _events: cx.subscribe_in(&terminal, window, Self::terminal_event),
            _settings: cx.observe_global::<SettingsStore>(|_, cx| cx.notify()),
            terminal,
        }
    }

    /// The window and the terminal's header wear the program's title.
    fn terminal_event(
        &mut self,
        _: &Entity<TerminalView>,
        event: &TerminalEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let TerminalEvent::Title(title) = event {
            let title = if title.is_empty() { TITLE } else { title };
            if self.title.as_ref() != title {
                log::debug!("app: the title is {title:?}");
                self.title = SharedString::from(title.to_string());
                window.set_window_title(title);
                cx.notify();
            }
        }
    }

    /// Shows or hides a side column, as the settings keep it.
    fn toggle(&mut self, flip: fn(&mut Settings), cx: &mut Context<Self>) {
        match SettingsStore::update(cx, flip) {
            Ok(()) => {
                let settings = SettingsStore::get(cx);
                log::info!(
                    "app: sessions {}, right column {}",
                    if settings.show_sessions {
                        "shown"
                    } else {
                        "hidden"
                    },
                    if settings.show_panel {
                        "shown"
                    } else {
                        "hidden"
                    }
                );
            }
            Err(error) => log::error!("app: the columns stay: {error:#}"),
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = SettingsStore::get(cx);
        let width = cx.theme().sidebar_width(false);
        let mut shell = ColumnShell::new("workspace");
        if settings.show_sessions {
            shell = shell.column(ShellColumn::new("sessions").width(width));
        }
        shell = shell.column(
            ShellColumn::new("terminal")
                .title(self.title.clone())
                .child(self.terminal.clone()),
        );
        if settings.show_panel {
            shell = shell.column(ShellColumn::new("panel").width(width));
        }
        ResizeBorder::new().child(
            FocusScope::new(&self.focus).root().size_full().child(
                div()
                    .size_full()
                    .key_context(CONTEXT)
                    .on_action(cx.listener(|this, _: &ToggleSessions, _, cx| {
                        this.toggle(
                            |settings| settings.show_sessions = !settings.show_sessions,
                            cx,
                        )
                    }))
                    .on_action(cx.listener(|this, _: &TogglePanel, _, cx| {
                        this.toggle(|settings| settings.show_panel = !settings.show_panel, cx)
                    }))
                    .child(shell),
            ),
        )
    }
}
