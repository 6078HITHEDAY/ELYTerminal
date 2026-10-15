use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex},
};

use ely_gpui_component::{
    forms::TextInput,
    git::{ChangeAction, ChangeSection, Commit, DiffLayout, Stretch},
};
use gpui::{AppContext, Context, Entity, EventEmitter, SharedString, Task, Window};

use self::view::{is_folder, shown};
use super::{Change, Repo};

mod view;

/// The newest commits the panel lists.
const RECENT: usize = 50;

/// What the shown folder reads as.
#[derive(PartialEq)]
enum State {
    Reading,
    Outside,
    Failed(SharedString),
    Read(Repo, Rc<Vec<Commit>>),
}

/// An open diff: its file, the section it came from, and its lines or why there are none.
struct Diff {
    change: Change,
    section: ChangeSection,
    lines: Result<Rc<Vec<Stretch>>, &'static str>,
}

/// The panel changed the repository, so other views read it again.
pub struct Acted;

/// The changes `section` lists.
fn listed(repo: &Repo, section: ChangeSection) -> &[Change] {
    match section {
        ChangeSection::Conflicted => &repo.conflicted,
        ChangeSection::Staged => &repo.staged,
        ChangeSection::Unstaged => &repo.unstaged,
        ChangeSection::Untracked => &repo.untracked,
        ChangeSection::Ignored => &[],
    }
}

/// The Git panel: the shown folder's repository, its changes, a commit, recent commits and diffs.
pub struct GitPanel {
    folder: PathBuf,
    state: State,
    /// The last action's failure: what failed, and what git said.
    error: Option<(&'static str, SharedString)>,
    subject: Entity<TextInput>,
    body: Entity<TextInput>,
    amend: bool,
    diff: Option<Diff>,
    layout: DiffLayout,
    discarding: Option<(Change, ChangeSection)>,
    /// One action at a time, in the order asked.
    turn: Arc<Mutex<()>>,
    reading: Task<()>,
    opening: Task<()>,
}

impl EventEmitter<Acted> for GitPanel {}

impl GitPanel {
    pub fn new(folder: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subject = cx.new(|cx| TextInput::new(window, cx).placeholder("Summary"));
        let body = cx.new(|cx| {
            TextInput::new(window, cx)
                .multi_line(3, 6)
                .placeholder("Description")
        });
        Self {
            folder,
            state: State::Reading,
            error: None,
            subject,
            body,
            amend: false,
            diff: None,
            layout: DiffLayout::Unified,
            discarding: None,
            turn: Arc::default(),
            reading: Task::ready(()),
            opening: Task::ready(()),
        }
    }

    /// Shows `folder`'s repository, read afresh; a new folder drops what the old one had open.
    pub fn show(&mut self, folder: &Path, cx: &mut Context<Self>) {
        if folder != self.folder {
            log::info!("git panel: shows {}", folder.display());
            self.folder = folder.to_path_buf();
            (self.error, self.diff, self.discarding) = (None, None, None);
        }
        self.read(cx);
    }

    /// Reads the repository and its recent commits, off the main thread.
    fn read(&mut self, cx: &mut Context<Self>) {
        let folder = self.folder.clone();
        let read = cx.background_spawn(async move {
            let Some(repo) = super::repo(&folder)? else {
                return Ok(None);
            };
            // Git refuses a log before the first commit.
            let commits = match repo.commit {
                Some(_) => super::log(&repo.top, RECENT)?,
                None => Vec::new(),
            };
            anyhow::Ok(Some((repo, commits)))
        });
        self.reading = cx.spawn(async move |panel, cx| {
            let read = read.await;
            panel.update(cx, |panel, cx| panel.store(read, cx)).ok();
        });
    }

    fn store(&mut self, read: anyhow::Result<Option<(Repo, Vec<Commit>)>>, cx: &mut Context<Self>) {
        let state = match read {
            Ok(Some((repo, commits))) => State::Read(repo, Rc::new(commits)),
            Ok(None) => State::Outside,
            Err(error) => {
                let folder = self.folder.display();
                log::error!("git panel: {folder} did not read: {error:#}");
                State::Failed(format!("{error:#}").into())
            }
        };
        if state == self.state {
            return;
        }
        match &state {
            State::Read(repo, commits) => log::debug!(
                "git panel: {} on {:?}, {} conflicted, {} staged, {} unstaged, {} untracked, {} commits",
                repo.top.display(),
                repo.branch,
                repo.conflicted.len(),
                repo.staged.len(),
                repo.unstaged.len(),
                repo.untracked.len(),
                commits.len()
            ),
            State::Outside => log::debug!("git panel: {} is no repository", self.folder.display()),
            State::Reading | State::Failed(_) => {}
        }
        self.state = state;
        cx.notify();
    }

