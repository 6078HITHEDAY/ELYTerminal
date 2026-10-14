use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    lists::ListItem,
    primitives::{Icon, IconName},
    theme::{ActiveTheme, ControlSize},
};
use gpui::{App, Entity, Subscription};

use crate::{
    term::TerminalView,
    workspace::{TITLE, Workspace},
};

/// A program on its own terminal, as the sessions column lists it.
pub struct Session {
    pub terminal: Entity<TerminalView>,
    /// The program's title; empty while it names none.
    pub title: String,
    /// The folder it reports, else the one it started in.
    pub folder: PathBuf,
    /// Its shell has integration, or it reported a folder anyway.
    pub reports: bool,
    /// A bell rang while it was hidden.
    pub bell: bool,
    pub _events: Subscription,
}

/// `folder` as a row shows it: the home folder as `~`, on one line.
pub fn shown(folder: &Path, home: Option<&Path>) -> String {
    let shown = match home.and_then(|home| folder.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~{MAIN_SEPARATOR}{}", rest.display()),
        None => folder.display().to_string(),
    };
    one_line(&shown)
}

/// `text` on one line, as gpui shapes labels: each control character as its picture, `␊` for a newline.
pub fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\0'..='\x1f' => char::from_u32(0x2400 + c as u32).expect("a control picture"),
            '\x7f' => '\u{2421}',
            c => c,
        })
        .collect()
}

/// The session shown once session `closed` of `left + 1` is gone, `active` having been shown.
pub fn after_close(active: usize, closed: usize, left: usize) -> usize {
    if closed < active || active == left {
        active - 1
    } else {
        active
    }
}

/// Session `index`'s row: its title and folder, a bell while one rang unseen, a button that closes it.
pub fn row(
    workspace: &Entity<Workspace>,
    index: usize,
    session: &Session,
    active: bool,
    home: Option<&Path>,
    cx: &App,
) -> ListItem {
    let pid = session.terminal.read(cx).pid() as usize;
    let title = if session.title.is_empty() {
        TITLE
    } else {
        &session.title
    };
    let (select, close) = (workspace.clone(), workspace.clone());
    let item = ListItem::new(("session", pid), title.to_string())
        .description(shown(&session.folder, home))
        .selected(active)
        .strong(session.bell)
        .on_click(move |_, window, cx| select.update(cx, |this, cx| this.select(index, window, cx)))
        .trailing(
            IconButton::new(("close-session", pid), IconName::X)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .tooltip("Close session")
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    close.update(cx, |this, cx| this.close(index, window, cx))
                }),
        );
    if session.bell {
        item.leading(Icon::new(IconName::Bell).color(cx.theme().colors.warning))
    } else {
        item
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_under_home_read_from_a_tilde() {
        let home = PathBuf::from("/Users/ely");
        let shown = |folder: &str| shown(Path::new(folder), Some(&home));
        assert_eq!(shown("/Users/ely"), "~");
        assert_eq!(
            shown("/Users/ely/src/app"),
            format!("~{MAIN_SEPARATOR}src/app")
        );
        assert_eq!(
            shown("/Users/elyse"),
            "/Users/elyse",
            "a sibling is no child"
        );
        assert_eq!(shown("/tmp"), "/tmp");
        assert_eq!(
            shown("/Users/ely/line\nbreak"),
            format!("~{MAIN_SEPARATOR}line␊break")
        );
    }

    #[test]
    fn control_characters_show_as_their_pictures() {
        assert_eq!(one_line("a\nb\tc\rd\0e\u{7f}"), "a␊b␉c␍d␀e␡");
        assert_eq!(one_line("café ✓"), "café ✓", "printable text stays");
    }

    #[test]
    fn closing_shows_the_next_session_or_keeps_the_shown_one() {
        assert_eq!(after_close(2, 0, 3), 1, "one before it went");
        assert_eq!(after_close(1, 1, 3), 1, "it went: the next one");
        assert_eq!(after_close(3, 3, 3), 2, "the last went: the one before");
        assert_eq!(after_close(1, 2, 3), 1, "one after it went");
    }
}
