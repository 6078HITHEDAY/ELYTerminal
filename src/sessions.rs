use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    lists::ListItem,
    motion::ProgressRing,
    primitives::{Icon, IconName, Tooltip},
    theme::{ActiveTheme, ControlSize, IconSize},
};
use gpui::{AnyElement, App, Entity, Hsla, SharedString, Subscription, div, prelude::*, rems};

use crate::{
    term::{Kind, Record, Records, State, TerminalView},
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
    /// It started waiting on the user while hidden.
    pub waiting: bool,
    pub _events: Subscription,
}

/// The most records hover lists, and the most characters it shows of each part.
const LISTED: usize = 10;
const SHORT: usize = 120;

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

/// Session `index`'s row: its title and folder, the mark of its status, a bell while one rang unseen, a button that closes it.
pub fn row(
    workspace: &Entity<Workspace>,
    index: usize,
    session: &Session,
    active: bool,
    home: Option<&Path>,
    cx: &App,
) -> ListItem {
    let terminal = session.terminal.read(cx);
    let pid = terminal.pid() as usize;
    let title = if session.title.is_empty() {
        TITLE
    } else {
        &session.title
    };
    let (select, close) = (workspace.clone(), workspace.clone());
    let item = ListItem::new(("session", pid), title.to_string())
        .description(shown(&session.folder, home))
        .selected(active)
        .strong(session.bell || session.waiting)
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
    let mark = mark(&session.terminal, cx);
    let bell = session
        .bell
        .then(|| Icon::new(IconName::Bell).color(cx.theme().colors.warning));
    if mark.is_none() && bell.is_none() {
        return item;
    }
    item.leading(
        div()
            .flex()
            .items_center()
            .gap_1()
            .children(mark)
            .children(bell),
    )
}

/// The mark of what the session's records say, else of its last command's failure; hover lists them.
fn mark(terminal: &Entity<TerminalView>, cx: &App) -> Option<AnyElement> {
    let theme = cx.theme();
    let colors = &theme.colors;
    let view = terminal.read(cx);
    let (pid, records) = (view.pid() as usize, view.records());
    let icon = |name: IconName, color: Hsla| Icon::new(name).color(color).into_any_element();
    let failed = view.ended().filter(|&code| code != 0);
    let mark = match (records.shown().map(|(_, record)| record), failed) {
        (
            Some(Record {
                state: State::Blocked,
                kind,
                ..
            }),
            _,
        ) => {
            let name = match kind {
                Some(Kind::Permission) => IconName::Shield,
                Some(Kind::Question) => IconName::CircleHelp,
                Some(Kind::Auth) => IconName::Key,
                None => IconName::Hand,
            };
            icon(name, colors.warning)
        }
        (
            Some(Record {
                state: State::Error,
                ..
            }),
            _,
        ) => icon(IconName::CircleX, colors.danger),
        (
            Some(Record {
                state: State::Working,
                progress: Some(value),
                ..
            }),
            _,
        ) => ProgressRing::new(("session-progress", pid), f32::from(*value) / 100.0)
            .size(theme.icon_size(IconSize::Xl))
            // The track takes the border's color, which a selected row's ground may match.
            .inside(
                Icon::new(IconName::LoaderCircle)
                    .size(IconSize::Xs)
                    .color(colors.info),
            )
            .into_any_element(),
        // A turning spinner would draw the window at every frame while the program works.
        (
            Some(Record {
                state: State::Working,
                ..
            }),
            _,
        ) => icon(IconName::LoaderCircle, colors.info),
        (
            Some(Record {
                state: State::Done, ..
            }),
            _,
        ) => icon(IconName::CircleCheck, colors.success),
        (_, Some(_)) => icon(IconName::CircleAlert, colors.danger),
        (
            Some(Record {
                state: State::Idle, ..
            }),
            None,
        ) => icon(IconName::Circle, colors.fg_muted),
        (None, None) => return None,
    };
    // Hover reads the records as they stand, so an open one follows them, and scrolls within the window.
    let watched = terminal.downgrade();
    let tip = Tooltip::rich(move |window, cx| {
        let lines = watched.upgrade().map_or_else(Vec::new, |terminal| {
            let view = terminal.read(cx);
            hover(view.records(), view.ended().filter(|&code| code != 0))
        });
        let room = window.viewport_size().height - rems(3.).to_pixels(window.rem_size());
        div()
            .id("session-hover")
            .max_h(room)
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(lines)
            .into_any_element()
    });
    // Every mark takes the ring's box, so the title stays put as the state changes.
    Some(
        div()
            .id(("session-status", pid))
            .size(theme.icon_size(IconSize::Xl))
            .flex()
            .items_center()
            .justify_center()
            .hoverable_tooltip(tip)
            .child(mark)
            .into_any_element(),
    )
}

