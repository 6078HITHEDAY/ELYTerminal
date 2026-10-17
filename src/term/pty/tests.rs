use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Line},
};
use futures::StreamExt;

use super::*;
use crate::term::status::{Record, State};

struct Ran {
    folders: Vec<PathBuf>,
    marks: Vec<Mark>,
    /// What programs said of their status, a full reset as none.
    statuses: Vec<Option<Status>>,
    text: String,
    /// The first line's cells that carry a prompt's link.
    prompt: Vec<usize>,
    success: bool,
    pid: u32,
    columns: usize,
}

fn run(script: &str, size: (usize, usize)) -> Ran {
    crate::testlog::init();
    let launch = Launch {
        program: Some(("/bin/sh".into(), vec!["-c".into(), script.into()])),
        cwd: std::env::temp_dir(),
        env: vec![("ELY".into(), "ok".into())],
        reports: false,
    };
    let (pty, mut events) =
        spawn(&launch, size, (8, 16), config(&Settings::default())).expect("sh starts");
    let (folders, marks, statuses, success) = futures::executor::block_on(async {
        let (mut folders, mut marks, mut statuses, mut success) =
            (Vec::new(), Vec::new(), Vec::new(), false);
        while let Some(event) = events.next().await {
            match event {
                PtyEvent::Folder(folder) => folders.push(folder),
                PtyEvent::Mark(mark) => marks.push(mark),
                PtyEvent::Status(status) => statuses.push(Some(status)),
                PtyEvent::Reset => statuses.push(None),
                PtyEvent::Term(Event::ChildExit(status)) => success = status.success(),
                PtyEvent::Term(Event::Exit) => break,
                PtyEvent::Term(_) | PtyEvent::Mouse(_) => {}
            }
        }
        (folders, marks, statuses, success)
    });
    let term = pty.term.lock();
    let row = &term.grid()[Line(0)];
    let text: String = (0..24).map(|column| row[Column(column)].c).collect();
    let prompt = (0..24).filter(|&column| {
        row[Column(column)]
            .hyperlink()
            .is_some_and(|link| link.uri() == crate::term::prompts::LINK)
    });
    Ran {
        folders,
        marks,
        statuses,
        text: text.trim_end().to_string(),
        prompt: prompt.collect(),
        success,
        pid: pty.pid,
        columns: term.columns(),
    }
}

#[test]
fn a_program_prints_into_the_grid_with_its_variables_and_id() {
    let ran = run("printf 'ely %s %s' \"$ELY\" $$", (40, 4));
    assert!(ran.folders.is_empty());
    assert_eq!(ran.text, format!("ely ok {}", ran.pid));
    assert!(ran.success);
}

#[test]
fn osc_7_reaches_the_session_as_a_folder() {
    let ran = run(
        "printf '\\033]7;file:///tmp/a%%20b\\007x\\033]7;file://elsewhere.invalid/c\\033\\\\'",
        (40, 4),
    );
    assert_eq!(ran.folders, [PathBuf::from("/tmp/a b")]);
    assert_eq!(ran.text, "x");
}

#[test]
fn osc_133_reaches_the_session_as_marks() {
    let ran = run(
        "printf '\\033]133;A\\007>\\033]133;B\\007\\033]133;C\\033\\\\x\\033]133;D;3\\007'",
        (40, 4),
    );
    let marks = [Mark::Prompt, Mark::Input, Mark::Output, Mark::Done(Some(3))];
    assert_eq!(ran.marks, marks);
    assert_eq!(ran.text, ">x");
    assert_eq!(ran.prompt, [0], "the prompt's cell, not the output's");
}

#[test]
fn osc_7501_and_a_full_reset_reach_the_session_in_order() {
    let ran = run(
        "printf '\\033]7501;state=working:progress=40\\033\\\\\\033]7501;?\\007\\033cx'",
        (40, 4),
    );
    let working = Record {
        state: State::Working,
        kind: None,
        progress: Some(40),
        app: None,
        title: None,
        msg: None,
    };
    let said = [
        Some(Status::Report(String::new(), working)),
        Some(Status::Query),
        None,
    ];
    assert_eq!(ran.statuses, said);
    assert_eq!(ran.text, "x");
}

#[test]
fn a_long_escape_of_controls_keeps_the_pty_reading() {
    let ran = run(
        "printf '\\033'; head -c 3000000 /dev/zero; printf '[0mREADY'",
        (40, 4),
    );
    assert_eq!(ran.text, "READY");
}

