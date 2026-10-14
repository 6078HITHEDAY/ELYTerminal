use std::path::PathBuf;

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    lists::List,
    primitives::{FocusScope, IconName},
    shell::{ColumnShell, ResizeBorder, ShellColumn},
    theme::{ActiveTheme, ControlSize},
};
use gpui::{
    Action, App, AppContext, Context, EntityId, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyBinding, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Window, actions, div,
};

use crate::{
    sessions::{self, Session},
    settings::{Settings, SettingsStore, StartIn},
    term::{
        Events, Pty, TerminalEvent, TerminalView, config,
        shell::{ShellIntegration, launch},
        spawn,
    },
};

actions!(
    elyterminal,
    [
        ToggleSessions,
        TogglePanel,
        NewSession,
        CloseSession,
        NextSession,
        PreviousSession,
        LastSession
    ]
);

/// Shows the session at this place, from 0; past the last one, the key goes on to the program.
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = elyterminal, no_json)]
pub struct SelectSession(pub usize);

/// The key context of the main window's content.
const CONTEXT: &str = "Workspace";

/// The window's title while the program names none.
pub const TITLE: &str = "ELYTerminal";

/// Ghostty's keys: Cmd on macOS; off it, Ctrl-Shift for keys a terminal takes, Alt for digits.
pub fn bind_keys(cx: &mut App) {
    let (sessions, panel, new, close, digit) = if cfg!(target_os = "macos") {
        ("cmd-b", "cmd-alt-b", "cmd-t", "cmd-w", "cmd")
    } else {
        (
            "ctrl-shift-b",
            "ctrl-shift-alt-b",
            "ctrl-shift-t",
            "ctrl-shift-w",
            "alt",
        )
    };
    let context = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new(sessions, ToggleSessions, context),
        KeyBinding::new(panel, TogglePanel, context),
        KeyBinding::new(new, NewSession, context),
        KeyBinding::new(close, CloseSession, context),
        KeyBinding::new("ctrl-tab", NextSession, context),
        KeyBinding::new("ctrl-shift-tab", PreviousSession, context),
        KeyBinding::new(&format!("{digit}-9"), LastSession, context),
    ]);
    cx.bind_keys(
        (1..=8).map(|n| KeyBinding::new(&format!("{digit}-{n}"), SelectSession(n - 1), context)),
    );
}

/// The main window's content: sessions, the terminal, and Git or Files, in Ely's columns.
pub struct Workspace {
    focus: FocusHandle,
    sessions: Vec<Session>,
    /// The session shown.
    active: usize,
    _settings: Subscription,
}

impl Workspace {
    /// The window's content with its first session, `pty` started in `folder`.
    pub fn new(
        pty: Pty,
        events: Events,
        folder: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut workspace = Self {
            focus: cx.focus_handle(),
            sessions: Vec::new(),
            active: 0,
            _settings: cx.observe_global::<SettingsStore>(|_, cx| cx.notify()),
        };
        workspace.add(pty, events, folder, window, cx);
        workspace.select(0, window, cx);
        workspace
    }

    fn add(
        &mut self,
        pty: Pty,
        events: Events,
        folder: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let terminal = cx.new(|cx| TerminalView::new(pty, events, window, cx));
        let id = terminal.entity_id();
        let events = cx.subscribe_in(&terminal, window, move |this, _, event, window, cx| {
            this.session_event(id, event, window, cx)
        });
        let pid = terminal.read(cx).pid();
        log::info!("app: session {pid} opens in {}", folder.display());
        self.sessions.push(Session {
            terminal,
            title: String::new(),
            folder,
            bell: false,
            _events: events,
        });
    }