    /// Runs `work` on the repository at `top` off the main thread, one action at a time; on success `then`; after it, a fresh read and word to other views.
    fn act(
        &mut self,
        what: &'static str,
        top: PathBuf,
        work: impl FnOnce(&Path) -> anyhow::Result<()> + Send + 'static,
        then: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        let turn = self.turn.clone();
        let done = cx.background_spawn(async move {
            let _turn = turn.lock().expect("git panel: an action panicked");
            work(&top)
        });
        cx.spawn(async move |panel, cx| {
            let done = done.await;
            let acted = |panel: &mut Self, cx: &mut Context<Self>| {
                match done {
                    Ok(()) => then(panel, cx),
                    Err(error) => {
                        log::error!("git panel: {what}: {error:#}");
                        panel.error = Some((what, format!("{error:#}").into()));
                    }
                }
                panel.read(cx);
                cx.emit(Acted);
                cx.notify();
            };
            panel.update(cx, acted).ok();
        })
        .detach();
        cx.notify();
    }

    /// What a row asked: `row` names its file in `section`.
    fn row_action(
        &mut self,
        row: &SharedString,
        section: ChangeSection,
        action: ChangeAction,
        cx: &mut Context<Self>,
    ) {
        let State::Read(repo, _) = &self.state else {
            return;
        };
        let found = listed(repo, section)
            .iter()
            .find(|change| shown(&change.path) == *row);
        let (Some(change), top) = (found.cloned(), repo.top.clone()) else {
            log::warn!("git panel: {row} is no longer in {section:?}");
            return;
        };
        log::debug!("git panel: {action:?} {row} in {section:?}");
        match action {
            ChangeAction::Stage => {
                let work = move |top: &Path| super::stage(top, &[change]);
                self.act("Stage failed", top, work, |_, _| {}, cx)
            }
            ChangeAction::Unstage => {
                let work = move |top: &Path| super::unstage(top, &[change]);
                self.act("Unstage failed", top, work, |_, _| {}, cx)
            }
            ChangeAction::Discard => {
                self.discarding = Some((change, section));
                cx.notify();
            }
            ChangeAction::Open => self.open(top, change, section, cx),
        }
    }

    /// Stages every unstaged change, or unstages every staged one.
    fn all(&mut self, stage: bool, cx: &mut Context<Self>) {
        let State::Read(repo, _) = &self.state else {
            return;
        };
        let top = repo.top.clone();
        if stage {
            let changes = repo.unstaged.clone();
            let work = move |top: &Path| super::stage(top, &changes);
            self.act("Stage failed", top, work, |_, _| {}, cx);
        } else {
            let changes = repo.staged.clone();
            let work = move |top: &Path| super::unstage(top, &changes);
            self.act("Unstage failed", top, work, |_, _| {}, cx);
        }
    }

    fn discard(&mut self, change: Change, section: ChangeSection, cx: &mut Context<Self>) {
        let State::Read(repo, _) = &self.state else {
            return;
        };
        let top = repo.top.clone();
        log::info!("git panel: discarding {} confirmed", change.path.display());
        let work = move |top: &Path| super::discard(top, &change, section);
        self.act("Discard failed", top, work, |_, _| {}, cx);
    }

    /// Commits what is staged with the typed message; on success the fields empty.
    fn commit(&mut self, cx: &mut Context<Self>) {
        let State::Read(repo, _) = &self.state else {
            return;
        };
        let top = repo.top.clone();
        let subject = self.subject.read(cx).text().trim().to_string();
        let body = self.body.read(cx).text().trim().to_string();
        let message = match body.is_empty() {
            true => subject,
            false => format!("{subject}\n\n{body}"),
        };
        let amend = self.amend;
        let work = move |top: &Path| super::commit(top, &message, amend);
        let then = |panel: &mut Self, cx: &mut Context<Self>| {
            panel.amend = false;
            panel.subject.update(cx, |input, cx| input.set_text("", cx));
            panel.body.update(cx, |input, cx| input.set_text("", cx));
        };
        self.act("Commit failed", top, work, then, cx);
    }

    /// Opens `change`'s diff as `section` holds it, once git gives it.
    fn open(
        &mut self,
        top: PathBuf,
        change: Change,
        section: ChangeSection,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        // Git keeps an untracked folder whole, so it has no lines.
        if section == ChangeSection::Untracked && is_folder(&change.path) {
            let lines = Err("An untracked folder has no diff.");
            return self.shows(
                Diff {
                    change,
                    section,
                    lines,
                },
                cx,
            );
        }
        let asked = change.clone();
        let read = cx.background_spawn(async move { super::diff(&top, &asked, section) });
        self.opening = cx.spawn(async move |panel, cx| {
            let read = read.await;
            let opened = |panel: &mut Self, cx: &mut Context<Self>| {
                let lines = match read {
                    Ok(Some(lines)) if lines.is_empty() => Err("No line changed."),
                    Ok(Some(lines)) => Ok(Rc::new(lines)),
                    Ok(None) => Err("A binary file."),
                    Err(error) => {
                        log::error!("git panel: Diff failed: {error:#}");
                        panel.error = Some(("Diff failed", format!("{error:#}").into()));
                        return cx.notify();
                    }
                };
                panel.shows(
                    Diff {
                        change,
                        section,
                        lines,
                    },
                    cx,
                )
            };
            panel.update(cx, opened).ok();
        });
    }

    fn shows(&mut self, diff: Diff, cx: &mut Context<Self>) {
        let (path, section) = (diff.change.path.display(), diff.section);
        log::info!("git panel: shows the diff of {path} in {section:?}");
        self.diff = Some(diff);
        cx.notify();
    }
}
