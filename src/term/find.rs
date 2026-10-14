use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Direction, Point},
    term::{
        Term,
        search::{Match, RegexIter, RegexSearch},
    },
};
use ely_gpui_component::{
    editor::{FindOptions, FindWidget},
    forms::{InputEvent, TextInput},
};
use gpui::{
    AppContext, Context, Entity, Focusable, Hsla, InteractiveElement, IntoElement, ParentElement,
    SharedString, Subscription, Window, div,
};

use super::{
    input::{FIND, FindNewer},
    view::TerminalView,
};

/// An open find: its query and options, the matches oldest first, the one stepped to.
pub struct Finder {
    query: Entity<TextInput>,
    options: FindOptions,
    hits: Vec<Match>,
    current: Option<usize>,
    error: Option<SharedString>,
    _input: Subscription,
}

impl Finder {
    /// The matches on screen, each with its wash; the current one's is stronger.
    pub fn lit<T>(&self, term: &Term<T>, color: Hsla) -> Vec<(Match, Hsla)> {
        let offset = term.grid().display_offset() as i32;
        let (top, bottom) = (-offset, term.screen_lines() as i32 - 1 - offset);
        self.hits
            .iter()
            .enumerate()
            .filter(|(_, hit)| hit.end().line.0 >= top && hit.start().line.0 <= bottom)
            .map(|(at, hit)| {
                let wash = if Some(at) == self.current { 0.55 } else { 0.25 };
                (hit.clone(), color.opacity(wash))
            })
            .collect()
    }
}

/// `query` under `options` as the grid's search reads it; word edges are ASCII, which its lazy DFA needs.
fn pattern(query: &str, options: FindOptions) -> String {
    let body = if options.regex {
        query.to_string()
    } else {
        regex::escape(query)
    };
    let body = if options.word {
        format!(r"(?-u:\b)(?:{body})(?-u:\b)")
    } else {
        body
    };
    let case = if options.case { "(?-i)" } else { "(?i)" };
    format!("{case}{body}")
}

/// Every match of `pattern` in the scrollback and the screen, oldest first.
fn search_all<T>(term: &Term<T>, pattern: &str) -> Result<Vec<Match>, String> {
    let mut regex = RegexSearch::new(pattern).map_err(|error| error.to_string())?;
    let start = Point::new(term.topmost_line(), Column(0));
    let end = Point::new(term.bottommost_line(), term.last_column());
    Ok(RegexIter::new(start, end, Direction::Right, term, &mut regex).collect())
}

/// The match one newer than `current`, or one older, round the ends.
fn stepped(current: Option<usize>, count: usize, newer: bool) -> Option<usize> {
    let at = current?;
    Some(if newer {
        (at + 1) % count
    } else {
        (at + count - 1) % count
    })
}

