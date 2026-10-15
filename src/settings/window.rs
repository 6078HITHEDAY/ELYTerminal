use ely_gpui_component::{
    buttons::SegmentedControl,
    feedback::InlineMessage,
    forms::{InputEvent, PathInput, TextInput},
    primitives::{FocusScope, IconName, Severity},
    settings::{SettingsLayout, SettingsRow, SettingsSection},
    shell::{ColumnShell, ResizeBorder, ShellColumn},
    theme::{ActiveTheme, ControlSize},
};
use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyBinding, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Subscription, TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowOptions, actions,
    div, px, size,
};

use super::{SettingsStore, StartIn};

actions!(elyterminal, [OpenSettings, CloseSettings]);

/// The key context of the settings window's content.
const CONTEXT: &str = "Settings";

/// Each category's key, name and icon.
const SECTIONS: [(&str, &str, IconName); 1] = [("general", "General", IconName::Settings)];

/// Cmd-, opens the settings, Cmd-W closes them; off macOS Ctrl-, and Ctrl-Shift-W, as Ghostty's.
pub fn bind_keys(cx: &mut App) {
    let (open, close) = if cfg!(target_os = "macos") {
        ("cmd-,", "cmd-w")
    } else {
        ("ctrl-,", "ctrl-shift-w")
    };
    cx.bind_keys([
        KeyBinding::new(open, OpenSettings, None),
        KeyBinding::new(close, CloseSettings, Some(CONTEXT)),
    ]);
    cx.on_action(|_: &OpenSettings, cx| open_window(cx));
}

/// Opens the settings window, or brings the open one forward.
fn open_window(cx: &mut App) {
    let open = cx
        .windows()
        .into_iter()
        .find_map(|window| window.downcast::<SettingsWindow>());
    if let Some(window) = open {
        log::info!("settings: the window comes forward");
        let forward = window.update(cx, |_, window, _| window.activate_window());
        if let Err(error) = forward {
            log::error!("settings: the window did not come forward: {error:#}");
        }
        return;
    }
    let bounds = Bounds::centered(None, size(px(760.0), px(560.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Settings".into()),
            appears_transparent: true,
            traffic_light_position: Some(cx.theme().traffic_light_origin()),
        }),
        // Ely's column draws the controls; only Linux reads this.
        window_decorations: Some(WindowDecorations::Client),
        ..Default::default()
    };
    match cx.open_window(options, |window, cx| {
        cx.new(|cx| SettingsWindow::new(window, cx))
    }) {
        Ok(_) => log::info!("settings: the window opens"),
        Err(error) => log::error!("settings: the window did not open: {error:#}"),
    }
}

/// The settings window: categories down the side, the open one's rows beside them.
pub struct SettingsWindow {
    focus: FocusHandle,
    section: SharedString,
    shell: Entity<TextInput>,
    /// Why the typed shell is not saved.
    refused: Option<SharedString>,
    _typed: Subscription,
    _settings: Subscription,
}

impl SettingsWindow {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let shell = cx.new(|cx| {
            let mut input = TextInput::new(window, cx).placeholder("Login shell");
            input.set_text(SettingsStore::get(cx).shell.clone(), cx);
            input
        });
        let typed = cx.subscribe(&shell, |this, input, event: &InputEvent, cx| {
            if let InputEvent::Changed = event {
                let shell = input.read(cx).text().to_string();
                this.save_shell(shell, cx);
            }
        });
        // Focus inside, so the window's keys find its context.
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            focus,
            section: SECTIONS[0].0.into(),
            shell,
            refused: None,
            _typed: typed,
            _settings: cx.observe_global::<SettingsStore>(|_, cx| cx.notify()),
        }
    }

    /// Saves `shell` when it names a program; else says why and keeps the saved one.
    fn save_shell(&mut self, shell: String, cx: &mut Context<Self>) {
        let refused = match SettingsStore::update(cx, |settings| settings.shell = shell.clone()) {
            Ok(()) => {
                log::info!("settings: new sessions run {shell:?}");
                None
            }
            Err(error) => {
                log::debug!("settings: the shell stays: {error:#}");
                Some(format!("{error:#}").into())
            }
        };
        if refused != self.refused {
            self.refused = refused;
            cx.notify();
        }
    }

    fn general(&self, cx: &Context<Self>) -> SettingsSection {
        let start = match SettingsStore::get(cx).start_in {
            StartIn::Home => "home",
            StartIn::Session => "session",
        };
        let shell = div()
            .w(cx.theme().prose_width())
            .flex()
            .flex_col()
            .gap_1()
            .child(PathInput::new("settings-shell", &self.shell))
            .children((self.refused.clone()).map(|why| InlineMessage::new(Severity::Danger, why)));
        let start = SegmentedControl::new("settings-start-in", start)
            .segment("home", "Home", None)
            .segment("session", "Session's folder", None)
            .size(ControlSize::Sm)
            .on_change(|value, _, cx| start_in(value, cx));
        SettingsSection::new("General")
            .row(SettingsRow::new("Shell").control(shell))
            .row(SettingsRow::new("Start in").control(start))
    }
}

/// Where new sessions start, as the settings keep it.
fn start_in(value: &SharedString, cx: &mut App) {
    let start = match value.as_ref() {
        "home" => StartIn::Home,
        "session" => StartIn::Session,
        other => unreachable!("settings: no start {other}"),
    };
    match SettingsStore::update(cx, |settings| settings.start_in = start) {
        Ok(()) => log::info!("settings: new sessions start in {start:?}"),
        Err(error) => log::error!("settings: the start stays: {error:#}"),
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pick = cx.entity();
        let layout = SettingsLayout::new("settings", SECTIONS, self.section.clone(), {
            move |key, _, cx| {
                pick.update(cx, |this, cx| {
                    this.section = key.clone();
                    cx.notify();
                })
            }
        })
        .page(self.general(cx));
        let body = div()
            .id("settings-body")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .child(layout);
        let shell = ColumnShell::new("settings").column(
            ShellColumn::new("settings-column")
                .title("Settings")
                .child(body),
        );
        // The context sits above the focus root, so its keys work wherever focus is.
        div()
            .size_full()
            .key_context(CONTEXT)
            .on_action(|_: &CloseSettings, window, _| {
                log::info!("settings: the window closes");
                window.remove_window();
            })
            .on_action(|_: &OpenSettings, window, _| {
                log::info!("settings: the window comes forward");
                window.activate_window();
            })
            .child(
                ResizeBorder::new()
                    .child(FocusScope::new(&self.focus).root().size_full().child(shell)),
            )
    }
}
