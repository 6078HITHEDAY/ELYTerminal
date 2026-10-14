use std::time::{Duration, Instant};

use alacritty_terminal::event::Event;
use futures::channel::mpsc::TryRecvError;

use super::*;
use crate::term::{PtyEvent, spawn};

/// A fresh folder under the temp dir, by its real path.
fn folder(name: &str) -> PathBuf {
    let folder = env::temp_dir().join(format!("elyterminal-{name}-{}", std::process::id()));
    if folder.exists() {
        fs::remove_dir_all(&folder).expect("a stale test folder goes");
    }
    fs::create_dir_all(&folder).expect("a test folder");
    folder.canonicalize().expect("a real path")
}

fn installed(shell: &str) -> String {
    let found = std::process::Command::new("/bin/sh")
        .args(["-c", &format!("command -v {shell}")])
        .output()
        .expect("sh runs");
    let path = String::from_utf8(found.stdout).expect("utf-8");
    assert!(found.status.success(), "{shell} is not installed");
    path.trim().to_string()
}

/// Starts `shell` with integration in a fresh folder, cds once, and returns folders and titles it reported.
fn reports(shell: &str) -> (PathBuf, Vec<PathBuf>, Vec<String>) {
    crate::testlog::init();
    let root = folder(&format!(
        "shell-{}",
        shell.rsplit('/').next().expect("a name")
    ));
    let (dir, home, start) = (
        root.join("integration"),
        root.join("home"),
        root.join("start"),
    );
    let inner = start.join("in ner%");
    for made in [&home, &inner] {
        fs::create_dir_all(made).expect("a folder");
    }
    install(&dir).expect("scripts install");
    let mut launch = launch(shell, start.clone(), &dir).expect("a launch");
    launch.env.push(("HOME".into(), home.display().to_string()));
    let mut pty = spawn(&launch, (80, 24), (8, 16), 100).expect("the shell starts");
    let (mut folders, mut titles) = (Vec::new(), Vec::new());
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut asked = false;
    while !folders.contains(&inner) {
        assert!(
            Instant::now() < deadline,
            "{shell} reported {folders:?} and {titles:?} on\n{}",
            screen(&pty)
        );
        match pty.events.try_recv() {
            Ok(PtyEvent::Folder(folder)) => folders.push(folder),
            Ok(PtyEvent::Term(Event::Title(title))) => titles.push(title),
            Ok(PtyEvent::Term(Event::PtyWrite(reply))) => pty.write(reply.into_bytes()),
            Ok(PtyEvent::Term(Event::ChildExit(status))) => {
                panic!("{shell} ended, {status}, after {folders:?}")
            }
            Ok(_) => {}
            Err(TryRecvError::Closed) => panic!("{shell}'s events ended"),
            Err(TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(10)),
        }
        if !asked && folders.contains(&start) {
            pty.write(format!("cd '{}'\r", inner.display()).into_bytes());
            asked = true;
        }
    }
    pty.write(&b"exit\r"[..]);
    loop {
        assert!(
            Instant::now() < deadline,
            "{shell} did not exit:\n{}",
            screen(&pty)
        );
        match pty.events.try_recv() {
            Ok(PtyEvent::Term(Event::ChildExit(_))) | Err(TryRecvError::Closed) => break,
            Ok(PtyEvent::Term(Event::Title(title))) => titles.push(title),
            Ok(PtyEvent::Term(Event::PtyWrite(reply))) => pty.write(reply.into_bytes()),
            Ok(_) => {}
            Err(TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    drop(pty);
    fs::remove_dir_all(&root).expect("cleanup");
    (start, folders, titles)
}

/// The screen's rows, for a failure's message.
fn screen(pty: &crate::term::Pty) -> String {
    use alacritty_terminal::{grid::Dimensions, index::Line};
    let term = pty.term.lock();
    (0..term.screen_lines())
        .map(|line| {
            let row = &term.grid()[Line(line as i32)];
            let text: String = row.into_iter().map(|cell| cell.c).collect();
            text.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_scripts_install_once_and_mend_a_changed_one() {
    let dir = folder("install");
    install(&dir).expect("installs");
    let zshenv = dir.join("zsh/.zshenv");
    assert_eq!(fs::read_to_string(&zshenv).expect("written"), SCRIPTS[0].1);
    fs::write(&zshenv, "changed").expect("write");
    install(&dir).expect("installs again");
    assert_eq!(fs::read_to_string(&zshenv).expect("mended"), SCRIPTS[0].1);
    fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn zsh_reports_its_folder_and_title() {
    let (start, folders, titles) = reports(&installed("zsh"));
    assert_eq!(folders[0], start);
    assert!(titles.contains(&"start".to_string()), "{titles:?}");
    assert!(
        titles.iter().any(|title| title.starts_with("cd ")),
        "{titles:?}"
    );
}

#[test]
fn bash_reports_its_folder_and_title() {
    let (start, folders, titles) = reports(&installed("bash"));
    assert_eq!(folders[0], start);
    assert!(titles.contains(&"in ner%".to_string()), "{titles:?}");
}

#[test]
fn fish_reports_its_folder_and_keeps_its_own_title() {
    let (start, folders, titles) = reports(&installed("fish"));
    assert_eq!(folders[0], start);
    assert!(
        titles.iter().any(|title| title.ends_with("start")),
        "{titles:?}"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn powershell_reports_its_folder_and_title() {
    let (start, folders, titles) = reports(&installed("pwsh"));
    assert_eq!(folders[0], start);
    assert!(titles.contains(&"in ner%".to_string()), "{titles:?}");
}

#[test]
fn an_unknown_shell_starts_without_integration() {
    let dir = PathBuf::from("/nonexistent");
    let launch = launch("/bin/sh", PathBuf::from("/"), &dir).expect("a launch");
    assert_eq!(launch.program, Some(("/bin/sh".into(), Vec::new())));
    assert!(launch.env.is_empty());
}