impl TerminalView {
    /// Opens the find box, or takes the focus back to it.
    pub(super) fn find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = match &self.finder {
            Some(finder) => finder.query.clone(),
            None => {
                let query = cx.new(|cx| TextInput::new(window, cx));
                let input = cx.subscribe_in(&query, window, |view, _, event, _, cx| match event {
                    InputEvent::Changed => view.search(true, cx),
                    InputEvent::Submit => view.step(false, cx),
                    InputEvent::Focus | InputEvent::Blur => {}
                });
                log::debug!("terminal {}: find opens", self.pty.pid);
                self.finder = Some(Finder {
                    query: query.clone(),
                    options: FindOptions::default(),
                    hits: Vec::new(),
                    current: None,
                    error: None,
                    _input: input,
                });
                query
            }
        };
        query.update(cx, |input, cx| {
            let all = 0..input.text().len();
            input.select(all, cx);
        });
        window.focus(&query.focus_handle(cx), cx);
        cx.notify();
    }

    fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.finder.take().is_some() {
            log::debug!("terminal {}: find closes", self.pty.pid);
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    /// Finds the query anew; a `fresh` query starts at the newest match, new output keeps the current one.
    pub(super) fn search(&mut self, fresh: bool, cx: &mut Context<Self>) {
        let Some(finder) = self.finder.as_mut() else {
            return;
        };
        let query = finder.query.read(cx).text().to_string();
        let found = if query.is_empty() {
            Ok(Vec::new())
        } else {
            search_all(&self.pty.term.lock(), &pattern(&query, finder.options))
        };
        let (hits, error) = match found {
            Ok(hits) => (hits, None),
            Err(error) => {
                log::debug!(
                    "terminal {}: {query:?} is no pattern: {error}",
                    self.pty.pid
                );
                (Vec::new(), Some(SharedString::from(error)))
            }
        };
        let newest = hits.len().checked_sub(1);
        let kept = finder.current.filter(|at| *at < hits.len());
        finder.current = if fresh { newest } else { kept.or(newest) };
        finder.error = error;
        finder.hits = hits;
        if fresh {
            log::debug!(
                "terminal {}: {} matches of {query:?}",
                self.pty.pid,
                finder.hits.len()
            );
            self.reveal(cx);
        } else {
            cx.notify();
        }
    }

    /// Steps to a newer match, or an older one, and scrolls it into view.
    fn step(&mut self, newer: bool, cx: &mut Context<Self>) {
        let Some(finder) = self.finder.as_mut() else {
            return;
        };
        finder.current = stepped(finder.current, finder.hits.len(), newer);
        self.reveal(cx);
        cx.notify();
    }

    fn reveal(&mut self, cx: &mut Context<Self>) {
        let start = self.finder.as_ref().and_then(|finder| {
            let at = finder.current?;
            Some(*finder.hits[at].start())
        });
        if let Some(start) = start {
            log::trace!("terminal {}: match at {start:?}", self.pty.pid);
            self.pty.term.lock().scroll_to_point(start);
        }
        cx.notify();
    }

    fn set_options(&mut self, options: FindOptions, cx: &mut Context<Self>) {
        if let Some(finder) = self.finder.as_mut() {
            finder.options = options;
            self.search(true, cx);
        }
    }

    /// The find box over the grid; Shift-Enter steps to a newer match.
    pub(super) fn find_box(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let finder = self.finder.as_ref()?;
        let (flip, step, close, back) = (cx.entity(), cx.entity(), cx.entity(), cx.entity());
        let total = finder.hits.len();
        let widget = FindWidget::new(
            ("find", cx.entity_id()),
            &finder.query,
            finder.current,
            total,
        )
        .options(finder.options)
        .on_options(move |options, _, cx| flip.update(cx, |view, cx| view.set_options(options, cx)))
        .on_step(move |newer, _, cx| step.update(cx, |view, cx| view.step(newer, cx)))
        .on_close(move |window, cx| close.update(cx, |view, cx| view.close_find(window, cx)));
        let widget = match &finder.error {
            Some(error) => widget.error(error.clone()),
            None => widget,
        };
        Some(
            div()
                .key_context(FIND)
                .on_action(move |_: &FindNewer, _, cx| {
                    back.update(cx, |view, cx| view.step(true, cx))
                })
                .child(widget),
        )
    }
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::term::test::mock_term;

    use super::*;

    #[test]
    fn find_takes_case_words_and_patterns() {
        let term = mock_term("one two One\r\nthree phone one");
        let count = |query: &str, options: FindOptions| {
            search_all(&term, &pattern(query, options)).map(|all| all.len())
        };
        assert_eq!(count("one", FindOptions::default()), Ok(4));
        let case = FindOptions {
            case: true,
            ..FindOptions::default()
        };
        assert_eq!(count("One", case), Ok(1));
        let word = FindOptions {
            word: true,
            ..FindOptions::default()
        };
        assert_eq!(count("one", word), Ok(3), "phone holds no whole word");
        let regex = FindOptions {
            regex: true,
            ..FindOptions::default()
        };
        assert_eq!(count("t[wh]", regex), Ok(2));
        assert_eq!(
            count("(", FindOptions::default()),
            Ok(0),
            "plain text is escaped"
        );
        assert!(count("(", regex).is_err());
    }

    #[test]
    fn steps_go_round_the_ends() {
        assert_eq!(stepped(Some(0), 3, false), Some(2));
        assert_eq!(stepped(Some(2), 3, true), Some(0));
        assert_eq!(stepped(Some(1), 3, false), Some(0));
        assert_eq!(stepped(None, 0, true), None);
    }
}
