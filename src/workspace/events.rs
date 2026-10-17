use gpui::{Context, EntityId, Window};

use super::Workspace;
use crate::term::{State, TerminalEvent};

impl Workspace {
    /// What session `id`'s terminal reported.
    pub(super) fn session_event(
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
            TerminalEvent::Folder(folder) => {
                session.folder.clone_from(folder);
                session.reports = true;
                if index == self.active {
                    self.files.update(cx, |files, cx| files.show(folder, cx));
                    self.git.update(cx, |git, cx| git.show(folder, cx));
                }
            }
            TerminalEvent::Bell if index != self.active => {
                if !session.bell {
                    log::debug!("app: a bell in hidden session {}", index + 1);
                }
                session.bell = true;
            }
            TerminalEvent::Bell => {}
            TerminalEvent::Finished(status) => {
                if session.status != *status {
                    let pid = session.terminal.read(cx).pid();
                    let ended =
                        status.map_or("with no status".into(), |code| format!("with {code}"));
                    log::info!("app: session {pid}'s last command ended {ended}");
                }
                session.status = *status;
            }
            TerminalEvent::Records => {
                let terminal = session.terminal.read(cx);
                let state = terminal.records().shown().map(|record| record.state);
                if state != session.state {
                    log::info!("app: session {} shows {state:?}", terminal.pid());
                    // A hidden session that starts waiting marks its row until it is shown.
                    if state == Some(State::Blocked) && index != self.active {
                        log::debug!("app: hidden session {} waits", index + 1);
                        session.waiting = true;
                    }
                    session.state = state;
                }
            }
            TerminalEvent::Notice(ask) => {
                let pid = session.terminal.read(cx).pid();
                log::info!("app: session {pid} asks {ask:?}");
            }
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
}
