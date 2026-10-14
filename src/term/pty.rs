use std::{borrow::Cow, collections::HashMap, path::PathBuf, sync::Arc};

use alacritty_terminal::{
    event::{Event, EventListener, Notify, OnResize, WindowSize},
    event_loop::{EventLoop, Msg, Notifier},
    sync::FairMutex,
    term::{Config, Term, test::TermSize},
    tty::{self, Options, Shell},
};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

use super::tap::Tapped;

/// What a session hears from its pty: the grid's events and the folder the shell reports.
#[derive(Debug)]
pub enum PtyEvent {
    Term(Event),
    Folder(PathBuf),
}

/// Hands the grid's events to the session.
#[derive(Clone)]
pub struct Listener(UnboundedSender<PtyEvent>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        if self.0.unbounded_send(PtyEvent::Term(event)).is_err() {
            log::debug!("pty: an event after its session closed");
        }
    }
}

/// A program, the folder it starts in, and variables over the app's own.
#[derive(Clone, Debug)]
pub struct Launch {
    /// None runs the login shell.
    pub program: Option<(String, Vec<String>)>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

/// A running program on a pseudo-terminal and the grid it draws into.
pub struct Pty {
    pub term: Arc<FairMutex<Term<Listener>>>,
    pub events: UnboundedReceiver<PtyEvent>,
    notifier: Notifier,
}

/// Starts `launch` on a pty of `size` cells, each `cell` pixels, keeping `scrollback` lines.
pub fn spawn(
    launch: &Launch,
    size: TermSize,
    cell: (u16, u16),
    scrollback: usize,
) -> anyhow::Result<Pty> {
    anyhow::ensure!(
        launch.cwd.is_dir(),
        "pty: cannot start in {}: no such folder",
        launch.cwd.display()
    );
    let (sender, events) = unbounded();
    let listener = Listener(sender.clone());
    let config = Config {
        scrolling_history: scrollback,
        kitty_keyboard: true,
        ..Config::default()
    };
    let term = Arc::new(FairMutex::new(Term::new(config, &size, listener.clone())));
    let mut env = HashMap::from([
        ("TERM".to_string(), "xterm-256color".to_string()),
        ("COLORTERM".to_string(), "truecolor".to_string()),
    ]);
    if std::env::var_os("LANG").is_none() {
        env.insert("LANG".into(), "en_US.UTF-8".into());
    }
    env.extend(launch.env.iter().cloned());
    let options = Options {
        shell: launch
            .program
            .clone()
            .map(|(program, args)| Shell::new(program, args)),
        working_directory: Some(launch.cwd.clone()),
        drain_on_exit: true,
        env,
        #[cfg(windows)]
        escape_args: true,
    };
    let pty = tty::new(&options, window_size(&size, cell), 0)?;
    log::info!(
        "pty: started {:?} in {}, child {}",
        launch.program,
        launch.cwd.display(),
        child_id(&pty)
    );
    let event_loop = EventLoop::new(
        term.clone(),
        listener,
        Tapped::new(pty, sender),
        true,
        false,
    )?;
    let notifier = Notifier(event_loop.channel());
    event_loop.spawn();
    Ok(Pty {
        term,
        events,
        notifier,
    })
}

impl Pty {
    /// Sends bytes to the program, as typing does.
    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        self.notifier.notify(bytes);
    }

    /// Fits the grid and tells the program its new size.
    pub fn resize(&mut self, size: TermSize, cell: (u16, u16)) {
        self.notifier.on_resize(window_size(&size, cell));
        self.term.lock().resize(size);
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        if self.notifier.0.send(Msg::Shutdown).is_err() {
            log::debug!("pty: the event loop had already stopped");
        }
    }
}

#[cfg(unix)]
fn child_id(pty: &tty::Pty) -> String {
    pty.child().id().to_string()
}

#[cfg(windows)]
fn child_id(pty: &tty::Pty) -> String {
    pty.child_watcher()
        .pid()
        .map_or_else(|| "without an id".into(), |pid| pid.to_string())
}

/// A pty's size; counts past what the kernel holds are cut and logged.
pub fn window_size(size: &TermSize, cell: (u16, u16)) -> WindowSize {
    let cells = |count: usize, side: u16| {
        let most = (i16::MAX as u16).min(u16::MAX / side.max(1));
        let kept = u16::try_from(count).unwrap_or(u16::MAX).min(most);
        if usize::from(kept) < count {
            log::error!("pty: {count} cells of {side} px pass what a pty holds; {kept} kept");
        }
        kept
    };
    WindowSize {
        num_lines: cells(size.screen_lines, cell.1),
        num_cols: cells(size.columns, cell.0),
        cell_width: cell.0,
        cell_height: cell.1,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use alacritty_terminal::index::{Column, Line};
    use futures::StreamExt;

    use super::*;

    fn run(script: &str) -> (Vec<PathBuf>, String, bool) {
        let launch = Launch {
            program: Some(("/bin/sh".into(), vec!["-c".into(), script.into()])),
            cwd: std::env::temp_dir(),
            env: vec![("ELY".into(), "ok".into())],
        };
        let mut pty = spawn(&launch, TermSize::new(40, 4), (8, 16), 100).expect("sh starts");
        let (folders, success) = futures::executor::block_on(async {
            let (mut folders, mut success) = (Vec::new(), false);
            while let Some(event) = pty.events.next().await {
                match event {
                    PtyEvent::Folder(folder) => folders.push(folder),
                    PtyEvent::Term(Event::ChildExit(status)) => success = status.success(),
                    PtyEvent::Term(Event::Exit) => break,
                    PtyEvent::Term(_) => {}
                }
            }
            (folders, success)
        });
        let term = pty.term.lock();
        let row = &term.grid()[Line(0)];
        let text: String = (0..20).map(|column| row[Column(column)].c).collect();
        (folders, text.trim_end().to_string(), success)
    }

    #[test]
    fn a_program_prints_into_the_grid_with_its_variables() {
        let (folders, text, success) = run("printf 'ely %s' \"$ELY\"");
        assert!(folders.is_empty());
        assert_eq!(text, "ely ok");
        assert!(success);
    }

    #[test]
    fn osc_7_reaches_the_session_as_a_folder() {
        let (folders, text, _) = run(
            "printf '\\033]7;file:///tmp/a%%20b\\007x\\033]7;file://elsewhere.invalid/c\\033\\\\'",
        );
        assert_eq!(folders, [PathBuf::from("/tmp/a b")]);
        assert_eq!(text, "x");
    }

    #[test]
    fn a_missing_folder_stops_the_start() {
        let launch = Launch {
            program: None,
            cwd: "/nonexistent/ely".into(),
            env: Vec::new(),
        };
        let error = spawn(&launch, TermSize::new(20, 4), (8, 16), 100)
            .err()
            .expect("a missing folder is an error");
        assert!(error.to_string().contains("/nonexistent/ely"));
    }

    #[test]
    fn more_cells_than_a_pty_holds_cap_at_its_most() {
        let size = window_size(&TermSize::new(70_000, 24), (8, 16));
        assert_eq!((size.num_cols, size.num_lines), (u16::MAX / 8, 24));
    }
}