#[test]
fn the_foreground_is_the_program_s_own_group() {
    let launch = Launch {
        program: Some(("/bin/sh".into(), vec!["-c".into(), "exec sleep 30".into()])),
        cwd: std::env::temp_dir(),
        env: Vec::new(),
        reports: false,
    };
    let (pty, _events) =
        spawn(&launch, (20, 4), (8, 16), config(&Settings::default())).expect("sh starts");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while pty.foreground().ok() != Some(pty.pid as i32) {
        assert!(
            std::time::Instant::now() < deadline,
            "{:?}",
            pty.foreground()
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn a_size_past_the_kernel_is_one_size_for_grid_and_program() {
    let ran = run("stty size", (9000, 8));
    assert_eq!(ran.columns, usize::from(u16::MAX / 8));
    assert_eq!(ran.text, format!("8 {}", u16::MAX / 8));
}

/// Starts `script`, waits for it to print READY, closes it, and waits for the reap's log.
fn close_after_ready(script: &str, reaped: &str) {
    crate::testlog::init();
    let launch = Launch {
        program: Some(("/bin/sh".into(), vec!["-c".into(), script.into()])),
        cwd: std::env::temp_dir(),
        env: Vec::new(),
        reports: false,
    };
    let (pty, mut events) =
        spawn(&launch, (20, 4), (8, 16), config(&Settings::default())).expect("sh starts");
    futures::executor::block_on(async {
        while let Some(event) = events.next().await {
            let term = pty.term.lock();
            let text: String = (0..5)
                .map(|column| term.grid()[Line(0)][Column(column)].c)
                .collect();
            drop(term);
            if matches!(event, PtyEvent::Term(Event::Wakeup)) && text == "READY" {
                break;
            }
        }
    });
    let pid = pty.pid;
    drop(pty);
    let hung_up = format!("pty {pid}: hung up, {reaped}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !crate::testlog::logged(&hung_up) {
        assert!(std::time::Instant::now() < deadline, "no `{hung_up}`");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(crate::testlog::logged(&format!("pty {pid}: closing")));
    let alive = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
    assert!(!alive, "the program was reaped");
}

#[test]
fn closing_a_session_hangs_up_its_program() {
    close_after_ready("printf READY; exec cat", "signal: 1 (SIGHUP)");
}

#[test]
fn a_program_that_ignores_sighup_ends_when_the_pty_closes() {
    // cat reads EOF on macOS and EIO on Linux; the shell's own exit holds on both.
    close_after_ready("trap '' HUP; printf READY; cat; exit 7", "exit status: 7");
}

#[test]
fn a_missing_folder_stops_the_start() {
    let launch = Launch {
        program: None,
        cwd: "/nonexistent/ely".into(),
        env: Vec::new(),
        reports: false,
    };
    let error = spawn(&launch, (20, 4), (8, 16), config(&Settings::default()))
        .err()
        .expect("a missing folder is an error");
    assert!(error.to_string().contains("/nonexistent/ely"));
}

#[test]
fn a_folder_that_cannot_be_entered_stops_the_start() {
    use std::os::unix::fs::PermissionsExt;
    let folder = std::env::temp_dir().join(format!("elyterminal-shut-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("a temp folder");
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o600)).expect("600");
    let launch = Launch {
        program: Some(("/bin/sh".into(), vec!["-c".into(), "true".into()])),
        cwd: folder.clone(),
        env: Vec::new(),
        reports: false,
    };
    let started = spawn(&launch, (20, 4), (8, 16), config(&Settings::default()));
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).expect("700");
    std::fs::remove_dir(&folder).expect("the temp folder goes");
    let error = started.err().expect("a folder it cannot enter is an error");
    let error = error.to_string();
    assert!(error.contains("Permission denied"), "{error}");
    assert!(error.contains(&folder.display().to_string()), "{error}");
}

#[test]
fn sizes_hold_at_least_two_by_one_and_cap_at_the_kernel() {
    let size = fit(70_000, 24, (8, 16));
    assert_eq!((size.num_cols, size.num_lines), (u16::MAX / 8, 24));
    let size = fit(0, 0, (8, 16));
    assert_eq!((size.num_cols, size.num_lines), (2, 1));
}