    /// A session of the shell the settings name, in home or the shown session's folder.
    fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(home) = dirs::home_dir() else {
            log::error!("app: no home folder, so no new session");
            return;
        };
        let settings = SettingsStore::get(cx);
        let folder = match settings.start_in {
            StartIn::Home => home,
            StartIn::Session => self
                .sessions
                .get(self.active)
                .map_or(home, |session| session.folder.clone()),
        };
        let started = launch(
            &settings.shell,
            folder.clone(),
            &cx.global::<ShellIntegration>().0,
        )
        .and_then(|launch| spawn(&launch, (80, 24), (8, 16), config(settings)));
        match started {
            Ok((pty, events)) => {
                self.add(pty, events, folder, window, cx);
                self.select(self.sessions.len() - 1, window, cx);
            }
            Err(error) => log::error!("app: no new session: {error:#}"),
        }
    }

    /// Shows session `index`, clears its bell, and gives its terminal the keys.
    pub fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.sessions.get_mut(index) else {
            return;
        };
        session.bell = false;
        window.focus(&session.terminal.focus_handle(cx), cx);
        self.active = index;
        log::debug!("app: session {} shown", index + 1);
        self.retitle(window);
        cx.notify();
    }

    /// Closes session `index`; its program hangs up. The last one closes the window.
    pub fn close(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.sessions.len() {
            return;
        }
        let session = self.sessions.remove(index);
        log::info!("app: session {} closes", session.terminal.read(cx).pid());
        if self.sessions.is_empty() {
            log::info!("app: no session left; the window closes");
            window.remove_window();
            return;
        }
        let shown = index == self.active;
        self.active = sessions::after_close(self.active, index, self.sessions.len());
        if shown {
            self.select(self.active, window, cx);
        } else {
            cx.notify();
        }
    }

    /// The shown session's title, or the app's name while its program names none.
    fn title(&self) -> &str {
        self.sessions
            .get(self.active)
            .map(|session| session.title.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or(TITLE)
    }

    fn retitle(&self, window: &mut Window) {
        let title = self.title();
        if window.window_title() != title {
            log::debug!("app: the title is {title:?}");
            window.set_window_title(title);
        }
    }

    fn session_event(
        &mut self,
        id: EntityId,
        event: &TerminalEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.terminal.entity_id() == id)
        else {
            return;
        };
        let session = &mut self.sessions[index];
        match event {
            TerminalEvent::Title(title) => session.title.clone_from(title),
            TerminalEvent::Folder(folder) => session.folder.clone_from(folder),
            TerminalEvent::Bell if index != self.active => {
                if !session.bell {
                    log::debug!("app: a bell in hidden session {}", index + 1);
                }
                session.bell = true;
            }
            TerminalEvent::Bell => {}
            TerminalEvent::Exited(status) => {
                let pid = session.terminal.read(cx).pid();
                log::info!("app: session {pid}'s program ended, {status}");
                self.close(index, window, cx);
                return;
            }
        }
        if index == self.active {
            self.retitle(window);
        }
        cx.notify();
    }

    /// Shows or hides a side column, as the settings keep it.
    fn toggle(&mut self, flip: fn(&mut Settings), cx: &mut Context<Self>) {
        match SettingsStore::update(cx, flip) {
            Ok(()) => {
                let settings = SettingsStore::get(cx);
                let shown = |on: bool| if on { "shown" } else { "hidden" };
                log::info!(
                    "app: sessions {}, right column {}",
                    shown(settings.show_sessions),
                    shown(settings.show_panel)
                );
            }
            Err(error) => log::error!("app: the columns stay: {error:#}"),
        }
    }

    /// Shows the next session, or the one before, round the ends.
    fn step(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.sessions.len();
        let by = if forward { 1 } else { count - 1 };
        self.select((self.active + by) % count, window, cx);
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = SettingsStore::get(cx);
        let width = cx.theme().sidebar_width(false);
        let mut shell = ColumnShell::new("workspace");
        if settings.show_sessions {
            let (workspace, home) = (cx.entity(), dirs::home_dir());
            let rows = self.sessions.iter().enumerate().map(|(index, session)| {
                let active = index == self.active;
                sessions::row(&workspace, index, session, active, home.as_deref(), cx)
            });
            let list = List::new().children(rows);
            let new = IconButton::new("new-session", IconName::Plus)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .tooltip("New session")
                .on_click(move |_, window, cx| {
                    workspace.update(cx, |this, cx| this.new_session(window, cx))
                });
            shell = shell.column(
                ShellColumn::new("sessions").width(width).action(new).child(
                    div()
                        .id("sessions")
                        .size_full()
                        .overflow_y_scroll()
                        .p_2()
                        .child(list),
                ),
            );
        }
        let shown = self
            .sessions
            .get(self.active)
            .map(|session| session.terminal.clone());
        shell = shell.column(
            ShellColumn::new("terminal")
                .title(self.title().to_string())
                .children(shown),
        );
        if settings.show_panel {
            shell = shell.column(ShellColumn::new("panel").width(width));
        }
        // The context sits above the focus root, so its keys work wherever focus is.
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
            .on_action(cx.listener(|this, _: &NewSession, window, cx| this.new_session(window, cx)))
            .on_action(
                cx.listener(|this, _: &CloseSession, window, cx| {
                    this.close(this.active, window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &NextSession, window, cx| this.step(true, window, cx)))
            .on_action(
                cx.listener(|this, _: &PreviousSession, window, cx| this.step(false, window, cx)),
            )
            .on_action(cx.listener(|this, _: &LastSession, window, cx| {
                this.select(this.sessions.len().saturating_sub(1), window, cx)
            }))
            .on_action(cx.listener(|this, select: &SelectSession, window, cx| {
                if select.0 < this.sessions.len() {
                    this.select(select.0, window, cx);
                } else {
                    cx.propagate();
                }
            }))
            .child(
                ResizeBorder::new()
                    .child(FocusScope::new(&self.focus).root().size_full().child(shell)),
            )
    }
}