/// What hover shows: a line for each record, ten at most, then a failed command's status.
pub fn hover(records: &Records, failed: Option<i32>) -> Vec<SharedString> {
    let mut lines: Vec<SharedString> = records
        .iter()
        .take(LISTED)
        .map(|(id, record)| line(records, id, record).into())
        .collect();
    let more = records.iter().count().saturating_sub(LISTED);
    if more > 0 {
        lines.push(format!("{more} more").into());
    }
    if let Some(code) = failed {
        lines.push(format!("Last command exited with {code}").into());
    }
    lines
}

/// A record as hover and notifications say it: its state, then its app, title and message.
pub fn line(records: &Records, id: &str, record: &Record) -> String {
    let parts = [
        records.app(id),
        record.title.as_deref(),
        record.msg.as_deref(),
    ];
    let said: Vec<String> = parts.into_iter().flatten().map(plain).collect();
    let state = label(record);
    if said.is_empty() {
        state
    } else {
        format!("{state}: {}", said.join(" · "))
    }
}

/// A record's state, as hover names it.
fn label(record: &Record) -> String {
    match (record.state, record.kind, record.progress) {
        (State::Idle, ..) => "Idle".into(),
        (State::Working, _, Some(value)) => format!("Working, {value}%"),
        (State::Working, ..) => "Working".into(),
        (State::Blocked, Some(Kind::Permission), _) => "Waiting for permission".into(),
        (State::Blocked, Some(Kind::Question), _) => "Waiting for an answer".into(),
        (State::Blocked, Some(Kind::Auth), _) => "Waiting for a login".into(),
        (State::Blocked, None, _) => "Waiting".into(),
        (State::Done, ..) => "Done".into(),
        (State::Error, ..) => "Error".into(),
    }
}

/// A program's text as hover shows it: on one line, 120 characters at most, without what could disguise it.
pub fn plain(text: &str) -> String {
    let kept: String = text.chars().filter(|&c| !invisible(c)).collect();
    let line = one_line(&kept);
    match line.char_indices().nth(SHORT) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line,
    }
}

/// C1 controls, direction controls and invisible formatting; joiners stay, as scripts and emoji need them.
fn invisible(c: char) -> bool {
    matches!(
        c,
        '\u{80}'..='\u{9f}'
            | '\u{ad}'
            | '\u{61c}'
            | '\u{180e}'
            | '\u{200b}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{e0000}'..='\u{e007f}'
    )
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

    #[test]
    fn hover_shows_a_program_s_text_as_text() {
        assert_eq!(plain("evil\u{202e}txt.exe"), "eviltxt.exe");
        assert_eq!(
            plain("a\u{200b}b\u{feff}c\u{e0041}d\u{85}e\u{2066}f"),
            "abcdef"
        );
        assert_eq!(
            plain("👨\u{200d}💻 two\nlines"),
            "👨\u{200d}💻 two␊lines",
            "joiners stay"
        );
        assert_eq!(plain(&"x".repeat(SHORT)), "x".repeat(SHORT));
        assert_eq!(
            plain(&"é".repeat(SHORT + 1)),
            format!("{}…", "é".repeat(SHORT))
        );
    }

    #[test]
    fn hover_lists_each_record_with_its_app_then_a_failed_command() {
        let record =
            |state: State, kind: Option<Kind>, progress: Option<u8>, texts: [Option<&str>; 3]| {
                let [app, title, msg] = texts.map(|text| text.map(String::from));
                Record {
                    state,
                    kind,
                    progress,
                    app,
                    title,
                    msg,
                }
            };
        let mut records = Records::default();
        records.report(
            String::new(),
            record(
                State::Working,
                None,
                Some(40),
                [Some("deploy"), None, Some("Deploying")],
            ),
        );
        records.report(
            "eu".into(),
            record(
                State::Blocked,
                Some(Kind::Permission),
                None,
                [None, Some("EU West"), Some("Approve?")],
            ),
        );
        records.report(
            "eu/db".into(),
            record(State::Done, None, None, [Some("db"), None, None]),
        );
        let lines: Vec<String> = hover(&records, Some(2))
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            lines,
            [
                "Working, 40%: deploy · Deploying",
                "Waiting for permission: deploy · EU West · Approve?",
                "Done: db",
                "Last command exited with 2"
            ]
        );
        for at in 0..LISTED {
            records.report(format!("r{at}"), record(State::Idle, None, None, [None; 3]));
        }
        let lines = hover(&records, None);
        assert_eq!(lines.len(), LISTED + 1);
        assert_eq!(lines[LISTED].to_string(), "3 more");
    }
}
